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
  decodeWorkspaceFileRequest,
  decodeWorkspaceGitDiffRequest,
  decodeWorkspaceSaveRequest,
} from "./workspace-contract.ts"
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
  FableLocalStartChannel,
  decodeFableLocalAnswerQuestionRequest,
  decodeFableLocalEventEnvelope,
  decodeFableLocalInterruptRequest,
  decodeFableLocalStartRequest,
  type FableLocalEventEnvelope,
} from "./fable-local-contract.ts"
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
  decodeDesktopDeferredCommandOrNull,
  type DesktopDeferredCommand,
} from "./desktop-command-contract.ts"

contextBridge.exposeInMainWorld("openagentsDesktop", {
  host: "electron",
  platform: process.platform,
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
