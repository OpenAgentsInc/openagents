import { randomUUID } from "node:crypto"

import { Effect, Layer, ManagedRuntime, Schema as S, Stream } from "effect"

import {
  OWNER_BOUND_CANDIDATE_SET_SCHEMA_LITERAL,
  OwnerBoundCandidateSet,
  TurnRequestRef,
} from "@openagentsinc/agent-runtime-schema"
import {
  ProviderStartError,
  TurnService,
  TurnServiceLayer,
  type ProviderRegistryInterface,
  type TurnProgressFrame,
} from "@openagentsinc/agent-turn-runtime"

import type { makeThreadStore } from "../thread-store.ts"
import type { LocalTurnJournal } from "../local-turn-journal.ts"
import {
  DesktopTurnCancelChannel,
  DesktopTurnEventChannel,
  DesktopTurnEventFrame,
  DesktopTurnStartChannel,
  DesktopTurnStatusChannel,
  DesktopTurnSubmitChannel,
  type DesktopTurnSubmitResult,
  decodeDesktopTurnCancelRequest,
  decodeDesktopTurnStartRequest,
  decodeDesktopTurnStatusRequest,
  decodeDesktopTurnSubmitRequest,
  encodeDesktopTurnSubmitResult,
} from "./desktop-turn-ipc.ts"
import { ProviderRegistry } from "./desktop-provider-lane.ts"
import {
  desktopActionBrokerLayer,
  desktopArtifactResolverLayer,
  desktopContextSourceLayer,
  desktopTurnPolicyLayer,
} from "./desktop-turn-policy.ts"
import { desktopThreadRepositoryLayer } from "./desktop-thread-repository.ts"
import { desktopTurnJournalLayer } from "./desktop-turn-journal.ts"

/**
 * AFS-01 first production composition: Electron main composes the shared
 * `TurnService` over the Desktop transition adapters.
 *
 * AFS-03 flips the kernel to the LIVE local chat path: the renderer's local
 * "OpenAgents authority" turn now submits one typed intent through this kernel
 * over `turn:submit`. The kernel path is on by default; the rollback opt-out is
 * `OPENAGENTS_DESKTOP_AFS_TURN_KERNEL=0` (kept until the gate proves out).
 *
 * `ipcMain` and the renderer sender are injected structurally so this module is
 * unit-testable without importing Electron.
 */
const encodeEventFrame = S.encodeUnknownSync(DesktopTurnEventFrame)
const decodeCandidateSet = S.decodeUnknownSync(OwnerBoundCandidateSet)
const decodeRequestRef = S.decodeUnknownSync(TurnRequestRef)

/** The stable owner-bound policy artifact for the AFS-03 local composition. */
const DESKTOP_LOCAL_POLICY_ARTIFACT_REF = "artifact.policy.desktop-local.v1" as const

const unavailableSubmit: DesktopTurnSubmitResult = {
  outcome: "unavailable",
  text: null,
  provider: null,
  placement: null,
  dataDestination: null,
  usageTruth: null,
}
const failedSubmit: DesktopTurnSubmitResult = { ...unavailableSubmit, outcome: "failed" }

/** The narrow Electron surfaces the composition needs. */
export interface DesktopTurnIpcMain {
  readonly handle: (channel: string, handler: (event: unknown, value: unknown) => unknown) => void
  readonly removeHandler: (channel: string) => void
}
export interface DesktopTurnSender {
  readonly isDestroyed: () => boolean
  readonly send: (channel: string, payload: unknown) => void
}

export interface InstallDesktopTurnKernelDeps {
  readonly ipcMain: DesktopTurnIpcMain
  readonly sender: () => DesktopTurnSender | null
  readonly threadStore: ReturnType<typeof makeThreadStore>
  readonly journalFilePath: string
  /**
   * The inference provider registry. Real Desktop lane adapters wire here in
   * AFS-02; AFS-01 defaults to a placeholder that refuses provider start so the
   * kernel path installs and journals without a live provider.
   */
  readonly providerRegistry?: ProviderRegistryInterface
  readonly legacyJournal?: {
    readonly journal: LocalTurnJournal
    readonly laneRef: (record: { readonly effective: unknown }) => string
  }
}

