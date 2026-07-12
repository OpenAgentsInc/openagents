/**
 * Sandboxed preload (#8574) — the ONLY renderer-visible host bridge.
 *
 * Boundary law: no ipcRenderer, no MessagePort/oRPC bridge, no Node or
 * Electron builtins, no filesystem/process authority, no credentials. Static
 * host identity only; the renderer decodes it with Effect Schema. Every
 * future capability crossing this line arrives as a typed, schema-validated
 * service — never a raw channel.
 *
 * CommonJS (.cts -> dist/preload.cjs) because sandboxed preloads cannot be
 * ESM.
 */
import { contextBridge, ipcRenderer } from "electron"

import {
  CodexAccountsChannel,
  CodexConnectOpenChannel,
  CodexConnectStartChannel,
  CodexConnectStatusChannel,
  CodexReconnectStartChannel,
} from "./codex-connect-contract.ts"
import {
  FleetStageChannel,
  decodeFleetStageRequest,
  unavailableFleetStageResult,
} from "./fleet-contract.ts"
import {
  ProviderAccountsListChannel,
  ProviderAccountsUsageChannel,
  decodeProviderAccountUsageRequest,
  unavailableProviderAccountUsageResult,
} from "./provider-accounts-contract.ts"
import { DesktopChatTurnChannel, DesktopHydrateThreadChannel, DesktopNewThreadChannel, DesktopOpenThreadChannel, DesktopThreadsChannel, decode, DesktopThreadRequestSchema, DesktopTurnRequestSchema } from "./chat-contract.ts"
import {
  DesktopWorkspaceChooseChannel,
  DesktopWorkspaceFilesChannel,
  DesktopWorkspaceGitDiffChannel,
  DesktopWorkspaceGitStatusChannel,
  DesktopWorkspaceReadChannel,
  DesktopWorkspaceSaveChannel,
  DesktopWorkspaceSummaryChannel,
  DesktopWorkspaceTreeChannel,
  DesktopWorkspaceRefreshChannel,
  DesktopWorkspaceWatchChannel,
  DesktopWorkspaceChangeChannel,
  decodeWorkspaceChange,
  decodeWorkspaceFileRequest,
  decodeWorkspaceGitDiffRequest,
  decodeWorkspaceSaveRequest,
  decodeWorkspaceTreePage,
  decodeWorkspaceTreeRequest,
  decodeWorkspaceWatchRequest,
  type DesktopWorkspaceChange,
} from "./workspace-contract.ts"
import { DesktopWindowFullscreenChannel } from "./window-contract.ts"
import {
  DesktopRuntimeGatewayEventChannel,
  DesktopRuntimeGatewayInvokeChannel,
  decodeDesktopRuntimeGatewayEvent,
  decodeDesktopRuntimeGatewayRequest,
  decodeDesktopRuntimeGatewayResponse,
  invalidDesktopRuntimeGatewayResponse,
  type DesktopRuntimeGatewayEvent,
} from "./runtime-gateway-contract.ts"
import {
  FableLocalAnswerQuestionChannel,
  FableLocalAvailabilityChannel,
  FableLocalEventChannel,
  FableLocalInterruptChannel,
  FableLocalQueueFollowupChannel,
  FableLocalStartChannel,
  FableLocalSteerChildChannel,
  decodeFableLocalAnswerQuestionRequest,
  decodeFableLocalEventEnvelope,
  decodeFableLocalInterruptRequest,
  decodeFableLocalQueueFollowupRequest,
  decodeFableLocalStartRequest,
  decodeFableLocalSteerChildRequest,
  type FableLocalEventEnvelope,
} from "./fable-local-contract.ts"
import {
  CodexLocalAvailabilityChannel,
  CodexLocalEventChannel,
  CodexLocalInterruptChannel,
  CodexLocalStartChannel,
} from "./codex-local-contract.ts"
import {
  UsageLedgerEventChannel,
  UsageLedgerSnapshotChannel,
  decodeUsageLedgerSnapshot,
  type UsageLedgerSnapshot,
} from "./usage-ledger-contract.ts"
import {
  DesktopCodingCatalogArchiveChannel,
  DesktopCodingCatalogChooseChannel,
  DesktopCodingCatalogFocusChannel,
  DesktopCodingCatalogOpenChannel,
  DesktopCodingCatalogRecoverChannel,
  DesktopCodingCatalogSnapshotChannel,
  decodeDesktopCodingCatalogProjection,
  decodeDesktopCodingFocusRequest,
  decodeDesktopCodingSessionRequest,
  emptyDesktopCodingCatalogProjection,
} from "./coding-catalog-contract.ts"
import {
  DesktopCommandEventChannel,
  DesktopCommandReadyChannel,
  DesktopCommandBindingsChannel,
  DesktopCommandBindingSaveChannel,
  DesktopCommandBindingsResetChannel,
  decodeDesktopCommandBindingProjectionOrNull,
  decodeDesktopCommandBindingUpdateOrNull,
  decodeDesktopDeferredCommandOrNull,
  type DesktopDeferredCommand,
} from "./desktop-command-contract.ts"

