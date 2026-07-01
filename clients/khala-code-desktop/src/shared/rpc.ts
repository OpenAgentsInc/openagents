import { Schema as S } from "effect"
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

export type KhalaCodeDesktopRpcJsonPrimitive = string | number | boolean | null
export type KhalaCodeDesktopRpcJsonValue =
  | KhalaCodeDesktopRpcJsonPrimitive
  | readonly KhalaCodeDesktopRpcJsonValue[]
  | { readonly [key: string]: KhalaCodeDesktopRpcJsonValue }

export const KhalaCodeDesktopRpcJsonValue: S.Schema<KhalaCodeDesktopRpcJsonValue> =
  S.Union([
    S.String,
    S.Number,
    S.Boolean,
    S.Null,
    S.Array(S.suspend(() => KhalaCodeDesktopRpcJsonValue)),
    S.Record(S.String, S.suspend(() => KhalaCodeDesktopRpcJsonValue)),
  ])

export const KhalaCodeDesktopRpcDecodeFailure = S.Struct({
  error: S.String,
  method: S.String,
  ok: S.Literal(false),
  tag: S.Literal("rpc_decode_failed"),
})
export type KhalaCodeDesktopRpcDecodeFailure =
  typeof KhalaCodeDesktopRpcDecodeFailure.Type

export const KhalaCodeDesktopRpcHandlerFailure = S.Struct({
  error: S.String,
  method: S.String,
  ok: S.Literal(false),
  tag: S.Literal("rpc_handler_failed"),
})
export type KhalaCodeDesktopRpcHandlerFailure =
  typeof KhalaCodeDesktopRpcHandlerFailure.Type

export const KhalaCodeDesktopRpcMethodNotAllowedFailure = S.Struct({
  error: S.Literal("method_not_allowed"),
  method: S.String,
  ok: S.Literal(false),
  tag: S.Literal("rpc_method_not_allowed"),
})
export type KhalaCodeDesktopRpcMethodNotAllowedFailure =
  typeof KhalaCodeDesktopRpcMethodNotAllowedFailure.Type

export const KhalaCodeDesktopRpcUnknownMethodFailure = S.Struct({
  error: S.Literal("unknown_method"),
  method: S.String,
  ok: S.Literal(false),
  tag: S.Literal("rpc_unknown_method"),
})
export type KhalaCodeDesktopRpcUnknownMethodFailure =
  typeof KhalaCodeDesktopRpcUnknownMethodFailure.Type

export const KhalaCodeDesktopRpcBridgeFailure = S.Union([
  KhalaCodeDesktopRpcDecodeFailure,
  KhalaCodeDesktopRpcHandlerFailure,
  KhalaCodeDesktopRpcMethodNotAllowedFailure,
  KhalaCodeDesktopRpcUnknownMethodFailure,
])
export type KhalaCodeDesktopRpcBridgeFailure =
  typeof KhalaCodeDesktopRpcBridgeFailure.Type

export type KhalaCodeDesktopMessageRole = "user" | "assistant" | "system" | "tool"

export type KhalaCodeDesktopRuntimeMode =
  | "codex_harness"
  | "khala_native_runtime"

export type KhalaCodeDesktopToolCatalogKind =
  | "codex_harness_supplemental"
  | "khala_native_legacy"

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
      readonly threadId: string
      readonly turnId: string
      readonly type: "thread_ready"
    }
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
  readonly attachments?: readonly KhalaCodeDesktopChatTurnAttachment[]
  readonly messages: readonly KhalaCodeDesktopMessage[]
  readonly sessionId: string
  readonly startNewThread?: boolean
  readonly threadId?: string
  readonly turnId?: string
}

export type KhalaCodeDesktopChatTurnAttachment = {
  readonly dataBase64?: string
  readonly id: string
  readonly kind: "image"
  readonly mime: string
  readonly name: string
  readonly path?: string
  readonly sizeBytes: number
}

