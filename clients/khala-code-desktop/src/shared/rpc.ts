import { Schema as S } from "effect"
import type { KhalaToolEvent } from "@openagentsinc/khala-tools"
import type { KhalaAppleFmReadiness } from "./apple-fm-readiness.js"
import type {
  KhalaCodeDesktopSlashCommandWithAvailability,
} from "./codex-slash-commands.js"
import type { OnDeviceDeciderSelection } from "./on-device-decider.js"
import type {
  KhalaCodeQaMetricSample,
  KhalaCodeQaMetricsSnapshot,
} from "./qa-metrics.js"
import {
  KhalaCodeModelRoleSchema,
  KhalaCodeModelRoleEntrySchema,
  KhalaCodeModelRoleRegistrySchema,
} from "./model-roles.js"
import {
  KHALA_CODE_DESKTOP_TRACE_CAPTURE_DISCLOSURE_REF,
  KHALA_CODE_DESKTOP_TRACE_CAPTURE_INGEST_AUDIENCE,
  KHALA_CODE_DESKTOP_TRACE_CAPTURE_OWNER_GATE_ENV,
  KHALA_CODE_DESKTOP_TRACE_CAPTURE_PROMISE_ID,
} from "./trace-capture.js"
import {
  KhalaCodeEditorDirectoryReadRequest,
  KhalaCodeEditorDirectoryReadResult,
  KhalaCodeEditorFileReadRequest,
  KhalaCodeEditorFileReadResult,
  KhalaCodeEditorProviderListResult,
  KhalaCodeEditorWorkspaceReadResult,
} from "./editor.js"

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

export type KhalaCodeDesktopMessageRole =
  typeof KhalaCodeDesktopMessageSchema.Type["role"]

export type KhalaCodeDesktopRuntimeMode = typeof RpcRuntimeMode.Type

export type KhalaCodeDesktopToolCatalogKind = typeof RpcToolCatalogKind.Type

export type KhalaCodeDesktopCodexItemCard =
  typeof RpcCodexItemCard.Type

export type KhalaCodeDesktopMessage =
  typeof KhalaCodeDesktopMessageSchema.Type

export type KhalaCodeDesktopChatTurnEvent =
  typeof KhalaCodeDesktopChatTurnEventSchema.Type
export type KhalaCodeDesktopFleetLifecycleEvent = Readonly<{
  line: string
  observedAt: string
}>

export type KhalaCodeDesktopUsage = typeof RpcUsage.Type
export type KhalaCodeDesktopChatTurnAttachment = typeof RpcChatAttachment.Type
export type KhalaCodeDesktopChatTurnRequest = typeof RpcChatTurnRequest.Type
export type KhalaCodeDesktopBackendProjection = typeof RpcBackendProjection.Type
export type KhalaCodeDesktopChatTurnResponse = typeof RpcChatTurnResponse.Type
export type KhalaCodeDesktopToolCatalogResponse = typeof RpcToolCatalogResponse.Type
export type KhalaCodeDesktopAppInfo = typeof RpcAppInfo.Type
export type KhalaCodeDesktopRuntimeStatus = typeof RpcRuntimeStatus.Type
export type KhalaCodeDesktopQaMetricSample = KhalaCodeQaMetricSample
export type KhalaCodeDesktopQaMetricsSnapshot = KhalaCodeQaMetricsSnapshot
export type KhalaCodeDesktopQaMetricSampleResult =
  typeof RpcQaMetricSampleResult.Type
export type KhalaCodeDesktopThreadTokenSummaryRequest = typeof RpcThreadTokenSummaryRequest.Type
export type KhalaCodeDesktopThreadTokenSummary = typeof RpcThreadTokenSummary.Type
export type KhalaCodeDesktopCodexHarnessStatus = typeof RpcCodexHarnessStatus.Type
export type KhalaCodeDesktopCodexAppServerStatus = typeof RpcCodexAppServerStatus.Type
export type KhalaCodeDesktopCodexAppServerControlResult = typeof RpcCodexAppServerControlResult.Type
export type KhalaCodeDesktopCodexThreadStartRequest = typeof RpcThreadStartRequest.Type
export type KhalaCodeDesktopCodexThreadResumeRequest = typeof RpcThreadResumeRequest.Type
export type KhalaCodeDesktopCodexThreadListRequest = typeof RpcThreadListRequest.Type
export type KhalaCodeDesktopCodexThreadResult = typeof RpcThreadResult.Type
export type KhalaCodeDesktopCodexThreadListResult = typeof RpcThreadListResult.Type
export type KhalaCodeDesktopSessionCatalogRequest = typeof RpcSessionCatalogRequest.Type
export type KhalaCodeDesktopSessionCatalogResult = typeof RpcSessionCatalogResult.Type
export type KhalaCodeDesktopCodexThreadReadRequest = typeof RpcThreadReadRequest.Type
export type KhalaCodeDesktopCodexThreadForkRequest = typeof RpcThreadForkRequest.Type
export type KhalaCodeDesktopCodexThreadIdRequest = typeof RpcThreadIdRequest.Type
export type KhalaCodeDesktopCodexThreadRenameRequest = typeof RpcThreadRenameRequest.Type
export type KhalaCodeDesktopCodexThreadMutationResult = typeof RpcThreadMutationResult.Type
export type KhalaCodeDesktopCodexTurnStartRequest = typeof RpcTurnStartRequest.Type
export type KhalaCodeDesktopCodexTurnSteerRequest = typeof RpcTurnSteerRequest.Type
export type KhalaCodeDesktopCodexTurnInterruptRequest = typeof RpcTurnInterruptRequest.Type
export type KhalaCodeDesktopCodexTurnActionResult = typeof RpcTurnActionResult.Type
export type KhalaCodeDesktopCodexThreadCompactRequest = typeof RpcThreadCompactRequest.Type
export type KhalaCodeDesktopCodexApprovalRespondRequest = typeof RpcApprovalRespondRequest.Type
export type KhalaCodeDesktopCodexApprovalRespondResult = typeof RpcApprovalRespondResult.Type
export type KhalaCodeDesktopClaudeApprovalPendingResult = typeof RpcClaudeApprovalPendingResult.Type
export type KhalaCodeDesktopClaudeApprovalRequestProjection = typeof RpcClaudeApprovalRequestProjection.Type
export type KhalaCodeDesktopClaudeApprovalRespondRequest = typeof RpcClaudeApprovalRespondRequest.Type
export type KhalaCodeDesktopClaudeApprovalRespondResult = typeof RpcClaudeApprovalRespondResult.Type
export type KhalaCodeDesktopClaudeSettingsReadResult = typeof RpcClaudeSettingsProjection.Type
export type KhalaCodeDesktopCodexSettingsReadRequest = typeof RpcCodexSettingsReadRequest.Type
export type KhalaCodeDesktopCodexSettingsReadResult = typeof RpcCodexSettingsProjection.Type
export type KhalaCodeDesktopModelRoleRegistryReadResult =
  typeof RpcModelRoleRegistryReadResult.Type
export type KhalaCodeDesktopModelRoleRegistryWriteRequest =
  typeof RpcModelRoleRegistryWriteRequest.Type
export type KhalaCodeDesktopModelRoleRegistryWriteResult =
  typeof RpcModelRoleRegistryWriteResult.Type
export type KhalaCodeDesktopCodexConfigValueWriteRequest = typeof RpcCodexConfigValueWriteRequest.Type
export type KhalaCodeDesktopCodexConfigValueWriteResult = typeof RpcCodexConfigValueWriteResult.Type
export type KhalaCodeDesktopCodexEcosystemReadRequest = typeof RpcCodexEcosystemReadRequest.Type
export type KhalaCodeDesktopCodexEcosystemReadResult = typeof RpcCodexEcosystemProjection.Type
export type KhalaCodeDesktopCodexAppServerActionResult = typeof RpcCodexAppServerActionResult.Type
export type KhalaCodeDesktopEditorProviderListResult = typeof KhalaCodeEditorProviderListResult.Type
export type KhalaCodeDesktopEditorWorkspaceReadResult = typeof KhalaCodeEditorWorkspaceReadResult.Type
export type KhalaCodeDesktopEditorDirectoryReadRequest = typeof KhalaCodeEditorDirectoryReadRequest.Type
export type KhalaCodeDesktopEditorDirectoryReadResult = typeof KhalaCodeEditorDirectoryReadResult.Type
export type KhalaCodeDesktopEditorFileReadRequest = typeof KhalaCodeEditorFileReadRequest.Type
export type KhalaCodeDesktopEditorFileReadResult = typeof KhalaCodeEditorFileReadResult.Type
export type KhalaCodeDesktopCodexBackgroundTerminalsListRequest = typeof RpcBackgroundTerminalsListRequest.Type
export type KhalaCodeDesktopCodexBackgroundTerminalsCleanRequest = typeof RpcBackgroundTerminalsCleanRequest.Type
export type KhalaCodeDesktopCodexBackgroundTerminalsTerminateRequest = typeof RpcBackgroundTerminalsTerminateRequest.Type
export type KhalaCodeDesktopCodexMentionCandidate = typeof RpcMentionCandidate.Type
export type KhalaCodeDesktopCodexMentionCandidatesRequest = typeof RpcMentionCandidatesRequest.Type
export type KhalaCodeDesktopCodexMentionCandidatesResult = typeof RpcMentionCandidatesResult.Type
export type KhalaCodeDesktopCodexSkillsExtraRootsSetRequest = typeof RpcSkillsExtraRootsSetRequest.Type
export type KhalaCodeDesktopCodexSkillsConfigWriteRequest = typeof RpcSkillsConfigWriteRequest.Type
export type KhalaCodeDesktopCodexExternalAgentConfigDetectRequest = typeof RpcExternalAgentConfigDetectRequest.Type
export type KhalaCodeDesktopCodexExternalAgentConfigMigrationItem = typeof RpcExternalAgentConfigMigrationItem.Type
export type KhalaCodeDesktopCodexExternalAgentConfigImportRequest = typeof RpcExternalAgentConfigImportRequest.Type
export type KhalaCodeDesktopCodexFsPathRequest = typeof RpcFsPathRequest.Type
export type KhalaCodeDesktopCodexFsWriteFileRequest = typeof RpcFsWriteFileRequest.Type
export type KhalaCodeDesktopCodexMcpResourceReadRequest = typeof RpcMcpResourceReadRequest.Type
export type KhalaCodeDesktopCodexMcpToolCallRequest = typeof RpcMcpToolCallRequest.Type
export type KhalaCodeDesktopCodexMcpOauthLoginRequest = typeof RpcMcpOauthLoginRequest.Type
export type KhalaCodeDesktopCodexMarketplaceAddRequest = typeof RpcMarketplaceAddRequest.Type
export type KhalaCodeDesktopCodexMarketplaceRemoveRequest = typeof RpcMarketplaceRemoveRequest.Type
export type KhalaCodeDesktopCodexMarketplaceUpgradeRequest = typeof RpcMarketplaceUpgradeRequest.Type
export type KhalaCodeDesktopCodexPluginInstallRequest = typeof RpcPluginInstallRequest.Type
export type KhalaCodeDesktopCodexPluginUninstallRequest = typeof RpcPluginUninstallRequest.Type
export type KhalaCodeDesktopSlashCommandListRequest = typeof RpcSlashCommandListRequest.Type
export type KhalaCodeDesktopSlashCommandListResponse = typeof RpcSlashCommandListResponse.Type
export type KhalaCodeDesktopSlashCommandDispatchRequest = typeof RpcSlashCommandDispatchRequest.Type
export type KhalaCodeDesktopSlashCommandDispatchResult = typeof RpcSlashCommandDispatchResult.Type
export type KhalaCodeDesktopCodexAccountStatus = typeof RpcCodexAccountStatus.Type
export type KhalaCodeDesktopCodexAccountsStatus = typeof RpcCodexAccountsStatus.Type
export type KhalaCodeDesktopCodexRateLimitResetResult = typeof RpcRateLimitResetResult.Type
export type KhalaCodeDesktopFleetAccount = typeof RpcFleetAccount.Type
export type KhalaCodeDesktopFleetCapacity = typeof RpcFleetCapacity.Type
export type KhalaCodeDesktopFleetAssignmentTokenRate = typeof RpcFleetAssignmentTokenRate.Type
export type KhalaCodeDesktopFleetTokenRate = typeof RpcFleetTokenRate.Type
export type KhalaCodeDesktopFleetTokenMeasurementStatus =
  typeof RpcFleetTokenMeasurementStatus.Type