export interface InstalledDesktopTurnKernel {
  readonly dispose: () => Promise<void>
}

/** The kernel path is live by default (AFS-03). `=0` is the rollback opt-out. */
export const desktopAfsTurnKernelEnabled = (env: NodeJS.ProcessEnv = process.env): boolean =>
  env.OPENAGENTS_DESKTOP_AFS_TURN_KERNEL !== "0"

/** AFS-01 placeholder registry: refuses provider start until AFS-02 wires lanes. */
export const placeholderProviderRegistry: ProviderRegistryInterface = {
  describe: Effect.succeed([]),
  start: () => Effect.fail(new ProviderStartError({ reason: "unavailable" })),
}

/**
 * Compose the kernel over the Desktop adapters and register the typed turn IPC.
 * Returns a disposer that removes the handlers and releases the runtime.
 */
export const installDesktopTurnKernel = (
  deps: InstallDesktopTurnKernelDeps,
): InstalledDesktopTurnKernel => {
  const resolvedRegistry = deps.providerRegistry ?? placeholderProviderRegistry
  const providerRegistryLayer = Layer.succeed(ProviderRegistry, ProviderRegistry.of(resolvedRegistry))

  const layer = TurnServiceLayer.pipe(
    Layer.provide(desktopContextSourceLayer),
    Layer.provide(desktopTurnPolicyLayer),
    Layer.provide(providerRegistryLayer),
    Layer.provide(
      desktopTurnJournalLayer({
        filePath: deps.journalFilePath,
        ...(deps.legacyJournal === undefined ? {} : { legacy: deps.legacyJournal }),
      }),
    ),
    Layer.provide(desktopThreadRepositoryLayer(deps.threadStore)),
    Layer.provide(desktopArtifactResolverLayer),
    Layer.provide(desktopActionBrokerLayer),
  )

  const runtime = ManagedRuntime.make(layer)

  const forwardFrame = (frame: TurnProgressFrame): void => {
    const sender = deps.sender()
    if (sender === null || sender.isDestroyed()) return
    const terminal =
      frame.projection.cardState === "done" ||
      frame.projection.cardState === "refused" ||
      frame.projection.cardState === "failed" ||
      frame.projection.cardState === "cancelled"
    // Progress-stream frames carry the live card. The terminal receipt is
    // delivered from the start handler; here we forward the bounded projection
    // fenced by request + generation identity.
    if (terminal) return
    sender.send(
      DesktopTurnEventChannel,
      encodeEventFrame({
        kind: "progress",
        requestRef: frame.requestRef,
        generation: frame.generation,
        projection: frame.projection,
      }),
    )
  }

  // Background progress forwarder, scoped to the runtime.
  runtime.runFork(
    Effect.gen(function* () {
      const service = yield* TurnService
      yield* service.progress.pipe(Stream.runForEach((frame) => Effect.sync(() => forwardFrame(frame))))
    }),
  )

  deps.ipcMain.handle(DesktopTurnStartChannel, (_event, value) => {
    const request = decodeDesktopTurnStartRequest(value)
    if (request._tag === "None") return { accepted: false, error: "invalid turn start request" }
    const started = request.value
    return runtime
      .runPromise(
        Effect.gen(function* () {
          const service = yield* TurnService
          const result = yield* service.start({
            requestRef: started.requestRef,
            threadRef: started.threadRef,
            intent: started.intent,
            candidateSet: started.candidateSet,
          })
          const sender = deps.sender()
          if (sender !== null && !sender.isDestroyed()) {
            sender.send(
              DesktopTurnEventChannel,
              encodeEventFrame({
                kind: "terminal",
                requestRef: started.requestRef,
                generation: result.generation,
                projection: result.projection,
                receipt: result.receipt,
              }),
            )
          }
          return { accepted: true as const, requestRef: started.requestRef }
        }),
      )
      .catch(() => ({ accepted: false as const, error: "turn failed to start" }))
  })

  deps.ipcMain.handle(DesktopTurnCancelChannel, (_event, value) => {
    const request = decodeDesktopTurnCancelRequest(value)
    if (request._tag === "None") return { ok: false }
    return runtime
      .runPromise(
        Effect.gen(function* () {
          const service = yield* TurnService
          yield* service.cancel(request.value.requestRef)
          return { ok: true as const }
        }),
      )
      .catch(() => ({ ok: false as const }))
  })

  deps.ipcMain.handle(DesktopTurnStatusChannel, (_event, value) => {
    const request = decodeDesktopTurnStatusRequest(value)
    if (request._tag === "None") return null
    return runtime
      .runPromise(
        Effect.gen(function* () {
          const service = yield* TurnService
          return yield* service.status(request.value.requestRef)
        }),
      )
      .catch(() => null)
  })

  // AFS-03 one-shot local-turn submit: the renderer sends only a thread ref and
  // a bounded message. The HOST builds the intent and the owner-bound candidate
  // set (from the described local providers), runs the shared kernel, and
  // resolves the compact terminal facts. The renderer makes no route/prompt
  // decision.
  deps.ipcMain.handle(DesktopTurnSubmitChannel, (_event, value) => {
    const request = decodeDesktopTurnSubmitRequest(value)
    if (request._tag === "None") return encodeDesktopTurnSubmitResult(failedSubmit)
    const submit = request.value
    return Effect.runPromise(resolvedRegistry.describe)
      .then((descriptors) => {
        if (descriptors.length === 0) return unavailableSubmit
        const candidateSet = decodeCandidateSet({
          schema: OWNER_BOUND_CANDIDATE_SET_SCHEMA_LITERAL,
          ordered: descriptors.map((descriptor) => descriptor.providerRef),
          policyArtifactRef: DESKTOP_LOCAL_POLICY_ARTIFACT_REF,
        })
        return runtime.runPromise(
          Effect.gen(function* () {
            const service = yield* TurnService
            const result = yield* service.start({
              requestRef: decodeRequestRef(`request.${randomUUID()}`),
              threadRef: submit.threadRef,
              intent: { _tag: "Ask" as const, text: submit.message },
              candidateSet,
            })
            const projection = result.projection
            const provider = projection.candidate ?? null
            const descriptor =
              provider === null ? undefined : descriptors.find((entry) => entry.candidate === provider)
            const answerText =
              result.candidate !== null && result.candidate.kind === "answer" ? result.candidate.text : null
            const outcome: DesktopTurnSubmitResult["outcome"] =
              projection.cardState === "done"
                ? answerText !== null
                  ? "answered"
                  : "refused"
                : projection.cardState === "refused"
                  ? "refused"
                  : projection.cardState === "cancelled"
                    ? "cancelled"
                    : "failed"
            const submitResult: DesktopTurnSubmitResult = {
              outcome,
              text: outcome === "answered" ? answerText : null,
              provider,
              placement: descriptor?.placement ?? null,
              dataDestination: projection.dataDestination,
              usageTruth: projection.usageTruth,
            }
            return submitResult
          }),
        )
      })
      .catch(() => failedSubmit)
      .then((result) => encodeDesktopTurnSubmitResult(result))
      .catch(() => encodeDesktopTurnSubmitResult(failedSubmit))
  })

  return {
    dispose: async () => {
      deps.ipcMain.removeHandler(DesktopTurnStartChannel)
      deps.ipcMain.removeHandler(DesktopTurnCancelChannel)
      deps.ipcMain.removeHandler(DesktopTurnStatusChannel)
      deps.ipcMain.removeHandler(DesktopTurnSubmitChannel)
      await runtime.dispose()
    },
  }
}
