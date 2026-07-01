import type { KhalaToolEvent } from "@openagentsinc/khala-tools"
import type { KhalaAppleFmReadiness } from "./apple-fm-readiness.js"
import type {
  KhalaCodexRateLimitProviderStatus,
  KhalaCodexRateLimitResetOutcome,
} from "./codex-rate-limits.js"
import type {
  KhalaCodeDesktopCodexApprovalAction,
  KhalaCodeDesktopCodexApprovalMethod,
  KhalaCodeDesktopCodexApprovalProjection,
  KhalaCodeDesktopCodexNetworkPolicyAmendment,
  KhalaCodeDesktopCodexPermissionProfile,
  KhalaCodeDesktopJsonRpcId,
} from "./codex-approval-decisions.js"
import type {
  KhalaCodeDesktopSlashCommandWithAvailability,
} from "./codex-slash-commands.js"
import type {
  KhalaCodeDesktopCodexJsonValue,
  KhalaCodeDesktopCodexSettingsProjection,
} from "./codex-settings.js"
import type {
  KhalaCodeDesktopCodexEcosystemProjection,
} from "./codex-ecosystem.js"
import type {
  KhalaCodeDesktopCodexThreadGroup,
  KhalaCodeDesktopCodexThreadSummary,
} from "./codex-threads.js"
import type { OnDeviceDeciderSelection } from "./on-device-decider.js"

// Electrobun treats Infinity as no local request timeout; chat turns stream progress
// over events while hosted model calls and local tools can legitimately exceed 30s.
export const KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS = Number.POSITIVE_INFINITY
export const KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT = 50021

export type KhalaCodeDesktopMessageRole = "user" | "assistant" | "system" | "tool"

export type KhalaCodeDesktopCodexItemCard = {
  readonly approval?: KhalaCodeDesktopCodexApprovalProjection
  readonly itemId: string
  readonly itemType: string
  readonly status: string
  readonly title: string
  readonly requestId?: string
  readonly subtitle?: string
  readonly threadId?: string
  readonly turnId?: string
}

export type KhalaCodeDesktopMessage = {
  readonly codexItem?: KhalaCodeDesktopCodexItemCard
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
  readonly kind: "codex_app_server" | "hosted_openagents" | "mock"
  readonly model: string
  readonly provider?: "openrouter"
  readonly threadId?: string
  readonly turnId?: string
  readonly turnStatus?: "completed" | "failed" | "inProgress" | "interrupted" | string
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
  readonly capability: "codex_accounts" | "codex_harness" | "coding" | "pylon" | "token_accounting"
  readonly observedAt: string
  readonly reason: string
  readonly status: "error" | "not_configured" | "ready" | "unavailable"
}

export type KhalaCodeDesktopCodexHarnessStatus =
  KhalaCodeDesktopRuntimeStatus & {
    readonly capability: "codex_harness"
    readonly binary: {
      readonly command: string
      readonly source:
        | "PATH"
        | "env:KHALA_CODE_CODEX_BINARY"
        | "env:KHALA_CODE_CODEX_COMMAND"
        | "input"
      readonly available: boolean
      readonly version: string | null
      readonly error: string | null
    }
    readonly home: {
      readonly path: string
      readonly source: "default:~/.codex" | "env:CODEX_HOME" | "input"
      readonly role: "main_user_codex_home"
      readonly authPath: string
      readonly fleetIsolation: "fleet_accounts_use_pylon_isolated_homes"
    }
    readonly auth: {
      readonly state: "credentials_missing" | "error" | "invalid" | "ready"
      readonly blockerRefs: readonly string[]
      readonly accessTokenPresent: boolean
      readonly accountIdPresent: boolean
      readonly refreshTokenPresent: boolean
      readonly error?: string
    }
    readonly signIn: {
      readonly required: boolean
      readonly command: "codex login"
      readonly warning: string
    }
  }

export type KhalaCodeDesktopCodexAppServerStatus = {
  readonly ok: true
  readonly app: "Khala Code Desktop"
  readonly adapterVersion: string
  readonly codexCommand: string
  readonly codexHome: string
  readonly diagnostics: readonly string[]
  readonly initialized: boolean
  readonly initializeResult: unknown
  readonly lastError: string | null
  readonly pendingRequestCount: number
  readonly pid: number | null
  readonly state: "errored" | "running" | "starting" | "stopped"
  readonly transport: "stdio"
}