export type KhalaCodeDesktopFleetSessionRole = typeof RpcFleetSessionRole.Type
export type KhalaCodeDesktopFleetHomeRole = typeof RpcFleetHomeRole.Type
export type KhalaCodeDesktopFleetQueuePolicy = typeof RpcFleetQueuePolicy.Type
export type KhalaCodeDesktopFleetSessionLayer = typeof RpcFleetSessionLayer.Type
export type KhalaCodeDesktopFleetWorkerSession = typeof RpcFleetWorkerSession.Type
export type KhalaCodeDesktopFleetAssignment = typeof RpcFleetAssignment.Type
export type KhalaCodeDesktopFleetProcess = typeof RpcFleetProcess.Type
export type KhalaCodeDesktopFleetStatus = typeof RpcFleetStatus.Type
export type KhalaCodeDesktopFleetPromotionContextBoundary = typeof RpcFleetPromotionContextBoundary.Type
export type KhalaCodeDesktopFleetPromotionRequest = typeof RpcFleetPromotionRequest.Type
export type KhalaCodeDesktopFleetPromotionResult = typeof RpcFleetPromotionResult.Type
export type KhalaCodeDesktopFleetDelegateRunMode = typeof RpcFleetDelegateRunRequest.Type["mode"]
export type KhalaCodeDesktopFleetDelegateRunRequest = typeof RpcFleetDelegateRunRequest.Type
export type KhalaCodeDesktopFleetDelegateRunStep = typeof RpcFleetDelegateRunStep.Type
export type KhalaCodeDesktopFleetDelegateRunResult = typeof RpcFleetDelegateRunResult.Type
export type KhalaCodeDesktopFleetRunControlVerb = typeof RpcFleetRunControlVerb.Type
export type KhalaCodeDesktopFleetRunState = typeof RpcFleetRunState.Type
export type KhalaCodeDesktopFleetRunStartRequest = typeof RpcFleetRunStartRequest.Type
export type KhalaCodeDesktopFleetRunStartResult = typeof RpcFleetRunStartResult.Type
export type KhalaCodeDesktopFleetRunProjection = typeof RpcFleetRunProjection.Type
export type KhalaCodeDesktopFleetRunStatusRequest = typeof RpcFleetRunStatusRequest.Type
export type KhalaCodeDesktopFleetRunStatusResult = typeof RpcFleetRunStatusResult.Type
export type KhalaCodeDesktopFleetRunControlRequest = typeof RpcFleetRunControlRequest.Type
export type KhalaCodeDesktopFleetRunControlResult = typeof RpcFleetRunControlResult.Type
export type KhalaCodeDesktopFleetRunListRequest = typeof RpcFleetRunListRequest.Type
export type KhalaCodeDesktopFleetRunListResult = typeof RpcFleetRunListResult.Type
export type KhalaCodeDesktopArchitectPlanArtifact = typeof RpcArchitectPlanArtifact.Type
export type KhalaCodeDesktopArchitectPlanRunRequest = typeof RpcArchitectPlanRunRequest.Type
export type KhalaCodeDesktopArchitectPlanRunResult = typeof RpcArchitectPlanRunResult.Type
export type KhalaCodeDesktopArchitectPlanDecisionRequest = typeof RpcArchitectPlanDecisionRequest.Type
export type KhalaCodeDesktopArchitectPlanDecisionResult = typeof RpcArchitectPlanDecisionResult.Type
export type KhalaCodeDesktopFleetWorkerControlRequest = typeof RpcFleetWorkerControlRequest.Type
export type KhalaCodeDesktopFleetWorkerControlResult = typeof RpcFleetWorkerControlResult.Type
export type KhalaCodeDesktopKhalaSyncFleetPhase = typeof RpcKhalaSyncFleetPhase.Type
export type KhalaCodeDesktopKhalaSyncFleetRun = typeof RpcKhalaSyncFleetRun.Type
export type KhalaCodeDesktopKhalaSyncFleetWorker = typeof RpcKhalaSyncFleetWorker.Type
export type KhalaCodeDesktopKhalaSyncFleetAssignment = typeof RpcKhalaSyncFleetAssignment.Type
export type KhalaCodeDesktopKhalaSyncFleetAccount = typeof RpcKhalaSyncFleetAccount.Type
export type KhalaCodeDesktopKhalaSyncFleetRejection = typeof RpcKhalaSyncFleetRejection.Type
export type KhalaCodeDesktopKhalaSyncFleetStateRequest = typeof RpcKhalaSyncFleetStateRequest.Type
export type KhalaCodeDesktopKhalaSyncFleetStateResult = typeof RpcKhalaSyncFleetStateResult.Type
export type KhalaCodeDesktopKhalaSyncFleetMutateRequest = typeof RpcKhalaSyncFleetMutateRequest.Type
export type KhalaCodeDesktopKhalaSyncFleetMutateResult = typeof RpcKhalaSyncFleetMutateResult.Type
export type KhalaCodeDesktopKhalaSyncFleetReportAccountStateRequest =
  typeof RpcKhalaSyncFleetReportAccountStateRequest.Type
export type KhalaCodeDesktopKhalaSyncFleetReportAccountStateResult =
  typeof RpcKhalaSyncFleetReportAccountStateResult.Type
export type KhalaCodeDesktopKhalaSyncChatThread = typeof RpcKhalaSyncChatThread.Type
export type KhalaCodeDesktopKhalaSyncChatMessage = typeof RpcKhalaSyncChatMessage.Type
export type KhalaCodeDesktopKhalaSyncRuntimeMessage =
  typeof RpcKhalaSyncRuntimeMessage.Type
export type KhalaCodeDesktopKhalaSyncChatRejection = typeof RpcKhalaSyncChatRejection.Type
export type KhalaCodeDesktopKhalaSyncChatThreadsRequest = typeof RpcKhalaSyncChatThreadsRequest.Type
export type KhalaCodeDesktopKhalaSyncChatThreadsResult = typeof RpcKhalaSyncChatThreadsResult.Type
export type KhalaCodeDesktopKhalaSyncChatMessagesRequest =
  typeof RpcKhalaSyncChatMessagesRequest.Type
export type KhalaCodeDesktopKhalaSyncChatMessagesResult =
  typeof RpcKhalaSyncChatMessagesResult.Type
export type KhalaCodeDesktopKhalaSyncChatCreateThreadRequest =
  typeof RpcKhalaSyncChatCreateThreadRequest.Type
export type KhalaCodeDesktopKhalaSyncChatAppendMessageRequest =
  typeof RpcKhalaSyncChatAppendMessageRequest.Type
export type KhalaCodeDesktopKhalaSyncChatRenameThreadRequest =
  typeof RpcKhalaSyncChatRenameThreadRequest.Type
export type KhalaCodeDesktopKhalaSyncChatMutationResult =
  typeof RpcKhalaSyncChatMutationResult.Type
export type KhalaCodeDesktopForumRequest = typeof RpcForumRequest.Type
export type KhalaCodeDesktopForumResponse = typeof RpcForumResponse.Type
export type KhalaCodeDesktopPlanKind = typeof RpcKhalaCodePlanKind.Type
export type KhalaCodeDesktopPlan = typeof RpcKhalaCodePlan.Type
export type KhalaCodeDesktopPlanCatalog = typeof RpcKhalaCodePlanCatalog.Type
export type KhalaCodeDesktopPlanCatalogResult = typeof RpcKhalaCodePlanCatalogResult.Type
export type KhalaCodeDesktopPlanStatusPlan = typeof RpcKhalaCodePlanStatusPlan.Type
export type KhalaCodeDesktopPlanStatusResult = typeof RpcKhalaCodePlanStatusResult.Type
export type KhalaCodeDesktopPlanPurchaseRequest = typeof RpcKhalaCodePlanPurchaseRequest.Type
export type KhalaCodeDesktopPlanPurchaseResult = typeof RpcKhalaCodePlanPurchaseResult.Type
export type KhalaCodeDesktopOpenAgentsAuthPendingAttempt =
  typeof RpcKhalaCodeOpenAgentsAuthPendingAttempt.Type
export type KhalaCodeDesktopOpenAgentsAuthStatusResult =
  typeof RpcKhalaCodeOpenAgentsAuthStatusResult.Type
export type KhalaCodeDesktopOpenAgentsAuthStartResult =
  typeof RpcKhalaCodeOpenAgentsAuthStartResult.Type
export type KhalaCodeDesktopOpenAgentsAuthPollResult =
  typeof RpcKhalaCodeOpenAgentsAuthPollResult.Type
export type KhalaCodeDesktopTraceCaptureStatusResult =
  typeof RpcKhalaCodeTraceCaptureStatusResult.Type
export type KhalaCodeDesktopTraceCaptureConsentWriteRequest =
  typeof RpcKhalaCodeTraceCaptureConsentWriteRequest.Type
export type KhalaCodeDesktopTraceCaptureConsentWriteResult =
  typeof RpcKhalaCodeTraceCaptureConsentWriteResult.Type
export type KhalaCodeDesktopOutsideUserRunReportRequest = typeof RpcKhalaCodeOutsideUserRunReportRequest.Type
export type KhalaCodeDesktopOutsideUserRunReportResult = typeof RpcKhalaCodeOutsideUserRunReportResult.Type
export type KhalaCodeDesktopOutsideUserRunReceipt = typeof RpcKhalaCodeOutsideUserRunReceipt.Type
export type KhalaCodeDesktopRemoveAccountResult = typeof RpcRemoveAccountResult.Type
export type KhalaCodeDesktopConnectStart = typeof RpcConnectStart.Type

const RpcJson = KhalaCodeDesktopRpcJsonValue
const RpcStringArray = S.Array(S.String)
const RpcJsonObject = S.Record(S.String, RpcJson)
const RpcStringNull = S.NullOr(S.String)
const RpcNumberNull = S.NullOr(S.Number)
const RpcRuntimeMode = S.Literals(["claude_runtime", "codex_harness", "khala_native_runtime"])
const RpcToolCatalogKind = S.Literals(["codex_harness_supplemental", "khala_native_legacy"])

const RpcToolEvent = S.Struct({
  eventId: S.String,
  invocationId: S.optional(S.String),
  kind: S.String,
  payload: S.Unknown,
  sessionId: S.String,
}) as S.Schema<KhalaToolEvent>

const RpcCodexPermissionProfile = S.Struct({
  fileSystem: S.optional(S.Struct({
    entries: S.optional(S.Array(S.Unknown)),
    globScanMaxDepth: S.optional(S.Number),
    read: S.optional(S.NullOr(RpcStringArray)),
    write: S.optional(S.NullOr(RpcStringArray)),
  })),
  network: S.optional(S.Struct({
    enabled: S.optional(S.NullOr(S.Boolean)),
  })),
})

const RpcCodexApprovalProjection = S.Struct({
  additionalPermissions: S.optional(S.Unknown),
  availableDecisions: S.optional(S.Array(S.Unknown)),
  command: S.optional(S.String),
  cwd: S.optional(S.String),
  grantRoot: S.optional(S.String),
  method: S.Literals([
    "item/commandExecution/requestApproval",
    "item/fileChange/requestApproval",
    "item/permissions/requestApproval",
  ]),
  networkApprovalContext: S.optional(S.Unknown),
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
  harnessItem: S.optional(RpcCodexItemCard),
  id: S.String,
  role: S.Literals(["user", "assistant", "system", "tool"]),
  body: S.String,
})

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

const RpcUsage = S.Struct({
  input: S.Number,
  cachedInput: S.Number,
  output: S.Number,
  reasoningOutput: S.Number,
})

const RpcSessionExactTotals = S.Struct({
  cachedInputTokens: S.optional(S.Number),
  inputTokens: S.optional(S.Number),
  outputTokens: S.optional(S.Number),
  reasoningOutputTokens: S.optional(S.Number),
  totalTokens: S.Number,
  source: S.String,
})

const RpcSessionCatalogEntry = S.Struct({
  catalogEntryId: S.String,
  harnessKind: S.Literals(["claude", "codex"]),
  sessionRef: S.String,
  threadRef: RpcStringNull,
  desktopSessionRef: RpcStringNull,
  lastTurnRef: RpcStringNull,
  title: S.String,
  preview: S.String,
  cwd: RpcStringNull,
  projectLabel: S.String,
  status: S.String,
  statusLabel: S.String,
  source: S.String,
  createdAt: RpcNumberNull,
  updatedAt: RpcNumberNull,
  recencyAt: RpcNumberNull,
  exactTotals: S.optional(RpcSessionExactTotals),
})

const RpcSessionCatalogRequest = S.Struct({
  scope: S.optional(S.Literals(["app", "all_home"])),
  limit: S.optional(S.Number),
  searchTerm: S.optional(S.String),
})

