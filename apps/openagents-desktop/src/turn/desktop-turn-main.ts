import { Effect, Layer, ManagedRuntime, Schema as S, Stream } from "effect"

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
  decodeDesktopTurnCancelRequest,
  decodeDesktopTurnStartRequest,
  decodeDesktopTurnStatusRequest,
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
 * This is additive and gated. The old renderer provider-lane path stays the
 * default; the kernel path is enabled by an explicit compatibility flag for
 * rollback (`OPENAGENTS_DESKTOP_AFS_TURN_KERNEL=1`), and removed only in AFS-03.
 * The module never forces a cutover of the live chat path.
 *
 * `ipcMain` and the renderer sender are injected structurally so this module is
 * unit-testable without importing Electron.
 */
const encodeEventFrame = S.encodeUnknownSync(DesktopTurnEventFrame)

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

/** The compatibility flag gating the kernel path. Default off preserves rollback. */
export const desktopAfsTurnKernelEnabled = (env: NodeJS.ProcessEnv = process.env): boolean =>
  env.OPENAGENTS_DESKTOP_AFS_TURN_KERNEL === "1"

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
  const providerRegistryLayer = Layer.succeed(
    ProviderRegistry,
    ProviderRegistry.of(deps.providerRegistry ?? placeholderProviderRegistry),
  )

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

  return {
    dispose: async () => {
      deps.ipcMain.removeHandler(DesktopTurnStartChannel)
      deps.ipcMain.removeHandler(DesktopTurnCancelChannel)
      deps.ipcMain.removeHandler(DesktopTurnStatusChannel)
      await runtime.dispose()
    },
  }
}
