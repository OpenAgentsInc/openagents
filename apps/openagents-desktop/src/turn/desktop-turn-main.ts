import { randomUUID } from "node:crypto"

import { Effect, Layer, ManagedRuntime, Schema as S, Stream } from "effect"

import {
  OWNER_BOUND_CANDIDATE_SET_SCHEMA_LITERAL,
  OwnerBoundCandidateSet,
  TurnRequestRef,
  type InferenceProviderDescriptor,
  type RouteRecommendation,
  type TurnThreadRef,
} from "@openagentsinc/agent-runtime-schema"
import {
  ProviderStartError,
  TurnService,
  TurnServiceLayer,
  type ProviderRegistryInterface,
  type ProviderStartInput,
  type TurnProgressFrame,
} from "@openagentsinc/agent-turn-runtime"

import type { CodexLaneReadiness } from "./desktop-codex-provider.ts"
import { decideDelegation } from "./desktop-delegation.ts"
import type { EditorContextRegistry } from "./editor-context-binding.ts"

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
  desktopArtifactResolverLayer,
  desktopContextSourceLayer,
  desktopTurnPolicyLayer,
} from "./desktop-turn-policy.ts"
import { desktopActionBrokerLayer } from "./turn-action-broker.ts"
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
  delegationRequestRef: null,
  objective: null,
}
const failedSubmit: DesktopTurnSubmitResult = { ...unavailableSubmit, outcome: "failed" }

/**
 * Compose several inference-provider registries into one. `start` routes to the
 * registry that describes the requested provider ref. This lets the AFS-04 local
 * composition carry the Apple FM router lane AND the codex delegate lane behind
 * one kernel `ProviderRegistry`.
 */
const composeRegistries = (
  registries: ReadonlyArray<ProviderRegistryInterface>,
): ProviderRegistryInterface => ({
  describe: Effect.forEach(registries, (registry) => registry.describe).pipe(
    Effect.map((lists) => lists.flat()),
  ),
  start: (input: ProviderStartInput) =>
    Effect.gen(function* () {
      for (const registry of registries) {
        const descriptors = yield* registry.describe
        if (descriptors.some((descriptor) => descriptor.providerRef === input.providerRef)) {
          return yield* registry.start(input)
        }
      }
      return yield* Effect.fail(new ProviderStartError({ reason: "unavailable" }))
    }),
})