const RpcSessionCatalogResult = S.Struct({
  ok: S.Literal(true),
  schemaVersion: S.Literal("khala-code-desktop.session-catalog.v1"),
  scope: S.Literals(["app", "all_home"]),
  entries: S.Array(RpcSessionCatalogEntry),
  diagnostics: S.Array(S.String),
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

const RpcComposerSelection = S.Struct({
  agentRole: KhalaCodeModelRoleSchema,
  model: RpcStringNull,
  modelProvider: RpcStringNull,
  providerDisplayName: RpcStringNull,
  reasoningEffort: RpcStringNull,
  serviceTier: RpcStringNull,
  variant: RpcStringNull,
  runtimeAdapter: S.Literals(["codex_app_server", "khala_ai_sdk_core"]),
})

const RpcChatTurnRequest = S.Struct({
  attachments: S.optional(S.Array(RpcChatAttachment)),
  composerSelection: S.optional(RpcComposerSelection),
  messages: S.Array(KhalaCodeDesktopMessageSchema),
  sessionId: S.String,
  startNewThread: S.optional(S.Boolean),
  threadId: S.optional(S.String),
  turnId: S.optional(S.String),
})

const RpcBackendProjection = S.Struct({
  baseUrl: S.optional(S.String),
  blockerRefs: S.optional(RpcStringArray),
  credentialSource: S.optional(
    S.Literals([
      "env:OPENROUTER_API_KEY",
      "env:KHALA_CODE_HOSTED_BYOK_OPENROUTER_API_KEY",
      "khala-provider-key",
    ]),
  ),
  kind: S.Literals(["claude_app_sdk", "codex_app_server", "hosted_openagents", "mock"]),
  model: S.String,
  provider: S.optional(S.Literal("openrouter")),
  runtimeMode: S.optional(RpcRuntimeMode),
  threadId: S.optional(S.String),
  toolCatalogKind: S.optional(S.Union([RpcToolCatalogKind, S.Literal("codex_app_server")])),
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
  capability: S.Literals(["codex_accounts", "codex_harness", "coding", "pylon", "token_accounting"]),
  observedAt: S.String,
  reason: S.String,
  status: S.Literals(["error", "not_configured", "ready", "unavailable"]),
})

const RpcAppInfo = S.Struct({
  ok: S.Literal(true),
  app: S.Literal("Khala Code Desktop"),
  observedAt: S.String,
})

const RpcQaMetricName = S.Literals([
  "app_server.spawn_ready_ms",
  "cache.hit",
  "cockpit.render_ms",
  "composer.keystroke_echo_ms",
  "first_render.ms",
  "lifecycle_event_to_card.ms",
  "panel.open_ms",
  "sse.event_to_ui_ms",
  "startup.interactive_ms",
  "supervisor.tick_ms",
  "thread_switch.full_render_ms",
  "thread_switch.hydrated_render_ms",
  "thread_switch.optimistic_render_ms",
  "thread_switch.rpc_ms",
  "transcript.scroll_dropped_frames_pct",
  "turn_start.first_event_ms",
  "turn_start.latency_ms",
])
const RpcQaMetricUnit = S.Literals(["count", "ms", "percent"])
const RpcQaMetricBudgetUnit = S.Literals(["ms", "percent"])
const RpcQaMetricContext = S.Record(
  S.String,
  S.Union([S.String, S.Number, S.Boolean]),
)
const RpcQaMetricSample = S.Struct({
  context: S.optional(RpcQaMetricContext),
  metric: RpcQaMetricName,
  observedAt: S.String,
  unit: RpcQaMetricUnit,
  value: S.Number,
})
const RpcQaMetricSampleResult = S.Struct({
  ok: S.Literal(true),
  observedAt: S.String,
})
const RpcQaMetricDefinition = S.Struct({
  description: S.String,
  kind: S.Literals(["counter", "gauge", "timer"]),
  name: RpcQaMetricName,
  unit: RpcQaMetricUnit,
})
const RpcQaMetricBudget = S.Struct({
  budgetId: S.String,
  description: S.String,
  metric: RpcQaMetricName,
  operator: S.Literal("lte"),
  percentile: S.optional(S.Number),
  requiredContext: S.optional(RpcQaMetricContext),
  threshold: S.Number,
  unit: RpcQaMetricBudgetUnit,
})
const RpcQaMetricBudgetEvaluation = S.Struct({
  actual: RpcNumberNull,
  budgetId: S.String,
  metric: RpcQaMetricName,
  ok: S.Boolean,
  sampleCount: S.Number,
  status: S.Literals(["pass", "fail", "inconclusive"]),
  threshold: S.Number,
  unit: RpcQaMetricBudgetUnit,
})
const RpcQaMetricsSnapshot = S.Struct({
  budgets: S.Array(RpcQaMetricBudget),
  definitions: S.Array(RpcQaMetricDefinition),
  evaluations: S.Array(RpcQaMetricBudgetEvaluation),
  ok: S.Literal(true),
  observedAt: S.String,
  samples: S.Array(RpcQaMetricSample),
  schema: S.Literal("openagents.khala_code.qa_metrics.v1"),
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
  status: S.Literals(["error", "not_configured", "ready", "unavailable"]),
  binary: S.Struct({
    command: S.String,
    source: S.Literals(["PATH", "env:KHALA_CODE_CODEX_BINARY", "env:KHALA_CODE_CODEX_COMMAND", "input"]),
    available: S.Boolean,
    version: RpcStringNull,
    error: RpcStringNull,
  }),
  home: S.Struct({
    path: S.String,
    source: S.Literals(["default:~/.codex", "env:CODEX_HOME", "input"]),
    role: S.Literal("main_user_codex_home"),
    authPath: S.String,
    fleetIsolation: S.Literal("fleet_accounts_use_pylon_isolated_homes"),
  }),
  auth: S.Struct({
    state: S.Literals(["credentials_missing", "error", "invalid", "ready"]),
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

const RpcRateLimitResetConsumeRequest = S.Struct({
  accountRef: S.String,
})

const RpcCodexAccountStatus = S.Struct({
  provider: S.Literal("codex"),
  accountRef: S.Literal("default"),
  credentialSource: S.Literals(["CODEX_HOME", "default_home"]),
  homeRef: S.Literals(["env:CODEX_HOME", "default:~/.codex"]),
  homeRole: S.Literal("main_user_codex_home"),
  readiness: S.Struct({
    state: S.Literals(["error", "ready", "credentials_missing", "invalid"]),
    blockerRefs: RpcStringArray,
  }),
  rateLimits: RpcRateLimitStatus,
})

const RpcCodexAccountsStatus = S.Struct({
  ok: S.Literal(true),
  app: S.Literal("Khala Code Desktop"),
  available: S.Boolean,
  capability: S.Literal("codex_accounts"),
  observedAt: S.String,
  reason: S.String,
  status: S.Literals(["error", "not_configured", "ready", "unavailable"]),
  accounts: S.Array(RpcCodexAccountStatus),
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
  initializeResult: S.Unknown,
  lastError: RpcStringNull,
  pendingRequestCount: S.Number,
  pid: S.NullOr(S.Number),
  state: S.Literals(["errored", "running", "starting", "stopped"]),
  transport: S.Literal("stdio"),
})

const RpcCodexAppServerControlResult = S.Struct({
  ok: S.Boolean,
  action: S.Literals(["restart", "start", "stop"]),
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
  resumable: S.optional(S.Boolean),
  unavailableReason: S.optional(RpcStringNull),
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
  thread: S.Unknown,
  threadId: S.String,
})
const RpcThreadListResult = S.Struct({
  ok: S.Literal(true),
  backwardsCursor: S.optional(RpcStringNull),
  data: S.Array(S.Unknown),
  groups: S.optional(S.Array(RpcThreadGroup)),
  nextCursor: S.optional(RpcStringNull),
  threads: S.optional(S.Array(RpcThreadSummary)),
})
const RpcThreadMutationResult = S.Struct({
  action: S.Literals(["archive", "delete", "fork", "rename", "unarchive"]),
  ok: S.Boolean,
  messages: S.optional(S.Array(KhalaCodeDesktopMessageSchema)),
  response: S.optional(S.Unknown),
  thread: S.optional(S.Unknown),
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
  response: S.optional(S.Unknown),
  threadId: S.optional(S.String),
})
const RpcThreadCompactRequest = S.Struct({
  sessionId: S.optional(S.String),
  threadId: S.optional(S.String),
})

const RpcApprovalRespondRequest = S.Struct({
  action: S.Literals([
    "accept",
    "acceptForSession",
    "acceptWithExecpolicyAmendment",
    "applyNetworkPolicyAmendment",
    "cancel",
    "decline",
    "grantPermissions",
    "grantPermissionsForSession",
    "grantPermissionsWithStrictReview",
  ]),
  execpolicyAmendment: S.optional(RpcStringArray),
  method: S.Literals([
    "item/commandExecution/requestApproval",
    "item/fileChange/requestApproval",
    "item/permissions/requestApproval",
  ]),
  networkPolicyAmendment: S.optional(S.Struct({ action: S.String, host: S.String })),
  permissions: S.optional(RpcCodexPermissionProfile),
  requestId: S.Union([S.String, S.Number]),
})
const RpcApprovalRespondResult = S.Struct({
  method: S.Literals([
    "item/commandExecution/requestApproval",
    "item/fileChange/requestApproval",
    "item/permissions/requestApproval",
  ]),
  ok: S.Boolean,
  payload: S.optional(S.Unknown),
  requestId: S.Union([S.String, S.Number]),
  error: S.optional(S.String),
})
const RpcClaudeApprovalDecision = S.Union([
  S.Struct({
    behavior: S.Literal("allow"),
    decisionClassification: S.optional(S.String),
    updatedInput: S.optional(S.Record(S.String, S.Unknown)),
    updatedPermissions: S.optional(S.Array(S.Record(S.String, S.Unknown))),
  }),
  S.Struct({
    behavior: S.Literal("deny"),
    decisionClassification: S.optional(S.String),
    interrupt: S.optional(S.Boolean),
    message: S.String,
  }),
])
const RpcClaudeApprovalRequestProjection = S.Struct({
  id: S.String,
  input: S.Record(S.String, S.Unknown),
  options: S.Struct({
    blockedPath: S.optional(S.String),
    decisionReason: S.optional(S.String),
    description: S.optional(S.String),
    displayName: S.optional(S.String),
    suggestions: S.optional(S.Array(S.Record(S.String, S.Unknown))),
    title: S.optional(S.String),
  }),
  toolName: S.String,
})
const RpcClaudeApprovalPendingResult = S.Struct({
  ok: S.Boolean,
  requests: S.Array(RpcClaudeApprovalRequestProjection),
})
const RpcClaudeApprovalRespondRequest = S.Struct({
  decision: RpcClaudeApprovalDecision,
  requestId: S.String,
})
const RpcClaudeApprovalRespondResult = S.Struct({
  ok: S.Boolean,
  requestId: S.String,
  decision: RpcClaudeApprovalDecision,
  error: S.optional(S.String),
})
const RpcClaudeSettingsModel = S.Struct({
  description: RpcStringNull,
  displayName: S.String,
  selected: S.Boolean,
  supportsAdaptiveThinking: S.NullOr(S.Boolean),
  supportsEffort: S.NullOr(S.Boolean),
  supportedEffortLevels: RpcStringArray,
  value: S.String,
})
const RpcClaudeSettingsProjection = S.Struct({
  ok: S.Boolean,
  observedAt: S.String,
  errors: RpcStringArray,
  account: S.Struct({
    apiProvider: RpcStringNull,
    apiKeySource: RpcStringNull,
    email: RpcStringNull,
    organization: RpcStringNull,
    subscriptionType: RpcStringNull,
    tokenSource: RpcStringNull,
  }),
  init: S.Struct({
    permissionMode: RpcStringNull,
    model: RpcStringNull,
    system: S.Unknown,
  }),
  models: S.Struct({
    options: S.Array(RpcClaudeSettingsModel),
    selected: S.NullOr(RpcClaudeSettingsModel),
  }),
})

const RpcCodexSettingsReadRequest = S.Struct({
  cwd: S.optional(S.String),
  includeHiddenModels: S.optional(S.Boolean),
})
const RpcCodexSettingsModelOption = S.Struct({
  id: S.String,
  model: S.String,
  displayName: S.String,
  description: RpcStringNull,
  providerId: RpcStringNull,
  providerDisplayName: RpcStringNull,
  hidden: S.Boolean,
  isDefault: S.Boolean,
  supportsPersonality: S.Boolean,
  defaultReasoningEffort: RpcStringNull,
  supportedReasoningEfforts: S.Array(S.Struct({
    value: S.String,
    description: RpcStringNull,
  })),
  serviceTiers: S.Array(S.Struct({
    id: S.String,
    name: S.String,
    description: RpcStringNull,
  })),
  defaultServiceTier: RpcStringNull,
})
const RpcCodexSettingsProviderOption = S.Struct({
  id: S.String,
  displayName: S.String,
  modelCount: S.Number,
})
const RpcCodexSettingsProjection = S.Struct({
  ok: S.Boolean,
  observedAt: S.String,
  cwd: RpcStringNull,
  errors: RpcStringArray,
  config: S.Struct({
    model: RpcStringNull,
    modelProvider: RpcStringNull,
    reasoningEffort: RpcStringNull,
    reasoningSummary: RpcStringNull,
    verbosity: RpcStringNull,
    serviceTier: RpcStringNull,
    approvalPolicy: S.Unknown,
    approvalsReviewer: S.Unknown,
    sandboxMode: RpcStringNull,
    defaultPermissions: RpcStringNull,
    webSearch: RpcStringNull,
    personality: RpcStringNull,
    layersAvailable: S.Boolean,
    originKeys: RpcStringArray,
  }),
  appearance: S.Struct({
    keymap: RpcJson,
    keyPaths: S.Struct({
      keymap: S.Literal("tui.keymap"),
      pet: S.Literal("tui.pet"),
      petAnchor: S.Literal("tui.pet_anchor"),
      personality: S.Literal("personality"),
      statusLine: S.Literal("tui.status_line"),
      statusLineUseColors: S.Literal("tui.status_line_use_colors"),
      theme: S.Literal("tui.theme"),
      vimModeDefault: S.Literal("tui.vim_mode_default"),
    }),
    pet: RpcStringNull,
    petAnchor: RpcStringNull,
    personality: RpcStringNull,
    statusLine: S.NullOr(RpcStringArray),
    statusLineUseColors: S.NullOr(S.Boolean),
    theme: RpcStringNull,
    vimModeDefault: S.NullOr(S.Boolean),
  }),
  models: S.Struct({
    selected: S.NullOr(RpcCodexSettingsModelOption),
    options: S.Array(RpcCodexSettingsModelOption),
    serviceTierCommands: RpcStringArray,
  }),
  providers: S.Struct({
    selected: S.NullOr(RpcCodexSettingsProviderOption),
    options: S.Array(RpcCodexSettingsProviderOption),
  }),
  providerCapabilities: S.Struct({
    namespaceTools: S.NullOr(S.Boolean),
    imageGeneration: S.NullOr(S.Boolean),
    webSearch: S.NullOr(S.Boolean),
  }),
  permissions: S.Struct({
    selectedProfile: RpcStringNull,
    profiles: S.Array(S.Struct({
      id: S.String,
      description: RpcStringNull,
      allowed: S.Boolean,
      selected: S.Boolean,
    })),
    blockedProfileIds: RpcStringArray,
  }),
  requirements: S.Struct({
    managed: S.Boolean,
    allowedApprovalPolicies: S.NullOr(S.Array(S.Unknown)),
    allowedSandboxModes: S.NullOr(RpcStringArray),
    allowedPermissionProfiles: S.NullOr(RpcStringArray),
    defaultPermissions: RpcStringNull,
    blockers: S.Array(S.Struct({
      key: S.String,
      message: S.String,
    })),
  }),
  usage: S.Struct({
    summary: S.Unknown,
    dailyUsageBuckets: S.NullOr(S.Array(S.Unknown)),
    available: S.Boolean,
  }),
  collaboration: S.Struct({
    modes: S.Array(S.Struct({
      name: S.String,
      mode: RpcStringNull,
      model: RpcStringNull,
      reasoningEffort: RpcStringNull,
    })),
    currentMode: RpcStringNull,
    personality: RpcStringNull,
  }),
  modelRolePresets: S.Struct({
    keyPath: S.Literal("openagents.model_roles"),
    activePreset: RpcStringNull,
    presets: S.Array(S.Struct({
      id: S.Literal("architect-coder-judge"),
      title: S.String,
      description: S.String,
      configKeyPath: S.Literal("openagents.model_roles"),
      promiseRef: S.Literal("khala_code.architect_coder_judge.v1"),
      noProxyRails: S.Literal(true),
      noResale: S.Literal(true),
      copyGate: S.Literal("copy_gated_until_end_to_end_verifiable"),
      selected: S.Boolean,
      roleSummary: RpcStringArray,
      registry: RpcJson,
    })),
  }),
})
const RpcCodexModelRolePresetApplyRequest = S.Struct({
  cwd: S.optional(S.String),
  preset: S.Literal("architect-coder-judge"),
})
const RpcCodexModelRolePresetApplyResult = S.Struct({
  ok: S.Boolean,
  preset: S.Literal("architect-coder-judge"),
  keyPath: S.Literal("openagents.model_roles"),
  settings: S.optional(RpcCodexSettingsProjection),
  error: S.optional(S.String),
})
const RpcModelRoleRegistryReadResult = S.Struct({
  ok: S.Literal(true),
  path: S.String,
  registry: KhalaCodeModelRoleRegistrySchema,
})
const RpcModelRoleRegistryWriteRequest = S.Union([
  S.Struct({
    entry: KhalaCodeModelRoleEntrySchema,
  }),
  S.Struct({
    registry: KhalaCodeModelRoleRegistrySchema,
  }),
])
const RpcModelRoleRegistryWriteResult = S.Struct({
  ok: S.Literal(true),
  path: S.String,
  registry: KhalaCodeModelRoleRegistrySchema,
  saved: S.Boolean,
})
const RpcCodexConfigValueWriteRequest = S.Struct({
  cwd: S.optional(S.String),
  expectedVersion: S.optional(S.String),
  filePath: S.optional(S.String),
  keyPath: S.String,
  mergeStrategy: S.optional(S.Literals(["replace", "upsert"])),
  value: RpcJson,
})
const RpcCodexConfigValueWriteResult = S.Struct({
  ok: S.Boolean,
  keyPath: S.String,
  response: S.optional(S.Unknown),
  settings: S.optional(RpcCodexSettingsProjection),
  error: S.optional(S.String),
})
const RpcCodexEcosystemReadRequest = S.Struct({
  cwd: S.optional(S.String),
  forceRefetchApps: S.optional(S.Boolean),
  forceReloadSkills: S.optional(S.Boolean),
  threadId: S.optional(S.String),
})
const RpcCodexEcosystemSource = S.Literals([
  "apps",
  "hooks",
  "imports",
  "khala",
  "marketplace",
  "mcp",
  "plugins",
  "skills",
])
const RpcCodexEcosystemState = S.Literals([
  "auth_required",
  "desktop_extension",
  "disabled",
  "disabled_by_admin",
  "error",
  "install_required",
  "managed",
  "ready",
  "unknown",
])
const RpcCodexEcosystemSeverity = S.Literals(["critical", "info", "warning"])
const RpcCodexEcosystemItem = S.Struct({
  id: S.String,
  name: S.String,
  source: RpcCodexEcosystemSource,
  state: RpcCodexEcosystemState,
  detail: S.String,
  authRequired: S.Boolean,
  enabled: S.NullOr(S.Boolean),
  installed: S.NullOr(S.Boolean),
  managed: S.Boolean,
  marketplaceName: S.optional(S.String),
  pluginId: S.optional(S.String),
})
const RpcCodexEcosystemSection = S.Struct({
  source: RpcCodexEcosystemSource,
  label: S.String,
  count: S.Number,
  readyCount: S.Number,
  disabledCount: S.Number,
  managedCount: S.Number,
  authRequiredCount: S.Number,
  installRequiredCount: S.Number,
  errorCount: S.Number,
  unknownCount: S.Number,
  items: S.Array(RpcCodexEcosystemItem),
})
const RpcCodexEcosystemProjection = S.Struct({
  ok: S.Boolean,
  cwd: RpcStringNull,
  observedAt: S.String,
  errors: RpcStringArray,
  notifications: S.Array(S.Struct({
    method: S.String,
    receivedAt: S.String,
    summary: S.String,
    severity: RpcCodexEcosystemSeverity,
  })),
  sections: S.Struct({
    apps: RpcCodexEcosystemSection,
    hooks: RpcCodexEcosystemSection,
    imports: RpcCodexEcosystemSection,
    khala: RpcCodexEcosystemSection,
    marketplace: RpcCodexEcosystemSection,
    mcp: RpcCodexEcosystemSection,
    plugins: RpcCodexEcosystemSection,
    skills: RpcCodexEcosystemSection,
  }),
  diagnostics: S.Array(S.Struct({
    ref: S.String,
    source: RpcCodexEcosystemSource,
    severity: RpcCodexEcosystemSeverity,
    title: S.String,
    detail: S.String,
    action: S.Literals(["authenticate", "install", "open_settings", "refresh", "review"]),
    itemId: S.optional(S.String),
    observedAt: S.String,
  })),
})
const RpcCodexAppServerActionResult = S.Struct({
  ok: S.Boolean,
  method: S.String,
  response: S.optional(S.Unknown),
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
  source: S.Literals(["fs/readDirectory", "fuzzyFileSearch"]),
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

const RpcSlashCommandAvailability = S.Struct({
  available: S.Boolean,
  reason: S.optional(S.String),
})
const RpcSlashCommandVisibility = S.Union([
  S.Struct({ kind: S.Literal("always") }),
  S.Struct({ kind: S.Literal("debug") }),
  S.Struct({ kind: S.Literal("not_android") }),
  S.Struct({ kind: S.Literal("platform"), platforms: RpcStringArray }),
])
const RpcSlashCommandDispatch = S.Union([
  S.Struct({
    kind: S.Literal("app_server"),
    method: S.String,
    appServerDependency: S.optional(S.String),
    experimental: S.optional(S.Boolean),
    requiresArgs: S.optional(S.Boolean),
    requiresThread: S.optional(S.Boolean),
  }),
  S.Struct({
    kind: S.Literal("client"),
    action: S.String,
  }),
  S.Struct({
    kind: S.Literal("gap"),
    dependency: S.String,
    unavailable: S.optional(S.Struct({
      gapId: S.String,
      kind: S.Literal("upstream_app_server_gap"),
    })),
    issueRef: S.String,
  }),
])
const RpcSlashCommandWithAvailability = S.Struct({
  aliases: RpcStringArray,
  availability: RpcSlashCommandAvailability,
  availableDuringTask: S.Boolean,
  availableInSideConversation: S.Boolean,
  command: S.String,
  debug: S.Boolean,
  description: S.String,
  dispatch: RpcSlashCommandDispatch,
  enumName: S.String,
  group: S.Literals([
    "background",
    "diagnostics",
    "ecosystem",
    "exit",
    "session",
    "settings",
    "turn_task",
    "workspace",
  ]),
  supportsInlineArgs: S.Boolean,
  visibility: RpcSlashCommandVisibility,
}) as S.Schema<KhalaCodeDesktopSlashCommandWithAvailability>

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
  commands: S.Array(RpcSlashCommandWithAvailability),
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
  response: S.optional(S.Unknown),
  status: S.Literals(["blocked", "client_action", "dispatched", "gap", "not_found", "unavailable"]),
  threadId: S.optional(S.String),
})

const RpcThreadTokenSummaryRequest = S.Struct({
  threadId: S.optional(RpcStringNull),
})
const RpcThreadRoleEconomicsState = S.Literals(["measured", "not_measured", "subscription_covered"])
const RpcThreadRoleEconomicsRow = S.Struct({
  costAmount: RpcNumberNull,
  costCurrency: RpcStringNull,
  pricingState: RpcThreadRoleEconomicsState,
  roleRef: S.String,
  tokenRows: S.Number,
  tokens: S.Number,
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
  roleEconomics: S.Array(RpcThreadRoleEconomicsRow),
  threadId: RpcStringNull,
  totalTokens: S.Number,
  updatedAt: RpcStringNull,
  usageEventRows: S.Number,
})

const RpcToolCatalogResponse = S.Struct({
  catalogKind: RpcToolCatalogKind,
  defaultEnabled: S.Boolean,
  description: S.String,
  runtimeMode: RpcRuntimeMode,
  toolCount: S.Number,
  tools: S.Array(S.Struct({
    authority: S.String,
    name: S.String,
    role: S.Literals(["legacy_codex_equivalent", "supplemental_swarm"]),
  })),
})

const RpcHarnessSetting = S.Struct({
  ok: S.Literal(true),
  mode: RpcRuntimeMode,
  persistedMode: RpcRuntimeMode,
  envOverride: S.NullOr(RpcRuntimeMode),
  path: S.String,
})
const RpcHarnessSettingWriteRequest = S.Struct({
  mode: RpcRuntimeMode,
})
const RpcHarnessSettingWriteResult = S.Struct({
  ok: S.Literal(true),
  mode: RpcRuntimeMode,
  persistedMode: RpcRuntimeMode,
  envOverride: S.NullOr(RpcRuntimeMode),
  path: S.String,
  saved: S.Boolean,
})

const RpcFleetCapacity = S.Struct({
  available: RpcNumberNull,
  busy: RpcNumberNull,
  queued: RpcNumberNull,
  ready: RpcNumberNull,
})
const RpcFleetAccountProvider = S.Literals(["claude_agent", "codex"])
const RpcFleetRunWorkerKind = S.Literals(["codex", "claude", "auto"])
const RpcFleetTokenMeasurementStatus = S.Literals(["exact", "estimated", "not_measured", "pending"])
const RpcFleetSessionRole = S.Literals(["main_local_codex_session", "swarm_worker_codex_session"])
const RpcFleetHomeRole = S.Literals(["main_user_codex_home_display_only", "pylon_isolated_worker_codex_home"])
const RpcFleetQueuePolicy = S.Struct({
  admission: S.Literal("pylon_capacity_gate"),
  cooldown: S.Literals(["none_reported", "ready", "cooling_down", "unknown"]),
  refill: S.Literal("pylon_presence_heartbeat"),
  queued: RpcNumberNull,
})
const RpcFleetSessionLayer = S.Struct({
  label: S.String,
  role: RpcFleetSessionRole,
  homeRole: RpcFleetHomeRole,
  runtime: S.Literal("codex_harness"),
  transcriptSurface: S.Literals(["chat", "fleet"]),
  mutationPolicy: S.Literals(["codex_app_server_owned", "pylon_isolated_home_only"]),
})
const RpcFleetAccount = S.Struct({
  accountRef: S.String,
  provider: RpcFleetAccountProvider,
  readiness: S.String,
  quotaState: RpcStringNull,
  accountKey: RpcStringNull,
  capacity: S.NullOr(RpcFleetCapacity),
  paused: S.optional(S.Boolean),
  rateLimits: S.optional(RpcRateLimitStatus),
  homeRole: S.optional(RpcFleetHomeRole),
  queuePolicy: S.optional(RpcFleetQueuePolicy),
  sessionRole: S.optional(RpcFleetSessionRole),
  email: RpcStringNull,
})
const RpcFleetAssignmentTokenRate = S.Struct({
  source: S.String,
  status: RpcFleetTokenMeasurementStatus,
  tokenCountKind: RpcStringNull,
  tokens: RpcNumberNull,
  tokensPerMinute: RpcNumberNull,
})
const RpcFleetTokenRate = S.Struct({
  activeAdjustedTokensPerMinute: RpcNumberNull,
  completedStatus: RpcFleetTokenMeasurementStatus,
  completedTokenRows: RpcNumberNull,
  completedTokensPerMinute: RpcNumberNull,
  tokensWindow: S.optional(RpcNumberNull),
  inFlightTokens: RpcNumberNull,
  inFlightTokensPerMinute: RpcNumberNull,
  source: S.Literals(["pylon_khala_apm", "unavailable"]),
  unavailableReason: RpcStringNull,
})
const RpcFleetWorkerSession = S.Struct({
  approvalState: S.Literals(["approval_required", "blocked", "none", "ready_for_review"]),
  blockerRefs: RpcStringArray,
  closeoutStatus: RpcStringNull,
  executionRuntime: S.Literal("codex_harness"),
  homeRole: RpcFleetHomeRole,
  queuePolicy: RpcFleetQueuePolicy,
  reviewState: S.Literals(["active", "blocked", "pending_closeout", "ready_for_review"]),
  role: S.Literal("swarm_worker_codex_session"),
  transcriptRef: RpcStringNull,
})
const RpcFleetAssignment = S.Struct({
  assignmentRef: RpcStringNull,
  blockerRefs: S.optional(RpcStringArray),
  closeoutStatus: S.optional(RpcStringNull),
  elapsedMs: RpcNumberNull,
  issueRef: RpcStringNull,
  runRef: S.optional(RpcStringNull),
  workerSession: S.optional(RpcFleetWorkerSession),
  tokenRate: RpcFleetAssignmentTokenRate,
  updatedAt: RpcStringNull,
})
const RpcFleetProcess = S.Struct({
  pid: S.String,
  parentPid: S.String,
  elapsed: S.String,
})
const RpcFleetStatus = S.Struct({
  ok: S.Boolean,
  observedAt: S.String,
  sessionLayers: S.optional(S.Struct({
    main: RpcFleetSessionLayer,
    workers: RpcFleetSessionLayer,
  })),
  pylon: S.Struct({
    status: S.Literals(["online", "started", "unavailable"]),
    pylonRef: RpcStringNull,
    message: S.String,
  }),
  availableCodexAssignments: RpcNumberNull,
  maxCodexAssignments: RpcNumberNull,
  tokenRate: RpcFleetTokenRate,
  accounts: S.Array(RpcFleetAccount),
  activeAssignments: S.Array(RpcFleetAssignment),
  processes: S.Array(RpcFleetProcess),
})

const RpcFleetPromotionContextBoundary = S.Struct({
  allowedRefs: RpcStringArray,
  includeTranscript: S.Literal(false),
  mode: S.Literals(["explicit_objective", "summary_only"]),
  summary: RpcStringNull,
})
const RpcFleetPromotionRequest = S.Struct({
  accountRef: S.optional(S.String),
  branch: S.optional(S.String),
  claimRef: S.optional(S.String),
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
const RpcFleetWorkerRuntime = S.Struct({
  assignmentTool: S.Literal("codex_spawn"),
  homeRole: S.Literal("pylon_isolated_worker_codex_home"),
  role: S.Literal("swarm_worker_codex_session"),
  runtime: S.Literal("codex_harness"),
})
const RpcFleetPromotionResult = S.Struct({
  ok: S.Boolean,
  acceptedCount: S.Number,
  contextBoundary: RpcFleetPromotionContextBoundary,
  origin: S.Struct({
    role: S.Literal("main_local_codex_session"),
    sessionId: S.String,
    threadId: S.String,
  }),
  pylonRef: RpcStringNull,
  requestedCount: S.Number,
  workerRuntime: RpcFleetWorkerRuntime,
  results: S.Array(S.Struct({
    accountRef: RpcStringNull,
    assignmentRef: RpcStringNull,
    closeoutStatus: RpcStringNull,
    status: S.Literals(["accepted", "failed"]),
    summary: S.String,
    tokensVerified: RpcNumberNull,
    transcriptRef: RpcStringNull,
  })),
})
const RpcFleetDelegateRunRequest = S.Struct({
  accountRef: S.optional(S.String),
  branch: S.optional(S.String),
  claimRef: S.optional(S.String),
  commit: S.optional(S.String),
  count: S.optional(S.Number),
  mode: S.Literals(["fixture", "real_work"]),
  noRun: S.optional(S.Boolean),
  objective: S.String,
  repo: S.optional(S.String),
  timeoutMs: S.optional(S.Number),
  verify: S.optional(S.String),
})
const RpcFleetDelegateRunProjection = S.Struct({
  localPathsProjected: S.Literal(false),
  objectiveProjected: S.Literal(false),
  providerPayloadProjected: S.Literal(false),
  rawTraceMessagesProjected: S.Literal(false),
})
const RpcFleetDelegateRunStep = S.Struct({
  blockerCode: RpcStringNull,
  fallbackModule: RpcStringNull,
  module: S.String,
  precondition: S.String,
  refs: RpcStringArray,
  status: S.String,
  summary: S.String,
})
const RpcFleetDelegateRunResult = S.Struct({
  ok: S.Boolean,
  acceptedCount: S.Number,
  delegateSignature: S.Literal("khala.fleet.delegate"),
  delegateStatus: S.Literals(["blocked", "completed"]),
  mode: S.Literals(["fixture", "real_work"]),
  projection: RpcFleetDelegateRunProjection,
  pylonRef: RpcStringNull,
  requestedCount: S.Number,
  results: S.Array(S.Struct({
    accountRef: RpcStringNull,
    assignmentRef: RpcStringNull,
    blockerRefs: RpcStringArray,
    closeoutStatus: RpcStringNull,
    slot: S.Number,
    status: S.Literals(["accepted", "failed"]),
    tokensVerified: RpcNumberNull,
    transcriptRef: RpcStringNull,
  })),
  trace: S.Array(RpcFleetDelegateRunStep),
  validation: S.Struct({
    fixture: S.Boolean,
    repoPinsComplete: S.Boolean,
  }),
  workerRuntime: RpcFleetWorkerRuntime,
})

const RpcFleetRunState = S.Literals(["draft", "running", "paused", "draining", "completed", "stopped"])
const RpcFleetRunControlVerb = S.Literals(["pause", "resume", "drain", "stop"])
const RpcFleetRunRefillPolicy = S.Struct({
  cooldownAware: S.Boolean,
  maxPerAccount: S.Number,
  stopCondition: S.Literals(["backlog_empty", "target_reached", "manual_stop"]),
})
const RpcFleetRunRefillPolicyPatch = S.Struct({
  cooldownAware: S.optional(S.Boolean),
  maxPerAccount: S.optional(S.Number),
  stopCondition: S.optional(S.Literals(["backlog_empty", "target_reached", "manual_stop"])),
})
const RpcFleetRunCounters = S.Struct({
  activeAssignments: S.Number,
  blockedAssignments: S.Number,
  completedAssignments: S.Number,
  failedAssignments: S.Number,
  workUnitsTotal: S.Number,
})
const RpcFleetRunPlanDagNode = S.Struct({
  ref: S.String,
  title: S.String,
  objective: S.String,
  dependsOn: S.optional(RpcStringArray),
  repo: S.optional(S.String),
  branch: S.optional(S.String),
  baseCommit: S.optional(S.String),
  verify: S.optional(S.String),
  issue: S.optional(S.Number),
  labels: S.optional(RpcStringArray),
  url: S.optional(S.String),
})
const RpcFleetRunWorkSource = S.Struct({
  kind: S.Literals(["github_backlog", "issue_list", "fixture", "plan_dag"]),
  repo: S.optional(S.String),
  branch: S.optional(S.String),
  baseCommit: S.optional(S.String),
  verify: S.optional(S.String),
  limit: S.optional(S.Number),
  count: S.optional(S.Number),
  planRef: S.optional(S.String),
  nodes: S.optional(S.Array(RpcFleetRunPlanDagNode)),
  issues: S.optional(S.Array(S.Union([
    S.Number,
    S.Struct({
      kind: S.optional(S.Literals(["issue", "pr"])),
      labels: S.optional(RpcStringArray),
      number: S.Number,
      state: S.optional(S.Literals(["open", "closed", "merged", "OPEN", "CLOSED", "MERGED"])),
      title: S.optional(S.String),
      url: S.optional(S.String),
    }),
  ]))),
})
const RpcFleetRunStartRequest = S.Struct({
  objective: S.String,
  runRef: S.optional(S.String),
  targetConcurrency: S.Number,
  workSource: RpcFleetRunWorkSource,
  workerKind: S.optional(RpcFleetRunWorkerKind),
  refillPolicy: S.optional(RpcFleetRunRefillPolicyPatch),
  tickImmediately: S.optional(S.Boolean),
})
const RpcFleetRunProjection = S.Struct({
  counters: RpcFleetRunCounters,
  createdAt: S.String,
  dispatchKind: S.Literal("supervised_dispatch"),
  objectiveProjected: S.Literal(false),
  pylonRef: RpcStringNull,
  refillPolicy: RpcFleetRunRefillPolicy,
  runRef: S.String,
  startedAt: RpcStringNull,
  state: RpcFleetRunState,
  targetConcurrency: S.Number,
  updatedAt: S.String,
  workerKind: RpcFleetRunWorkerKind,
  workSource: RpcFleetRunWorkSource,
})
const RpcFleetRunStartResult = S.Struct({
  ok: S.Boolean,
  run: RpcFleetRunProjection,
  supervisorStarted: S.Boolean,
})
const RpcFleetRunStatusRequest = S.Struct({ runRef: S.String })
const RpcFleetRunStatusResult = S.Struct({
  ok: S.Boolean,
  run: S.NullOr(RpcFleetRunProjection),
  supervisorActive: S.Boolean,
})
const RpcFleetRunControlRequest = S.Struct({
  runRef: S.String,
  verb: RpcFleetRunControlVerb,
})
const RpcFleetRunControlResult = S.Struct({
  ok: S.Boolean,
  previousState: RpcFleetRunState,
  run: RpcFleetRunProjection,
  supervisorActive: S.Boolean,
  verb: RpcFleetRunControlVerb,
})
const RpcFleetRunListRequest = S.Struct({
  state: S.optional(RpcFleetRunState),
})
const RpcFleetRunListResult = S.Struct({
  ok: S.Boolean,
  runs: S.Array(RpcFleetRunProjection),
})
// ---------------------------------------------------------------------------
// Khala Sync fleet cockpit (KS-6.2, #8303; docs/khala-sync/SPEC.md §6).
// Default-on source for the Fleet screen: state reads the synced fleet_run
// scope through the local Khala Sync store + overlay; mutate routes operator
// intents through the session's optimistic mutators. KHALA_SYNC_FLEET is now
// an explicit opt-out only. The wire entity shapes mirror
// packages/khala-sync/src/fleet.ts.
// ---------------------------------------------------------------------------
const RpcKhalaSyncFleetPhase = S.Literals([
  "disabled",
  "idle",
  "bootstrapping",
  "catching_up",
  "live",
  "must_refetch",
  "denied",
])
const RpcKhalaSyncFleetRun = S.Struct({
  counters: RpcFleetRunCounters,
  desiredSlots: S.Number,
  runId: S.String,
  startedAt: RpcStringNull,
  status: RpcFleetRunState,
  updatedAt: S.String,
  workerKind: RpcFleetRunWorkerKind,
})
const RpcKhalaSyncFleetWorker = S.Struct({
  accountRefHash: RpcStringNull,
  assignmentRef: RpcStringNull,
  lastProgressAt: RpcStringNull,
  phase: S.Literals(["idle", "dispatched", "completed", "failed", "blocked", "circuit_broken", "paused"]),
  updatedAt: S.String,
  workerId: S.String,
})
const RpcKhalaSyncFleetAssignment = S.Struct({
  assignmentRef: S.String,
  closeoutClass: RpcStringNull,
  issueRef: RpcStringNull,
  status: S.String,
  updatedAt: S.String,
})
const RpcKhalaSyncFleetAccount = S.Struct({
  accountRefHash: S.String,
  rateLimitClass: RpcStringNull,
  readiness: S.Literals(["ready", "cooldown", "unavailable", "unknown"]),
  updatedAt: S.String,
})
// In-band mutation rejections (SPEC §2.4): surfaced to the UI as state, never
// thrown — the queue keeps draining.
const RpcKhalaSyncFleetRejection = S.Struct({
  errorCode: S.String,
  messageSafe: S.String,
  mutationId: S.Number,
  mutatorName: S.String,
  observedAt: S.String,
  runId: RpcStringNull,
})
const RpcKhalaSyncFleetStateRequest = S.Struct({ runId: S.String })
const RpcKhalaSyncFleetStateResult = S.Struct({
  accounts: S.Array(RpcKhalaSyncFleetAccount),
  assignments: S.Array(RpcKhalaSyncFleetAssignment),
  authState: S.Literals(["connected", "missing"]),
  cursor: S.NullOr(S.Number),
  enabled: S.Boolean,
  error: S.optional(S.String),
  ok: S.Boolean,
  pendingMutations: S.Number,
  phase: RpcKhalaSyncFleetPhase,
  reason: RpcStringNull,
  rejections: S.Array(RpcKhalaSyncFleetRejection),
  run: S.NullOr(RpcKhalaSyncFleetRun),
  workers: S.Array(RpcKhalaSyncFleetWorker),
})
const RpcKhalaSyncFleetMutateRequest = S.Struct({
  action: S.Literals([
    "pause",
    "resume",
    "set_desired_slots",
    "pause_worker",
    "resume_worker",
    "acknowledge_inbox_flag",
    "stop",
  ]),
  desiredSlots: S.optional(S.Number),
  runId: S.String,
  /** Required for pause_worker / resume_worker. */
  workerId: S.optional(S.String),
  /** Required for acknowledge_inbox_flag. */
  flagRef: S.optional(S.String),
  /** Required (true) for the terminal stop action. */
  confirm: S.optional(S.Boolean),
})
const RpcKhalaSyncFleetMutateResult = S.Struct({
  error: S.optional(S.String),
  ok: S.Boolean,
  queuedMutationId: S.optional(S.Number),
})
const RpcKhalaSyncFleetReportAccountStateRequest = S.Struct({
  accountRefHash: S.String,
  capacityAvailable: S.optional(S.Number),
  capacityBusy: S.optional(S.Number),
  capacityQueued: S.optional(S.Number),
  provider: S.optional(S.String),
  rateLimitClass: S.optional(S.String),
  readiness: S.Literals(["ready", "cooldown", "unavailable", "unknown"]),
  runId: S.String,
})
const RpcKhalaSyncFleetReportAccountStateResult = S.Struct({
  error: S.optional(S.String),
  ok: S.Boolean,
})
const RpcKhalaSyncChatThread = S.Struct({
  createdAt: S.String,
  lastMessageAt: RpcStringNull,
  messageCount: S.Number,
  ownerUserId: S.String,
  status: S.Literal("active"),
  threadId: S.String,
  title: S.String,
  updatedAt: S.String,
})
const RpcKhalaSyncChatMessage = S.Struct({
  authorUserId: S.String,
  body: S.String,
  createdAt: S.String,
  deletedAt: RpcStringNull,
  messageId: S.String,
  threadId: S.String,
  updatedAt: S.String,
})
/**
 * #8425 desktop render-gap closeout: one synthesized assistant reply folded
 * from a thread's `runtime_turn` + `runtime_event` rows (see
 * `khala-runtime-transcript-desktop-core.ts`). A turn dispatched from
 * mobile (or any non-desktop-composer surface) never produces a
 * `chat_message` for its reply — this is the only wire shape that carries
 * it to desktop's chat surface.
 */
const RpcKhalaSyncRuntimeMessage = S.Struct({
  body: S.String,
  role: S.Literal("assistant"),
  sortKey: S.String,
  turnId: S.String,
})
const RpcKhalaSyncChatRejection = S.Struct({
  errorCode: S.String,
  messageSafe: S.String,
  mutationId: S.Number,
  mutatorName: S.String,
  observedAt: S.String,
  threadId: RpcStringNull,
})
const RpcKhalaSyncChatThreadsRequest = S.Struct({
  limit: S.optional(S.Number),
  searchTerm: S.optional(S.String),
})
const RpcKhalaSyncChatThreadsResult = S.Struct({
  authState: S.Literals(["connected", "missing"]),
  cursor: S.NullOr(S.Number),
  enabled: S.Boolean,
  error: S.optional(S.String),
  ok: S.Boolean,
  ownerUserId: RpcStringNull,
  pendingMutations: S.Number,
  phase: RpcKhalaSyncFleetPhase,
  reason: RpcStringNull,
  rejections: S.Array(RpcKhalaSyncChatRejection),
  threads: S.Array(RpcKhalaSyncChatThread),
})
const RpcKhalaSyncChatMessagesRequest = S.Struct({
  limit: S.optional(S.Number),
  threadId: S.String,
})
const RpcKhalaSyncChatMessagesResult = S.Struct({
  authState: S.Literals(["connected", "missing"]),
  cursor: S.NullOr(S.Number),
  enabled: S.Boolean,
  error: S.optional(S.String),
  messages: S.Array(RpcKhalaSyncChatMessage),
  ok: S.Boolean,
  ownerUserId: RpcStringNull,
  pendingMutations: S.Number,
  phase: RpcKhalaSyncFleetPhase,
  reason: RpcStringNull,
  rejections: S.Array(RpcKhalaSyncChatRejection),
  /** #8425: synthesized assistant replies folded from runtime_turn/runtime_event
   * rows, additive to `messages` (which stays chat_message-only). Always
   * present (empty array when there is nothing to fold) so callers never
   * need to guard on `undefined`. */
  runtimeMessages: S.Array(RpcKhalaSyncRuntimeMessage),
  threadId: S.String,
})
const RpcKhalaSyncChatCreateThreadRequest = S.Struct({
  threadId: S.String,
  title: S.String,
})
const RpcKhalaSyncChatAppendMessageRequest = S.Struct({
  body: S.String,
  messageId: S.String,
  threadId: S.String,
})
const RpcKhalaSyncChatRenameThreadRequest = S.Struct({
  threadId: S.String,
  title: S.String,
})
const RpcKhalaSyncChatMutationResult = S.Struct({
  error: S.optional(S.String),
  messageId: S.optional(S.String),
  ok: S.Boolean,
  threadId: S.String,
})
const RpcArchitectPlanDispatchMode = S.Literals(["in_thread", "fleet_run"])
const RpcArchitectPlanDagNode = S.Struct({
  nodeRef: S.String,
  title: S.String,
  objective: S.String,
  dependsOn: S.optional(RpcStringArray),
  repo: S.optional(S.String),
  branch: S.optional(S.String),
  baseCommit: S.optional(S.String),
  verify: S.optional(S.String),
  issue: S.optional(S.Number),
  labels: S.optional(RpcStringArray),
  evidenceRefs: S.optional(RpcStringArray),
})
const RpcArchitectPlanDag = S.Struct({
  schema: S.Literal("openagents.khala_code.claude_plan_fanout_dag.v1"),
  planRef: S.String,
  source: S.Literal("claude_plan_mode"),
  generatedAt: S.String,
  objective: S.String,
  repo: S.optional(S.String),
  branch: S.optional(S.String),
  baseCommit: S.optional(S.String),
  verify: S.optional(S.String),
  evidenceRefs: S.optional(RpcStringArray),
  nodes: S.Array(RpcArchitectPlanDagNode),
})
const RpcArchitectPlanArtifact = S.Struct({
  schema: S.Literal("openagents.khala_code.architect_plan_artifact.v1"),
  planRef: S.String,
  sessionId: S.String,
  createdAt: S.String,
  updatedAt: S.String,
  status: S.Literals(["pending_approval", "approved", "rejected", "dispatched"]),
  architectRole: S.Struct({
    role: S.Literal("architect"),
    harness: S.Literal("claude"),
    mode: S.Literal("plan"),
    readOnly: S.Literal(true),
  }),
  dispatchMode: RpcArchitectPlanDispatchMode,
  dag: RpcArchitectPlanDag,
  fleetRunRef: RpcStringNull,
  coderTurnId: RpcStringNull,
})
const RpcArchitectPlanRunRequest = S.Struct({
  sessionId: S.String,
  objective: S.String,
  threadId: S.optional(S.String),
  repo: S.optional(S.String),
  branch: S.optional(S.String),
  baseCommit: S.optional(S.String),
  verify: S.optional(S.String),
})
const RpcArchitectPlanRunResult = S.Union([
  S.Struct({ ok: S.Literal(true), artifact: RpcArchitectPlanArtifact }),
  S.Struct({ ok: S.Literal(false), error: S.String }),
])
const RpcArchitectPlanDecisionRequest = S.Struct({
  sessionId: S.String,
  planRef: S.String,
  decision: S.Literals(["approve", "reject"]),
  threadId: S.optional(S.String),
})
const RpcArchitectPlanDecisionResult = S.Union([
  S.Struct({
    ok: S.Literal(true),
    artifact: RpcArchitectPlanArtifact,
    message: S.String,
  }),
  S.Struct({ ok: S.Literal(false), error: S.String }),
])
const RpcFleetWorkerControlVerb = S.Literals(["interrupt", "retry", "flag"])
const RpcFleetWorkerControlRequest = S.Struct({
  assignmentRef: RpcStringNull,
  issueRef: RpcStringNull,
  note: S.optional(S.String),
  runRef: RpcStringNull,
  verb: RpcFleetWorkerControlVerb,
  workerRefHash: S.String,
})
const RpcFleetWorkerControlResult = S.Struct({
  accepted: S.Boolean,
  assignmentRef: RpcStringNull,
  inboxItemRef: RpcStringNull,
  ok: S.Boolean,
  runRef: RpcStringNull,
  verb: RpcFleetWorkerControlVerb,
  workerRefHash: S.String,
})
const RpcForumRequest = S.Struct({
  body: S.optional(KhalaCodeDesktopRpcJsonValue),
  headers: S.optional(S.Record(S.String, S.String)),
  method: S.optional(S.Literals(["GET", "POST"])),
  path: S.String,
})
const RpcForumResponse = S.Struct({
  ok: S.Boolean,
  payload: KhalaCodeDesktopRpcJsonValue,
  status: S.Number,
  error: S.optional(S.String),
})

// Khala Code plans (promise khala_code.free_paid_plans.v1). The desktop app is a
// read-and-render surface: the current plan is resolved server-side and the paid
// plan purchase seam stays honest about being flag-gated default-OFF. When the
// server arms it, purchase success is two-phase: payment_required first, then a
// fulfilled receipt only after Stripe/Lightning settlement.
const RpcKhalaCodePlanKind = S.Literals(["free", "paid"])
const RpcKhalaCodePlanPaymentRail = S.Literals(["stripe_checkout", "lightning_mpp"])
const RpcKhalaCodePlanPurchaseSeam = S.Struct({
  armed: S.Boolean,
  envFlag: S.String,
  route: S.String,
})
const RpcKhalaCodePlan = S.Struct({
  captureExcluded: S.Boolean,
  isDefault: S.Boolean,
  kind: RpcKhalaCodePlanKind,
  label: S.String,
  planId: S.String,
  priceLabel: S.String,
  purchase: S.optional(RpcKhalaCodePlanPurchaseSeam),
  tagline: S.String,
  terms: RpcStringArray,
})
const RpcKhalaCodePlanCatalog = S.Struct({
  authorityBoundary: S.String,
  blockerRefs: RpcStringArray,
  catalogVersion: S.String,
  plans: S.Array(RpcKhalaCodePlan),
  promiseId: S.String,
  relatedPromiseIds: RpcStringArray,
  schemaVersion: S.Literal("openagents.khala_code.plan_catalog.v1"),
  summary: S.String,
})
const RpcKhalaCodePlanCatalogResult = S.Union([
  S.Struct({ ok: S.Literal(true), catalog: RpcKhalaCodePlanCatalog }),
  S.Struct({ ok: S.Literal(false), error: S.Literal("catalog_unavailable") }),
])
const RpcKhalaCodePlanStatusPlan = S.Struct({
  captureExcluded: S.Boolean,
  kind: RpcKhalaCodePlanKind,
  planId: S.String,
  reasonRef: S.optional(S.String),
})
const RpcKhalaCodePlanStatusResult = S.Union([
  S.Struct({ state: S.Literal("ok"), plan: RpcKhalaCodePlanStatusPlan }),
  S.Struct({ state: S.Literal("unauthenticated") }),
  S.Struct({ state: S.Literal("unavailable") }),
])
const RpcKhalaCodeOpenAgentsAuthPendingAttempt = S.Struct({
  attemptId: S.String,
  expiresAt: S.String,
  intervalSeconds: S.Number,
  userCode: S.String,
  verificationUrl: S.String,
})
const RpcKhalaCodeOpenAgentsAuthStatusResult = S.Struct({
  ok: S.Literal(true),
  path: S.String,
  pendingAttempt: S.NullOr(RpcKhalaCodeOpenAgentsAuthPendingAttempt),
  source: S.NullOr(S.Literals(["env", "persisted"])),
  state: S.Literals(["connected", "missing", "pending"]),
  tokenPrefix: RpcStringNull,
})
const RpcKhalaCodeOpenAgentsAuthStartSuccess = S.Struct({
  ...RpcKhalaCodeOpenAgentsAuthPendingAttempt.fields,
  ok: S.Literal(true),
  status: S.Literal("pending"),
})
const RpcKhalaCodeOpenAgentsAuthStartResult = S.Union([
  RpcKhalaCodeOpenAgentsAuthStartSuccess,
  S.Struct({
    ok: S.Literal(false),
    error: S.Literal("connect_unavailable"),
  }),
])
const RpcKhalaCodeOpenAgentsAuthPollResult = S.Union([
  S.Struct({
    ...RpcKhalaCodeOpenAgentsAuthPendingAttempt.fields,
    ok: S.Literal(true),
    status: S.Literal("pending"),
  }),
  S.Struct({
    ok: S.Literal(true),
    saved: S.Literal(true),
    source: S.Literal("persisted"),
    status: S.Literal("linked"),
    tokenPrefix: S.String,
  }),
  S.Struct({
    attemptId: S.String,
    ok: S.Literal(true),
    status: S.Literal("expired"),
  }),
  S.Struct({
    ok: S.Literal(false),
    error: S.Literals(["connect_unavailable", "no_pending_attempt"]),
  }),
])
const RpcKhalaCodePlanPurchaseRequest = S.Struct({
  idempotencyKey: S.optional(S.String),
  lightningPaymentHash: S.optional(S.String),
  preimage: S.optional(S.String),
  rail: S.optional(RpcKhalaCodePlanPaymentRail),
})
const RpcKhalaCodePlanPurchaseSuccess = S.Struct({
  ok: S.Literal(true),
  captureExcluded: S.Boolean,
  entitlementRef: S.String,
  planId: S.String,
  purchaseRef: S.optional(S.String),
  rail: S.optional(RpcKhalaCodePlanPaymentRail),
  receiptRef: S.String,
  receiptUrl: S.optional(S.String),
  status: S.optional(S.Literal("fulfilled")),
})
const RpcKhalaCodePlanPurchaseStripePaymentRequired = S.Struct({
  ok: S.Literal(true),
  checkoutUrl: S.String,
  planId: S.String,
  purchaseRef: S.String,
  rail: S.Literal("stripe_checkout"),
  status: S.Literal("payment_required"),
  stripeCheckoutSessionId: S.String,
})
const RpcKhalaCodePlanPurchaseLightningPaymentRequired = S.Struct({
  ok: S.Literal(true),
  bolt11: S.String,
  invoiceExpiresAt: S.optional(S.String),
  network: S.Literals(["mainnet", "regtest", "signet"]),
  paymentHash: S.String,
  planId: S.String,
  purchaseRef: S.String,
  rail: S.Literal("lightning_mpp"),
  status: S.Literal("payment_required"),
})
const RpcKhalaCodePlanPurchaseResult = S.Union([
  RpcKhalaCodePlanPurchaseSuccess,
  RpcKhalaCodePlanPurchaseStripePaymentRequired,
  RpcKhalaCodePlanPurchaseLightningPaymentRequired,
  S.Struct({
    ok: S.Literal(false),
    error: S.Literals([
      "khala_code_paid_plans_not_enabled",
      "unauthenticated",
      "purchase_unavailable",
    ]),
  }),
])
const RpcKhalaCodeTraceCaptureReason = S.Literals([
  "consent_disabled",
  "owner_not_armed",
  "paid_plan_capture_excluded",
  "ready_for_redacted_owner_only_ingest",
])
const RpcKhalaCodeTraceCaptureMarker = S.Struct({
  payoutEligible: S.Literal(false),
  revenueShareEligible: S.Literal(false),
  settlementEligible: S.Literal(false),
})
const RpcKhalaCodeTraceCapturePipeline = S.Struct({
  ingestAudience: S.Literal(KHALA_CODE_DESKTOP_TRACE_CAPTURE_INGEST_AUDIENCE),
  redaction: S.Literal("rampart_required"),
  sessionEvents: S.Literal("explicit_consent_only"),
})
const RpcKhalaCodeTraceCaptureStatusFields = {
  blockerRefs: RpcStringArray,
  disclosureRef: S.Literal(KHALA_CODE_DESKTOP_TRACE_CAPTURE_DISCLOSURE_REF),
  enabled: S.Boolean,
  marker: RpcKhalaCodeTraceCaptureMarker,
  ok: S.Literal(true),
  ownerArmed: S.Boolean,
  ownerGateEnv: S.Literal(KHALA_CODE_DESKTOP_TRACE_CAPTURE_OWNER_GATE_ENV),
  path: S.String,
  pipeline: RpcKhalaCodeTraceCapturePipeline,
  promiseId: S.Literal(KHALA_CODE_DESKTOP_TRACE_CAPTURE_PROMISE_ID),
  reason: RpcKhalaCodeTraceCaptureReason,
  schemaVersion: S.Literal("openagents.khala_code.desktop_trace_capture_status.v1"),
  state: S.Literal("not_captured"),
} as const
const RpcKhalaCodeTraceCaptureStatusResult = S.Struct(RpcKhalaCodeTraceCaptureStatusFields)
const RpcKhalaCodeTraceCaptureConsentWriteRequest = S.Struct({
  enabled: S.Boolean,
})
const RpcKhalaCodeTraceCaptureConsentWriteResult = S.Struct({
  ...RpcKhalaCodeTraceCaptureStatusFields,
  saved: S.Literal(true),
})
const RpcKhalaCodeOutsideUserRunHarnessReadiness = S.Struct({
  codexCli: S.Literals(["ready", "missing", "unknown"]),
  codexAuth: S.Literals(["ready", "credentials_missing", "invalid", "error", "unknown"]),
  pylon: S.Literals(["ready", "unavailable", "not_configured", "unknown"]),
})
const RpcKhalaCodeOutsideUserRunStaleness = S.Struct({
  composition: S.Literal("live_at_read"),
  contractVersion: S.String,
  maxStalenessSeconds: S.Number,
  rebuildsOn: RpcStringArray,
})
const RpcKhalaCodeOutsideUserRunReceipt = S.Struct({
  schemaVersion: S.Literal("openagents.khala_code.outside_user_run_receipt.v1"),
  product: S.Literal("khala-code"),
  promiseId: S.Literal("khala_code.desktop_codex_wrapper.v1"),
  receiptRef: S.String,
  receiptUrl: S.String,
  generatedAt: S.String,
  submittedAt: S.String,
  appVersion: S.String,
  platform: S.Literals(["darwin", "linux", "win32", "other"]),
  arch: S.Literals(["arm64", "x64", "other"]),
  distributionChannel: S.Literals(["desktop_dmg", "npm_cli", "source_build", "unknown"]),
  harnessReadiness: RpcKhalaCodeOutsideUserRunHarnessReadiness,
  publicSafety: S.Struct({
    userActionRequired: S.Literal(true),
    noPhoneHome: S.Literal(true),
    noPaths: S.Literal(true),
    noPrompts: S.Literal(true),
    noTokens: S.Literal(true),
    noLogs: S.Literal(true),
  }),
  evidenceRefs: RpcStringArray,
  caveatRefs: RpcStringArray,
  sourceRefs: RpcStringArray,
  staleness: RpcKhalaCodeOutsideUserRunStaleness,
})
const RpcKhalaCodeOutsideUserRunReportRequest = S.Struct({
  idempotencyKey: S.optional(S.String),
})
const RpcKhalaCodeOutsideUserRunReportResult = S.Union([
  S.Struct({
    ok: S.Literal(true),
    idempotent: S.Boolean,
    generatedAt: S.String,
    staleness: RpcKhalaCodeOutsideUserRunStaleness,
    receipt: RpcKhalaCodeOutsideUserRunReceipt,
  }),
  S.Struct({
    ok: S.Literal(false),
    error: S.Literal("outside_user_run_receipt_unavailable"),
  }),
])

// Exported for the bun-side RPC handlers so wire payloads are schema-validated
// before they are surfaced as typed plan results.
export const KhalaCodeDesktopPlanCatalogSchema = RpcKhalaCodePlanCatalog
export const KhalaCodeDesktopPlanStatusPlanSchema = RpcKhalaCodePlanStatusPlan
export const KhalaCodeDesktopPlanPurchaseSuccessSchema = RpcKhalaCodePlanPurchaseSuccess
export const KhalaCodeDesktopPlanPurchaseResultSchema = RpcKhalaCodePlanPurchaseResult
export const KhalaCodeDesktopTraceCaptureStatusResultSchema =
  RpcKhalaCodeTraceCaptureStatusResult
export const KhalaCodeDesktopTraceCaptureConsentWriteResultSchema =
  RpcKhalaCodeTraceCaptureConsentWriteResult
export const KhalaCodeDesktopOutsideUserRunReportResultSchema =
  RpcKhalaCodeOutsideUserRunReportResult

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

type KhalaCodeDesktopRpcMethodSchemaSpec = {
  readonly parameters: readonly {
    readonly optional: boolean
    readonly schema: S.Schema<unknown>
  }[]
  readonly result: S.Schema<unknown>
}

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
  fleetRunControl: { parameters: [param(RpcFleetRunControlRequest)], result: RpcFleetRunControlResult },
  fleetRunList: { parameters: [optionalParam(RpcFleetRunListRequest)], result: RpcFleetRunListResult },
  fleetRunStart: { parameters: [param(RpcFleetRunStartRequest)], result: RpcFleetRunStartResult },
  fleetRunStatus: { parameters: [param(RpcFleetRunStatusRequest)], result: RpcFleetRunStatusResult },
  architectPlanRun: { parameters: [param(RpcArchitectPlanRunRequest)], result: RpcArchitectPlanRunResult },
  architectPlanDecision: { parameters: [param(RpcArchitectPlanDecisionRequest)], result: RpcArchitectPlanDecisionResult },
  fleetWorkerControl: { parameters: [param(RpcFleetWorkerControlRequest)], result: RpcFleetWorkerControlResult },
  khalaSyncFleetState: { parameters: [param(RpcKhalaSyncFleetStateRequest)], result: RpcKhalaSyncFleetStateResult },
  khalaSyncFleetMutate: { parameters: [param(RpcKhalaSyncFleetMutateRequest)], result: RpcKhalaSyncFleetMutateResult },
  khalaSyncFleetReportAccountState: { parameters: [param(RpcKhalaSyncFleetReportAccountStateRequest)], result: RpcKhalaSyncFleetReportAccountStateResult },
  khalaSyncChatThreads: { parameters: [optionalParam(RpcKhalaSyncChatThreadsRequest)], result: RpcKhalaSyncChatThreadsResult },
  khalaSyncChatMessages: { parameters: [param(RpcKhalaSyncChatMessagesRequest)], result: RpcKhalaSyncChatMessagesResult },
  khalaSyncChatCreateThread: { parameters: [param(RpcKhalaSyncChatCreateThreadRequest)], result: RpcKhalaSyncChatMutationResult },
  khalaSyncChatAppendMessage: { parameters: [param(RpcKhalaSyncChatAppendMessageRequest)], result: RpcKhalaSyncChatMutationResult },
  khalaSyncChatRenameThread: { parameters: [param(RpcKhalaSyncChatRenameThreadRequest)], result: RpcKhalaSyncChatMutationResult },
  editorProviderList: { parameters: noParams(), result: KhalaCodeEditorProviderListResult },
  editorWorkspaceRead: { parameters: noParams(), result: KhalaCodeEditorWorkspaceReadResult },
  editorDirectoryRead: { parameters: [optionalParam(KhalaCodeEditorDirectoryReadRequest)], result: KhalaCodeEditorDirectoryReadResult },
  editorFileRead: { parameters: [param(KhalaCodeEditorFileReadRequest)], result: KhalaCodeEditorFileReadResult },
  forumRequest: { parameters: [param(RpcForumRequest)], result: RpcForumResponse },
  khalaCodePlanCatalog: { parameters: noParams(), result: RpcKhalaCodePlanCatalogResult },
  khalaCodePlanStatus: { parameters: noParams(), result: RpcKhalaCodePlanStatusResult },
  khalaCodeOpenAgentsAuthStatus: { parameters: noParams(), result: RpcKhalaCodeOpenAgentsAuthStatusResult },
  khalaCodeOpenAgentsAuthStart: { parameters: noParams(), result: RpcKhalaCodeOpenAgentsAuthStartResult },
  khalaCodeOpenAgentsAuthPoll: { parameters: noParams(), result: RpcKhalaCodeOpenAgentsAuthPollResult },
  khalaCodePlanPurchase: { parameters: [optionalParam(RpcKhalaCodePlanPurchaseRequest)], result: RpcKhalaCodePlanPurchaseResult },
  khalaCodeTraceCaptureStatus: { parameters: noParams(), result: RpcKhalaCodeTraceCaptureStatusResult },
  khalaCodeTraceCaptureConsentWrite: { parameters: [param(RpcKhalaCodeTraceCaptureConsentWriteRequest)], result: RpcKhalaCodeTraceCaptureConsentWriteResult },
  khalaCodeOutsideUserRunReport: { parameters: [optionalParam(RpcKhalaCodeOutsideUserRunReportRequest)], result: RpcKhalaCodeOutsideUserRunReportResult },
  claudeApprovalPending: { parameters: noParams(), result: RpcClaudeApprovalPendingResult },
  claudeApprovalRespond: { parameters: [param(RpcClaudeApprovalRespondRequest)], result: RpcClaudeApprovalRespondResult },
  claudeSettingsRead: { parameters: noParams(), result: RpcClaudeSettingsProjection },
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
  codexModelRolePresetApply: { parameters: [param(RpcCodexModelRolePresetApplyRequest)], result: RpcCodexModelRolePresetApplyResult },
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
  sessionCatalog: { parameters: [optionalParam(RpcSessionCatalogRequest)], result: RpcSessionCatalogResult },
  codexTurnInterrupt: { parameters: [param(RpcTurnInterruptRequest)], result: RpcTurnActionResult },
  codexTurnStart: { parameters: [param(RpcTurnStartRequest)], result: RpcChatTurnResponse },
  codexTurnSteer: { parameters: [param(RpcTurnSteerRequest)], result: RpcTurnActionResult },
  connectCodexAccount: { parameters: [param(S.String)], result: RpcConnectStart },
  harnessSettingRead: { parameters: noParams(), result: RpcHarnessSetting },
  harnessSettingWrite: { parameters: [param(RpcHarnessSettingWriteRequest)], result: RpcHarnessSettingWriteResult },
  modelRoleRegistryRead: { parameters: noParams(), result: RpcModelRoleRegistryReadResult },
  modelRoleRegistryWrite: { parameters: [param(RpcModelRoleRegistryWriteRequest)], result: RpcModelRoleRegistryWriteResult },
  openExternalUrl: { parameters: [param(S.String)], result: S.Boolean },
  removeCodexAccount: { parameters: [param(S.String)], result: RpcRemoveAccountResult },
  setCodexAccountPaused: { parameters: [param(S.Struct({ accountRef: S.String, paused: S.Boolean }))], result: RpcRemoveAccountResult },
  codingStatus: { parameters: noParams(), result: RpcRuntimeStatus },
  consumeCodexRateLimitResetCredit: { parameters: [param(RpcRateLimitResetConsumeRequest)], result: RpcRateLimitResetResult },
  onDeviceDeciderStatus: { parameters: noParams(), result: RpcOnDeviceDeciderSelection },
  pylonStatus: { parameters: noParams(), result: RpcRuntimeStatus },
  qaMetricSample: { parameters: [param(RpcQaMetricSample)], result: RpcQaMetricSampleResult },
  qaMetrics: { parameters: noParams(), result: RpcQaMetricsSnapshot },
  slashCommandDispatch: { parameters: [param(RpcSlashCommandDispatchRequest)], result: RpcSlashCommandDispatchResult },
  slashCommandList: { parameters: [optionalParam(RpcSlashCommandListRequest)], result: RpcSlashCommandListResponse },
  submitChatMessage: { parameters: [param(RpcChatTurnRequest)], result: RpcChatTurnResponse },
  tokenAccountingStatus: { parameters: noParams(), result: RpcRuntimeStatus },
  threadTokenSummary: { parameters: [optionalParam(RpcThreadTokenSummaryRequest)], result: RpcThreadTokenSummary },
  toolCatalog: { parameters: noParams(), result: RpcToolCatalogResponse },
} as const satisfies Record<
  keyof KhalaCodeDesktopRPCSchema["requests"],
  KhalaCodeDesktopRpcMethodSchemaSpec
>

export type KhalaCodeDesktopRpcMethodName =
  keyof typeof KhalaCodeDesktopRpcMethodSchemas

export const KhalaCodeDesktopRpcMethodNames =
  Object.keys(KhalaCodeDesktopRpcMethodSchemas) as KhalaCodeDesktopRpcMethodName[]

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
  S.decodeUnknownSync(schema as never, { onExcessProperty: "error" })(value)

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
    fleetRunControl(request: KhalaCodeDesktopFleetRunControlRequest): Promise<KhalaCodeDesktopFleetRunControlResult>
    fleetRunList(request?: KhalaCodeDesktopFleetRunListRequest): Promise<KhalaCodeDesktopFleetRunListResult>
    fleetRunStart(request: KhalaCodeDesktopFleetRunStartRequest): Promise<KhalaCodeDesktopFleetRunStartResult>
    fleetRunStatus(request: KhalaCodeDesktopFleetRunStatusRequest): Promise<KhalaCodeDesktopFleetRunStatusResult>
    architectPlanRun(request: KhalaCodeDesktopArchitectPlanRunRequest): Promise<KhalaCodeDesktopArchitectPlanRunResult>
    architectPlanDecision(request: KhalaCodeDesktopArchitectPlanDecisionRequest): Promise<KhalaCodeDesktopArchitectPlanDecisionResult>
    fleetWorkerControl(request: KhalaCodeDesktopFleetWorkerControlRequest): Promise<KhalaCodeDesktopFleetWorkerControlResult>
    khalaSyncFleetState(request: KhalaCodeDesktopKhalaSyncFleetStateRequest): Promise<KhalaCodeDesktopKhalaSyncFleetStateResult>
    khalaSyncFleetMutate(request: KhalaCodeDesktopKhalaSyncFleetMutateRequest): Promise<KhalaCodeDesktopKhalaSyncFleetMutateResult>
    khalaSyncFleetReportAccountState(request: KhalaCodeDesktopKhalaSyncFleetReportAccountStateRequest): Promise<KhalaCodeDesktopKhalaSyncFleetReportAccountStateResult>
    khalaSyncChatThreads(request?: KhalaCodeDesktopKhalaSyncChatThreadsRequest): Promise<KhalaCodeDesktopKhalaSyncChatThreadsResult>
    khalaSyncChatMessages(request: KhalaCodeDesktopKhalaSyncChatMessagesRequest): Promise<KhalaCodeDesktopKhalaSyncChatMessagesResult>
    khalaSyncChatCreateThread(request: KhalaCodeDesktopKhalaSyncChatCreateThreadRequest): Promise<KhalaCodeDesktopKhalaSyncChatMutationResult>
    khalaSyncChatAppendMessage(request: KhalaCodeDesktopKhalaSyncChatAppendMessageRequest): Promise<KhalaCodeDesktopKhalaSyncChatMutationResult>
    khalaSyncChatRenameThread(request: KhalaCodeDesktopKhalaSyncChatRenameThreadRequest): Promise<KhalaCodeDesktopKhalaSyncChatMutationResult>
    editorProviderList(): Promise<KhalaCodeDesktopEditorProviderListResult>
    editorWorkspaceRead(): Promise<KhalaCodeDesktopEditorWorkspaceReadResult>
    editorDirectoryRead(request?: KhalaCodeDesktopEditorDirectoryReadRequest): Promise<KhalaCodeDesktopEditorDirectoryReadResult>
    editorFileRead(request: KhalaCodeDesktopEditorFileReadRequest): Promise<KhalaCodeDesktopEditorFileReadResult>
    forumRequest(request: KhalaCodeDesktopForumRequest): Promise<KhalaCodeDesktopForumResponse>
    khalaCodePlanCatalog(): Promise<KhalaCodeDesktopPlanCatalogResult>
    khalaCodePlanStatus(): Promise<KhalaCodeDesktopPlanStatusResult>
    khalaCodeOpenAgentsAuthStatus(): Promise<KhalaCodeDesktopOpenAgentsAuthStatusResult>
    khalaCodeOpenAgentsAuthStart(): Promise<KhalaCodeDesktopOpenAgentsAuthStartResult>
    khalaCodeOpenAgentsAuthPoll(): Promise<KhalaCodeDesktopOpenAgentsAuthPollResult>
    khalaCodePlanPurchase(request?: KhalaCodeDesktopPlanPurchaseRequest): Promise<KhalaCodeDesktopPlanPurchaseResult>
    khalaCodeTraceCaptureStatus(): Promise<KhalaCodeDesktopTraceCaptureStatusResult>
    khalaCodeTraceCaptureConsentWrite(request: KhalaCodeDesktopTraceCaptureConsentWriteRequest): Promise<KhalaCodeDesktopTraceCaptureConsentWriteResult>
    khalaCodeOutsideUserRunReport(request?: KhalaCodeDesktopOutsideUserRunReportRequest): Promise<KhalaCodeDesktopOutsideUserRunReportResult>
    claudeApprovalPending(): Promise<KhalaCodeDesktopClaudeApprovalPendingResult>
    claudeApprovalRespond(request: KhalaCodeDesktopClaudeApprovalRespondRequest): Promise<KhalaCodeDesktopClaudeApprovalRespondResult>
    claudeSettingsRead(): Promise<KhalaCodeDesktopClaudeSettingsReadResult>
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
    codexModelRolePresetApply(request: typeof RpcCodexModelRolePresetApplyRequest.Type): Promise<typeof RpcCodexModelRolePresetApplyResult.Type>
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
    sessionCatalog(request?: KhalaCodeDesktopSessionCatalogRequest): Promise<KhalaCodeDesktopSessionCatalogResult>
    codexTurnInterrupt(request: KhalaCodeDesktopCodexTurnInterruptRequest): Promise<KhalaCodeDesktopCodexTurnActionResult>
    codexTurnStart(request: KhalaCodeDesktopCodexTurnStartRequest): Promise<KhalaCodeDesktopChatTurnResponse>
    codexTurnSteer(request: KhalaCodeDesktopCodexTurnSteerRequest): Promise<KhalaCodeDesktopCodexTurnActionResult>
    connectCodexAccount(accountRef: string): Promise<KhalaCodeDesktopConnectStart>
    harnessSettingRead(): Promise<typeof RpcHarnessSetting.Type>
    harnessSettingWrite(request: typeof RpcHarnessSettingWriteRequest.Type): Promise<typeof RpcHarnessSettingWriteResult.Type>
    modelRoleRegistryRead(): Promise<KhalaCodeDesktopModelRoleRegistryReadResult>
    modelRoleRegistryWrite(request: KhalaCodeDesktopModelRoleRegistryWriteRequest): Promise<KhalaCodeDesktopModelRoleRegistryWriteResult>
    openExternalUrl(url: string): Promise<boolean>
    removeCodexAccount(accountRef: string): Promise<KhalaCodeDesktopRemoveAccountResult>
    setCodexAccountPaused(request: { accountRef: string; paused: boolean }): Promise<KhalaCodeDesktopRemoveAccountResult>
    codingStatus(): Promise<KhalaCodeDesktopRuntimeStatus>
    consumeCodexRateLimitResetCredit(request: { accountRef: string }): Promise<KhalaCodeDesktopCodexRateLimitResetResult>
    onDeviceDeciderStatus(): Promise<OnDeviceDeciderSelection>
    pylonStatus(): Promise<KhalaCodeDesktopRuntimeStatus>
    qaMetricSample(sample: KhalaCodeDesktopQaMetricSample): Promise<KhalaCodeDesktopQaMetricSampleResult>
    qaMetrics(): Promise<KhalaCodeDesktopQaMetricsSnapshot>
    slashCommandDispatch(request: KhalaCodeDesktopSlashCommandDispatchRequest): Promise<KhalaCodeDesktopSlashCommandDispatchResult>
    slashCommandList(request?: KhalaCodeDesktopSlashCommandListRequest): Promise<KhalaCodeDesktopSlashCommandListResponse>
    submitChatMessage(request: KhalaCodeDesktopChatTurnRequest): Promise<KhalaCodeDesktopChatTurnResponse>
    tokenAccountingStatus(): Promise<KhalaCodeDesktopRuntimeStatus>
    threadTokenSummary(request?: KhalaCodeDesktopThreadTokenSummaryRequest): Promise<KhalaCodeDesktopThreadTokenSummary>
    toolCatalog(): Promise<KhalaCodeDesktopToolCatalogResponse>
  }
  messages: {
    chatTurnEvent(event: KhalaCodeDesktopChatTurnEvent): void
    fleetLifecycleEvent(event: KhalaCodeDesktopFleetLifecycleEvent): void
    /**
     * KS-6.9 (#8419): pushed the instant a Claude Agent SDK tool call needs
     * approval, so the desktop UI can react immediately instead of waiting
     * on the 1s `claudeApprovalPending` poll tick. The poll remains as a
     * fallback safety net; this message only shortens the common-case
     * detection latency.
     */
    claudeApprovalRequested(request: KhalaCodeDesktopClaudeApprovalRequestProjection): void
  }
}
