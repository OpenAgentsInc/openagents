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
import { FleetRunProjectionListChannel } from "./fleet-run-projection-contract.ts"
import {
  ProviderAccountsListChannel,
  ProviderAccountsUsageChannel,
  decodeProviderAccountUsageRequest,
  unavailableProviderAccountUsageResult,
} from "./provider-accounts-contract.ts"
import {
  DesktopChatTurnChannel,
  DesktopLocalTurnRecoveryUpdateChannel,
  DesktopForkHistoryThreadChannel,
  DesktopForkHistoryThreadRequestSchema,
  DesktopHydrateThreadChannel,
  DesktopLocalThreadsChannel,
  DesktopNewThreadChannel,
  DesktopOpenThreadChannel,
  DesktopResumeLocalThreadChannel,
  DesktopResumeLocalThreadRequestSchema,
  DesktopThreadsChannel,
  decode,
  DesktopThreadRequestSchema,
  DesktopThreadSchema,
  DesktopTurnRequestSchema,
} from "./chat-contract.ts"
import {
  DesktopWorkspaceChooseChannel,
  DesktopWorkspaceTreeChannel,
  DesktopWorkspaceSearchChannel,
  DesktopWorkspaceSearchCancelChannel,
  DesktopWorkspaceCreateChannel,
  DesktopWorkspaceRenameChannel,
  DesktopWorkspaceDeleteChannel,
  DesktopWorkspaceRevealChannel,
  DesktopWorkspaceDocumentOpenChannel,
  DesktopWorkspaceDocumentSaveChannel,
  DesktopWorkspaceDocumentSaveAsChannel,
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
  decodeWorkspaceDocumentRequest,
  decodeWorkspaceDocumentSaveRequest,
  decodeWorkspaceDocumentSaveAsRequest,
  decodeWorkspaceDocumentResult,
  decodeWorkspaceOperationResult,
  decodeWorkspaceTreePage,
  decodeWorkspaceTreeRequest,
  decodeWorkspaceWatchRequest,
  type DesktopWorkspaceChange,
} from "./workspace-contract.ts"
import { DesktopWindowFullscreenChannel } from "./window-contract.ts"
import {
  ProductSpecCreateChannel,
  ProductSpecEditConfirmChannel,
  ProductSpecEditProposeChannel,
  ProductSpecEvidenceRecordChannel,
  ProductSpecEvidenceVerifyChannel,
  ProductSpecOpenChannel,
  ProductSpecPacketAdmitChannel,
  ProductSpecPacketBlockChannel,
  ProductSpecPlanAcceptChannel,
  ProductSpecPlanProposeChannel,
  ProductSpecRunGetChannel,
  decodeProductSpecCreateRequest,
  decodeProductSpecEditConfirmRequest,
  decodeProductSpecEditConfirmationResult,
  decodeProductSpecEditProposalRequest,
  decodeProductSpecEditProposalResult,
  decodeProductSpecEvidenceRequest,
  decodeProductSpecOpenRequest,
  decodeProductSpecPacketAdmitRequest,
  decodeProductSpecPacketBlockRequest,
  decodeProductSpecPlanAcceptRequest,
  decodeProductSpecPlanProposalRequest,
  decodeProductSpecPlanResult,
  decodeProductSpecProjectionResult,
  decodeProductSpecRunGetRequest,
  decodeProductSpecRunResult,
  decodeProductSpecVerificationRequest,
} from "./product-spec-workroom-contract.ts"
import { GitGithubChannel, decodeGitGithubRequest, gitGithubError } from "./git-github-contract.ts"
import {
  TerminalCloseChannel,
  TerminalCreateChannel,
  TerminalEventChannel,
  TerminalInputChannel,
  TerminalInterruptChannel,
  TerminalPreviewOpenChannel,
  TerminalResizeChannel,
  TerminalRestartChannel,
  TerminalSnapshotChannel,
  decodeTerminalCreateRequest,
  decodeTerminalCreateResult,
  decodeTerminalAckResult,
  decodeTerminalEvent,
  decodeTerminalInputRequest,
  decodeTerminalPreviewOpenRequest,
  decodeTerminalPreviewOpenResult,
  decodeTerminalResizeRequest,
  decodeTerminalSessionRequest,
  decodeTerminalSnapshot,
  type TerminalEvent,
} from "./terminal-contract.ts"
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
  CodexLocalQueueFollowupChannel,
  CodexLocalStartChannel,
  CodexLocalSteerTurnChannel,
} from "./codex-local-contract.ts"
import {
  CodexHandoffOpenChannel,
  decodeCodexHandoffOpenRequest,
  decodeCodexHandoffOpenResult,
} from "./codex-handoff-contract.ts"
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
  PluginConfigChooseChannel,
  PluginConfigListChannel,
  PluginConfigRemoveChannel,
  PluginConfigToggleChannel,
  decodePluginRefRequest,
  decodePluginToggleRequest,
} from "./plugin-config-contract.ts"
import {
  DiagnosticsActionChannel,
  DiagnosticsExportChannel,
  DiagnosticsGatherChannel,
  decodeDiagnosticsAction,
} from "./diagnostics-contract.ts"
import {
  DesktopPreferencesGetChannel,
  DesktopPreferencesResetChannel,
  DesktopPreferencesUpdateChannel,
} from "./desktop-preferences-contract.ts"
import {
  UsageLedgerEventChannel,
  UsageLedgerSnapshotChannel,
  decodeUsageLedgerSnapshot,
  type UsageLedgerSnapshot,
} from "./usage-ledger-contract.ts"
import {
  LiveAgentGraphSnapshotChannel,
  LiveAgentGraphUpdateChannel,
  decodeLiveAgentGraphHostSnapshot,
  decodeLiveAgentGraphUpdate,
  type LiveAgentGraphHostSnapshot,
  type LiveAgentGraphUpdate,
} from "./live-agent-graph-contract.ts"
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
  localTurnRecovery: {
    onUpdate: (listener: (thread: import("./chat-contract.ts").DesktopThread) => void) => {
      const handler = (_event: unknown, value: unknown): void => {
        const thread = decode(DesktopThreadSchema, value) as import("./chat-contract.ts").DesktopThread | null
        if (thread !== null) listener(thread)
      }
      ipcRenderer.on(DesktopLocalTurnRecoveryUpdateChannel, handler)
      return () => ipcRenderer.removeListener(DesktopLocalTurnRecoveryUpdateChannel, handler)
    },
  },
  newThread: () => ipcRenderer.invoke(DesktopNewThreadChannel),
  openThread: (value: unknown) => {
    const request = decode(DesktopThreadRequestSchema, value) as { id: string } | null
    return request === null ? Promise.resolve(null) : ipcRenderer.invoke(DesktopOpenThreadChannel, request)
  },
  hydrateThread: (value: unknown) => {
    const request = decode(DesktopThreadRequestSchema, value) as { id: string } | null
    return request === null ? Promise.resolve(null) : ipcRenderer.invoke(DesktopHydrateThreadChannel, request)
  },
  /** H1/H2 history lifecycle bridge: fixed schema-decoded invokes only. */
  historyThreads: {
    listLocal: () => ipcRenderer.invoke(DesktopLocalThreadsChannel),
    resumeLocal: (value: unknown) => {
      const request = decode(DesktopResumeLocalThreadRequestSchema, value)
      return request === null ? Promise.resolve(null) : ipcRenderer.invoke(DesktopResumeLocalThreadChannel, request)
    },
    fork: (value: unknown) => {
      const request = decode(DesktopForkHistoryThreadRequestSchema, value)
      return request === null ? Promise.resolve(null) : ipcRenderer.invoke(DesktopForkHistoryThreadChannel, request)
    },
  },
  sendMessage: (value: unknown) => {
    const request = decode(DesktopTurnRequestSchema, value) as { id: string; message: string } | null
    return request === null ? Promise.resolve({ ok: false, error: "That message could not be sent." }) : ipcRenderer.invoke(DesktopChatTurnChannel, request)
  },
  chooseWorkspace: async (): Promise<boolean> => {
    const selected = await ipcRenderer.invoke(DesktopWorkspaceChooseChannel)
    return typeof selected === "object" && selected !== null
  },
  productSpec: {
    open: async (value: unknown) => {
      const request = decodeProductSpecOpenRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request", message: "The ProductSpec open request is invalid." }
      return decodeProductSpecProjectionResult(await ipcRenderer.invoke(ProductSpecOpenChannel, request)) ??
        { ok: false, reason: "read_failed", message: "The ProductSpec open response is invalid." }
    },
    create: async (value: unknown) => {
      const request = decodeProductSpecCreateRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request", message: "The ProductSpec create request is invalid." }
      return decodeProductSpecProjectionResult(await ipcRenderer.invoke(ProductSpecCreateChannel, request)) ??
        { ok: false, reason: "write_failed", message: "The ProductSpec create response is invalid." }
    },
    proposeEdit: async (value: unknown) => {
      const request = decodeProductSpecEditProposalRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request", message: "The ProductSpec edit proposal is invalid." }
      return decodeProductSpecEditProposalResult(await ipcRenderer.invoke(ProductSpecEditProposeChannel, request)) ??
        { ok: false, reason: "write_failed", message: "The ProductSpec edit response is invalid." }
    },
    confirmEdit: async (value: unknown) => {
      const request = decodeProductSpecEditConfirmRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request", message: "The ProductSpec edit confirmation is invalid." }
      return decodeProductSpecEditConfirmationResult(await ipcRenderer.invoke(ProductSpecEditConfirmChannel, request)) ??
        { ok: false, reason: "write_failed", message: "The ProductSpec confirmation response is invalid." }
    },
    proposePlan: async (value: unknown) => {
      const request = decodeProductSpecPlanProposalRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request", message: "The ProductSpec plan proposal is invalid." }
      return decodeProductSpecPlanResult(await ipcRenderer.invoke(ProductSpecPlanProposeChannel, request)) ??
        { ok: false, reason: "write_failed", message: "The ProductSpec plan response is invalid." }
    },
    acceptPlan: async (value: unknown) => {
      const request = decodeProductSpecPlanAcceptRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request", message: "The ProductSpec plan acceptance is invalid." }
      return decodeProductSpecRunResult(await ipcRenderer.invoke(ProductSpecPlanAcceptChannel, request)) ??
        { ok: false, reason: "write_failed", message: "The ProductSpec acceptance response is invalid." }
    },
    admitPacket: async (value: unknown) => {
      const request = decodeProductSpecPacketAdmitRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request", message: "The ProductSpec packet admission is invalid." }
      return decodeProductSpecRunResult(await ipcRenderer.invoke(ProductSpecPacketAdmitChannel, request)) ??
        { ok: false, reason: "invalid_transition", message: "The ProductSpec admission response is invalid." }
    },
    blockPacket: async (value: unknown) => {
      const request = decodeProductSpecPacketBlockRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request", message: "The ProductSpec packet block request is invalid." }
      return decodeProductSpecRunResult(await ipcRenderer.invoke(ProductSpecPacketBlockChannel, request)) ??
        { ok: false, reason: "invalid_transition", message: "The ProductSpec block response is invalid." }
    },
    recordEvidence: async (value: unknown) => {
      const request = decodeProductSpecEvidenceRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request", message: "The ProductSpec evidence request is invalid." }
      return decodeProductSpecRunResult(await ipcRenderer.invoke(ProductSpecEvidenceRecordChannel, request)) ??
        { ok: false, reason: "invalid_transition", message: "The ProductSpec evidence response is invalid." }
    },
    verifyEvidence: async (value: unknown) => {
      const request = decodeProductSpecVerificationRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request", message: "The ProductSpec verification request is invalid." }
      return decodeProductSpecRunResult(await ipcRenderer.invoke(ProductSpecEvidenceVerifyChannel, request)) ??
        { ok: false, reason: "invalid_transition", message: "The ProductSpec verification response is invalid." }
    },
    run: async (value: unknown) => {
      const request = decodeProductSpecRunGetRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request", message: "The ProductSpec run request is invalid." }
      return decodeProductSpecRunResult(await ipcRenderer.invoke(ProductSpecRunGetChannel, request)) ??
        { ok: false, reason: "read_failed", message: "The ProductSpec run response is invalid." }
    },
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
  openWorkspaceDocument: async (value: unknown) => {
    const request = decodeWorkspaceDocumentRequest(value)
    if (request === null) return { state: "unavailable", reason: "invalid_ref", message: "The document request is invalid." }
    const response = await ipcRenderer.invoke(DesktopWorkspaceDocumentOpenChannel, request)
    return decodeWorkspaceDocumentResult(response) ?? { state: "unavailable", reason: "unavailable", message: "The document response is invalid." }
  },
  saveWorkspaceDocument: async (value: unknown) => {
    const request = decodeWorkspaceDocumentSaveRequest(value)
    if (request === null) return { state: "unavailable", reason: "invalid_ref", message: "The document save request is invalid." }
    const response = await ipcRenderer.invoke(DesktopWorkspaceDocumentSaveChannel, request)
    return decodeWorkspaceDocumentResult(response) ?? { state: "unavailable", reason: "unavailable", message: "The document save response is invalid." }
  },
  saveWorkspaceDocumentAs: async (value: unknown) => {
    const request = decodeWorkspaceDocumentSaveAsRequest(value)
    if (request === null) return { state: "unavailable", reason: "invalid_ref", message: "The Save As request is invalid." }
    const response = await ipcRenderer.invoke(DesktopWorkspaceDocumentSaveAsChannel, request)
    return decodeWorkspaceDocumentResult(response) ?? { state: "unavailable", reason: "unavailable", message: "The Save As response is invalid." }
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
  /**
   * Workspace-bounded PTY terminals (CUT-20, #8700). Every op is a typed
   * intent schema-decoded HERE before it crosses the bridge — the renderer
   * sends a session ref and, for input/resize, bounded data / integer geometry.
   * It never sends a shell, an argv, a cwd, or an env: main binds those to the
   * authorized workspace. Output arrives as bounded, pre-redacted typed events.
   */
  terminal: {
    create: async (value: unknown) => {
      const request = decodeTerminalCreateRequest(value ?? {})
      if (request === null) {
        return { ok: false, reason: "invalid_request", message: "The terminal request is invalid." }
      }
      return decodeTerminalCreateResult(await ipcRenderer.invoke(TerminalCreateChannel, request))
    },
    input: async (value: unknown) => {
      const request = decodeTerminalInputRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request" }
      return decodeTerminalAckResult(await ipcRenderer.invoke(TerminalInputChannel, request))
    },
    resize: async (value: unknown) => {
      const request = decodeTerminalResizeRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request" }
      return decodeTerminalAckResult(await ipcRenderer.invoke(TerminalResizeChannel, request))
    },
    interrupt: async (value: unknown) => {
      const request = decodeTerminalSessionRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request" }
      return decodeTerminalAckResult(await ipcRenderer.invoke(TerminalInterruptChannel, request))
    },
    restart: async (value: unknown) => {
      const request = decodeTerminalSessionRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request" }
      return decodeTerminalAckResult(await ipcRenderer.invoke(TerminalRestartChannel, request))
    },
    close: async (value: unknown) => {
      const request = decodeTerminalSessionRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request" }
      return decodeTerminalAckResult(await ipcRenderer.invoke(TerminalCloseChannel, request))
    },
    snapshot: async () => decodeTerminalSnapshot(await ipcRenderer.invoke(TerminalSnapshotChannel)) ?? { sessions: [] },
    openPreview: async (value: unknown) => {
      const request = decodeTerminalPreviewOpenRequest(value)
      if (request === null) return { ok: false, reason: "invalid_request" }
      return decodeTerminalPreviewOpenResult(await ipcRenderer.invoke(TerminalPreviewOpenChannel, request))
    },
    onEvent: (listener: (event: TerminalEvent) => void) => {
      const handler = (_event: unknown, value: unknown): void => {
        const decoded = decodeTerminalEvent(value)
        if (decoded !== null) listener(decoded)
      }
      ipcRenderer.on(TerminalEventChannel, handler)
      return () => ipcRenderer.removeListener(TerminalEventChannel, handler)
    },
  },
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
  fleetRuns: {
    list: () => ipcRenderer.invoke(FleetRunProjectionListChannel),
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
    steerCurrent: (value: unknown) => {
      const request = decodeFableLocalQueueFollowupRequest(value)
      return request === null
        ? Promise.resolve({ ok: false, outcome: "not_found" })
        : ipcRenderer.invoke(CodexLocalSteerTurnChannel, request)
    },
    queueFollowup: (value: unknown) => {
      const request = decodeFableLocalQueueFollowupRequest(value)
      return request === null
        ? Promise.resolve({ ok: false, queued: false, reason: "no_active_turn" })
        : ipcRenderer.invoke(CodexLocalQueueFollowupChannel, request)
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
  codexHandoff: {
    open: async (value: unknown) => {
      const request = decodeCodexHandoffOpenRequest(value)
      if (request === null) {
        return { state: "refused", reason: "invalid_request", message: "The Open in Codex request is invalid." }
      }
      return decodeCodexHandoffOpenResult(await ipcRenderer.invoke(CodexHandoffOpenChannel, request)) ??
        { state: "refused", reason: "invalid_request", message: "The Open in Codex response is invalid." }
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
  /**
   * CUT-11 (#8691): canonical desktop-local live agent graph — validated
   * openagents.live_agent_graph.v1 post-images per thread (fable roots,
   * codex delegate children, codex-local roots), snapshot on invoke and
   * push on change. Presentation stays CUT-12; this only delivers the
   * canonical contract.
   */
  liveAgentGraph: {
    snapshot: async (): Promise<LiveAgentGraphHostSnapshot | null> =>
      decodeLiveAgentGraphHostSnapshot(await ipcRenderer.invoke(LiveAgentGraphSnapshotChannel)),
    onUpdate: (listener: (update: LiveAgentGraphUpdate) => void) => {
      const handler = (_event: unknown, value: unknown): void => {
        const decoded = decodeLiveAgentGraphUpdate(value)
        if (decoded !== null) listener(decoded)
      }
      ipcRenderer.on(LiveAgentGraphUpdateChannel, handler)
      return () => ipcRenderer.removeListener(LiveAgentGraphUpdateChannel, handler)
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
  pluginConfig: {
    list: () => ipcRenderer.invoke(PluginConfigListChannel),
    choose: () => ipcRenderer.invoke(PluginConfigChooseChannel),
    toggle: (value: unknown) => {
      const request = decodePluginToggleRequest(value)
      return request === null
        ? Promise.resolve({ state: "rejected", reason: "invalid plugin toggle" })
        : ipcRenderer.invoke(PluginConfigToggleChannel, request)
    },
    remove: (value: unknown) => {
      const request = decodePluginRefRequest(value)
      return request === null
        ? Promise.resolve({ state: "rejected", reason: "invalid plugin ref" })
        : ipcRenderer.invoke(PluginConfigRemoveChannel, request)
    },
  },
  /**
   * Typed durable preferences (CUT-24 #8704). Pass-through invoke; the renderer
   * schema-decodes every result. `update` sends a partial patch; main
   * field-normalizes it through the migrator, so an out-of-contract value can
   * never persist.
   */
  preferences: {
    get: () => ipcRenderer.invoke(DesktopPreferencesGetChannel),
    update: (value: unknown) => ipcRenderer.invoke(DesktopPreferencesUpdateChannel, value),
    reset: () => ipcRenderer.invoke(DesktopPreferencesResetChannel),
  },
  /**
   * Diagnostics / watchdog (CUT-24 #8704). Health gather + always-redacted
   * export + bounded recovery actions. The action name is validated here to the
   * bounded enum before it crosses; main re-validates.
   */
  diagnostics: {
    gather: () => ipcRenderer.invoke(DiagnosticsGatherChannel),
    exportRedacted: () => ipcRenderer.invoke(DiagnosticsExportChannel),
    runAction: (value: unknown) => {
      const action = decodeDiagnosticsAction(value)
      return action === null
        ? Promise.resolve({ ok: false, notice: "Unknown action" })
        : ipcRenderer.invoke(DiagnosticsActionChannel, action)
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