const workspaceChangeListeners = new Set<
  (change: DesktopWorkspaceChange) => void
>()
let workspaceChangeHandler: ((_event: unknown, value: unknown) => void) | null = null

const subscribeWorkspaceChanges = (
  listener: (change: DesktopWorkspaceChange) => void,
): (() => void) => {
  if (workspaceChangeListeners.size === 0) {
    workspaceChangeHandler = (_event: unknown, value: unknown): void => {
      const change = decodeWorkspaceChange(value)
      if (change === null) return
      for (const activeListener of workspaceChangeListeners) activeListener(change)
    }
    ipcRenderer.on(DesktopWorkspaceChangeChannel, workspaceChangeHandler)
    void ipcRenderer.invoke(
      DesktopWorkspaceWatchChannel,
      decodeWorkspaceWatchRequest({ active: true }),
    )
  }
  workspaceChangeListeners.add(listener)
  let closed = false
  return () => {
    if (closed) return
    closed = true
    workspaceChangeListeners.delete(listener)
    if (workspaceChangeListeners.size !== 0 || workspaceChangeHandler === null) return
    ipcRenderer.removeListener(DesktopWorkspaceChangeChannel, workspaceChangeHandler)
    workspaceChangeHandler = null
    void ipcRenderer.invoke(
      DesktopWorkspaceWatchChannel,
      decodeWorkspaceWatchRequest({ active: false }),
    )
  }
}