export type KhalaCodeDesktopBackendProjection = {
  readonly baseUrl?: string
  readonly blockerRefs?: readonly string[]
  readonly credentialSource?: "env:OPENROUTER_API_KEY" | "khala-provider-key"
  readonly kind: "codex_app_server" | "hosted_openagents" | "mock"
  readonly model: string
  readonly provider?: "openrouter"
  readonly runtimeMode?: KhalaCodeDesktopRuntimeMode
  readonly threadId?: string
  readonly toolCatalogKind?: KhalaCodeDesktopToolCatalogKind | "codex_app_server"
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
  readonly catalogKind: KhalaCodeDesktopToolCatalogKind
  readonly defaultEnabled: boolean
  readonly description: string
  readonly runtimeMode: KhalaCodeDesktopRuntimeMode
  readonly toolCount: number
  readonly tools: readonly {
    readonly authority: string
    readonly name: string
    readonly role: "legacy_codex_equivalent" | "supplemental_swarm"
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

export type KhalaCodeDesktopThreadTokenSummaryRequest = {
  readonly threadId?: string | null
}

export type KhalaCodeDesktopThreadTokenSummary = {
  readonly auditRows: number
  readonly codexStateDbPath: string
  readonly codexStateTokens: number
  readonly leaderboardLabel: "OpenAgents Stats"
  readonly leaderboardSyncedTokens: number
  readonly localLedgerPath: string
  readonly localMessageAuditLedgerPath: string
  readonly missingUsageTurns: number
  readonly ok: true
  readonly pendingSyncTokens: number
  readonly remoteConfigured: boolean
  readonly remoteDisabled: boolean
  readonly threadId: string | null
  readonly totalTokens: number
  readonly updatedAt: string | null
  readonly usageEventRows: number
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
  readonly useStateDbOnly?: boolean
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

export type KhalaCodeDesktopCodexBackgroundTerminalsListRequest = {
  readonly cursor?: string | null
  readonly limit?: number | null
  readonly threadId: string
}

export type KhalaCodeDesktopCodexBackgroundTerminalsCleanRequest = {
  readonly threadId: string
}

export type KhalaCodeDesktopCodexBackgroundTerminalsTerminateRequest = {
  readonly processId: string
  readonly threadId: string
}

export type KhalaCodeDesktopCodexMentionCandidate = {
  readonly fileName: string
  readonly kind: "directory" | "file"
  readonly path: string
  readonly root?: string
  readonly score?: number
}

export type KhalaCodeDesktopCodexMentionCandidatesRequest = {
  readonly cwd?: string
  readonly query?: string
}

export type KhalaCodeDesktopCodexMentionCandidatesResult = {
  readonly ok: boolean
  readonly candidates: readonly KhalaCodeDesktopCodexMentionCandidate[]
  readonly source: "fs/readDirectory" | "fuzzyFileSearch"
  readonly truncated: boolean
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

export type KhalaCodeDesktopCodexExternalAgentConfigDetectRequest = {
  readonly cwds?: readonly string[] | null
  readonly includeHome?: boolean
}

export type KhalaCodeDesktopCodexExternalAgentConfigMigrationItem = {
  readonly itemType:
    | "AGENTS_MD"
    | "COMMANDS"
    | "CONFIG"
    | "HOOKS"
    | "MCP_SERVER_CONFIG"
    | "PLUGINS"
    | "SESSIONS"
    | "SKILLS"
    | "SUBAGENTS"
    | string
  readonly description: string
  readonly cwd: string | null
  readonly details?: KhalaCodeDesktopCodexJsonValue | null
}

export type KhalaCodeDesktopCodexExternalAgentConfigImportRequest = {
  readonly migrationItems: readonly KhalaCodeDesktopCodexExternalAgentConfigMigrationItem[]
  readonly source?: string | null
}

export type KhalaCodeDesktopCodexFsPathRequest = {
  readonly path: string
}

export type KhalaCodeDesktopCodexFsWriteFileRequest = {
  readonly dataBase64: string
  readonly path: string
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
  readonly gap?: {
    readonly gapId: string
    readonly kind: "upstream_app_server_gap"
  }
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
    | "unavailable"
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
  readonly homeRole?: KhalaCodeDesktopFleetHomeRole
  readonly queuePolicy?: KhalaCodeDesktopFleetQueuePolicy
  readonly sessionRole?: KhalaCodeDesktopFleetSessionRole
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

export type KhalaCodeDesktopFleetSessionRole =
  | "main_local_codex_session"
  | "swarm_worker_codex_session"

export type KhalaCodeDesktopFleetHomeRole =
  | "main_user_codex_home_display_only"
  | "pylon_isolated_worker_codex_home"

export type KhalaCodeDesktopFleetQueuePolicy = {
  readonly admission: "pylon_capacity_gate"
  readonly cooldown: "none_reported" | "ready" | "cooling_down" | "unknown"
  readonly refill: "pylon_presence_heartbeat"
  readonly queued: number | null
}

export type KhalaCodeDesktopFleetSessionLayer = {
  readonly label: string
  readonly role: KhalaCodeDesktopFleetSessionRole
  readonly homeRole: KhalaCodeDesktopFleetHomeRole
  readonly runtime: "codex_harness"
  readonly transcriptSurface: "chat" | "fleet"
  readonly mutationPolicy: "codex_app_server_owned" | "pylon_isolated_home_only"
}

export type KhalaCodeDesktopFleetWorkerSession = {
  readonly approvalState:
    | "approval_required"
    | "blocked"
    | "none"
    | "ready_for_review"
  readonly blockerRefs: readonly string[]
  readonly closeoutStatus: string | null
  readonly executionRuntime: "codex_harness"
  readonly homeRole: KhalaCodeDesktopFleetHomeRole
  readonly queuePolicy: KhalaCodeDesktopFleetQueuePolicy
  readonly reviewState: "active" | "blocked" | "pending_closeout" | "ready_for_review"
  readonly role: "swarm_worker_codex_session"
  readonly transcriptRef: string | null
}

export type KhalaCodeDesktopFleetAssignment = {
  readonly assignmentRef: string | null
  readonly blockerRefs?: readonly string[]
  readonly closeoutStatus?: string | null
  readonly elapsedMs: number | null
  readonly issueRef: string | null
  readonly workerSession?: KhalaCodeDesktopFleetWorkerSession
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
  readonly sessionLayers?: {
    readonly main: KhalaCodeDesktopFleetSessionLayer
    readonly workers: KhalaCodeDesktopFleetSessionLayer
  }
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

export type KhalaCodeDesktopFleetPromotionContextBoundary = {
  readonly allowedRefs: readonly string[]
  readonly includeTranscript: false
  readonly mode: "explicit_objective" | "summary_only"
  readonly summary: string | null
}

export type KhalaCodeDesktopFleetPromotionRequest = {
  readonly accountRef?: string
  readonly branch?: string
  readonly commit?: string
  readonly contextBoundary: KhalaCodeDesktopFleetPromotionContextBoundary
  readonly count?: number
  readonly fixture?: boolean
  readonly noRun?: boolean
  readonly objective: string
  readonly repo?: string
  readonly sessionId: string
  readonly threadId: string
  readonly timeoutMs?: number
  readonly verify?: string
}

export type KhalaCodeDesktopFleetPromotionResult = {
  readonly ok: boolean
  readonly acceptedCount: number
  readonly contextBoundary: KhalaCodeDesktopFleetPromotionContextBoundary
  readonly origin: {
    readonly role: "main_local_codex_session"
    readonly sessionId: string
    readonly threadId: string
  }
  readonly pylonRef: string | null
  readonly requestedCount: number
  readonly workerRuntime: {
    readonly assignmentTool: "codex_spawn"
    readonly homeRole: "pylon_isolated_worker_codex_home"
    readonly role: "swarm_worker_codex_session"
    readonly runtime: "codex_harness"
  }
  readonly results: readonly {
    readonly accountRef: string | null
    readonly assignmentRef: string | null
    readonly closeoutStatus: string | null
    readonly status: "accepted" | "failed"
    readonly summary: string
    readonly tokensVerified: number | null
    readonly transcriptRef: string | null
  }[]
}

export type KhalaCodeDesktopFleetDelegateRunMode = "fixture" | "real_work"

export type KhalaCodeDesktopFleetDelegateRunRequest = {
  readonly accountRef?: string
  readonly branch?: string
  readonly commit?: string
  readonly count?: number
  readonly mode: KhalaCodeDesktopFleetDelegateRunMode
  readonly noRun?: boolean
  readonly objective: string
  readonly repo?: string
  readonly timeoutMs?: number
  readonly verify?: string
}

export type KhalaCodeDesktopFleetDelegateRunStep = {
  readonly blockerCode: string | null
  readonly fallbackModule: string | null
  readonly module: string
  readonly precondition: string
  readonly refs: readonly string[]
  readonly status: "blocked" | "recovered" | "satisfied" | string
  readonly summary: string
}

export type KhalaCodeDesktopFleetDelegateRunResult = {
  readonly ok: boolean
  readonly acceptedCount: number
  readonly delegateSignature: "khala.fleet.delegate"
  readonly delegateStatus: "blocked" | "completed"
  readonly mode: KhalaCodeDesktopFleetDelegateRunMode
  readonly projection: {
    readonly localPathsProjected: false
    readonly objectiveProjected: false
    readonly providerPayloadProjected: false
    readonly rawTraceMessagesProjected: false
  }
  readonly pylonRef: string | null
  readonly requestedCount: number
  readonly results: readonly {
    readonly accountRef: string | null
    readonly assignmentRef: string | null
    readonly blockerRefs: readonly string[]
    readonly closeoutStatus: string | null
    readonly slot: number
    readonly status: "accepted" | "failed"
    readonly tokensVerified: number | null
    readonly transcriptRef: string | null
  }[]
  readonly trace: readonly KhalaCodeDesktopFleetDelegateRunStep[]
  readonly validation: {
    readonly fixture: boolean
    readonly repoPinsComplete: boolean
  }
  readonly workerRuntime: {
    readonly assignmentTool: "codex_spawn"
    readonly homeRole: "pylon_isolated_worker_codex_home"
    readonly role: "swarm_worker_codex_session"
    readonly runtime: "codex_harness"
  }
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

const RpcJson = KhalaCodeDesktopRpcJsonValue
const RpcStringArray = S.Array(S.String)
const RpcJsonObject = S.Record(S.String, RpcJson)
const RpcStringNull = S.NullOr(S.String)
const RpcNumberNull = S.NullOr(S.Number)

const RpcToolEvent = S.Struct({
  eventId: S.String,
  invocationId: S.optional(S.String),
  kind: S.String,
  payload: RpcJson,
  sessionId: S.String,
})

const RpcCodexPermissionProfile = S.Struct({
  fileSystem: S.optional(S.Struct({
    entries: S.optional(S.Array(RpcJson)),
    globScanMaxDepth: S.optional(S.Number),
    read: S.optional(S.NullOr(RpcStringArray)),
    write: S.optional(S.NullOr(RpcStringArray)),
  })),
  network: S.optional(S.Struct({
    enabled: S.optional(S.NullOr(S.Boolean)),
  })),
})

const RpcCodexApprovalProjection = S.Struct({
  additionalPermissions: S.optional(RpcJson),
  availableDecisions: S.optional(S.Array(RpcJson)),
  command: S.optional(S.String),
  cwd: S.optional(S.String),
  grantRoot: S.optional(S.String),
  method: S.String,
  networkApprovalContext: S.optional(RpcJson),
  permissions: S.optional(RpcCodexPermissionProfile),
  proposedExecpolicyAmendment: S.optional(RpcStringArray),
  proposedNetworkPolicyAmendments: S.optional(S.Array(S.Struct({
    action: S.String,
    host: S.String,
  }))),
  reason: S.optional(S.String),
  requestId: S.Union([S.String, S.Number]),
})

const RpcCodexItemCard = S.Struct({
  approval: S.optional(RpcCodexApprovalProjection),
  itemId: S.String,
  itemType: S.String,
  status: S.String,
  title: S.String,
  requestId: S.optional(S.String),
  subtitle: S.optional(S.String),
  threadId: S.optional(S.String),
  turnId: S.optional(S.String),
})

export const KhalaCodeDesktopMessageSchema = S.Struct({
  codexItem: S.optional(RpcCodexItemCard),
  id: S.String,
  role: S.Literals(["user", "assistant", "system", "tool"]),
  body: S.String,
})
export type KhalaCodeDesktopMessageFromSchema =
  typeof KhalaCodeDesktopMessageSchema.Type

export const KhalaCodeDesktopChatTurnEventSchema = S.Union([
  S.Struct({
    threadId: S.String,
    turnId: S.String,
    type: S.Literal("thread_ready"),
  }),
  S.Struct({
    message: KhalaCodeDesktopMessageSchema,
    turnId: S.String,
    type: S.Literal("message_start"),
  }),
  S.Struct({
    delta: S.String,
    messageId: S.String,
    turnId: S.String,
    type: S.Literal("message_delta"),
  }),
  S.Struct({
    message: KhalaCodeDesktopMessageSchema,
    turnId: S.String,
    type: S.Literal("message_replace"),
  }),
  S.Struct({
    messageId: S.String,
    turnId: S.String,
    type: S.Literal("message_done"),
  }),
  S.Struct({
    event: RpcToolEvent,
    turnId: S.String,
    type: S.Literal("tool_event"),
  }),
])
export type KhalaCodeDesktopChatTurnEventFromSchema =
  typeof KhalaCodeDesktopChatTurnEventSchema.Type

const RpcUsage = S.Struct({
  input: S.Number,
  cachedInput: S.Number,
  output: S.Number,
  reasoningOutput: S.Number,
})

const RpcChatAttachment = S.Struct({
  dataBase64: S.optional(S.String),
  id: S.String,
  kind: S.Literal("image"),
  mime: S.String,
  name: S.String,
  path: S.optional(S.String),
  sizeBytes: S.Number,
})

const RpcChatTurnRequest = S.Struct({
  attachments: S.optional(S.Array(RpcChatAttachment)),
  messages: S.Array(KhalaCodeDesktopMessageSchema),
  sessionId: S.String,
  startNewThread: S.optional(S.Boolean),
  threadId: S.optional(S.String),
  turnId: S.optional(S.String),
})

const RpcBackendProjection = S.Struct({
  baseUrl: S.optional(S.String),
  blockerRefs: S.optional(RpcStringArray),
  credentialSource: S.optional(S.String),
  kind: S.String,
  model: S.String,
  provider: S.optional(S.String),
  runtimeMode: S.optional(S.String),
  threadId: S.optional(S.String),
  toolCatalogKind: S.optional(S.String),
  turnId: S.optional(S.String),
  turnStatus: S.optional(S.String),
})

const RpcChatTurnResponse = S.Struct({
  backend: RpcBackendProjection,
  messages: S.Array(KhalaCodeDesktopMessageSchema),
  ok: S.Boolean,
  toolNames: RpcStringArray,
  usage: S.optional(RpcUsage),
  usedTools: RpcStringArray,
})

const RpcRuntimeStatus = S.Struct({
  ok: S.Literal(true),
  app: S.Literal("Khala Code Desktop"),
  available: S.Boolean,
  capability: S.String,
  observedAt: S.String,
  reason: S.String,
  status: S.String,
})

const RpcAppInfo = S.Struct({
  ok: S.Literal(true),
  app: S.Literal("Khala Code Desktop"),
  observedAt: S.String,
})

const RpcAppleFmReadiness = S.Struct({
  schema: S.String,
  kind: S.Literal("khala_desktop_apple_fm_readiness"),
  supported: S.Boolean,
  available: S.Boolean,
  state: S.String,
  backendKind: S.String,
  profileId: S.String,
  model: S.String,
  capability: S.String,
  provider: S.String,
  demandKind: S.Literal("own_capacity"),
  demandSource: S.String,
  usageTruth: S.Literal("estimated"),
  pylonControlConfigured: S.Boolean,
  pylon: S.NullOr(RpcJsonObject),
  blockerRefs: RpcStringArray,
  observedAt: S.String,
  contentRedacted: S.Literal(true),
})

const RpcCodexHarnessStatus = S.Struct({
  ok: S.Literal(true),
  app: S.Literal("Khala Code Desktop"),
  available: S.Boolean,
  capability: S.Literal("codex_harness"),
  observedAt: S.String,
  reason: S.String,
  status: S.String,
  binary: S.Struct({
    command: S.String,
    source: S.String,
    available: S.Boolean,
    version: RpcStringNull,
    error: RpcStringNull,
  }),
  home: S.Struct({
    path: S.String,
    source: S.String,
    role: S.Literal("main_user_codex_home"),
    authPath: S.String,
    fleetIsolation: S.Literal("fleet_accounts_use_pylon_isolated_homes"),
  }),
  auth: S.Struct({
    state: S.String,
    blockerRefs: RpcStringArray,
    accessTokenPresent: S.Boolean,
    accountIdPresent: S.Boolean,
    refreshTokenPresent: S.Boolean,
    error: S.optional(S.String),
  }),
  signIn: S.Struct({
    required: S.Boolean,
    command: S.Literal("codex login"),
    warning: S.String,
  }),
})

const RpcRateLimitWindow = S.Struct({
  usedPercent: S.Number,
  remainingPercent: S.Number,
  windowMinutes: S.Number,
  resetsAtIso: RpcStringNull,
  resetDescription: RpcStringNull,
})

const RpcRateLimitStatus = S.Struct({
  provider: S.Literal("codex"),
  session: S.NullOr(RpcRateLimitWindow),
  weekly: S.NullOr(RpcRateLimitWindow),
  rateLimitResetCredits: S.optional(S.NullOr(S.Struct({
    availableCount: S.Number,
    totalEarnedCount: S.optional(S.Number),
    nextExpiresAtIso: RpcStringNull,
    credits: S.optional(S.Array(S.Struct({
      status: S.String,
      expiresAtIso: RpcStringNull,
      grantedAtIso: RpcStringNull,
    }))),
  }))),
  updatedAtIso: S.String,
  error: RpcStringNull,
  status: S.String,
})

const RpcCodexAccountsStatus = S.Struct({
  ok: S.Literal(true),
  app: S.Literal("Khala Code Desktop"),
  available: S.Boolean,
  capability: S.Literal("codex_accounts"),
  observedAt: S.String,
  reason: S.String,
  status: S.String,
  accounts: S.Array(S.Struct({
    provider: S.Literal("codex"),
    accountRef: S.Literal("default"),
    credentialSource: S.String,
    homeRef: S.String,
    homeRole: S.Literal("main_user_codex_home"),
    readiness: S.Struct({
      state: S.String,
      blockerRefs: RpcStringArray,
    }),
    rateLimits: RpcRateLimitStatus,
  })),
  harness: RpcCodexHarnessStatus,
  rateLimits: RpcRateLimitStatus,
})

const RpcCodexAppServerStatus = S.Struct({
  ok: S.Literal(true),
  app: S.Literal("Khala Code Desktop"),
  adapterVersion: S.String,
  codexCommand: S.String,
  codexHome: S.String,
  diagnostics: RpcStringArray,
  initialized: S.Boolean,
  initializeResult: RpcJson,
  lastError: RpcStringNull,
  pendingRequestCount: S.Number,
  pid: S.NullOr(S.Number),
  state: S.String,
  transport: S.Literal("stdio"),
})

const RpcCodexAppServerControlResult = S.Struct({
  ok: S.Boolean,
  action: S.String,
  changed: S.Boolean,
  status: RpcCodexAppServerStatus,
  error: S.optional(S.String),
})

const RpcThreadStartRequest = S.Struct({
  cwd: S.optional(S.String),
  sessionId: S.optional(S.String),
})
const RpcThreadResumeRequest = S.Struct({
  cwd: S.optional(S.String),
  sessionId: S.optional(S.String),
  threadId: S.String,
})
const RpcThreadListRequest = S.Struct({
  archived: S.optional(S.Boolean),
  cursor: S.optional(S.String),
  cwd: S.optional(S.String),
  limit: S.optional(S.Number),
  searchTerm: S.optional(S.String),
  sessionId: S.optional(S.String),
  useStateDbOnly: S.optional(S.Boolean),
})
const RpcThreadReadRequest = S.Struct({
  includeTurns: S.optional(S.Boolean),
  threadId: S.String,
})
const RpcThreadForkRequest = S.Struct({
  cwd: S.optional(S.String),
  lastTurnId: S.optional(S.String),
  sessionId: S.optional(S.String),
  threadId: S.String,
})
const RpcThreadIdRequest = S.Struct({ threadId: S.String })
const RpcThreadRenameRequest = S.Struct({ name: S.String, threadId: S.String })

const RpcThreadSummary = S.Struct({
  id: S.String,
  sessionId: RpcStringNull,
  title: S.String,
  preview: S.String,
  cwd: RpcStringNull,
  projectLabel: S.String,
  status: S.String,
  statusLabel: S.String,
  modelProvider: RpcStringNull,
  source: S.String,
  forkedFromId: RpcStringNull,
  parentThreadId: RpcStringNull,
  createdAt: RpcNumberNull,
  updatedAt: RpcNumberNull,
  recencyAt: RpcNumberNull,
  badges: RpcStringArray,
})
const RpcThreadGroup = S.Struct({
  key: S.String,
  label: S.String,
  threadIds: RpcStringArray,
})
const RpcThreadResult = S.Struct({
  ok: S.Literal(true),
  cwd: S.optional(S.String),
  desktopSessionId: S.optional(S.String),
  messages: S.optional(S.Array(KhalaCodeDesktopMessageSchema)),
  model: S.optional(S.String),
  modelProvider: S.optional(S.String),
  thread: RpcJson,
  threadId: S.String,
})
const RpcThreadListResult = S.Struct({
  ok: S.Literal(true),
  backwardsCursor: S.optional(RpcStringNull),
  data: S.Array(RpcJson),
  groups: S.optional(S.Array(RpcThreadGroup)),
  nextCursor: S.optional(RpcStringNull),
  threads: S.optional(S.Array(RpcThreadSummary)),
})
const RpcThreadMutationResult = S.Struct({
  action: S.String,
  ok: S.Boolean,
  messages: S.optional(S.Array(KhalaCodeDesktopMessageSchema)),
  response: S.optional(RpcJson),
  thread: S.optional(RpcJson),
  threadId: S.String,
  error: S.optional(S.String),
  newThreadId: S.optional(S.String),
})

const RpcTurnStartRequest = S.Struct({
  attachments: S.optional(S.Array(RpcChatAttachment)),
  messages: S.Array(KhalaCodeDesktopMessageSchema),
  sessionId: S.String,
  startNewThread: S.optional(S.Boolean),
  threadId: S.optional(S.String),
  turnId: S.optional(S.String),
  cwd: S.optional(S.String),
})
const RpcTurnSteerRequest = S.Struct({
  clientUserMessageId: S.optional(S.String),
  sessionId: S.String,
  text: S.String,
  turnId: S.optional(S.String),
})
const RpcTurnInterruptRequest = S.Struct({
  sessionId: S.String,
  turnId: S.optional(S.String),
})
const RpcTurnActionResult = S.Struct({
  ok: S.Boolean,
  codexTurnId: S.optional(S.String),
  desktopSessionId: S.String,
  desktopTurnId: S.optional(S.String),
  error: S.optional(S.String),
  response: S.optional(RpcJson),
  threadId: S.optional(S.String),
})
const RpcThreadCompactRequest = S.Struct({
  sessionId: S.optional(S.String),
  threadId: S.optional(S.String),
})

const RpcApprovalRespondRequest = S.Struct({
  action: S.String,
  execpolicyAmendment: S.optional(RpcStringArray),
  method: S.String,
  networkPolicyAmendment: S.optional(S.Struct({ action: S.String, host: S.String })),
  permissions: S.optional(RpcCodexPermissionProfile),
  requestId: S.Union([S.String, S.Number]),
})
const RpcApprovalRespondResult = S.Struct({
  method: S.String,
  ok: S.Boolean,
  payload: S.optional(RpcJson),
  requestId: S.Union([S.String, S.Number]),
  error: S.optional(S.String),
})

const RpcCodexSettingsReadRequest = S.Struct({
  cwd: S.optional(S.String),
  includeHiddenModels: S.optional(S.Boolean),
})
const RpcCodexSettingsProjection = RpcJsonObject
const RpcCodexConfigValueWriteRequest = S.Struct({
  cwd: S.optional(S.String),
  expectedVersion: S.optional(S.String),
  filePath: S.optional(S.String),
  keyPath: S.String,
  mergeStrategy: S.optional(S.String),
  value: RpcJson,
})
const RpcCodexConfigValueWriteResult = S.Struct({
  ok: S.Boolean,
  keyPath: S.String,
  response: S.optional(RpcJson),
  settings: S.optional(RpcCodexSettingsProjection),
  error: S.optional(S.String),
})
const RpcCodexEcosystemReadRequest = S.Struct({
  cwd: S.optional(S.String),
  forceRefetchApps: S.optional(S.Boolean),
  forceReloadSkills: S.optional(S.Boolean),
  threadId: S.optional(S.String),
})
const RpcCodexEcosystemProjection = RpcJsonObject
const RpcCodexAppServerActionResult = S.Struct({
  ok: S.Boolean,
  method: S.String,
  response: S.optional(RpcJson),
  error: S.optional(S.String),
})

const RpcBackgroundTerminalsListRequest = S.Struct({
  cursor: S.optional(RpcStringNull),
  limit: S.optional(RpcNumberNull),
  threadId: S.String,
})
const RpcBackgroundTerminalsCleanRequest = S.Struct({ threadId: S.String })
const RpcBackgroundTerminalsTerminateRequest = S.Struct({
  processId: S.String,
  threadId: S.String,
})
const RpcMentionCandidatesRequest = S.Struct({
  cwd: S.optional(S.String),
  query: S.optional(S.String),
})
const RpcMentionCandidate = S.Struct({
  fileName: S.String,
  kind: S.Literals(["directory", "file"]),
  path: S.String,
  root: S.optional(S.String),
  score: S.optional(S.Number),
})
const RpcMentionCandidatesResult = S.Struct({
  ok: S.Boolean,
  candidates: S.Array(RpcMentionCandidate),
  source: S.String,
  truncated: S.Boolean,
  error: S.optional(S.String),
})

const RpcSkillsExtraRootsSetRequest = S.Struct({ extraRoots: RpcStringArray })
const RpcSkillsConfigWriteRequest = S.Struct({
  enabled: S.Boolean,
  name: S.optional(RpcStringNull),
  path: S.optional(RpcStringNull),
})
const RpcExternalAgentConfigDetectRequest = S.Struct({
  cwds: S.optional(S.NullOr(RpcStringArray)),
  includeHome: S.optional(S.Boolean),
})
const RpcExternalAgentConfigMigrationItem = S.Struct({
  itemType: S.String,
  description: S.String,
  cwd: RpcStringNull,
  details: S.optional(S.NullOr(RpcJson)),
})
const RpcExternalAgentConfigImportRequest = S.Struct({
  migrationItems: S.Array(RpcExternalAgentConfigMigrationItem),
  source: S.optional(RpcStringNull),
})
const RpcFsPathRequest = S.Struct({ path: S.String })
const RpcFsWriteFileRequest = S.Struct({ dataBase64: S.String, path: S.String })
const RpcMcpResourceReadRequest = S.Struct({
  server: S.String,
  threadId: S.optional(RpcStringNull),
  uri: S.String,
})
const RpcMcpToolCallRequest = S.Struct({
  arguments: S.optional(RpcJson),
  meta: S.optional(RpcJson),
  server: S.String,
  threadId: S.String,
  tool: S.String,
})
const RpcMcpOauthLoginRequest = S.Struct({
  scopes: S.optional(S.NullOr(RpcStringArray)),
  server: S.String,
  threadId: S.optional(RpcStringNull),
  timeoutSecs: S.optional(RpcNumberNull),
})
const RpcMarketplaceAddRequest = S.Struct({
  refName: S.optional(RpcStringNull),
  source: S.String,
  sparsePaths: S.optional(S.NullOr(RpcStringArray)),
})
const RpcMarketplaceRemoveRequest = S.Struct({ marketplaceName: S.String })
const RpcMarketplaceUpgradeRequest = S.Struct({
  marketplaceName: S.optional(RpcStringNull),
})
const RpcPluginInstallRequest = S.Struct({
  marketplacePath: S.optional(RpcStringNull),
  pluginName: S.String,
  remoteMarketplaceName: S.optional(RpcStringNull),
})
const RpcPluginUninstallRequest = S.Struct({ pluginId: S.String })

const RpcSlashCommandListRequest = S.Struct({
  activeTurn: S.optional(S.Boolean),
  debug: S.optional(S.Boolean),
  platform: S.optional(S.String),
  sideConversation: S.optional(S.Boolean),
})
const RpcSlashCommandDispatchRequest = S.Struct({
  activeTurn: S.optional(S.Boolean),
  debug: S.optional(S.Boolean),
  platform: S.optional(S.String),
  sideConversation: S.optional(S.Boolean),
  cwd: S.optional(S.String),
  raw: S.String,
  sessionId: S.String,
  threadId: S.optional(S.String),
})
const RpcSlashCommandListResponse = S.Struct({
  commands: S.Array(RpcJsonObject),
  ok: S.Literal(true),
})
const RpcSlashCommandDispatchResult = S.Struct({
  action: S.optional(S.String),
  command: S.optional(S.String),
  gap: S.optional(S.Struct({
    gapId: S.String,
    kind: S.Literal("upstream_app_server_gap"),
  })),
  message: S.String,
  method: S.optional(S.String),
  ok: S.Boolean,
  response: S.optional(RpcJson),
  status: S.String,
  threadId: S.optional(S.String),
})

const RpcThreadTokenSummaryRequest = S.Struct({
  threadId: S.optional(RpcStringNull),
})
const RpcThreadTokenSummary = S.Struct({
  auditRows: S.Number,
  codexStateDbPath: S.String,
  codexStateTokens: S.Number,
  leaderboardLabel: S.Literal("OpenAgents Stats"),
  leaderboardSyncedTokens: S.Number,
  localLedgerPath: S.String,
  localMessageAuditLedgerPath: S.String,
  missingUsageTurns: S.Number,
  ok: S.Literal(true),
  pendingSyncTokens: S.Number,
  remoteConfigured: S.Boolean,
  remoteDisabled: S.Boolean,
  threadId: RpcStringNull,
  totalTokens: S.Number,
  updatedAt: RpcStringNull,
  usageEventRows: S.Number,
})

const RpcToolCatalogResponse = S.Struct({
  catalogKind: S.String,
  defaultEnabled: S.Boolean,
  description: S.String,
  runtimeMode: S.String,
  toolCount: S.Number,
  tools: S.Array(S.Struct({
    authority: S.String,
    name: S.String,
    role: S.String,
  })),
})

const RpcFleetCapacity = S.Struct({
  available: RpcNumberNull,
  busy: RpcNumberNull,
  queued: RpcNumberNull,
  ready: RpcNumberNull,
})
const RpcFleetQueuePolicy = S.Struct({
  admission: S.Literal("pylon_capacity_gate"),
  cooldown: S.String,
  refill: S.Literal("pylon_presence_heartbeat"),
  queued: RpcNumberNull,
})
const RpcFleetAccount = S.Struct({
  accountRef: S.String,
  provider: S.Literal("codex"),
  readiness: S.String,
  quotaState: RpcStringNull,
  accountKey: RpcStringNull,
  capacity: S.NullOr(RpcFleetCapacity),
  homeRole: S.optional(S.String),
  queuePolicy: S.optional(RpcFleetQueuePolicy),
  sessionRole: S.optional(S.String),
  email: RpcStringNull,
})
const RpcFleetAssignmentTokenRate = S.Struct({
  source: S.String,
  status: S.String,
  tokenCountKind: RpcStringNull,
  tokens: RpcNumberNull,
  tokensPerMinute: RpcNumberNull,
})
const RpcFleetTokenRate = S.Struct({
  activeAdjustedTokensPerMinute: RpcNumberNull,
  completedStatus: S.String,
  completedTokenRows: RpcNumberNull,
  completedTokensPerMinute: RpcNumberNull,
  inFlightTokens: RpcNumberNull,
  inFlightTokensPerMinute: RpcNumberNull,
  source: S.String,
  unavailableReason: RpcStringNull,
})
const RpcFleetWorkerSession = S.Struct({
  approvalState: S.String,
  blockerRefs: RpcStringArray,
  closeoutStatus: RpcStringNull,
  executionRuntime: S.Literal("codex_harness"),
  homeRole: S.String,
  queuePolicy: RpcFleetQueuePolicy,
  reviewState: S.String,
  role: S.Literal("swarm_worker_codex_session"),
  transcriptRef: RpcStringNull,
})
const RpcFleetAssignment = S.Struct({
  assignmentRef: RpcStringNull,
  blockerRefs: S.optional(RpcStringArray),
  closeoutStatus: S.optional(RpcStringNull),
  elapsedMs: RpcNumberNull,
  issueRef: RpcStringNull,
  workerSession: S.optional(RpcFleetWorkerSession),
  tokenRate: RpcFleetAssignmentTokenRate,
  updatedAt: RpcStringNull,
})
const RpcFleetStatus = S.Struct({
  ok: S.Boolean,
  observedAt: S.String,
  sessionLayers: S.optional(RpcJsonObject),
  pylon: S.Struct({
    status: S.String,
    pylonRef: RpcStringNull,
    message: S.String,
  }),
  availableCodexAssignments: RpcNumberNull,
  maxCodexAssignments: RpcNumberNull,
  tokenRate: RpcFleetTokenRate,
  accounts: S.Array(RpcFleetAccount),
  activeAssignments: S.Array(RpcFleetAssignment),
  processes: S.Array(S.Struct({
    pid: S.String,
    parentPid: S.String,
    elapsed: S.String,
  })),
})

const RpcFleetPromotionContextBoundary = S.Struct({
  allowedRefs: RpcStringArray,
  includeTranscript: S.Literal(false),
  mode: S.String,
  summary: RpcStringNull,
})
const RpcFleetPromotionRequest = S.Struct({
  accountRef: S.optional(S.String),
  branch: S.optional(S.String),
  commit: S.optional(S.String),
  contextBoundary: RpcFleetPromotionContextBoundary,
  count: S.optional(S.Number),
  fixture: S.optional(S.Boolean),
  noRun: S.optional(S.Boolean),
  objective: S.String,
  repo: S.optional(S.String),
  sessionId: S.String,
  threadId: S.String,
  timeoutMs: S.optional(S.Number),
  verify: S.optional(S.String),
})
const RpcFleetPromotionResult = RpcJsonObject
const RpcFleetDelegateRunRequest = S.Struct({
  accountRef: S.optional(S.String),
  branch: S.optional(S.String),
  commit: S.optional(S.String),
  count: S.optional(S.Number),
  mode: S.Literals(["fixture", "real_work"]),
  noRun: S.optional(S.Boolean),
  objective: S.String,
  repo: S.optional(S.String),
  timeoutMs: S.optional(S.Number),
  verify: S.optional(S.String),
})
const RpcFleetDelegateRunResult = RpcJsonObject

const RpcConnectStart = S.Struct({
  ok: S.Boolean,
  accountRef: S.String,
  verificationUrl: RpcStringNull,
  userCode: RpcStringNull,
  output: S.String,
  error: S.optional(S.String),
})
const RpcRemoveAccountResult = S.Struct({
  ok: S.Boolean,
  removed: S.Boolean,
  accountRef: S.String,
  error: S.optional(S.String),
})
const RpcRateLimitResetResult = S.Struct({
  ok: S.Boolean,
  observedAt: S.String,
  outcome: S.NullOr(S.String),
  status: RpcCodexAccountsStatus,
  error: S.optional(S.String),
})
const RpcOnDeviceDeciderSelection = S.Struct({
  selected: RpcStringNull,
  preferred: S.String,
  reason: S.String,
  readiness: S.Array(S.Struct({
    backend: S.String,
    available: S.Boolean,
    model: S.String,
    detail: S.String,
  })),
})

const noParams = () => [] as const
const param = <A>(schema: S.Schema<A>) => ({ optional: false, schema }) as const
const optionalParam = <A>(schema: S.Schema<A>) =>
  ({ optional: true, schema }) as const

export const KhalaCodeDesktopRpcMethodSchemas = {
  appInfo: { parameters: noParams(), result: RpcAppInfo },
  appleFmReadiness: { parameters: noParams(), result: RpcAppleFmReadiness },
  codexAccountsStatus: { parameters: noParams(), result: RpcCodexAccountsStatus },
  codexAppServerRestart: { parameters: noParams(), result: RpcCodexAppServerControlResult },
  codexAppServerStart: { parameters: noParams(), result: RpcCodexAppServerControlResult },
  codexAppServerStatus: { parameters: noParams(), result: RpcCodexAppServerStatus },
  codexAppServerStop: { parameters: noParams(), result: RpcCodexAppServerControlResult },
  codexFleetDelegateRun: { parameters: [param(RpcFleetDelegateRunRequest)], result: RpcFleetDelegateRunResult },
  codexFleetStatus: { parameters: noParams(), result: RpcFleetStatus },
  codexFleetPromoteThread: { parameters: [param(RpcFleetPromotionRequest)], result: RpcFleetPromotionResult },
  codexHarnessStatus: { parameters: noParams(), result: RpcCodexHarnessStatus },
  codexApprovalRespond: { parameters: [param(RpcApprovalRespondRequest)], result: RpcApprovalRespondResult },
  codexBackgroundTerminalsClean: { parameters: [param(RpcBackgroundTerminalsCleanRequest)], result: RpcCodexAppServerActionResult },
  codexBackgroundTerminalsList: { parameters: [param(RpcBackgroundTerminalsListRequest)], result: RpcCodexAppServerActionResult },
  codexBackgroundTerminalsTerminate: { parameters: [param(RpcBackgroundTerminalsTerminateRequest)], result: RpcCodexAppServerActionResult },
  codexConfigValueWrite: { parameters: [param(RpcCodexConfigValueWriteRequest)], result: RpcCodexConfigValueWriteResult },
  codexEcosystemRead: { parameters: [optionalParam(RpcCodexEcosystemReadRequest)], result: RpcCodexEcosystemProjection },
  codexExternalAgentConfigDetect: { parameters: [optionalParam(RpcExternalAgentConfigDetectRequest)], result: RpcCodexAppServerActionResult },
  codexExternalAgentConfigImport: { parameters: [param(RpcExternalAgentConfigImportRequest)], result: RpcCodexAppServerActionResult },
  codexExternalAgentConfigImportHistoriesRead: { parameters: noParams(), result: RpcCodexAppServerActionResult },
  codexFsGetMetadata: { parameters: [param(RpcFsPathRequest)], result: RpcCodexAppServerActionResult },
  codexFsReadFile: { parameters: [param(RpcFsPathRequest)], result: RpcCodexAppServerActionResult },
  codexFsWriteFile: { parameters: [param(RpcFsWriteFileRequest)], result: RpcCodexAppServerActionResult },
  codexMarketplaceAdd: { parameters: [param(RpcMarketplaceAddRequest)], result: RpcCodexAppServerActionResult },
  codexMarketplaceRemove: { parameters: [param(RpcMarketplaceRemoveRequest)], result: RpcCodexAppServerActionResult },
  codexMarketplaceUpgrade: { parameters: [optionalParam(RpcMarketplaceUpgradeRequest)], result: RpcCodexAppServerActionResult },
  codexMentionCandidates: { parameters: [optionalParam(RpcMentionCandidatesRequest)], result: RpcMentionCandidatesResult },
  codexMcpOauthLogin: { parameters: [param(RpcMcpOauthLoginRequest)], result: RpcCodexAppServerActionResult },
  codexMcpResourceRead: { parameters: [param(RpcMcpResourceReadRequest)], result: RpcCodexAppServerActionResult },
  codexMcpServerReload: { parameters: noParams(), result: RpcCodexAppServerActionResult },
  codexMcpToolCall: { parameters: [param(RpcMcpToolCallRequest)], result: RpcCodexAppServerActionResult },
  codexPluginInstall: { parameters: [param(RpcPluginInstallRequest)], result: RpcCodexAppServerActionResult },
  codexPluginUninstall: { parameters: [param(RpcPluginUninstallRequest)], result: RpcCodexAppServerActionResult },
  codexSettingsRead: { parameters: [optionalParam(RpcCodexSettingsReadRequest)], result: RpcCodexSettingsProjection },
  codexSkillsConfigWrite: { parameters: [param(RpcSkillsConfigWriteRequest)], result: RpcCodexAppServerActionResult },
  codexSkillsExtraRootsSet: { parameters: [param(RpcSkillsExtraRootsSetRequest)], result: RpcCodexAppServerActionResult },
  codexThreadArchive: { parameters: [param(RpcThreadIdRequest)], result: RpcThreadMutationResult },
  codexThreadCompact: { parameters: [param(RpcThreadCompactRequest)], result: RpcTurnActionResult },
  codexThreadDelete: { parameters: [param(RpcThreadIdRequest)], result: RpcThreadMutationResult },
  codexThreadFork: { parameters: [param(RpcThreadForkRequest)], result: RpcThreadMutationResult },
  codexThreadList: { parameters: [optionalParam(RpcThreadListRequest)], result: RpcThreadListResult },
  codexThreadRead: { parameters: [param(RpcThreadReadRequest)], result: RpcThreadResult },
  codexThreadRename: { parameters: [param(RpcThreadRenameRequest)], result: RpcThreadMutationResult },
  codexThreadResume: { parameters: [param(RpcThreadResumeRequest)], result: RpcThreadResult },
  codexThreadStart: { parameters: [optionalParam(RpcThreadStartRequest)], result: RpcThreadResult },
  codexThreadUnarchive: { parameters: [param(RpcThreadIdRequest)], result: RpcThreadMutationResult },
  codexTurnInterrupt: { parameters: [param(RpcTurnInterruptRequest)], result: RpcTurnActionResult },
  codexTurnStart: { parameters: [param(RpcTurnStartRequest)], result: RpcChatTurnResponse },
  codexTurnSteer: { parameters: [param(RpcTurnSteerRequest)], result: RpcTurnActionResult },
  connectCodexAccount: { parameters: [param(S.String)], result: RpcConnectStart },
  openExternalUrl: { parameters: [param(S.String)], result: S.Boolean },
  removeCodexAccount: { parameters: [param(S.String)], result: RpcRemoveAccountResult },
  codingStatus: { parameters: noParams(), result: RpcRuntimeStatus },
  consumeCodexRateLimitResetCredit: { parameters: noParams(), result: RpcRateLimitResetResult },
  onDeviceDeciderStatus: { parameters: noParams(), result: RpcOnDeviceDeciderSelection },
  pylonStatus: { parameters: noParams(), result: RpcRuntimeStatus },
  slashCommandDispatch: { parameters: [param(RpcSlashCommandDispatchRequest)], result: RpcSlashCommandDispatchResult },
  slashCommandList: { parameters: [optionalParam(RpcSlashCommandListRequest)], result: RpcSlashCommandListResponse },
  submitChatMessage: { parameters: [param(RpcChatTurnRequest)], result: RpcChatTurnResponse },
  tokenAccountingStatus: { parameters: noParams(), result: RpcRuntimeStatus },
  threadTokenSummary: { parameters: [optionalParam(RpcThreadTokenSummaryRequest)], result: RpcThreadTokenSummary },
  toolCatalog: { parameters: noParams(), result: RpcToolCatalogResponse },
} as const

export type KhalaCodeDesktopRpcMethodName =
  keyof typeof KhalaCodeDesktopRpcMethodSchemas

const isRpcMethodName = (method: string): method is KhalaCodeDesktopRpcMethodName =>
  Object.hasOwn(KhalaCodeDesktopRpcMethodSchemas, method)

export const khalaCodeDesktopRpcMethodSchema = (
  method: string,
): (typeof KhalaCodeDesktopRpcMethodSchemas)[KhalaCodeDesktopRpcMethodName] | null =>
  isRpcMethodName(method) ? KhalaCodeDesktopRpcMethodSchemas[method] : null

const parseErrorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const decodeWithSchema = (
  schema: S.Schema<unknown>,
  value: unknown,
): unknown =>
  S.decodeUnknownSync(schema as never)(value)

export const decodeKhalaCodeDesktopRpcParameters = (
  method: KhalaCodeDesktopRpcMethodName,
  args: unknown,
): readonly unknown[] => {
  const spec = KhalaCodeDesktopRpcMethodSchemas[method]
  if (!Array.isArray(args)) {
    throw new Error("RPC request args must be an array.")
  }
  if (args.length > spec.parameters.length) {
    throw new Error(
      `RPC method ${method} expected ${spec.parameters.length} args but received ${args.length}.`,
    )
  }
  return spec.parameters.map((parameter, index) => {
    const value = args[index]
    if (parameter.optional && (value === undefined || value === null)) return undefined
    return decodeWithSchema(parameter.schema, value)
  })
}

export const decodeKhalaCodeDesktopRpcResult = (
  method: KhalaCodeDesktopRpcMethodName,
  value: unknown,
): unknown => decodeWithSchema(KhalaCodeDesktopRpcMethodSchemas[method].result, value)

export const khalaCodeDesktopRpcDecodeFailure = (
  method: string,
  error: unknown,
): KhalaCodeDesktopRpcDecodeFailure => ({
  error: parseErrorMessage(error),
  method,
  ok: false,
  tag: "rpc_decode_failed",
})

export const khalaCodeDesktopRpcHandlerFailure = (
  method: string,
  error: unknown,
): KhalaCodeDesktopRpcHandlerFailure => ({
  error: parseErrorMessage(error),
  method,
  ok: false,
  tag: "rpc_handler_failed",
})

export type KhalaCodeDesktopRPCSchema = {
  requests: {
    appInfo(): Promise<KhalaCodeDesktopAppInfo>
    appleFmReadiness(): Promise<KhalaAppleFmReadiness>
    codexAccountsStatus(): Promise<KhalaCodeDesktopCodexAccountsStatus>
    codexAppServerRestart(): Promise<KhalaCodeDesktopCodexAppServerControlResult>
    codexAppServerStart(): Promise<KhalaCodeDesktopCodexAppServerControlResult>
    codexAppServerStatus(): Promise<KhalaCodeDesktopCodexAppServerStatus>
    codexAppServerStop(): Promise<KhalaCodeDesktopCodexAppServerControlResult>
    codexFleetDelegateRun(request: KhalaCodeDesktopFleetDelegateRunRequest): Promise<KhalaCodeDesktopFleetDelegateRunResult>
    codexFleetStatus(): Promise<KhalaCodeDesktopFleetStatus>
    codexFleetPromoteThread(request: KhalaCodeDesktopFleetPromotionRequest): Promise<KhalaCodeDesktopFleetPromotionResult>
    codexHarnessStatus(): Promise<KhalaCodeDesktopCodexHarnessStatus>
    codexApprovalRespond(request: KhalaCodeDesktopCodexApprovalRespondRequest): Promise<KhalaCodeDesktopCodexApprovalRespondResult>
    codexBackgroundTerminalsClean(request: KhalaCodeDesktopCodexBackgroundTerminalsCleanRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexBackgroundTerminalsList(request: KhalaCodeDesktopCodexBackgroundTerminalsListRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexBackgroundTerminalsTerminate(request: KhalaCodeDesktopCodexBackgroundTerminalsTerminateRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexConfigValueWrite(request: KhalaCodeDesktopCodexConfigValueWriteRequest): Promise<KhalaCodeDesktopCodexConfigValueWriteResult>
    codexEcosystemRead(request?: KhalaCodeDesktopCodexEcosystemReadRequest): Promise<KhalaCodeDesktopCodexEcosystemReadResult>
    codexExternalAgentConfigDetect(request?: KhalaCodeDesktopCodexExternalAgentConfigDetectRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexExternalAgentConfigImport(request: KhalaCodeDesktopCodexExternalAgentConfigImportRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexExternalAgentConfigImportHistoriesRead(): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexFsGetMetadata(request: KhalaCodeDesktopCodexFsPathRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexFsReadFile(request: KhalaCodeDesktopCodexFsPathRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexFsWriteFile(request: KhalaCodeDesktopCodexFsWriteFileRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexMarketplaceAdd(request: KhalaCodeDesktopCodexMarketplaceAddRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexMarketplaceRemove(request: KhalaCodeDesktopCodexMarketplaceRemoveRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexMarketplaceUpgrade(request?: KhalaCodeDesktopCodexMarketplaceUpgradeRequest): Promise<KhalaCodeDesktopCodexAppServerActionResult>
    codexMentionCandidates(request?: KhalaCodeDesktopCodexMentionCandidatesRequest): Promise<KhalaCodeDesktopCodexMentionCandidatesResult>
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
    threadTokenSummary(request?: KhalaCodeDesktopThreadTokenSummaryRequest): Promise<KhalaCodeDesktopThreadTokenSummary>
    toolCatalog(): Promise<KhalaCodeDesktopToolCatalogResponse>
  }
  messages: {
    chatTurnEvent(event: KhalaCodeDesktopChatTurnEvent): void
  }
}