export type KhalaCodeDesktopCodexAppServerControlResult = {
  readonly ok: boolean
  readonly action: "restart" | "start" | "stop"
  readonly changed: boolean
  readonly status: KhalaCodeDesktopCodexAppServerStatus
  readonly error?: string
}

export type KhalaCodeDesktopCodexThreadStartRequest = {
  readonly cwd?: string
  readonly sessionId?: string
}

export type KhalaCodeDesktopCodexThreadResumeRequest = {
  readonly cwd?: string
  readonly sessionId?: string
  readonly threadId: string
}

export type KhalaCodeDesktopCodexThreadListRequest = {
  readonly archived?: boolean
  readonly cursor?: string
  readonly cwd?: string
  readonly limit?: number
  readonly searchTerm?: string
  readonly sessionId?: string
}

export type KhalaCodeDesktopCodexThreadResult = {
  readonly ok: true
  readonly cwd?: string
  readonly desktopSessionId?: string
  readonly messages?: readonly KhalaCodeDesktopMessage[]
  readonly model?: string
  readonly modelProvider?: string
  readonly thread: unknown
  readonly threadId: string
}

export type KhalaCodeDesktopCodexThreadListResult = {
  readonly ok: true
  readonly backwardsCursor?: string | null
  readonly data: readonly unknown[]
  readonly groups?: readonly KhalaCodeDesktopCodexThreadGroup[]
  readonly nextCursor?: string | null
  readonly threads?: readonly KhalaCodeDesktopCodexThreadSummary[]
}

export type KhalaCodeDesktopCodexThreadReadRequest = {
  readonly includeTurns?: boolean
  readonly threadId: string
}

export type KhalaCodeDesktopCodexThreadForkRequest = {
  readonly cwd?: string
  readonly lastTurnId?: string
  readonly sessionId?: string
  readonly threadId: string
}

export type KhalaCodeDesktopCodexThreadIdRequest = {
  readonly threadId: string
}

export type KhalaCodeDesktopCodexThreadRenameRequest = {
  readonly name: string
  readonly threadId: string
}

export type KhalaCodeDesktopCodexThreadMutationResult = {
  readonly action: "archive" | "delete" | "fork" | "rename" | "unarchive"
  readonly ok: boolean
  readonly messages?: readonly KhalaCodeDesktopMessage[]
  readonly response?: unknown
  readonly thread?: unknown
  readonly threadId: string
  readonly error?: string
  readonly newThreadId?: string
}

export type KhalaCodeDesktopCodexTurnStartRequest =
  KhalaCodeDesktopChatTurnRequest & {
    readonly cwd?: string
  }

export type KhalaCodeDesktopCodexTurnSteerRequest = {
  readonly clientUserMessageId?: string
  readonly sessionId: string
  readonly text: string
  readonly turnId?: string
}

export type KhalaCodeDesktopCodexTurnInterruptRequest = {
  readonly sessionId: string
  readonly turnId?: string
}

export type KhalaCodeDesktopCodexTurnActionResult = {
  readonly ok: boolean
  readonly codexTurnId?: string
  readonly desktopSessionId: string
  readonly desktopTurnId?: string
  readonly error?: string
  readonly response?: unknown
  readonly threadId?: string
}

export type KhalaCodeDesktopCodexThreadCompactRequest = {
  readonly sessionId?: string
  readonly threadId?: string
}

export type KhalaCodeDesktopCodexApprovalRespondRequest = {
  readonly action: KhalaCodeDesktopCodexApprovalAction
  readonly execpolicyAmendment?: readonly string[]
  readonly method: KhalaCodeDesktopCodexApprovalMethod
  readonly networkPolicyAmendment?: KhalaCodeDesktopCodexNetworkPolicyAmendment
  readonly permissions?: KhalaCodeDesktopCodexPermissionProfile
  readonly requestId: KhalaCodeDesktopJsonRpcId
}