contextBridge.exposeInMainWorld("openagentsDesktop", {
  host: "electron",
  platform: process.platform,
  toggleFullScreen: async (): Promise<boolean> => {
    const result = await ipcRenderer.invoke(DesktopWindowFullscreenChannel)
    return result === true
  },
  runtimeRequest: async (value: unknown) => {
    const request = decodeDesktopRuntimeGatewayRequest(value)
    if (request === null) return invalidDesktopRuntimeGatewayResponse()
    const response = await ipcRenderer.invoke(DesktopRuntimeGatewayInvokeChannel, request)
    return decodeDesktopRuntimeGatewayResponse(response) ?? invalidDesktopRuntimeGatewayResponse()
  },
  runtimeSubscribe: (listener: (event: DesktopRuntimeGatewayEvent) => void) => {
    const handler = (_event: unknown, value: unknown): void => {
      const decoded = decodeDesktopRuntimeGatewayEvent(value)
      if (decoded !== null) listener(decoded)
    }
    ipcRenderer.on(DesktopRuntimeGatewayEventChannel, handler)
    return () => ipcRenderer.removeListener(DesktopRuntimeGatewayEventChannel, handler)
  },
  /** The sole renderer mutation: one schema-checked Fleet brief. */
  stageFleet: (value: unknown) => {
    const request = decodeFleetStageRequest(value)
    return request === null
      ? Promise.resolve(unavailableFleetStageResult())
      : ipcRenderer.invoke(FleetStageChannel, request)
  },
  listThreads: () => ipcRenderer.invoke(DesktopThreadsChannel),
  newThread: () => ipcRenderer.invoke(DesktopNewThreadChannel),
  openThread: (value: unknown) => {
    const request = decode(DesktopThreadRequestSchema, value) as { id: string } | null
    return request === null ? Promise.resolve(null) : ipcRenderer.invoke(DesktopOpenThreadChannel, request)
  },
  hydrateThread: (value: unknown) => {
    const request = decode(DesktopThreadRequestSchema, value) as { id: string } | null
    return request === null ? Promise.resolve(null) : ipcRenderer.invoke(DesktopHydrateThreadChannel, request)
  },
  sendMessage: (value: unknown) => {
    const request = decode(DesktopTurnRequestSchema, value) as { id: string; message: string } | null
    return request === null ? Promise.resolve({ ok: false, error: "That message could not be sent." }) : ipcRenderer.invoke(DesktopChatTurnChannel, request)
  },
  workspaceSummary: () => ipcRenderer.invoke(DesktopWorkspaceSummaryChannel),
  chooseWorkspace: () => ipcRenderer.invoke(DesktopWorkspaceChooseChannel),
  listWorkspaceFiles: () => ipcRenderer.invoke(DesktopWorkspaceFilesChannel),
  workspaceTree: async (value: unknown) => {
    const request = decodeWorkspaceTreeRequest(value)
    if (request === null) {
      return { state: "unavailable", message: "The workspace tree request is invalid." }
    }
    const response = await ipcRenderer.invoke(DesktopWorkspaceTreeChannel, request)
    return decodeWorkspaceTreePage(response) ?? {
      state: "unavailable",
      message: "The workspace tree response is invalid.",
    }
  },
  refreshWorkspace: async (): Promise<boolean> =>
    (await ipcRenderer.invoke(DesktopWorkspaceRefreshChannel)) === true,
  workspaceSubscribe: subscribeWorkspaceChanges,
  readWorkspaceFile: (value: unknown) => {
    const request = decodeWorkspaceFileRequest(value)
    return request === null ? Promise.resolve(null) : ipcRenderer.invoke(DesktopWorkspaceReadChannel, request)
  },
  saveWorkspaceFile: (value: unknown) => {
    const request = decodeWorkspaceSaveRequest(value)
    return request === null
      ? Promise.resolve({ state: "unavailable", message: "The file save request is invalid." })
      : ipcRenderer.invoke(DesktopWorkspaceSaveChannel, request)
  },
  workspaceGitStatus: () => ipcRenderer.invoke(DesktopWorkspaceGitStatusChannel),
  workspaceGitDiff: (value: unknown) => {
    const request = decodeWorkspaceGitDiffRequest(value)
    return request === null
      ? Promise.resolve({ state: "unavailable", message: "The diff request is invalid." })
      : ipcRenderer.invoke(DesktopWorkspaceGitDiffChannel, request)
  },
  /**
   * Codex account connect + reconnect (#8640 unblock; EP250 UI-owned
   * reconnect). Calls are renderer-argument-free except reconnect-start,
   * which carries one grammar-bounded account ref (main re-validates it
   * against its own registry listing). Main holds every other input
   * (including the verification URL it opens); the renderer only receives
   * public-safe typed projections it schema-checks.
   */
  codexAccounts: () => ipcRenderer.invoke(CodexAccountsChannel),
  codexConnectStart: () => ipcRenderer.invoke(CodexConnectStartChannel),
  codexReconnectStart: (ref: string) => ipcRenderer.invoke(CodexReconnectStartChannel, ref),
  codexConnectStatus: () => ipcRenderer.invoke(CodexConnectStatusChannel),
  codexConnectOpenVerification: () => ipcRenderer.invoke(CodexConnectOpenChannel),
  /**
   * Provider-neutral fleet accounts (#8712): read-only projections. List is
   * renderer-argument-free; usage carries only a schema-validated account ref.
   */
  providerAccounts: {
    list: () => ipcRenderer.invoke(ProviderAccountsListChannel),
    usage: (ref: unknown) => {
      const request = decodeProviderAccountUsageRequest({ ref })
      return request === null
        ? Promise.resolve(unavailableProviderAccountUsageResult("unknown", "invalid_request"))
        : ipcRenderer.invoke(ProviderAccountsUsageChannel, request)
    },
  },
  /**
   * Fable local lane (#8712): schema-checked on both sides of the boundary.
   * The renderer never sees tokens, account homes, or raw SDK payloads —
   * only bounded, redacted typed events and the typed availability/result.
   */
  fableLocal: {
    availability: () => ipcRenderer.invoke(FableLocalAvailabilityChannel),
    start: (value: unknown) => {
      const request = decodeFableLocalStartRequest(value)
      return request === null
        ? Promise.resolve({ ok: false, error: "That message could not be sent." })
        : ipcRenderer.invoke(FableLocalStartChannel, request)
    },
    interrupt: (value: unknown) => {
      const request = decodeFableLocalInterruptRequest(value)
      return request === null ? Promise.resolve(false) : ipcRenderer.invoke(FableLocalInterruptChannel, request)
    },
    /**
     * EP250 question flow: answers a pending AskUserQuestion
     * ({ turnRef, questionRef, answers: [{ question, labels }] }).
     * Resolves false (typed rejection) on schema-invalid or unknown refs.
     */
    answerQuestion: (value: unknown) => {
      const request = decodeFableLocalAnswerQuestionRequest(value)
      return request === null
        ? Promise.resolve(false)
        : ipcRenderer.invoke(FableLocalAnswerQuestionChannel, request)
    },
    /**
     * G4 substrate: steer/interrupt a running delegate child
     * ({ turnRef, childRef, action, body? }). Resolves a typed not_found
     * outcome on schema-invalid input, never throws.
     */
    steerChild: (value: unknown) => {
      const request = decodeFableLocalSteerChildRequest(value)
      return request === null
        ? Promise.resolve({ ok: false, outcome: "not_found" })
        : ipcRenderer.invoke(FableLocalSteerChildChannel, request)
    },
    /**
     * A3 substrate: enqueue a follow-up while a turn streams
     * ({ threadRef, message }). Resolves no_active_turn on schema-invalid
     * input; delivery is queue-until-idle (followup_promoted event on end).
     */
    queueFollowup: (value: unknown) => {
      const request = decodeFableLocalQueueFollowupRequest(value)
      return request === null
        ? Promise.resolve({ ok: false, queued: false, reason: "no_active_turn" })
        : ipcRenderer.invoke(FableLocalQueueFollowupChannel, request)
    },
    onEvent: (listener: (envelope: FableLocalEventEnvelope) => void) => {
      const handler = (_event: unknown, value: unknown): void => {
        const decoded = decodeFableLocalEventEnvelope(value)
        if (decoded !== null) listener(decoded)
      }
      ipcRenderer.on(FableLocalEventChannel, handler)
      return () => ipcRenderer.removeListener(FableLocalEventChannel, handler)
    },
  },
  /**
   * Codex local lane (EP250 codex-first-class): mirrors the fableLocal
   * bridge exactly — same frozen request/event schemas, its own channels.
   * The renderer never sees tokens, account homes, or raw exec payloads.
   */
  codexLocal: {
    availability: () => ipcRenderer.invoke(CodexLocalAvailabilityChannel),
    start: (value: unknown) => {
      const request = decodeFableLocalStartRequest(value)
      return request === null
        ? Promise.resolve({ ok: false, error: "That message could not be sent." })
        : ipcRenderer.invoke(CodexLocalStartChannel, request)
    },
    interrupt: (value: unknown) => {
      const request = decodeFableLocalInterruptRequest(value)
      return request === null ? Promise.resolve(false) : ipcRenderer.invoke(CodexLocalInterruptChannel, request)
    },
    onEvent: (listener: (envelope: FableLocalEventEnvelope) => void) => {
      const handler = (_event: unknown, value: unknown): void => {
        const decoded = decodeFableLocalEventEnvelope(value)
        if (decoded !== null) listener(decoded)
      }
      ipcRenderer.on(CodexLocalEventChannel, handler)
      return () => ipcRenderer.removeListener(CodexLocalEventChannel, handler)
    },
  },
  /**
   * Session usage ledger (#8712 Lane C): renderer-argument-free snapshot plus
   * a schema-decoded push stream. Only refs, provider names, requested
   * models (spawn-config truth), counts, and token totals cross this line.
   */
  usageLedger: {
    snapshot: () => ipcRenderer.invoke(UsageLedgerSnapshotChannel),
    onEvent: (listener: (snapshot: UsageLedgerSnapshot) => void) => {
      const handler = (_event: unknown, value: unknown): void => {
        const decoded = decodeUsageLedgerSnapshot(value)
        if (decoded !== null) listener(decoded)
      }
      ipcRenderer.on(UsageLedgerEventChannel, handler)
      return () => ipcRenderer.removeListener(UsageLedgerEventChannel, handler)
    },
  },
  codingCatalog: {
    snapshot: async () => decodeDesktopCodingCatalogProjection(
      await ipcRenderer.invoke(DesktopCodingCatalogSnapshotChannel),
    ) ?? emptyDesktopCodingCatalogProjection(),
    choose: async () => decodeDesktopCodingCatalogProjection(
      await ipcRenderer.invoke(DesktopCodingCatalogChooseChannel),
    ) ?? emptyDesktopCodingCatalogProjection(),
    open: async (value: unknown) => {
      const request = decodeDesktopCodingSessionRequest(value)
      if (request === null) return emptyDesktopCodingCatalogProjection()
      return decodeDesktopCodingCatalogProjection(
        await ipcRenderer.invoke(DesktopCodingCatalogOpenChannel, request),
      ) ?? emptyDesktopCodingCatalogProjection()
    },
    archive: async (value: unknown) => {
      const request = decodeDesktopCodingSessionRequest(value)
      if (request === null) return emptyDesktopCodingCatalogProjection()
      return decodeDesktopCodingCatalogProjection(
        await ipcRenderer.invoke(DesktopCodingCatalogArchiveChannel, request),
      ) ?? emptyDesktopCodingCatalogProjection()
    },
    recover: async (value: unknown) => {
      const request = decodeDesktopCodingSessionRequest(value)
      if (request === null) return emptyDesktopCodingCatalogProjection()
      return decodeDesktopCodingCatalogProjection(
        await ipcRenderer.invoke(DesktopCodingCatalogRecoverChannel, request),
      ) ?? emptyDesktopCodingCatalogProjection()
    },
    focus: async (value: unknown) => {
      const request = decodeDesktopCodingFocusRequest(value)
      if (request === null) return emptyDesktopCodingCatalogProjection()
      return decodeDesktopCodingCatalogProjection(
        await ipcRenderer.invoke(DesktopCodingCatalogFocusChannel, request),
      ) ?? emptyDesktopCodingCatalogProjection()
    },
  },
  commands: {
    bindings: async () => decodeDesktopCommandBindingProjectionOrNull(
      await ipcRenderer.invoke(DesktopCommandBindingsChannel),
    ),
    saveBinding: async (value: unknown) => {
      const update = decodeDesktopCommandBindingUpdateOrNull(value)
      return update === null ? null : decodeDesktopCommandBindingProjectionOrNull(
        await ipcRenderer.invoke(DesktopCommandBindingSaveChannel, update),
      )
    },
    resetBindings: async () => decodeDesktopCommandBindingProjectionOrNull(
      await ipcRenderer.invoke(DesktopCommandBindingsResetChannel),
    ),
    onCommand: (listener: (command: DesktopDeferredCommand) => void) => {
      const handler = (_event: unknown, value: unknown): void => {
        const command = decodeDesktopDeferredCommandOrNull(value)
        if (command !== null) listener(command)
      }
      ipcRenderer.on(DesktopCommandEventChannel, handler)
      return () => ipcRenderer.removeListener(DesktopCommandEventChannel, handler)
    },
    ready: () => ipcRenderer.invoke(DesktopCommandReadyChannel),
  },
})
