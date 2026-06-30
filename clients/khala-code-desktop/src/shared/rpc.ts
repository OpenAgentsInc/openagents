import type { KhalaToolEvent } from "@openagentsinc/khala-tools"
import type { KhalaAppleFmReadiness } from "./apple-fm-readiness.js"
import type {
  KhalaCodexRateLimitProviderStatus,
  KhalaCodexRateLimitResetOutcome,
} from "./codex-rate-limits.js"
import type { OnDeviceDeciderSelection } from "./on-device-decider.js"

// Electrobun treats Infinity as no local request timeout; chat turns stream progress
// over events while hosted model calls and local tools can legitimately exceed 30s.
export const KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS = Number.POSITIVE_INFINITY
export const KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT = 50021

export type KhalaCodeDesktopMessageRole = "user" | "assistant" | "system" | "tool"

export type KhalaCodeDesktopMessage = {
  readonly id: string
  readonly role: KhalaCodeDesktopMessageRole
  readonly body: string
}

export type KhalaCodeDesktopChatTurnEvent =
  | {
      readonly message: KhalaCodeDesktopMessage
      readonly turnId: string
      readonly type: "message_start"
    }
  | {
      readonly delta: string
      readonly messageId: string
      readonly turnId: string
      readonly type: "message_delta"
    }
  | {
      readonly message: KhalaCodeDesktopMessage
      readonly turnId: string
      readonly type: "message_replace"
    }
  | {
      readonly messageId: string
      readonly turnId: string
      readonly type: "message_done"
    }
  | {
      readonly event: KhalaToolEvent
      readonly turnId: string
      readonly type: "tool_event"
    }

export type KhalaCodeDesktopUsage = {
  readonly input: number
  readonly cachedInput: number
  readonly output: number
  readonly reasoningOutput: number
}

export type KhalaCodeDesktopChatTurnRequest = {
  readonly messages: readonly KhalaCodeDesktopMessage[]
  readonly sessionId: string
  readonly turnId?: string
}

export type KhalaCodeDesktopBackendProjection = {
  readonly baseUrl?: string
  readonly credentialSource?: "env:OPENROUTER_API_KEY" | "khala-provider-key"
  readonly kind: "hosted_openagents" | "mock"
  readonly model: string
  readonly provider?: "openrouter"
}

export type KhalaCodeDesktopChatTurnResponse = {
  readonly backend: KhalaCodeDesktopBackendProjection
  readonly messages: readonly KhalaCodeDesktopMessage[]
  readonly ok: boolean
  readonly toolNames: readonly string[]
  readonly usage?: KhalaCodeDesktopUsage
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
  readonly status: "error" | "not_configured" | "ready" | "unavailable"
}

export type KhalaCodeDesktopCodexAccountStatus = {
  readonly provider: "codex"
  readonly accountRef: "default"
  readonly credentialSource: "CODEX_HOME" | "default_home"
  readonly homeRef: "env:CODEX_HOME" | "default:~/.codex"
  readonly readiness: {
    readonly state: "error" | "ready" | "credentials_missing"
    readonly blockerRefs: readonly string[]
  }
  readonly rateLimits: KhalaCodexRateLimitProviderStatus
}

export type KhalaCodeDesktopCodexAccountsStatus =
  KhalaCodeDesktopRuntimeStatus & {
    readonly capability: "codex_accounts"
    readonly accounts: readonly KhalaCodeDesktopCodexAccountStatus[]
    readonly rateLimits: KhalaCodexRateLimitProviderStatus
  }

export type KhalaCodeDesktopCodexRateLimitResetResult = {
  readonly ok: boolean
  readonly observedAt: string
  readonly outcome: KhalaCodexRateLimitResetOutcome | null
  readonly status: KhalaCodeDesktopCodexAccountsStatus
  readonly error?: string
}

export type KhalaCodeDesktopFleetAccount = {
  readonly accountRef: string
  readonly provider: "codex"
  readonly readiness: string
  readonly quotaState: string | null
  readonly accountKey: string | null
  readonly email: string | null
  readonly capacity: {
    readonly available: number | null
    readonly busy: number | null
    readonly queued: number | null
    readonly ready: number | null
  } | null
}

export type KhalaCodeDesktopFleetAssignment = {
  readonly assignmentRef: string | null
  readonly issueRef: string | null
  readonly updatedAt: string | null
}

export type KhalaCodeDesktopFleetProcess = {
  readonly pid: string
  readonly parentPid: string
  readonly elapsed: string
}

export type KhalaCodeDesktopFleetStatus = {
  readonly ok: boolean
  readonly observedAt: string
  readonly pylon: {
    readonly status: "online" | "started" | "unavailable"
    readonly pylonRef: string | null
    readonly message: string
  }
  readonly availableCodexAssignments: number | null
  readonly maxCodexAssignments: number | null
  readonly accounts: readonly KhalaCodeDesktopFleetAccount[]
  readonly activeAssignments: readonly KhalaCodeDesktopFleetAssignment[]
  readonly processes: readonly KhalaCodeDesktopFleetProcess[]
}

export type KhalaCodeDesktopRemoveAccountResult = {
  readonly ok: boolean
  readonly removed: boolean
  readonly accountRef: string
  readonly error?: string
}

export type KhalaCodeDesktopConnectStart = {
  readonly ok: boolean
  readonly accountRef: string
  readonly verificationUrl: string | null
  readonly userCode: string | null
  readonly output: string
  readonly error?: string
}

export type KhalaCodeDesktopRPCSchema = {
  requests: {
    appInfo(): Promise<KhalaCodeDesktopAppInfo>
    appleFmReadiness(): Promise<KhalaAppleFmReadiness>
    codexAccountsStatus(): Promise<KhalaCodeDesktopCodexAccountsStatus>
    codexFleetStatus(): Promise<KhalaCodeDesktopFleetStatus>
    connectCodexAccount(accountRef: string): Promise<KhalaCodeDesktopConnectStart>
    openExternalUrl(url: string): Promise<boolean>
    removeCodexAccount(accountRef: string): Promise<KhalaCodeDesktopRemoveAccountResult>
    codingStatus(): Promise<KhalaCodeDesktopRuntimeStatus>
    consumeCodexRateLimitResetCredit(): Promise<KhalaCodeDesktopCodexRateLimitResetResult>
    onDeviceDeciderStatus(): Promise<OnDeviceDeciderSelection>
    pylonStatus(): Promise<KhalaCodeDesktopRuntimeStatus>
    submitChatMessage(request: KhalaCodeDesktopChatTurnRequest): Promise<KhalaCodeDesktopChatTurnResponse>
    tokenAccountingStatus(): Promise<KhalaCodeDesktopRuntimeStatus>
    toolCatalog(): Promise<KhalaCodeDesktopToolCatalogResponse>
  }
  messages: {
    chatTurnEvent(event: KhalaCodeDesktopChatTurnEvent): void
  }
}
