import type { KhalaAppleFmReadiness } from "./apple-fm-readiness.js"
import type { OnDeviceDeciderSelection } from "./on-device-decider.js"

export const KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS = 30_000
export const KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT = 50021

export type KhalaCodeDesktopMessageRole = "user" | "assistant" | "system" | "tool"

export type KhalaCodeDesktopMessage = {
  readonly id: string
  readonly role: KhalaCodeDesktopMessageRole
  readonly body: string
}

export type KhalaCodeDesktopChatTurnRequest = {
  readonly messages: readonly KhalaCodeDesktopMessage[]
  readonly sessionId: string
}

export type KhalaCodeDesktopBackendProjection = {
  readonly baseUrl?: string
  readonly credentialSource?: "env:OPENROUTER_API_KEY" | "khala-provider-key"
  readonly kind: "hosted_openagents" | "mock" | "openrouter_byok"
  readonly model: string
  readonly provider?: "openrouter"
}

export type KhalaCodeDesktopChatTurnResponse = {
  readonly backend: KhalaCodeDesktopBackendProjection
  readonly messages: readonly KhalaCodeDesktopMessage[]
  readonly ok: boolean
  readonly toolNames: readonly string[]
  readonly usedTools: readonly string[]
}

export type KhalaCodeDesktopToolCatalogResponse = {
  readonly defaultEnabled: true
  readonly toolCount: number
  readonly tools: readonly {
    readonly authority: string
    readonly name: string
  }[]
}

export type KhalaCodeDesktopAppInfo = {
  readonly ok: true
  readonly app: "Khala Code Desktop"
  readonly observedAt: string
}

export type KhalaCodeDesktopRuntimeStatus = {
  readonly ok: true
  readonly app: "Khala Code Desktop"
  readonly available: boolean
  readonly capability: "codex_accounts" | "coding" | "pylon" | "token_accounting"
  readonly observedAt: string
  readonly reason: string
  readonly status: "not_configured" | "ready"
}

export type KhalaCodeDesktopRPCSchema = {
  requests: {
    appInfo(): Promise<KhalaCodeDesktopAppInfo>
    appleFmReadiness(): Promise<KhalaAppleFmReadiness>
    codexAccountsStatus(): Promise<KhalaCodeDesktopRuntimeStatus>
    codingStatus(): Promise<KhalaCodeDesktopRuntimeStatus>
    onDeviceDeciderStatus(): Promise<OnDeviceDeciderSelection>
    pylonStatus(): Promise<KhalaCodeDesktopRuntimeStatus>
    submitChatMessage(request: KhalaCodeDesktopChatTurnRequest): Promise<KhalaCodeDesktopChatTurnResponse>
    tokenAccountingStatus(): Promise<KhalaCodeDesktopRuntimeStatus>
    toolCatalog(): Promise<KhalaCodeDesktopToolCatalogResponse>
  }
}