export type KhalaCodeDesktopCodexApprovalRespondResult = {
  readonly method: KhalaCodeDesktopCodexApprovalMethod
  readonly ok: boolean
  readonly payload?: unknown
  readonly requestId: KhalaCodeDesktopJsonRpcId
  readonly error?: string
}

export type KhalaCodeDesktopCodexSettingsReadRequest = {
  readonly cwd?: string
  readonly includeHiddenModels?: boolean
}

export type KhalaCodeDesktopCodexSettingsReadResult =
  KhalaCodeDesktopCodexSettingsProjection

export type KhalaCodeDesktopCodexConfigValueWriteRequest = {
  readonly cwd?: string
  readonly expectedVersion?: string
  readonly filePath?: string
  readonly keyPath: string
  readonly mergeStrategy?: "replace" | "upsert"
  readonly value: KhalaCodeDesktopCodexJsonValue
}

export type KhalaCodeDesktopCodexConfigValueWriteResult = {
  readonly ok: boolean
  readonly keyPath: string
  readonly response?: unknown
  readonly settings?: KhalaCodeDesktopCodexSettingsProjection
  readonly error?: string
}

export type KhalaCodeDesktopCodexEcosystemReadRequest = {
  readonly cwd?: string
  readonly forceRefetchApps?: boolean
  readonly forceReloadSkills?: boolean
  readonly threadId?: string
}

export type KhalaCodeDesktopCodexEcosystemReadResult =
  KhalaCodeDesktopCodexEcosystemProjection

export type KhalaCodeDesktopCodexAppServerActionResult = {
  readonly ok: boolean
  readonly method: string
  readonly response?: unknown
  readonly error?: string
}

export type KhalaCodeDesktopCodexSkillsExtraRootsSetRequest = {
  readonly extraRoots: readonly string[]
}

export type KhalaCodeDesktopCodexSkillsConfigWriteRequest = {
  readonly enabled: boolean
  readonly name?: string | null
  readonly path?: string | null
}

export type KhalaCodeDesktopCodexMcpResourceReadRequest = {
  readonly server: string
  readonly threadId?: string | null
  readonly uri: string
}

export type KhalaCodeDesktopCodexMcpToolCallRequest = {
  readonly arguments?: KhalaCodeDesktopCodexJsonValue
  readonly meta?: KhalaCodeDesktopCodexJsonValue
  readonly server: string
  readonly threadId: string
  readonly tool: string
}

export type KhalaCodeDesktopCodexMcpOauthLoginRequest = {
  readonly scopes?: readonly string[] | null
  readonly server: string
  readonly threadId?: string | null
  readonly timeoutSecs?: number | null
}

export type KhalaCodeDesktopCodexMarketplaceAddRequest = {
  readonly refName?: string | null
  readonly source: string
  readonly sparsePaths?: readonly string[] | null
}

export type KhalaCodeDesktopCodexMarketplaceRemoveRequest = {
  readonly marketplaceName: string
}

export type KhalaCodeDesktopCodexMarketplaceUpgradeRequest = {
  readonly marketplaceName?: string | null
}

export type KhalaCodeDesktopCodexPluginInstallRequest = {
  readonly marketplacePath?: string | null
  readonly pluginName: string
  readonly remoteMarketplaceName?: string | null
}

export type KhalaCodeDesktopCodexPluginUninstallRequest = {
  readonly pluginId: string
}

export type KhalaCodeDesktopSlashCommandListRequest = {
  readonly activeTurn?: boolean
  readonly debug?: boolean
  readonly platform?: string
  readonly sideConversation?: boolean
}

export type KhalaCodeDesktopSlashCommandListResponse = {
  readonly commands: readonly KhalaCodeDesktopSlashCommandWithAvailability[]
  readonly ok: true
}

export type KhalaCodeDesktopSlashCommandDispatchRequest =
  KhalaCodeDesktopSlashCommandListRequest & {
    readonly cwd?: string
    readonly raw: string
    readonly sessionId: string
    readonly threadId?: string
  }