/** Map a codex descriptor's readiness into the delegation readiness snapshot. */
const codexReadinessOf = (descriptor: InferenceProviderDescriptor): CodexLaneReadiness => {
  if (descriptor.readiness.state === "ready") {
    return {
      ready: true,
      ...(descriptor.accountRef === undefined ? {} : { accountRef: descriptor.accountRef }),
    }
  }
  const reason = descriptor.readiness.reason
  const unavailableReason: CodexLaneReadiness["unavailableReason"] =
    reason === "account_missing"
      ? "no_codex_account"
      : reason === "account_unhealthy"
        ? "no_verified_account"
        : reason === "permission_denied"
          ? "policy_denied"
          : "not_ready"
  return { ready: false, unavailableReason }
}

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
  /**
   * The codex delegate lane registry (AFS-04). When present, the Apple FM router
   * turn's admitted recommendation can start ONE real codex delegation turn
   * through the shared kernel. Absent → the local path answers only (AFS-03).
   */
  readonly codexProvider?: ProviderRegistryInterface
  /**
   * The AFS-05 editor-context registry. When present, an Editor agent-rail submit
   * that carries an `editorContext` binding registers it here before the kernel
   * runs, so the shared `ContextSource` feeds the Editor's IDE-08 context into the
   * SAME turn service that chat uses. Absent → editor context is never bound and
   * the local chat path is unchanged.
   */
  readonly editorContextRegistry?: EditorContextRegistry
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
  const baseRegistry = deps.providerRegistry ?? placeholderProviderRegistry
  // AFS-04: fold the codex delegate lane into the kernel registry so a router
  // recommendation can start one real codex turn on the same runtime.
  const resolvedRegistry =
    deps.codexProvider === undefined ? baseRegistry : composeRegistries([baseRegistry, deps.codexProvider])
  const providerRegistryLayer = Layer.succeed(ProviderRegistry, ProviderRegistry.of(resolvedRegistry))

  const layer = TurnServiceLayer.pipe(
    Layer.provide(desktopContextSourceLayer(deps.editorContextRegistry)),
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
  /**
   * Start ONE real codex delegation turn on the kernel runtime, forked in the
   * background, and forward its terminal frame. The global progress forwarder
   * already streams the running frames. The card is created by the renderer only
   * after this start receipt (the `delegated` result), and it cannot show
   * running before the kernel emits a running frame for this request.
   */
  const startCodexDelegation = (input: {
    readonly delegationRequestRef: TurnRequestRef
    readonly threadRef: TurnThreadRef
    readonly objective: string
    readonly codexProviderRef: string
    readonly recommendation: RouteRecommendation
  }): void => {
    const codexCandidateSet = decodeCandidateSet({
      schema: OWNER_BOUND_CANDIDATE_SET_SCHEMA_LITERAL,
      ordered: [input.codexProviderRef],
      policyArtifactRef: DESKTOP_LOCAL_POLICY_ARTIFACT_REF,
    })
    runtime.runFork(
      Effect.gen(function* () {
        const service = yield* TurnService
        const result = yield* service.start({
          requestRef: input.delegationRequestRef,
          threadRef: input.threadRef,
          intent: { _tag: "Ask" as const, text: input.objective },
          candidateSet: codexCandidateSet,
          recommendation: input.recommendation,
        })
        const sender = deps.sender()
        if (sender !== null && !sender.isDestroyed()) {
          sender.send(
            DesktopTurnEventChannel,
            encodeEventFrame({
              kind: "terminal",
              requestRef: input.delegationRequestRef,
              generation: result.generation,
              projection: result.projection,
              receipt: result.receipt,
            }),
          )
        }
      }),
    )
  }

  deps.ipcMain.handle(DesktopTurnSubmitChannel, (_event, value) => {
    const request = decodeDesktopTurnSubmitRequest(value)
    if (request._tag === "None") return encodeDesktopTurnSubmitResult(failedSubmit)
    const submit = request.value
    // AFS-05: an Editor agent-rail turn binds its IDE-08 context for this thread
    // before the shared kernel runs, so the ContextSource feeds it into the SAME
    // turn service. The host trusts the renderer only to DESCRIBE context; the
    // binding is validated against the authoritative editor identity in the
    // context source. Absent → the chat path is unchanged.
    if (submit.editorContext !== undefined && deps.editorContextRegistry !== undefined) {
      deps.editorContextRegistry.set(submit.editorContext)
      deps.editorContextRegistry.setExpectation(submit.editorContext.identity)
    }
    return Effect.runPromise(resolvedRegistry.describe)
      .then((descriptors) => {
        // The Apple FM router lane runs the turn. The codex lane is a delegate
        // target, never the router; restrict the router set to the local lanes.
        const routerDescriptors = descriptors.filter((descriptor) => descriptor.candidate === "apple_fm")
        const codexDescriptor = descriptors.find((descriptor) => descriptor.candidate === "codex")
        if (routerDescriptors.length === 0) return unavailableSubmit
        const candidateSet = decodeCandidateSet({
          schema: OWNER_BOUND_CANDIDATE_SET_SCHEMA_LITERAL,
          ordered: routerDescriptors.map((descriptor) => descriptor.providerRef),
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

            // AFS-04 router: an admitted delegate recommendation starts one real
            // codex turn. The host validates codex readiness (main-owned); an
            // unavailable lane produces no start and an honest refusal.
            if (codexDescriptor !== undefined && projection.cardState === "done" && answerText !== null) {
              const decision = decideDelegation({
                answerText,
                objective: submit.message,
                codexReadiness: codexReadinessOf(codexDescriptor),
              })
              if (decision.kind === "delegate") {
                const delegationRequestRef = decodeRequestRef(`request.codex.${randomUUID()}`)
                startCodexDelegation({
                  delegationRequestRef,
                  threadRef: submit.threadRef,
                  objective: decision.objective,
                  codexProviderRef: codexDescriptor.providerRef,
                  recommendation: decision.recommendation,
                })
                const delegated: DesktopTurnSubmitResult = {
                  outcome: "delegated",
                  text: null,
                  provider: "codex",
                  placement: codexDescriptor.placement,
                  dataDestination: codexDescriptor.dataDestination,
                  usageTruth: codexDescriptor.usageTruth,
                  delegationRequestRef,
                  objective: decision.objective,
                }
                return delegated
              }
              if (decision.kind === "refuse_delegation") {
                const refused: DesktopTurnSubmitResult = {
                  outcome: "refused",
                  text: null,
                  provider: "codex",
                  placement: null,
                  dataDestination: null,
                  usageTruth: null,
                  delegationRequestRef: null,
                  objective: null,
                }
                return refused
              }
              // decision.kind === "answer": fall through to the normal answer path.
            }

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
              delegationRequestRef: null,
              objective: null,
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
