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
   * Codex account reconnect (#8640 unblock): four renderer-argument-free
   * calls. Main holds every input (including the verification URL it opens);
   * the renderer only receives public-safe typed projections it schema-checks.
   */
  codexAccounts: () => ipcRenderer.invoke(CodexAccountsChannel),
  codexConnectStart: () => ipcRenderer.invoke(CodexConnectStartChannel),
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
})