export type KhalaCodeDesktopSlashCommandDispatchResult = {
  readonly action?: string
  readonly command?: string
  readonly message: string
  readonly method?: string
  readonly ok: boolean
  readonly response?: unknown
  readonly status:
    | "blocked"
    | "client_action"
    | "dispatched"
    | "gap"
    | "not_found"
  readonly threadId?: string
}

export type KhalaCodeDesktopCodexAccountStatus = {
  readonly provider: "codex"
  readonly accountRef: "default"
  readonly credentialSource: "CODEX_HOME" | "default_home"
  readonly homeRef: "env:CODEX_HOME" | "default:~/.codex"
  readonly homeRole: "main_user_codex_home"
  readonly readiness: {
    readonly state: "error" | "ready" | "credentials_missing" | "invalid"
    readonly blockerRefs: readonly string[]
  }
  readonly rateLimits: KhalaCodexRateLimitProviderStatus
}

export type KhalaCodeDesktopCodexAccountsStatus =
  KhalaCodeDesktopRuntimeStatus & {
    readonly capability: "codex_accounts"
    readonly accounts: readonly KhalaCodeDesktopCodexAccountStatus[]
    readonly harness: KhalaCodeDesktopCodexHarnessStatus
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
  readonly capacity: KhalaCodeDesktopFleetCapacity | null
  readonly email: string | null
}

export type KhalaCodeDesktopFleetCapacity = {
  readonly available: number | null
  readonly busy: number | null
  readonly queued: number | null
  readonly ready: number | null
}

export type KhalaCodeDesktopFleetTokenMeasurementStatus =
  | "exact"
  | "estimated"
  | "not_measured"
  | "pending"

export type KhalaCodeDesktopFleetAssignmentTokenRate = {
  readonly source: string
  readonly status: KhalaCodeDesktopFleetTokenMeasurementStatus
  readonly tokenCountKind: string | null
  readonly tokens: number | null
  readonly tokensPerMinute: number | null
}

export type KhalaCodeDesktopFleetTokenRate = {
  readonly activeAdjustedTokensPerMinute: number | null
  readonly completedStatus: KhalaCodeDesktopFleetTokenMeasurementStatus
  readonly completedTokenRows: number | null
  readonly completedTokensPerMinute: number | null
  readonly inFlightTokens: number | null
  readonly inFlightTokensPerMinute: number | null
  readonly source: "pylon_khala_apm" | "unavailable"
  readonly unavailableReason: string | null
}

