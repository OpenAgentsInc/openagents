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
  DesktopWorkspaceTreeChannel,
  DesktopWorkspaceSearchChannel,
  DesktopWorkspaceSearchCancelChannel,
  DesktopWorkspaceCreateChannel,
  DesktopWorkspaceRenameChannel,
  DesktopWorkspaceDeleteChannel,
  DesktopWorkspaceRevealChannel,
  DesktopWorkspaceRefreshChannel,
  DesktopWorkspaceWatchChannel,
  DesktopWorkspaceChangeChannel,
  decodeWorkspaceChange,
  decodeWorkspaceSearchBridgeRequest,
  decodeWorkspaceSearchCancelRequest,
  decodeWorkspaceSearchCancelResult,
  decodeWorkspaceSearchResponse,
  decodeWorkspaceCreateRequest,
  decodeWorkspaceRenameRequest,
  decodeWorkspaceDeleteRequest,
  decodeWorkspaceRevealRequest,
  decodeWorkspaceOperationResult,
  decodeWorkspaceTreePage,
  decodeWorkspaceTreeRequest,
  decodeWorkspaceWatchRequest,
  type DesktopWorkspaceChange,
} from "./workspace-contract.ts"
import { DesktopWindowFullscreenChannel } from "./window-contract.ts"
import { GitGithubChannel, decodeGitGithubRequest, gitGithubError } from "./git-github-contract.ts"
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
  FableLocalPickImagesChannel,
  FableLocalQueueFollowupChannel,
  FableLocalStartChannel,
  FableLocalSteerChildChannel,
  decodeFableLocalAnswerQuestionRequest,
  decodeFableLocalEventEnvelope,
  decodeFableLocalInterruptRequest,
  decodeFableLocalPickedImages,
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
  McpConfigAddChannel,
  McpConfigListChannel,
  McpConfigRemoveChannel,
  McpConfigToggleChannel,
  decodeMcpConfigAddRequest,
  decodeMcpConfigNameRequest,
  decodeMcpConfigToggleRequest,
} from "./mcp-config-contract.ts"
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
  chooseWorkspace: async (): Promise<boolean> => {
    const selected = await ipcRenderer.invoke(DesktopWorkspaceChooseChannel)
    return typeof selected === "object" && selected !== null
  },
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
  workspaceSearch: async (value: unknown) => {
    const request = decodeWorkspaceSearchBridgeRequest(value)
    if (request === null) {
      return {
        requestRef: "workspace.search.request.invalid",
        page: { state: "unavailable", message: "The workspace search request is invalid." },
      }
    }
    const response = await ipcRenderer.invoke(DesktopWorkspaceSearchChannel, request)
    return decodeWorkspaceSearchResponse(response) ?? {
      requestRef: request.requestRef,
      page: { state: "unavailable", message: "The workspace search response is invalid." },
    }
  },
  cancelWorkspaceSearch: async (value: unknown) => {
    const request = decodeWorkspaceSearchCancelRequest(value)
    if (request === null) {
      return { requestRef: "workspace.search.request.invalid", cancelled: false }
    }
    const response = await ipcRenderer.invoke(DesktopWorkspaceSearchCancelChannel, request)
    return decodeWorkspaceSearchCancelResult(response) ?? {
      requestRef: request.requestRef,
      cancelled: false,
    }
  },
  createWorkspaceEntry: async (value: unknown) => {
    const request = decodeWorkspaceCreateRequest(value)
    if (request === null) return { state: "unavailable", message: "The create request is invalid." }
    const response = await ipcRenderer.invoke(DesktopWorkspaceCreateChannel, request)
    return decodeWorkspaceOperationResult(response) ?? { state: "unavailable", message: "The create response is invalid." }
  },
  renameWorkspaceEntry: async (value: unknown) => {
    const request = decodeWorkspaceRenameRequest(value)
    if (request === null) return { state: "unavailable", message: "The rename request is invalid." }
    const response = await ipcRenderer.invoke(DesktopWorkspaceRenameChannel, request)
    return decodeWorkspaceOperationResult(response) ?? { state: "unavailable", message: "The rename response is invalid." }
  },
  deleteWorkspaceEntry: async (value: unknown) => {
    const request = decodeWorkspaceDeleteRequest(value)
    if (request === null) return { state: "unavailable", message: "The delete request is invalid." }
    const response = await ipcRenderer.invoke(DesktopWorkspaceDeleteChannel, request)
    return decodeWorkspaceOperationResult(response) ?? { state: "unavailable", message: "The delete response is invalid." }
  },
  revealWorkspaceEntry: async (value: unknown) => {
    const request = decodeWorkspaceRevealRequest(value)
    if (request === null) return { state: "unavailable", message: "The reveal request is invalid." }
    const response = await ipcRenderer.invoke(DesktopWorkspaceRevealChannel, request)
    return decodeWorkspaceOperationResult(response) ?? { state: "unavailable", message: "The reveal response is invalid." }
  },
  refreshWorkspace: async (): Promise<boolean> =>
    (await ipcRenderer.invoke(DesktopWorkspaceRefreshChannel)) === true,
  workspaceSubscribe: subscribeWorkspaceChanges,
  /**
   * Typed Git/GitHub surface (EP250 E2–E5): one narrow method over the closed
   * operation set. The request is schema-decoded here before it crosses the
   * bridge; a malformed request never reaches main. No raw ipcRenderer leaks.
   */
  gitGithub: {
    run: (value: unknown) => {
      const request = decodeGitGithubRequest(value)
      return request === null
        ? Promise.resolve(gitGithubError("status", "invalid_request", "The Git request could not be decoded."))
        : ipcRenderer.invoke(GitGithubChannel, request)
    },
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
     * Image file picker (capability I1): opens the native dialog in main and
     * returns decoded base64 attachments. The response is schema-decoded here
     * so a malformed/oversize payload can never cross into the renderer; an
     * invalid or absent response resolves to an empty array.
     */
    pickImages: async () => {
      const raw = await ipcRenderer.invoke(FableLocalPickImagesChannel)
      return decodeFableLocalPickedImages(raw) ?? []
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
  /**
   * User-configured MCP servers (I2, EP250 wave-2). The renderer sends the
   * Add form's typed config (schema-checked here) and only ever receives the
   * public-safe projection back — names/transport/enabled/command/url and
   * arg/env/header COUNTS, never secret values. Remove/toggle carry a
   * schema-validated name; main re-validates against the frozen schema.
   */
  mcpConfig: {
    list: () => ipcRenderer.invoke(McpConfigListChannel),
    add: (value: unknown) => {
      const request = decodeMcpConfigAddRequest(value)
      return request === null
        ? Promise.resolve({ state: "rejected", reason: "invalid server config" })
        : ipcRenderer.invoke(McpConfigAddChannel, request)
    },
    remove: (value: unknown) => {
      const request = decodeMcpConfigNameRequest(value)
      return request === null
        ? Promise.resolve({ state: "rejected", reason: "invalid server name" })
        : ipcRenderer.invoke(McpConfigRemoveChannel, request)
    },
    toggle: (value: unknown) => {
      const request = decodeMcpConfigToggleRequest(value)
      return request === null
        ? Promise.resolve({ state: "rejected", reason: "invalid toggle request" })
        : ipcRenderer.invoke(McpConfigToggleChannel, request)
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