export type KhalaCodeDesktopFleetAssignment = {
  readonly assignmentRef: string | null
  readonly elapsedMs: number | null
  readonly issueRef: string | null
  readonly tokenRate: KhalaCodeDesktopFleetAssignmentTokenRate
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
  readonly tokenRate: KhalaCodeDesktopFleetTokenRate
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
    codexAppServerRestart(): Promise<KhalaCodeDesktopCodexAppServerControlResult>
    codexAppServerStart(): Promise<KhalaCodeDesktopCodexAppServerControlResult>
    codexAppServerStatus(): Promise<KhalaCodeDesktopCodexAppServerStatus>
    codexAppServerStop(): Promise<KhalaCodeDesktopCodexAppServerControlResult>
    codexFleetStatus(): Promise<KhalaCodeDesktopFleetStatus>
    codexHarnessStatus(): Promise<KhalaCodeDesktopCodexHarnessStatus>
    codexApprovalRespond(request: KhalaCodeDesktopCodexApprovalRespondRequest): Promise<KhalaCodeDesktopCodexApprovalRespondResult>
    codexConfigValueWrite(request: KhalaCodeDesktopCodexConfigValueWriteRequest): Promise<KhalaCodeDesktopCodexConfigValueWriteResult>
    codexEcosystemRead(request?: KhalaCodeDesktopCodexEcosystemReadRequest): Promise<KhalaCodeDesktopCodexEcosystemReadResult>
    codexMarketplaceAdd(request: KhalaCodeDesktopCodexMarketplaceAddRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexMarketplaceRemove(request: KhalaCodeDesktopCodexMarketplaceRemoveRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexMarketplaceUpgrade(request?: KhalaCodeDesktopCodexMarketplaceUpgradeRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexMcpOauthLogin(request: KhalaCodeDesktopCodexMcpOauthLoginRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexMcpResourceRead(request: KhalaCodeDesktopCodexMcpResourceReadRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexMcpServerReload(): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexMcpToolCall(request: KhalaCodeDesktopCodexMcpToolCallRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexPluginInstall(request: KhalaCodeDesktopCodexPluginInstallRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexPluginUninstall(request: KhalaCodeDesktopCodexPluginUninstallRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexSettingsRead(request?: KhalaCodeDesktopCodexSettingsReadRequest): Promise<KhalaCodeDesktopCodexSettingsReadResult>
    codexSkillsConfigWrite(request: KhalaCodeDesktopCodexSkillsConfigWriteRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexSkillsExtraRootsSet(request: KhalaCodeDesktopCodexSkillsExtraRootsSetRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexThreadArchive(request: KhalaCodeDesktopCodexThreadIdRequest): Promise<KhalaCodeDesktopCodexThreadMutationResult>
    codexThreadCompact(request: KhalaCodeDesktopCodexThreadCompactRequest): Promise<KhalaCodeDesktopCodexTurnActionResult>
    codexThreadDelete(request: KhalaCodeDesktopCodexThreadIdRequest): Promise<KhalaCodeDesktopCodexThreadMutationResult>
    codexThreadFork(request: KhalaCodeDesktopCodexThreadForkRequest): Promise<KhalaCodeDesktopCodexThreadMutationResult>
    codexThreadList(request?: KhalaCodeDesktopCodexThreadListRequest): Promise<KhalaCodeDesktopCodexThreadListResult>
    codexThreadRead(request: KhalaCodeDesktopCodexThreadReadRequest): Promise<KhalaCodeDesktopCodexThreadResult>
    codexThreadRename(request: KhalaCodeDesktopCodexThreadRenameRequest): Promise<KhalaCodeDesktopCodexThreadMutationResult>
    codexThreadResume(request: KhalaCodeDesktopCodexThreadResumeRequest): Promise<KhalaCodeDesktopCodexThreadResult>
    codexThreadStart(request?: KhalaCodeDesktopCodexThreadStartRequest): Promise<KhalaCodeDesktopCodexThreadResult>
    codexThreadUnarchive(request: KhalaCodeDesktopCodexThreadIdRequest): Promise<KhalaCodeDesktopCodexThreadMutationResult>
    codexTurnInterrupt(request: KhalaCodeDesktopCodexTurnInterruptRequest): Promise<KhalaCodeDesktopCodexTurnActionResult>
    codexTurnStart(request: KhalaCodeDesktopCodexTurnStartRequest): Promise<KhalaCodeDesktopChatTurnResponse>
    codexTurnSteer(request: KhalaCodeDesktopCodexTurnSteerRequest): Promise<KhalaCodeDesktopCodexTurnActionResult>
    connectCodexAccount(accountRef: string): Promise<KhalaCodeDesktopConnectStart>
    openExternalUrl(url: string): Promise<boolean>
    removeCodexAccount(accountRef: string): Promise<KhalaCodeDesktopRemoveAccountResult>
    codingStatus(): Promise<KhalaCodeDesktopRuntimeStatus>
    consumeCodexRateLimitResetCredit(): Promise<KhalaCodeDesktopCodexRateLimitResetResult>
    onDeviceDeciderStatus(): Promise<OnDeviceDeciderSelection>
    pylonStatus(): Promise<KhalaCodeDesktopRuntimeStatus>
    slashCommandDispatch(request: KhalaCodeDesktopSlashCommandDispatchRequest): Promise<KhalaCodeDesktopSlashCommandDispatchResult>
    slashCommandList(request?: KhalaCodeDesktopSlashCommandListRequest): Promise<KhalaCodeDesktopSlashCommandListResponse>
    submitChatMessage(request: KhalaCodeDesktopChatTurnRequest): Promise<KhalaCodeDesktopChatTurnResponse>
    tokenAccountingStatus(): Promise<KhalaCodeDesktopRuntimeStatus>
    toolCatalog(): Promise<KhalaCodeDesktopToolCatalogResponse>
  }
  messages: {
    chatTurnEvent(event: KhalaCodeDesktopChatTurnEvent): void
  }
}
