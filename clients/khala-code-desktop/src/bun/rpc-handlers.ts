import { randomUUID } from "node:crypto"
import { Buffer } from "node:buffer"
import { mkdir, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { basename, join } from "node:path"
import { Effect, Schema as S } from "effect"

import type { KhalaFleetDelegateStep } from "@openagentsinc/khala-tools"
import type {
  KhalaCodexRateLimitProviderStatus,
  KhalaCodexRateLimitResetOutcome,
} from "../shared/codex-rate-limits.js"
import type { CodexAppServerHost } from "./codex-app-server-client.js"
import type { CodexAppServerNotification } from "./codex-app-server-client.js"
import {
  createCodexAppServerChatRuntime,
  type CodexAppServerChatRuntime,
} from "./codex-app-server-chat-runtime.js"
import {
  createKhalaCodeDesktopCodexMessageTokenAuditRecorder,
  createKhalaCodeDesktopCodexTokenUsageReporter,
  khalaCodeDesktopTokenUsageTelemetryStatus,
  readKhalaCodeDesktopTokenUsageInboxFlags,
  readKhalaCodeDesktopThreadTokenSummary,
} from "./codex-token-usage-telemetry.js"
import {
  ensureCodexFleetMcpBridge,
  type CodexFleetMcpBridgeEnsureResult,
} from "./codex-fleet-mcp-bridge.js"
import type { KhalaAppleFmReadiness } from "../shared/apple-fm-readiness.js"
import type { OnDeviceDeciderSelection } from "../shared/on-device-decider.js"
import {
  type KhalaCodeDesktopAppInfo,
  type KhalaCodeDesktopChatTurnAttachment,
  type KhalaCodeDesktopArchitectPlanArtifact,
  type KhalaCodeDesktopArchitectPlanDecisionResult,
  type KhalaCodeDesktopArchitectPlanRunResult,
  type KhalaCodeDesktopCodexAppServerActionResult,
  type KhalaCodeDesktopChatTurnEvent,
  type KhalaCodeDesktopChatTurnRequest,
  type KhalaCodeDesktopChatTurnResponse,
  type KhalaCodeDesktopCodexConfigValueWriteRequest,
  type KhalaCodeDesktopCodexBackgroundTerminalsCleanRequest,
  type KhalaCodeDesktopCodexBackgroundTerminalsListRequest,
  type KhalaCodeDesktopCodexBackgroundTerminalsTerminateRequest,
  type KhalaCodeDesktopCodexEcosystemReadRequest,
  type KhalaCodeDesktopCodexEcosystemReadResult,
  type KhalaCodeDesktopCodexMentionCandidate,
  type KhalaCodeDesktopCodexMentionCandidatesRequest,
  type KhalaCodeDesktopCodexMentionCandidatesResult,
  type KhalaCodeDesktopCodexSettingsReadRequest,
  type KhalaCodeDesktopCodexSettingsReadResult,
  type KhalaCodeDesktopSlashCommandDispatchRequest,
  type KhalaCodeDesktopSlashCommandDispatchResult,
  type KhalaCodeDesktopCodexAccountsStatus,
  type KhalaCodeDesktopCodexHarnessStatus,
  type KhalaCodeDesktopCodexRateLimitResetResult,
  type KhalaCodeDesktopFleetAccount,
  type KhalaCodeDesktopFleetAssignment,
  type KhalaCodeDesktopFleetDelegateRunRequest,
  type KhalaCodeDesktopFleetDelegateRunResult,
  type KhalaCodeDesktopFleetDelegateRunStep,
  type KhalaCodeDesktopFleetHomeRole,
  type KhalaCodeDesktopFleetPromotionRequest,
  type KhalaCodeDesktopFleetPromotionResult,
  type KhalaCodeDesktopFleetQueuePolicy,
  type KhalaCodeDesktopFleetRunControlRequest,
  type KhalaCodeDesktopFleetRunControlResult,
  type KhalaCodeDesktopFleetRunListRequest,
  type KhalaCodeDesktopFleetRunListResult,
  type KhalaCodeDesktopFleetRunProjection,
  type KhalaCodeDesktopFleetRunStartRequest,
  type KhalaCodeDesktopFleetRunStartResult,
  type KhalaCodeDesktopFleetRunState,
  type KhalaCodeDesktopFleetRunStatusRequest,
  type KhalaCodeDesktopFleetRunStatusResult,
  type KhalaCodeDesktopFleetSessionRole,
  type KhalaCodeDesktopFleetWorkerSession,
  type KhalaCodeDesktopFleetWorkerControlRequest,
  type KhalaCodeDesktopFleetWorkerControlResult,
  type KhalaCodeDesktopFleetStatus,
  type KhalaCodeDesktopForumRequest,
  type KhalaCodeDesktopForumResponse,
  type KhalaCodeDesktopModelRoleRegistryReadResult,
  type KhalaCodeDesktopModelRoleRegistryWriteRequest,
  type KhalaCodeDesktopModelRoleRegistryWriteResult,
  type KhalaCodeDesktopPlanCatalogResult,
  type KhalaCodeDesktopOutsideUserRunReportRequest,
  type KhalaCodeDesktopOutsideUserRunReportResult,
  type KhalaCodeDesktopPlanPurchaseRequest,
  type KhalaCodeDesktopPlanPurchaseResult,
  type KhalaCodeDesktopPlanStatusResult,
  KhalaCodeDesktopOutsideUserRunReportResultSchema,
  KhalaCodeDesktopPlanCatalogSchema,
  KhalaCodeDesktopPlanPurchaseSuccessSchema,
  KhalaCodeDesktopPlanStatusPlanSchema,
  type KhalaCodeDesktopQaMetricSample,
  type KhalaCodeDesktopQaMetricSampleResult,
  type KhalaCodeDesktopQaMetricsSnapshot,
  type KhalaCodeDesktopRPCSchema,
  type KhalaCodeDesktopRuntimeStatus,
  type KhalaCodeDesktopThreadTokenSummaryRequest,
} from "../shared/rpc.js"
import {
  emptyKhalaCodeQaMetricsSnapshot,
  khalaCodeQaMetricUnitFor,
} from "../shared/qa-metrics.js"
import {
  khalaCodeDesktopCodexApprovalResponsePayload,
  type KhalaCodeDesktopCodexApprovalResponseInput,
  type KhalaCodeDesktopJsonRpcId,
} from "../shared/codex-approval-decisions.js"
import { projectKhalaCodeDesktopCodexSettings } from "../shared/codex-settings.js"
import { projectKhalaCodeDesktopCodexEcosystem } from "../shared/codex-ecosystem.js"
import {
  KHALA_CODE_ARCHITECT_CODER_JUDGE_PRESET_ID,
  KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH,
  makeKhalaCodeArchitectCoderJudgeRegistry,
} from "../shared/model-role-preset.js"
import {
  evaluateKhalaCodeDesktopSlashCommandAvailability,
  khalaCodeDesktopSlashCommandsWithAvailability,
  parseKhalaCodeDesktopSlashCommand,
} from "../shared/codex-slash-commands.js"
import { inspectCodexHarnessStatus } from "./codex-harness-status.js"
import {
  createClaudeAppSdkChatRuntime,
  type ClaudeAppSdkChatRuntime,
} from "./claude-app-sdk-chat-runtime.js"
import { createArchitectPlanStore } from "./architect-plan-store.js"
import {
  claudePlanFanoutDagToWorkSource,
  claudePlanFanoutPlanModeInstructions,
  decodeClaudePlanFanoutDag,
  type ClaudePlanFanoutDag,
} from "./claude-plan-fanout.js"
import { readKhalaCodeDesktopSessionCatalog } from "./session-catalog.js"
import { inspectClaudeHarnessStatus } from "./claude-harness-status.js"
import {
  createClaudeApprovalService,
  type ClaudeApprovalService,
} from "./claude-approvals.js"
import {
  createKhalaCodeDesktopClaudeTokenUsageReporter,
  readKhalaCodeDesktopClaudeTokenUsageInboxFlags,
} from "./claude-token-usage-telemetry.js"
import {
  khalaCodeDesktopRuntimeEnvOverride,
  hasKhalaCodeDesktopPersistedModelRoleRegistry,
  readKhalaCodeDesktopHarnessSetting,
  readKhalaCodeDesktopModelRoleRegistry,
  readKhalaCodeDesktopPersistedHarnessMode,
  resolveKhalaCodeDesktopModelRole,
  writeKhalaCodeDesktopHarnessSetting,
  writeKhalaCodeDesktopModelRoleEntry,
  writeKhalaCodeDesktopModelRoleRegistry,
} from "./harness-setting.js"
import type {
  KhalaCodeModelRole,
  KhalaCodeModelRoleEntry,
} from "../shared/model-roles.js"
import {
  consumeKhalaCodexRateLimitResetCredit,
  fetchKhalaCodexRateLimitStatus,
} from "./codex-rate-limits.js"
import {
  khalaCodeDesktopToolCatalog,
  runKhalaCodeDesktopChatTurn,
} from "./khala-chat-runtime.js"
import {
  beginCodexConnect,
  collectCodexAccountEmails,
  ensureLocalPylon,
  inspectCodexFleet,
  spawnCodexInstances,
  type KhalaCodexFleetToolOptions,
  openExternalUrl,
  removeCodexAccount,
  setCodexAccountPaused,
} from "./khala-fleet-tools.js"

type ChatEnv = Readonly<Record<string, string | undefined>>
type MaybePromise<T> = T | Promise<T>
type ChatRuntime = CodexAppServerChatRuntime

type ChatRuntimeSelection =
  | {
    readonly kind: "claude"
    readonly modelRole?: KhalaCodeModelRoleEntry
    readonly runtime: ClaudeAppSdkChatRuntime
  }
  | {
    readonly kind: "codex"
    readonly modelRole?: KhalaCodeModelRoleEntry
    readonly runtime: ChatRuntime
  }
  | {
    readonly kind: "legacy"
  }

const legacyThreadLifecycleUnsupportedMessage =
  "Legacy Khala native runtime does not support thread lifecycle RPCs."
const OPENAGENTS_FORUM_BASE_URL = "https://openagents.com"

export type KhalaCodeDesktopFleetRunSupervisorRpc = {
  readonly control: (
    request: KhalaCodeDesktopFleetRunControlRequest,
  ) => MaybePromise<{
    readonly previousState: KhalaCodeDesktopFleetRunState
    readonly run: KhalaCodeDesktopFleetRunProjection
    readonly supervisorActive: boolean
  }>
  readonly list: (
    request?: KhalaCodeDesktopFleetRunListRequest,
  ) => MaybePromise<readonly KhalaCodeDesktopFleetRunProjection[]>
  readonly start: (
    request: KhalaCodeDesktopFleetRunStartRequest,
  ) => MaybePromise<{
    readonly run: KhalaCodeDesktopFleetRunProjection
    readonly supervisorStarted: boolean
  }>
  readonly status: (
    request: KhalaCodeDesktopFleetRunStatusRequest,
  ) => MaybePromise<{
    readonly run: KhalaCodeDesktopFleetRunProjection | null
    readonly supervisorActive: boolean
  }>
  readonly workerControl?: (
    request: KhalaCodeDesktopFleetWorkerControlRequest,
  ) => MaybePromise<KhalaCodeDesktopFleetWorkerControlResult>
}

export type KhalaCodeDesktopRpcHandlersInput = {
  readonly appleFmReadiness: () => MaybePromise<KhalaAppleFmReadiness>
  readonly codexAppServerHost?: CodexAppServerHost
  readonly codexChatRuntime?: CodexAppServerChatRuntime
  readonly claudeChatRuntime?: ClaudeAppSdkChatRuntime
  readonly claudeApprovalService?: ClaudeApprovalService
  readonly enableFleetMcpBridge?: boolean
  readonly codexRateLimitStatus?: () => MaybePromise<KhalaCodexRateLimitProviderStatus>
  readonly codexHarnessStatus?: () => MaybePromise<KhalaCodeDesktopCodexHarnessStatus>
  readonly consumeCodexRateLimitResetCredit?: (input: {
    readonly accountRef: string
    readonly codexHomePath: string | null
    readonly idempotencyKey: string
  }) => MaybePromise<KhalaCodexRateLimitResetOutcome>
  readonly codexFleetToolOptions?: KhalaCodexFleetToolOptions
  readonly fleetRunSupervisor?: KhalaCodeDesktopFleetRunSupervisorRpc
  readonly fleetMcpBridgeRepoRoot?: string
  readonly env: ChatEnv
  // Test seam for network-backed handlers (Khala Code plan routes). Defaults to
  // the global fetch in production.
  readonly fetch?: typeof fetch
  readonly emitChatTurnEvent?: (event: KhalaCodeDesktopChatTurnEvent) => void
  readonly legacyChatTurn?: typeof runKhalaCodeDesktopChatTurn
  readonly onDeviceDeciderStatus: () => MaybePromise<OnDeviceDeciderSelection>
  readonly recordQaMetricSample?: (sample: KhalaCodeDesktopQaMetricSample) => MaybePromise<void>
  readonly qaMetrics?: () => MaybePromise<KhalaCodeDesktopQaMetricsSnapshot>
  readonly workingDirectory: string
}

const appInfo = (): KhalaCodeDesktopAppInfo => ({
  ok: true,
  app: "Khala Code Desktop",
  observedAt: new Date().toISOString(),
})

const recordQaTimerSample = async (
  input: KhalaCodeDesktopRpcHandlersInput,
  metric: KhalaCodeDesktopQaMetricSample["metric"],
  startedAt: number,
  context?: KhalaCodeDesktopQaMetricSample["context"],
): Promise<void> => {
  if (input.recordQaMetricSample === undefined) return
  const value = performance.now() - startedAt
  if (!Number.isFinite(value)) return
  try {
    await input.recordQaMetricSample({
      ...(context === undefined ? {} : { context }),
      metric,
      observedAt: new Date().toISOString(),
      unit: khalaCodeQaMetricUnitFor(metric),
      value,
    })
  } catch {
    // QA telemetry must not make the user-facing RPC fail.
  }
}

const MAX_MENTION_CANDIDATES = 20
const MAX_DIRECTORY_CANDIDATES = 20
const MAX_DIFF_DISPLAY_CHARS = 80_000
const CHAT_ATTACHMENT_TMP_ROOT = join(tmpdir(), "khala-code-chat-attachments")

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null

const stringValue = (value: unknown): string | null =>
  typeof value === "string" ? value : null

const numberValue = (value: unknown): number | undefined =>
  typeof value === "number" && Number.isFinite(value) ? value : undefined

const forumFailureReason = (payload: unknown, fallback: string): string => {
  if (isRecord(payload)) {
    const reason = stringValue(payload.reason) ?? stringValue(payload.error)
    if (reason !== null && reason.trim().length > 0) return reason
  }
  return fallback
}

const forumRequestUrl = (path: string): URL => {
  if (!path.startsWith("/api/forum")) {
    throw new Error("Forum RPC path must stay under /api/forum.")
  }
  const url = new URL(path, OPENAGENTS_FORUM_BASE_URL)
  if (url.origin !== OPENAGENTS_FORUM_BASE_URL || !url.pathname.startsWith("/api/forum")) {
    throw new Error("Forum RPC path must stay on openagents.com /api/forum.")
  }
  return url
}

const fetchOpenAgentsForum = async (
  request: KhalaCodeDesktopForumRequest,
): Promise<KhalaCodeDesktopForumResponse> => {
  const method = request.method ?? "GET"
  const response = await fetch(forumRequestUrl(request.path), {
    method,
    headers: {
      accept: "application/json",
      ...(request.body === undefined ? {} : { "content-type": "application/json" }),
      ...(request.headers ?? {}),
    },
    ...(request.body === undefined ? {} : { body: JSON.stringify(request.body) }),
  })
  const payload = await response.json().catch(() => ({})) as unknown
  return {
    ok: response.ok,
    payload: payload as never,
    status: response.status,
    ...(response.ok ? {} : { error: forumFailureReason(payload, `Forum request failed with ${response.status}`) }),
  }
}

// Khala Code public product surfaces. The desktop host only touches the exact
// allowlisted paths below, pinned to the resolved OpenAgents origin. The bearer
// token for plan routes never leaves this module: it is read from env, sent as
// an Authorization header, and never logged or echoed into RPC results.
const KHALA_CODE_PLAN_CATALOG_PATH = "/api/public/khala-code/plans"
const KHALA_CODE_PLAN_STATUS_PATH = "/v1/khala-code/plan"
const KHALA_CODE_PLAN_PURCHASE_PATH = "/v1/khala-code/plans/purchases"
const KHALA_CODE_OUTSIDE_USER_RUNS_PATH = "/api/public/khala-code/outside-user-runs"
const KHALA_CODE_PLAN_ALLOWED_PATHS: ReadonlySet<string> = new Set([
  KHALA_CODE_PLAN_CATALOG_PATH,
  KHALA_CODE_PLAN_STATUS_PATH,
  KHALA_CODE_PLAN_PURCHASE_PATH,
  KHALA_CODE_OUTSIDE_USER_RUNS_PATH,
])

const khalaCodePlanBaseUrl = (env: ChatEnv): string => {
  const configured =
    env.PYLON_OPENAGENTS_BASE_URL?.trim() || env.OPENAGENTS_BASE_URL?.trim()
  return configured !== undefined && configured.length > 0
    ? configured
    : OPENAGENTS_FORUM_BASE_URL
}

const khalaCodePlanRequestUrl = (baseUrl: string, path: string): URL => {
  if (!KHALA_CODE_PLAN_ALLOWED_PATHS.has(path)) {
    throw new Error("Khala Code plan RPC path is not allowlisted.")
  }
  // Append after the FULL configured base (the khala-chat-runtime join style)
  // so a reverse-proxied base with a path prefix keeps its prefix; `new
  // URL(path, base)` would silently drop it and 404 plan requests only.
  const base = new URL(baseUrl)
  const basePath = base.pathname.replace(/\/+$/, "")
  const url = new URL(`${base.origin}${basePath}${path}`)
  if (url.origin !== base.origin || !url.pathname.endsWith(path)) {
    throw new Error("Khala Code plan RPC path must stay on the resolved OpenAgents origin.")
  }
  return url
}

const khalaCodeAgentToken = (env: ChatEnv): string | null => {
  const token = env.OPENAGENTS_AGENT_TOKEN?.trim() || env.OPENAGENTS_API_KEY?.trim()
  return token !== undefined && token.length > 0 ? token : null
}

const KHALA_CODE_DESKTOP_APP_VERSION = "0.0.1"

const khalaCodeDesktopAppVersion = (env: ChatEnv): string => {
  const version = env.KHALA_CODE_DESKTOP_VERSION?.trim()
  return version !== undefined && version.length > 0
    ? version
    : KHALA_CODE_DESKTOP_APP_VERSION
}

const khalaCodeOutsideUserPlatform = (platform: string) =>
  platform === "darwin" || platform === "linux" || platform === "win32"
    ? platform
    : "other"

const khalaCodeOutsideUserArch = (arch: string) =>
  arch === "arm64" || arch === "x64" ? arch : "other"

const khalaCodeDistributionChannel = (env: ChatEnv) => {
  const channel = env.KHALA_CODE_DISTRIBUTION_CHANNEL?.trim()
  return channel === "desktop_dmg" ||
    channel === "npm_cli" ||
    channel === "source_build" ||
    channel === "unknown"
    ? channel
    : "source_build"
}

const codexCliRunEvidenceState = (
  harness: KhalaCodeDesktopCodexHarnessStatus,
) =>
  harness.binary.available
    ? "ready" as const
    : harness.status === "not_configured" || harness.status === "unavailable"
      ? "missing" as const
      : "unknown" as const

const codexAuthRunEvidenceState = (
  harness: KhalaCodeDesktopCodexHarnessStatus,
) => harness.auth.state

const pylonRunEvidenceState = (status: KhalaCodeDesktopRuntimeStatus) =>
  status.status === "ready"
    ? "ready" as const
    : status.status === "not_configured"
      ? "not_configured" as const
      : status.status === "unavailable" || status.status === "error"
        ? "unavailable" as const
        : "unknown" as const

const safePathSegment = (value: string, fallback: string): string => {
  const sanitized = value.replace(/[^A-Za-z0-9._-]+/g, "_").replace(/^_+|_+$/g, "")
  return sanitized.length > 0 ? sanitized : fallback
}

const imageExtensionForMime = (mime: string): string => {
  switch (mime.toLowerCase()) {
    case "image/jpeg":
    case "image/jpg":
      return ".jpg"
    case "image/png":
      return ".png"
    case "image/gif":
      return ".gif"
    case "image/webp":
      return ".webp"
    case "image/heic":
      return ".heic"
    default:
      return ".img"
  }
}

const safeAttachmentFilename = (
  attachment: KhalaCodeDesktopChatTurnAttachment,
): string => {
  const rawName = basename(attachment.name.replace(/\\/g, "/"))
  const fallback = `image${imageExtensionForMime(attachment.mime)}`
  const sanitized = safePathSegment(rawName, fallback)
  return sanitized.includes(".")
    ? sanitized
    : `${sanitized}${imageExtensionForMime(attachment.mime)}`
}

const materializeChatAttachment = async (
  attachment: KhalaCodeDesktopChatTurnAttachment,
  input: {
    readonly index: number
    readonly sessionId: string
    readonly turnId: string
  },
): Promise<{
  readonly attachment: KhalaCodeDesktopChatTurnAttachment
  readonly cleanupPath: string | null
}> => {
  if (attachment.path !== undefined || attachment.dataBase64 === undefined) {
    return { attachment, cleanupPath: null }
  }
  if (!attachment.mime.toLowerCase().startsWith("image/")) {
    return { attachment, cleanupPath: null }
  }
  const directory = join(
    CHAT_ATTACHMENT_TMP_ROOT,
    safePathSegment(input.sessionId, "session"),
    safePathSegment(input.turnId, "turn"),
  )
  await mkdir(directory, { recursive: true })
  const filename = `${input.index}-${safePathSegment(attachment.id, "image")}-${safeAttachmentFilename(attachment)}`
  const path = join(directory, filename)
  await writeFile(path, Buffer.from(attachment.dataBase64, "base64"))
  return {
    attachment: {
      id: attachment.id,
      kind: "image",
      mime: attachment.mime,
      name: attachment.name,
      path,
      sizeBytes: attachment.sizeBytes,
    },
    cleanupPath: directory,
  }
}

const materializeChatAttachments = async (
  request: KhalaCodeDesktopChatTurnRequest,
): Promise<{
  readonly cleanupPaths: readonly string[]
  readonly request: KhalaCodeDesktopChatTurnRequest
}> => {
  if (request.attachments === undefined || request.attachments.length === 0) {
    return { cleanupPaths: [], request }
  }
  const turnId = request.turnId ?? randomUUID()
  const materialized = await Promise.all(request.attachments.map((attachment, index) =>
    materializeChatAttachment(attachment, {
      index,
      sessionId: request.sessionId,
      turnId,
    })
  ))
  const cleanupPaths = [...new Set(materialized.flatMap(item =>
    item.cleanupPath === null ? [] : [item.cleanupPath]
  ))]
  return {
    cleanupPaths,
    request: {
      ...request,
      attachments: materialized.map(item => item.attachment),
      turnId,
    },
  }
}

const withMaterializedChatAttachments = <A>(
  request: KhalaCodeDesktopChatTurnRequest,
  use: (request: KhalaCodeDesktopChatTurnRequest) => Promise<A>,
): Promise<A> =>
  Effect.runPromise(Effect.scoped(Effect.flatMap(Effect.acquireRelease(
    Effect.promise(() => materializeChatAttachments(request)),
    materialized => Effect.promise(async () => {
      await Promise.all(materialized.cleanupPaths.map(path =>
        rm(path, { force: true, recursive: true })
      ))
    }),
  ), materialized => Effect.promise(() => use(materialized.request)))))

const requireNonEmpty = (kind: string, field: string, value: string): void => {
  if (value.trim().length === 0) throw new Error(`${kind} requires ${field}`)
}

const normalizeMentionCandidate = (value: unknown): KhalaCodeDesktopCodexMentionCandidate | null => {
  if (!isRecord(value)) return null
  const path = stringValue(value.path)
  const fileName = stringValue(value.file_name) ?? stringValue(value.fileName) ?? path
  if (path === null || fileName === null || path.length === 0 || fileName.length === 0) return null
  const matchType = stringValue(value.match_type) ?? stringValue(value.matchType)
  const candidate: KhalaCodeDesktopCodexMentionCandidate = {
    fileName,
    kind: matchType === "directory" ? "directory" : "file",
    path,
  }
  const root = stringValue(value.root)
  const score = numberValue(value.score)
  return {
    ...candidate,
    ...(root === null ? {} : { root }),
    ...(score === undefined ? {} : { score }),
  }
}

const normalizeDirectoryCandidate = (value: unknown): KhalaCodeDesktopCodexMentionCandidate | null => {
  if (!isRecord(value)) return null
  const fileName = stringValue(value.fileName)
  if (fileName === null || fileName.length === 0) return null
  return {
    fileName,
    kind: value.isDirectory === true ? "directory" : "file",
    path: fileName,
  }
}

const mentionMessage = (input: {
  readonly candidates: readonly KhalaCodeDesktopCodexMentionCandidate[]
  readonly source: "fs/readDirectory" | "fuzzyFileSearch"
  readonly truncated: boolean
}): string => {
  if (input.candidates.length === 0) {
    return `Codex ${input.source} returned no mention candidates.`
  }
  const lines = input.candidates.map(candidate =>
    `- ${candidate.kind === "directory" ? "dir" : "file"} ${candidate.path}`,
  )
  return [
    `Codex ${input.source} mention candidates${input.truncated ? " (truncated)" : ""}:`,
    ...lines,
  ].join("\n")
}

const diffMessage = (response: unknown): { readonly message: string, readonly response: unknown } => {
  const record = isRecord(response) ? response : {}
  const diff = stringValue(record.diff) ?? ""
  const sha = stringValue(record.sha)
  if (diff.length === 0) {
    return {
      message: "Codex gitDiffToRemote returned no diff.",
      response,
    }
  }
  const truncated = diff.length > MAX_DIFF_DISPLAY_CHARS
  const displayDiff = truncated ? diff.slice(0, MAX_DIFF_DISPLAY_CHARS) : diff
  return {
    message: [
      `Codex gitDiffToRemote${sha === null ? "" : ` at ${sha}`}${truncated ? " (truncated)" : ""}:`,
      "```diff",
      displayDiff,
      "```",
    ].join("\n"),
    response: {
      sha,
      diff: displayDiff,
      truncated,
      originalLength: diff.length,
    },
  }
}

const ideMessage = (response: unknown): string => {
  const config = isRecord(response) && isRecord(response.config) ? response.config : {}
  const ide = config.ide ?? config.ide_integration ?? config.ideIntegration
  if (ide === undefined || ide === null) {
    return "Codex app-server config has no IDE integration status; IDE controls are unsupported by this Codex build."
  }
  return [
    "Codex app-server IDE integration status:",
    "```json",
    JSON.stringify(ide, null, 2),
    "```",
  ].join("\n")
}

const runtimeStatus = (input: {
  readonly available: boolean
  readonly capability: KhalaCodeDesktopRuntimeStatus["capability"]
  readonly reason: string
  readonly status: KhalaCodeDesktopRuntimeStatus["status"]
}): KhalaCodeDesktopRuntimeStatus => ({
  ok: true,
  app: "Khala Code Desktop",
  available: input.available,
  capability: input.capability,
  observedAt: new Date().toISOString(),
  reason: input.reason,
  status: input.status,
})

const isDisplayOnlyDefaultAccountRef = (accountRef: string): boolean =>
  /^(?:\(default\)|default)$/iu.test(accountRef.trim())

const accountSessionRole = (
  accountRef: string,
): KhalaCodeDesktopFleetSessionRole =>
  isDisplayOnlyDefaultAccountRef(accountRef)
    ? "main_local_codex_session"
    : "swarm_worker_codex_session"

const accountHomeRole = (
  accountRef: string,
): KhalaCodeDesktopFleetHomeRole =>
  isDisplayOnlyDefaultAccountRef(accountRef)
    ? "main_user_codex_home_display_only"
    : "pylon_isolated_worker_codex_home"

const fleetQueuePolicy = (
  capacity: KhalaCodeDesktopFleetAccount["capacity"],
  readiness: string,
): KhalaCodeDesktopFleetQueuePolicy => {
  const value = readiness.toLowerCase()
  const cooldown =
    value.includes("cooldown") || value.includes("cooling")
      ? "cooling_down"
      : value === "ready" || value === "available"
        ? "ready"
        : value === "unknown"
          ? "unknown"
          : "none_reported"
  return {
    admission: "pylon_capacity_gate",
    cooldown,
    refill: "pylon_presence_heartbeat",
    queued: capacity?.queued ?? null,
  }
}

const sessionLayers = (): NonNullable<KhalaCodeDesktopFleetStatus["sessionLayers"]> => ({
	  main: {
	    homeRole: "main_user_codex_home_display_only",
	    label: "Primary user Codex session",
    mutationPolicy: "codex_app_server_owned",
    role: "main_local_codex_session",
    runtime: "codex_harness",
    transcriptSurface: "chat",
  },
  workers: {
    homeRole: "pylon_isolated_worker_codex_home",
    label: "Khala swarm worker Codex sessions",
    mutationPolicy: "pylon_isolated_home_only",
    role: "swarm_worker_codex_session",
    runtime: "codex_harness",
    transcriptSurface: "fleet",
  },
})

const workerSessionForAssignment = (
  marker: {
    readonly assignmentRef: string | null
    readonly blockerRefs: readonly string[]
    readonly closeoutStatus: string | null
    readonly tokenRate: KhalaCodeDesktopFleetAssignment["tokenRate"]
    readonly transcriptRef: string | null
  },
): KhalaCodeDesktopFleetWorkerSession => {
  const hasBlocker = marker.blockerRefs.length > 0
  const hasCloseout = marker.closeoutStatus !== null
  const approvalRequired = marker.blockerRefs.some(ref => /approval|permission/iu.test(ref))
  return {
    approvalState: approvalRequired
      ? "approval_required"
      : hasBlocker
        ? "blocked"
        : hasCloseout
          ? "ready_for_review"
          : "none",
    blockerRefs: marker.blockerRefs,
    closeoutStatus: marker.closeoutStatus,
    executionRuntime: "codex_harness",
    homeRole: "pylon_isolated_worker_codex_home",
    queuePolicy: {
      admission: "pylon_capacity_gate",
      cooldown: hasBlocker ? "unknown" : "ready",
      refill: "pylon_presence_heartbeat",
      queued: null,
    },
    reviewState: hasBlocker
      ? "blocked"
      : hasCloseout || marker.tokenRate.status === "exact"
        ? "ready_for_review"
        : "active",
    role: "swarm_worker_codex_session",
    transcriptRef: marker.transcriptRef ?? marker.assignmentRef,
  }
}

const renderFleetPromotionObjective = (
  request: KhalaCodeDesktopFleetPromotionRequest,
): string => {
  const allowedRefs = request.contextBoundary.allowedRefs.length === 0
    ? "none"
    : request.contextBoundary.allowedRefs.join(", ")
  return [
    "Khala swarm delegation from a main local Codex thread.",
    `Origin thread: ${request.threadId}`,
    `Context boundary: ${request.contextBoundary.mode}; transcript included: false; allowed refs: ${allowedRefs}.`,
    request.contextBoundary.summary === null
      ? null
      : `User summary: ${request.contextBoundary.summary}`,
    `Objective: ${request.objective.trim()}`,
  ].filter((line): line is string => line !== null).join("\n")
}

const promoteThreadResult = (
  request: KhalaCodeDesktopFleetPromotionRequest,
  spawn: Awaited<ReturnType<typeof spawnCodexInstances>>,
): KhalaCodeDesktopFleetPromotionResult => ({
  acceptedCount: spawn.acceptedCount,
  contextBoundary: request.contextBoundary,
  ok: spawn.acceptedCount === spawn.requestedCount,
  origin: {
    role: "main_local_codex_session",
    sessionId: request.sessionId,
    threadId: request.threadId,
  },
  pylonRef: spawn.pylonRef,
  requestedCount: spawn.requestedCount,
  results: spawn.results.map(slot => ({
    accountRef: slot.accountRef,
    assignmentRef: slot.assignmentRef,
    closeoutStatus: slot.closeoutStatus,
    status: slot.status,
    summary: slot.summary,
    tokensVerified: slot.tokensVerified,
    transcriptRef: slot.transcriptRef,
  })),
  workerRuntime: {
    assignmentTool: "codex_spawn",
    homeRole: "pylon_isolated_worker_codex_home",
    role: "swarm_worker_codex_session",
    runtime: "codex_harness",
  },
})

const missingDelegateRunPins = (
  request: KhalaCodeDesktopFleetDelegateRunRequest,
): readonly string[] => [
  request.repo?.trim() ? null : "repo",
  request.claimRef?.trim() ? null : "claimRef",
  request.commit?.trim() ? null : "commit",
  request.verify?.trim() ? null : "verify",
].filter((value): value is string => value !== null)

const sanitizeDelegateRunRef = (ref: string): string => {
  if (/^repo:/iu.test(ref)) return "repo:pinned"
  if (/^commit:/iu.test(ref)) return "commit:pinned"
  if (/\/Users\/|auth\.json|bearer|credential|provider[_-]?payload|raw[_-]?(prompt|trace)|sk-[a-z0-9]/iu.test(ref)) {
    return "ref:redacted"
  }
  return ref
}

const delegateRunStepSummary = (
  step: KhalaFleetDelegateStep,
  mode: KhalaCodeDesktopFleetDelegateRunRequest["mode"],
): string => {
  const status = step.status === "blocked"
    ? "blocked"
    : step.status === "recovered"
      ? "recovered"
      : "satisfied"
  switch (step.module) {
    case "ensure_pylon":
      return `Pylon online gate ${status}.`
    case "advertise_capacity":
      return `Codex capacity advertisement ${status}.`
    case "select_account":
      return `Worker account selection ${status}.`
    case "prepare_work":
      return mode === "fixture"
        ? `Fixture work preparation ${status}.`
        : `Repo-pinned work preparation ${status}.`
    case "dispatch":
      return `Codex spawn dispatch ${status}.`
    case "verify_closeout":
      return `Closeout verification ${status}.`
  }
  return `Delegate run step ${status}.`
}

const projectDelegateRunStep = (
  step: KhalaFleetDelegateStep,
  mode: KhalaCodeDesktopFleetDelegateRunRequest["mode"],
): KhalaCodeDesktopFleetDelegateRunStep => ({
  blockerCode: step.blockerCode ?? null,
  fallbackModule: step.fallbackModule ?? null,
  module: step.module,
  precondition: step.precondition,
  refs: step.refs.map(sanitizeDelegateRunRef),
  status: step.status,
  summary: delegateRunStepSummary(step, mode),
})

const projectFleetDelegateRunResult = (
  request: KhalaCodeDesktopFleetDelegateRunRequest,
  spawn: Awaited<ReturnType<typeof spawnCodexInstances>>,
): KhalaCodeDesktopFleetDelegateRunResult => ({
  acceptedCount: spawn.acceptedCount,
  delegateSignature: spawn.delegateSignature ?? "khala.fleet.delegate",
  delegateStatus: spawn.delegateStatus ?? "completed",
  mode: request.mode,
  ok: spawn.acceptedCount === spawn.requestedCount && (spawn.delegateStatus ?? "completed") === "completed",
  projection: {
    localPathsProjected: false,
    objectiveProjected: false,
    providerPayloadProjected: false,
    rawTraceMessagesProjected: false,
  },
  pylonRef: spawn.pylonRef,
  requestedCount: spawn.requestedCount,
  results: spawn.results.map(slot => ({
    accountRef: slot.accountRef,
    assignmentRef: slot.assignmentRef,
    blockerRefs: slot.blockerRefs.map(sanitizeDelegateRunRef),
    closeoutStatus: slot.closeoutStatus,
    slot: slot.slot,
    status: slot.status,
    tokensVerified: slot.tokensVerified,
    transcriptRef: slot.transcriptRef,
  })),
  trace: (spawn.delegateTrace ?? []).map(step => projectDelegateRunStep(step, request.mode)),
  validation: {
    fixture: request.mode === "fixture",
    repoPinsComplete: request.mode === "fixture" || missingDelegateRunPins(request).length === 0,
  },
  workerRuntime: {
    assignmentTool: "codex_spawn",
    homeRole: "pylon_isolated_worker_codex_home",
    role: "swarm_worker_codex_session",
    runtime: "codex_harness",
  },
})

const codexStatusFromRateLimits = (
  rateLimits: KhalaCodexRateLimitProviderStatus,
  harness: KhalaCodeDesktopCodexHarnessStatus,
  env: ChatEnv,
): KhalaCodeDesktopCodexAccountsStatus => {
  const available = harness.available && rateLimits.status === "ok"
  const credentialSource = env.CODEX_HOME?.trim()
    ? "CODEX_HOME" as const
    : "default_home" as const
  const readinessState =
    harness.auth.state !== "ready"
      ? harness.auth.state === "invalid"
        ? "invalid" as const
        : harness.auth.state === "error"
          ? "error" as const
          : "credentials_missing" as const
      : rateLimits.status === "ok"
      ? "ready" as const
      : rateLimits.status === "unavailable"
        ? "credentials_missing" as const
        : "error" as const
  const blockerRefs =
    readinessState === "ready"
      ? []
      : harness.auth.blockerRefs.length > 0
        ? harness.auth.blockerRefs
        : readinessState === "credentials_missing"
          ? ["blocker.codex.credentials_missing"]
          : ["blocker.codex.rate_limit_status_error"]
  const status = available
    ? "ready" as const
    : harness.status === "unavailable" || rateLimits.status === "unavailable"
      ? "unavailable" as const
      : "error" as const

  return {
    ok: true,
    app: "Khala Code Desktop",
    available,
    capability: "codex_accounts",
    observedAt: new Date().toISOString(),
    reason: available
      ? "Codex CLI account is signed in, the harness is ready, and rate-limit windows are available."
      : harness.available
        ? rateLimits.error ?? "Codex account status is unavailable."
        : harness.reason,
    status,
    accounts: [
      {
        provider: "codex",
        accountRef: "default",
        credentialSource,
        homeRef: credentialSource === "CODEX_HOME" ? "env:CODEX_HOME" : "default:~/.codex",
        homeRole: "main_user_codex_home",
        readiness: {
          state: readinessState,
          blockerRefs,
        },
        rateLimits,
      },
    ],
    harness,
    rateLimits,
  }
}

const unavailableRateLimits = (
  error: string,
): KhalaCodexRateLimitProviderStatus => ({
  provider: "codex",
  session: null,
  weekly: null,
  rateLimitResetCredits: null,
  updatedAtIso: new Date().toISOString(),
  error,
  status: "unavailable",
})

type KhalaCodeDesktopCodexModelRolePresetApplyResult = {
  readonly ok: boolean
  readonly preset: typeof KHALA_CODE_ARCHITECT_CODER_JUDGE_PRESET_ID
  readonly keyPath: typeof KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH
  readonly settings?: KhalaCodeDesktopCodexSettingsReadResult
  readonly error?: string
}

export function createKhalaCodeDesktopRpcRequestHandlers(
  input: KhalaCodeDesktopRpcHandlersInput,
): KhalaCodeDesktopRPCSchema["requests"] {
  const codexChatRuntime =
    input.codexChatRuntime ??
    (input.codexAppServerHost === undefined
      ? null
      : createCodexAppServerChatRuntime({
        env: input.env,
        host: input.codexAppServerHost,
        ...(input.emitChatTurnEvent === undefined ? {} : { onEvent: input.emitChatTurnEvent }),
        messageTokenAuditRecorder:
          createKhalaCodeDesktopCodexMessageTokenAuditRecorder({ env: input.env }),
        tokenUsageReporter: createKhalaCodeDesktopCodexTokenUsageReporter({ env: input.env }),
        workingDirectory: input.workingDirectory,
      }))
  const legacyChatTurn = input.legacyChatTurn ?? runKhalaCodeDesktopChatTurn
  const claudeApprovalService = input.claudeApprovalService ?? createClaudeApprovalService()
  const architectPlanStore = createArchitectPlanStore({ env: input.env })
  const requireCodexChatRuntime = (): CodexAppServerChatRuntime => {
    if (codexChatRuntime === null) {
      throw new Error("Codex app-server chat runtime is not configured.")
    }
    return codexChatRuntime
  }
  // Memoized: interrupt must see the SAME runtime instance (and its
  // activeTurns map) that started the turn — a fresh instance per RPC call
  // makes stop a no-op in claude_runtime mode.
  let lazyClaudeChatRuntime: ClaudeAppSdkChatRuntime | undefined
  const requireClaudeChatRuntime = (): ClaudeAppSdkChatRuntime => {
    if (input.claudeChatRuntime !== undefined) return input.claudeChatRuntime
    lazyClaudeChatRuntime ??= createClaudeAppSdkChatRuntime({
      approvalService: claudeApprovalService,
      env: input.env,
      ...(input.emitChatTurnEvent === undefined ? {} : { onEvent: input.emitChatTurnEvent }),
      repoRoot: input.fleetMcpBridgeRepoRoot ?? input.workingDirectory,
      tokenUsageReporter: createKhalaCodeDesktopClaudeTokenUsageReporter({ env: input.env }),
      workingDirectory: input.workingDirectory,
    })
    return lazyClaudeChatRuntime
  }
  const requireFleetRunSupervisor = (): KhalaCodeDesktopFleetRunSupervisorRpc => {
    if (input.fleetRunSupervisor === undefined) {
      throw new Error("Fleet run supervisor is not configured.")
    }
    return input.fleetRunSupervisor
  }
  const selectedRuntimeMode = async () =>
    khalaCodeDesktopRuntimeEnvOverride(input.env) ??
      await readKhalaCodeDesktopPersistedHarnessMode(input.env)

  const roleRuntimeMode = async (
    role: KhalaCodeModelRole,
  ): Promise<{
    readonly mode: "claude_runtime" | "codex_harness" | "khala_native_runtime"
    readonly role: KhalaCodeModelRoleEntry
  }> => {
    const envOverride = khalaCodeDesktopRuntimeEnvOverride(input.env)
    const modelRole = await resolveKhalaCodeDesktopModelRole(role, input.env)
    if (envOverride !== null) return { mode: envOverride, role: modelRole }
    if (!await hasKhalaCodeDesktopPersistedModelRoleRegistry(input.env)) {
      return { mode: await readKhalaCodeDesktopPersistedHarnessMode(input.env), role: modelRole }
    }
    if (modelRole.harness === "claude") return { mode: "claude_runtime", role: modelRole }
    if (modelRole.harness === "khala") return { mode: "khala_native_runtime", role: modelRole }
    return { mode: "codex_harness", role: modelRole }
  }

  const selectRoleRuntime = async (
    role: KhalaCodeModelRole,
  ): Promise<ChatRuntimeSelection> => {
    const { mode, role: modelRole } = await roleRuntimeMode(role)
    if (mode === "claude_runtime") {
      return { kind: "claude", modelRole, runtime: requireClaudeChatRuntime() }
    }
    if (mode === "khala_native_runtime") {
      return { kind: "legacy" }
    }
    return { kind: "codex", modelRole, runtime: requireCodexChatRuntime() }
  }

  const selectChatRuntime = async (): Promise<ChatRuntimeSelection> =>
    selectRoleRuntime("coder")
  let fleetMcpBridgeReady = false

  const architectPlanDispatchMode = (
    dag: ClaudePlanFanoutDag,
  ): KhalaCodeDesktopArchitectPlanArtifact["dispatchMode"] =>
    dag.nodes.length <= 2 ? "in_thread" : "fleet_run"

  const extractArchitectPlanDag = (response: KhalaCodeDesktopChatTurnResponse): ClaudePlanFanoutDag => {
    const text = response.messages.map(message => message.body).join("\n\n")
    const fenced = /```(?:json)?\s*([\s\S]*?)```/iu.exec(text)?.[1]?.trim()
    const candidate = fenced ?? text.slice(text.indexOf("{"), text.lastIndexOf("}") + 1)
    if (candidate.length === 0) {
      throw new Error("Claude architect did not return a JSON plan artifact.")
    }
    const parsed: unknown = JSON.parse(candidate)
    return decodeClaudePlanFanoutDag(parsed)
  }

  const runArchitectPlan = async (
    request: Parameters<KhalaCodeDesktopRPCSchema["requests"]["architectPlanRun"]>[0],
  ): Promise<KhalaCodeDesktopArchitectPlanRunResult> => {
    try {
      requireNonEmpty("architectPlanRun", "objective", request.objective)
      const turnId = `architect-plan-${randomUUID()}`
      const prompt = [
        claudePlanFanoutPlanModeInstructions(),
        "",
        "Return only the JSON object. Keep every field public-safe.",
        `Session: ${request.sessionId}`,
        `Objective: ${request.objective}`,
        request.repo === undefined ? null : `Repo: ${request.repo}`,
        request.branch === undefined ? null : `Branch: ${request.branch}`,
        request.baseCommit === undefined ? null : `Base commit: ${request.baseCommit}`,
        request.verify === undefined ? null : `Verify: ${request.verify}`,
      ].filter((line): line is string => line !== null).join("\n")
      const response = await requireClaudeChatRuntime().startTurn({
        messages: [{ body: prompt, id: `${turnId}-request`, role: "user" }],
        sessionId: request.sessionId,
        startNewThread: request.threadId === undefined,
        ...(request.threadId === undefined ? {} : { threadId: request.threadId }),
        turnId,
        cwd: input.workingDirectory,
        claudePermissionMode: "plan",
      })
      const dag = extractArchitectPlanDag(response)
      const now = new Date().toISOString()
      const artifact: KhalaCodeDesktopArchitectPlanArtifact = {
        schema: "openagents.khala_code.architect_plan_artifact.v1",
        planRef: dag.planRef,
        sessionId: request.sessionId,
        createdAt: now,
        updatedAt: now,
        status: "pending_approval",
        architectRole: {
          role: "architect",
          harness: "claude",
          mode: "plan",
          readOnly: true,
        },
        dispatchMode: architectPlanDispatchMode(dag),
        dag,
        fleetRunRef: null,
        coderTurnId: null,
      }
      return { ok: true, artifact: await architectPlanStore.put(artifact) }
    } catch (error) {
      return {
        ok: false,
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }

  const renderCoderPlanPrompt = (artifact: KhalaCodeDesktopArchitectPlanArtifact): string =>
    [
      "Execute this approved architect plan in the current Codex thread.",
      "The Claude architect plan is advisory structure only; obey the deterministic verify command and local safety gates.",
      "",
      `Plan ${artifact.planRef}: ${artifact.dag.objective}`,
      ...artifact.dag.nodes.map((node, index) => [
        "",
        `${index + 1}. ${node.title}`,
        node.objective,
        node.dependsOn === undefined || node.dependsOn.length === 0
          ? null
          : `Depends on: ${node.dependsOn.join(", ")}`,
        node.verify ?? artifact.dag.verify ?? null,
      ].filter((line): line is string => line !== null).join("\n")),
    ].join("\n")

  const decideArchitectPlan = async (
    request: Parameters<KhalaCodeDesktopRPCSchema["requests"]["architectPlanDecision"]>[0],
  ): Promise<KhalaCodeDesktopArchitectPlanDecisionResult> => {
    try {
      const existing = await architectPlanStore.get(request.sessionId, request.planRef)
      if (existing === null) return { ok: false, error: "Plan artifact not found." }
      const now = new Date().toISOString()
      if (request.decision === "reject") {
        const rejected = await architectPlanStore.put({
          ...existing,
          status: "rejected",
          updatedAt: now,
        })
        return { ok: true, artifact: rejected, message: `Rejected plan ${request.planRef}.` }
      }

      if (existing.dispatchMode === "fleet_run") {
        const result = await requireFleetRunSupervisor().start({
          objective: existing.dag.objective,
          runRef: `architect-${existing.planRef}`,
          targetConcurrency: Math.min(5, Math.max(1, existing.dag.nodes.length)),
          tickImmediately: true,
          workerKind: "codex",
          workSource: claudePlanFanoutDagToWorkSource(existing.dag),
        })
        const dispatched = await architectPlanStore.put({
          ...existing,
          status: "dispatched",
          updatedAt: now,
          fleetRunRef: result.run.runRef,
        })
        return {
          ok: true,
          artifact: dispatched,
          message: `Approved plan ${request.planRef}; started FleetRun ${result.run.runRef}.`,
        }
      }

      const turnId = `architect-coder-${randomUUID()}`
      await requireCodexChatRuntime().startTurn({
        messages: [{ body: renderCoderPlanPrompt(existing), id: `${turnId}-request`, role: "user" }],
        sessionId: request.sessionId,
        ...(request.threadId === undefined ? { startNewThread: true } : { threadId: request.threadId }),
        turnId,
        cwd: input.workingDirectory,
      })
      const dispatched = await architectPlanStore.put({
        ...existing,
        status: "dispatched",
        updatedAt: now,
        coderTurnId: turnId,
      })
      return {
        ok: true,
        artifact: dispatched,
        message: `Approved plan ${request.planRef}; dispatched an in-thread Codex turn.`,
      }
    } catch (error) {
      return {
        ok: false,
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }

  const maybeEnsureFleetMcpBridge = async (): Promise<CodexFleetMcpBridgeEnsureResult | null> => {
    if (input.enableFleetMcpBridge !== true || fleetMcpBridgeReady) return null
    const result = await ensureCodexFleetMcpBridge({
      env: input.env,
      host: input.codexAppServerHost,
      repoRoot: input.fleetMcpBridgeRepoRoot ?? input.workingDirectory,
    })
    if (result.ok) fleetMcpBridgeReady = true
    return result
  }

  const withFleetMcpBridgeNote = (
    response: KhalaCodeDesktopChatTurnResponse,
    bridge: CodexFleetMcpBridgeEnsureResult | null,
  ): KhalaCodeDesktopChatTurnResponse => {
    if (bridge === null || bridge.ok) return response
    return {
      ...response,
      backend: {
        ...response.backend,
        blockerRefs: [
          ...(response.backend.blockerRefs ?? []),
          "blocker.local.khala_fleet_mcp_bridge.unavailable",
        ],
      },
      messages: [
        {
          id: `fleet-mcp-bridge-${Date.now().toString(36)}`,
          role: "system",
          body: "Khala Fleet message-trigger tools were not registered for this turn. The Fleet panel and direct delegate runner are still available.",
        },
        ...response.messages,
      ],
    }
  }

  const labelLegacyRuntimeResponse = (
    response: KhalaCodeDesktopChatTurnResponse,
  ): KhalaCodeDesktopChatTurnResponse => ({
    ...response,
    backend: {
      ...response.backend,
      runtimeMode: "khala_native_runtime",
      toolCatalogKind: "khala_native_legacy",
    },
    messages: [
      {
        id: `legacy-runtime-${Date.now().toString(36)}`,
        role: "system",
        body: "Legacy Khala native runtime handled this turn. The default Khala Code path wraps the local Codex harness.",
      },
      ...response.messages,
    ],
  })

  const failedCodexHarnessBlockerRefs = async (
    response: KhalaCodeDesktopChatTurnResponse,
  ): Promise<readonly string[]> => {
    if (response.ok || response.backend.kind !== "codex_app_server") return []
    if (response.backend.turnStatus !== "failed") return []
    try {
      const status = await codexHarnessStatus()
      return [...new Set(status.auth.blockerRefs)]
    } catch {
      return []
    }
  }

  const labelCodexHarnessResponse = async (
    response: KhalaCodeDesktopChatTurnResponse,
  ): Promise<KhalaCodeDesktopChatTurnResponse> => {
    const blockerRefs = response.backend.blockerRefs ?? await failedCodexHarnessBlockerRefs(response)
    return {
      ...response,
      backend: {
        ...response.backend,
        ...(blockerRefs.length === 0 ? {} : { blockerRefs }),
        runtimeMode: "codex_harness",
        toolCatalogKind: response.backend.toolCatalogKind ?? "codex_app_server",
      },
    }
  }

  const unsupportedLegacyThreadLifecycle = async (): Promise<never> => {
    throw new Error(legacyThreadLifecycleUnsupportedMessage)
  }

  const ecosystemNotifications: CodexAppServerNotification[] = []
  const ecosystemNotificationMethods = new Set([
    "app/list/updated",
    "externalAgentConfig/import/completed",
    "externalAgentConfig/import/progress",
    "mcpServer/oauthLogin/completed",
    "mcpServer/startupStatus/updated",
    "skills/changed",
  ])
  input.codexAppServerHost?.subscribe(notification => {
    if (!ecosystemNotificationMethods.has(notification.method)) return
    ecosystemNotifications.push(notification)
    if (ecosystemNotifications.length > 50) {
      ecosystemNotifications.splice(0, ecosystemNotifications.length - 50)
    }
  })

  const codexHarnessStatus = async (): Promise<KhalaCodeDesktopCodexHarnessStatus> =>
    input.codexHarnessStatus?.() ??
    inspectCodexHarnessStatus({ env: input.env as NodeJS.ProcessEnv })
  const claudeHarnessStatus = async () =>
    inspectClaudeHarnessStatus({ env: input.env })

  const codexAccountsStatus = async (): Promise<KhalaCodeDesktopCodexAccountsStatus> => {
    const harness = await codexHarnessStatus()
    if (!harness.available) {
      return codexStatusFromRateLimits(unavailableRateLimits(harness.reason), harness, input.env)
    }
    const rateLimits = await (input.codexRateLimitStatus?.() ??
      fetchKhalaCodexRateLimitStatus({ env: input.env as NodeJS.ProcessEnv }))
    return codexStatusFromRateLimits(rateLimits, harness, input.env)
  }

  const pylonRuntimeStatus = async (): Promise<KhalaCodeDesktopRuntimeStatus> => {
    const status = await ensureLocalPylon({
      start: false,
      timeoutMs: 10_000,
      waitMs: 0,
    }, {
      env: input.env,
    })
    return runtimeStatus({
      available: status.ok,
      capability: "pylon",
      reason: status.ok
        ? status.message
        : `${status.message}${status.unavailableReason ? ` ${status.unavailableReason}` : ""}`,
      status: status.ok ? "ready" : "unavailable",
    })
  }

  const codexHomePathForResetCredit = async (accountRef: string): Promise<string | null> => {
    if (accountRef === "default" || accountRef === "(default)") return null
    const fleet = await inspectCodexFleet(
      { includeProcesses: false, includeRateLimits: false, startPylon: false },
      { ...input.codexFleetToolOptions, env: input.env as NodeJS.ProcessEnv },
    )
    const account = fleet.accounts.find(candidate => candidate.accountRef === accountRef)
    if (account === undefined) throw new Error(`Codex account ${accountRef} was not found`)
    if (account.home === null) throw new Error(`Codex account ${accountRef} does not have an isolated home`)
    return account.home
  }

  const threadIdForSlashCommand = async (
    request: KhalaCodeDesktopSlashCommandDispatchRequest,
  ): Promise<string | null> => {
    const explicit = request.threadId?.trim()
    if (explicit !== undefined && explicit.length > 0) return explicit
    const selection = await selectChatRuntime()
    if (selection.kind === "legacy") return null
    return await selection.runtime.threadIdForSession(request.sessionId) ?? null
  }

  const blockedSlashCommand = (
    request: {
      readonly command?: string
      readonly message: string
      readonly method?: string
      readonly threadId?: string
    },
  ): KhalaCodeDesktopSlashCommandDispatchResult => ({
    ok: false,
    status: "blocked",
    ...request,
  })

  const dispatchedSlashCommand = (
    request: {
      readonly command: string
      readonly message: string
      readonly method: string
      readonly response?: unknown
      readonly threadId?: string
    },
  ): KhalaCodeDesktopSlashCommandDispatchResult => ({
    ok: true,
    status: "dispatched",
    ...request,
  })

  const requestCodexAppServer = async (
    method: string,
    params?: unknown,
  ): Promise<unknown> => {
    if (input.codexAppServerHost === undefined) {
      throw new Error("Codex app-server host is not configured.")
    }
    return input.codexAppServerHost.request(method, params)
  }

  const readCodexMentionCandidates = async (
    request: KhalaCodeDesktopCodexMentionCandidatesRequest = {},
  ): Promise<KhalaCodeDesktopCodexMentionCandidatesResult> => {
    const cwd = request.cwd ?? input.workingDirectory
    const query = request.query?.trim() ?? ""
    if (query.length === 0) {
      const response = await requestCodexAppServer("fs/readDirectory", { path: cwd })
      const entries = isRecord(response) && Array.isArray(response.entries)
        ? response.entries
        : []
      const candidates = entries
        .map(normalizeDirectoryCandidate)
        .filter((candidate): candidate is KhalaCodeDesktopCodexMentionCandidate => candidate !== null)
        .slice(0, MAX_DIRECTORY_CANDIDATES)
      return {
        ok: true,
        candidates,
        source: "fs/readDirectory",
        truncated: entries.length > MAX_DIRECTORY_CANDIDATES,
      }
    }

    const response = await requestCodexAppServer("fuzzyFileSearch", {
      query,
      roots: [cwd],
      cancellationToken: null,
    })
    const files = isRecord(response) && Array.isArray(response.files)
      ? response.files
      : []
    const candidates = files
      .map(normalizeMentionCandidate)
      .filter((candidate): candidate is KhalaCodeDesktopCodexMentionCandidate => candidate !== null)
      .slice(0, MAX_MENTION_CANDIDATES)
    return {
      ok: true,
      candidates,
      source: "fuzzyFileSearch",
      truncated: files.length > MAX_MENTION_CANDIDATES,
    }
  }

  const codexAppServerAction = async (
    method: string,
    params?: unknown,
  ): Promise<KhalaCodeDesktopCodexAppServerActionResult> => {
    try {
      return {
        ok: true,
        method,
        response: await requestCodexAppServer(method, params),
      }
    } catch (error) {
      return {
        ok: false,
        method,
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }

  const nonEmptyCodexField = (
    value: string,
    label: string,
  ): string => {
    const trimmed = value.trim()
    if (trimmed.length === 0) throw new Error(`${label} is required`)
    return trimmed
  }

  const codexBackgroundTerminalsList = async (
    request: KhalaCodeDesktopCodexBackgroundTerminalsListRequest,
  ): Promise<KhalaCodeDesktopCodexAppServerActionResult> =>
    codexAppServerAction("thread/backgroundTerminals/list", {
      threadId: nonEmptyCodexField(request.threadId, "threadId"),
      cursor: request.cursor ?? null,
      limit: request.limit ?? 50,
    })

  const codexBackgroundTerminalsClean = async (
    request: KhalaCodeDesktopCodexBackgroundTerminalsCleanRequest,
  ): Promise<KhalaCodeDesktopCodexAppServerActionResult> =>
    codexAppServerAction("thread/backgroundTerminals/clean", {
      threadId: nonEmptyCodexField(request.threadId, "threadId"),
    })

  const codexBackgroundTerminalsTerminate = async (
    request: KhalaCodeDesktopCodexBackgroundTerminalsTerminateRequest,
  ): Promise<KhalaCodeDesktopCodexAppServerActionResult> =>
    codexAppServerAction("thread/backgroundTerminals/terminate", {
      threadId: nonEmptyCodexField(request.threadId, "threadId"),
      processId: nonEmptyCodexField(request.processId, "processId"),
    })

  const readCodexEcosystem = async (
    request: KhalaCodeDesktopCodexEcosystemReadRequest = {},
  ): Promise<KhalaCodeDesktopCodexEcosystemReadResult> => {
    const cwd = request.cwd ?? input.workingDirectory
    if (input.codexAppServerHost === undefined) {
      return projectKhalaCodeDesktopCodexEcosystem({
        cwd,
        errors: ["Codex app-server host is not configured."],
      })
    }

    const errors: string[] = []
    const capture = async <Result>(
      label: string,
      method: string,
      params?: unknown,
    ): Promise<Result | undefined> => {
      try {
        return await input.codexAppServerHost!.request<Result>(method, params)
      } catch (error) {
        errors.push(`${label}: ${error instanceof Error ? error.message : String(error)}`)
        return undefined
      }
    }

    const [
      skillsList,
      hooksList,
      externalAgentConfigDetect,
      externalAgentConfigImportHistories,
      pluginList,
      pluginInstalled,
      appsList,
      mcpServerStatusList,
    ] = await Promise.all([
      capture("skills/list", "skills/list", {
        cwds: [cwd],
        ...(request.forceReloadSkills === undefined ? {} : { forceReload: request.forceReloadSkills }),
      }),
      capture("hooks/list", "hooks/list", { cwds: [cwd] }),
      capture("externalAgentConfig/detect", "externalAgentConfig/detect", {
        cwds: [cwd],
        includeHome: true,
      }),
      capture("externalAgentConfig/import/readHistories", "externalAgentConfig/import/readHistories"),
      capture("plugin/list", "plugin/list", { cwds: [cwd] }),
      capture("plugin/installed", "plugin/installed", { cwds: [cwd] }),
      capture("app/list", "app/list", {
        ...(request.threadId === undefined ? {} : { threadId: request.threadId }),
        ...(request.forceRefetchApps === undefined ? {} : { forceRefetch: request.forceRefetchApps }),
      }),
      capture("mcpServerStatus/list", "mcpServerStatus/list", {
        detail: "full",
        ...(request.threadId === undefined ? {} : { threadId: request.threadId }),
      }),
    ])

    return projectKhalaCodeDesktopCodexEcosystem({
      cwd,
      errors,
      skillsList,
      hooksList,
      externalAgentConfigDetect,
      externalAgentConfigImportHistories,
      pluginList,
      pluginInstalled,
      appsList,
      mcpServerStatusList,
      notifications: ecosystemNotifications.slice(-25),
    })
  }

  const readCodexSettings = async (
    request: KhalaCodeDesktopCodexSettingsReadRequest = {},
  ): Promise<KhalaCodeDesktopCodexSettingsReadResult> => {
    const cwd = request.cwd ?? input.workingDirectory
    if (input.codexAppServerHost === undefined) {
      return projectKhalaCodeDesktopCodexSettings({
        cwd,
        errors: ["Codex app-server host is not configured."],
      })
    }

    const errors: string[] = []
    const capture = async <Result>(
      label: string,
      method: string,
      params?: unknown,
    ): Promise<Result | undefined> => {
      try {
        return await input.codexAppServerHost!.request<Result>(method, params)
      } catch (error) {
        errors.push(`${label}: ${error instanceof Error ? error.message : String(error)}`)
        return undefined
      }
    }

    const [
      configRead,
      modelList,
      providerCapabilities,
      permissionProfileList,
      requirementsRead,
      usageRead,
      collaborationModeList,
    ] = await Promise.all([
      capture("config/read", "config/read", { cwd, includeLayers: true }),
      capture("model/list", "model/list", {
        includeHidden: request.includeHiddenModels === true,
      }),
      capture("modelProvider/capabilities/read", "modelProvider/capabilities/read", {}),
      capture("permissionProfile/list", "permissionProfile/list", { cwd }),
      capture("configRequirements/read", "configRequirements/read"),
      capture("account/usage/read", "account/usage/read"),
      capture("collaborationMode/list", "collaborationMode/list", {}),
    ])

    return projectKhalaCodeDesktopCodexSettings({
      cwd,
      errors,
      configRead,
      modelList,
      providerCapabilities,
      permissionProfileList,
      requirementsRead,
      usageRead,
      collaborationModeList,
    })
  }

  const writeCodexConfigValue = async (
    request: KhalaCodeDesktopCodexConfigValueWriteRequest,
  ) => {
    if (input.codexAppServerHost === undefined) {
      return {
        ok: false,
        keyPath: request.keyPath,
        error: "Codex app-server host is not configured.",
      }
    }
    try {
      const response = await input.codexAppServerHost.request("config/value/write", {
        keyPath: request.keyPath,
        value: request.value,
        mergeStrategy: request.mergeStrategy ?? "replace",
        ...(request.filePath === undefined ? {} : { filePath: request.filePath }),
        ...(request.expectedVersion === undefined ? {} : { expectedVersion: request.expectedVersion }),
      })
      return {
        ok: true,
        keyPath: request.keyPath,
        response,
        settings: await readCodexSettings({
          includeHiddenModels: true,
          ...(request.cwd === undefined ? {} : { cwd: request.cwd }),
        }),
      }
    } catch (error) {
      return {
        ok: false,
        keyPath: request.keyPath,
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }

  const applyCodexModelRolePreset = async (request: {
    readonly cwd?: string | undefined
    readonly preset: typeof KHALA_CODE_ARCHITECT_CODER_JUDGE_PRESET_ID
  }): Promise<KhalaCodeDesktopCodexModelRolePresetApplyResult> => {
    if (input.codexAppServerHost === undefined) {
      return {
        ok: false,
        preset: request.preset,
        keyPath: KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH as typeof KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH,
        error: "Codex app-server host is not configured.",
      }
    }
    try {
      await input.codexAppServerHost.request("config/value/write", {
        keyPath: KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH as typeof KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH,
        value: makeKhalaCodeArchitectCoderJudgeRegistry(),
        mergeStrategy: "replace",
      })
      return {
        ok: true,
        preset: request.preset,
        keyPath: KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH as typeof KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH,
        settings: await readCodexSettings({
          includeHiddenModels: true,
          ...(request.cwd === undefined ? {} : { cwd: request.cwd }),
        }),
      }
    } catch (error) {
      return {
        ok: false,
        preset: request.preset,
        keyPath: KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH,
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }

  const dispatchSlashAppServerCommand = async (
    request: KhalaCodeDesktopSlashCommandDispatchRequest,
  ): Promise<KhalaCodeDesktopSlashCommandDispatchResult> => {
    const parsed = parseKhalaCodeDesktopSlashCommand(request.raw, {
      ...(request.debug === undefined ? {} : { debug: request.debug }),
      ...(request.platform === undefined ? {} : { platform: request.platform }),
    })
    if (parsed === null) {
      return {
        ok: false,
        status: "not_found",
        message: "Unknown Codex slash command.",
      }
    }
    const command = parsed.command
    const availability = evaluateKhalaCodeDesktopSlashCommandAvailability(command, {
      ...(request.activeTurn === undefined ? {} : { activeTurn: request.activeTurn }),
      ...(request.sideConversation === undefined ? {} : { sideConversation: request.sideConversation }),
    })
    if (!availability.available) {
      return blockedSlashCommand({
        command: command.command,
        message: availability.reason ?? `/${command.command} is not available here.`,
      })
    }
    const dispatch = command.dispatch
    if (dispatch.kind === "gap") {
      return {
        ok: false,
        status: dispatch.unavailable === undefined ? "gap" : "unavailable",
        command: command.command,
        ...(dispatch.unavailable === undefined ? {} : { gap: dispatch.unavailable }),
        message: dispatch.dependency,
      }
    }
    if (dispatch.kind === "client") {
      return {
        ok: true,
        status: "client_action",
        action: dispatch.action,
        command: command.command,
        message: `/${command.command} is handled by the Khala Code desktop shell.`,
      }
    }

    const args = parsed.args
    if (dispatch.requiresArgs === true && args.length === 0) {
      return blockedSlashCommand({
        command: command.command,
        message: `/${command.command} requires inline arguments.`,
        method: dispatch.method,
      })
    }

    const threadId = await threadIdForSlashCommand(request)
    if (dispatch.requiresThread === true && threadId === null) {
      return blockedSlashCommand({
        command: command.command,
        message: `/${command.command} requires an active Codex thread.`,
        method: dispatch.method,
      })
    }

    let attemptedMethod = dispatch.method
    try {
      switch (command.command) {
        case "new": {
          const response = await requireCodexChatRuntime().startThread({
            cwd: request.cwd ?? input.workingDirectory,
            sessionId: request.sessionId,
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Started a new Codex thread.",
            response,
            threadId: response.threadId,
          })
        }
        case "resume": {
          const resumeThreadId = args.split(/\s+/)[0]?.trim()
          if (resumeThreadId === undefined || resumeThreadId.length === 0) {
            return blockedSlashCommand({
              command: command.command,
              message: "/resume requires a Codex thread id until the desktop picker lands.",
              method: dispatch.method,
            })
          }
          const response = await requireCodexChatRuntime().resumeThread({
            cwd: request.cwd ?? input.workingDirectory,
            sessionId: request.sessionId,
            threadId: resumeThreadId,
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: `Resumed Codex thread ${resumeThreadId}.`,
            response,
            threadId: response.threadId,
          })
        }
        case "compact": {
          const response = await requireCodexChatRuntime().compactThread({
            sessionId: request.sessionId,
            ...(threadId === null ? {} : { threadId }),
          })
          return {
            ok: response.ok,
            status: response.ok ? "dispatched" : "blocked",
            command: command.command,
            method: dispatch.method,
            message: response.ok
              ? "Requested Codex context compaction."
              : response.error ?? "Codex context compaction could not start.",
            response,
            ...(response.threadId === undefined ? {} : { threadId: response.threadId }),
          }
        }
        case "archive":
        case "delete":
        case "fork": {
          const response = await requestCodexAppServer(dispatch.method, { threadId })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: `/${command.command} was sent to Codex.`,
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "rename": {
          const response = await requestCodexAppServer(dispatch.method, {
            threadId,
            name: args,
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: `Renamed the Codex thread to "${args}".`,
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "goal": {
          const method =
            args.length === 0
              ? "thread/goal/get"
              : args.toLowerCase() === "clear"
                ? "thread/goal/clear"
                : dispatch.method
          const params = args.length === 0 || args.toLowerCase() === "clear"
            ? { threadId }
            : { threadId, objective: args, status: "active" }
          const response = await requestCodexAppServer(method, params)
          return dispatchedSlashCommand({
            command: command.command,
            method,
            message: args.length === 0
              ? "Loaded the current Codex goal."
              : args.toLowerCase() === "clear"
                ? "Cleared the current Codex goal."
                : "Updated the current Codex goal.",
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "review": {
          const response = await requestCodexAppServer(dispatch.method, {
            threadId,
            target: args.length === 0
              ? { type: "uncommittedChanges" }
              : { type: "custom", instructions: args },
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Started a Codex review turn.",
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "mention": {
          const response = await readCodexMentionCandidates({
            ...(request.cwd === undefined ? {} : { cwd: request.cwd }),
            query: args,
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: response.source,
            message: mentionMessage({
              candidates: response.candidates,
              source: response.source,
              truncated: response.truncated,
            }),
            response,
          })
        }
        case "diff": {
          const response = await requestCodexAppServer(dispatch.method, {
            cwd: request.cwd ?? input.workingDirectory,
          })
          const projected = diffMessage(response)
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: projected.message,
            response: projected.response,
          })
        }
        case "ide": {
          const response = await requestCodexAppServer(dispatch.method, {
            cwd: request.cwd ?? input.workingDirectory,
            includeLayers: true,
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: ideMessage(response),
            response,
          })
        }
        case "btw": {
          const response = await requireCodexChatRuntime().steerTurn({
            clientUserMessageId: `khala-code-slash-btw-${Date.now().toString(36)}`,
            sessionId: request.sessionId,
            text: args,
          })
          return {
            ok: response.ok,
            status: response.ok ? "dispatched" : "blocked",
            command: command.command,
            method: dispatch.method,
            message: response.ok
              ? "Steered the active Codex turn with a BTW side note."
              : response.error ?? "Codex turn steering could not be applied.",
            response,
            ...(response.threadId === undefined ? {} : { threadId: response.threadId }),
          }
        }
        case "ps":
        case "stop": {
          const response = command.command === "ps"
            ? await codexBackgroundTerminalsList({ threadId: threadId ?? "" })
            : await codexBackgroundTerminalsClean({ threadId: threadId ?? "" })
          if (!response.ok) {
            return blockedSlashCommand({
              command: command.command,
              message: response.error ?? "Codex background terminal request failed.",
              method: response.method,
              ...(threadId === null ? {} : { threadId }),
            })
          }
          return dispatchedSlashCommand({
            command: command.command,
            method: response.method,
            message: command.command === "ps"
              ? "Loaded Codex background terminals."
              : "Requested Codex background terminal cleanup.",
            response: response.response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "mcp": {
          const response = await requestCodexAppServer(dispatch.method, {
            ...(threadId === null ? {} : { threadId }),
            detail: "full",
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Loaded Codex MCP server status.",
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "app":
        case "apps": {
          const response = await requestCodexAppServer(dispatch.method, {
            ...(threadId === null ? {} : { threadId }),
            forceRefetch: args.toLowerCase() === "refresh",
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Loaded Codex app integrations.",
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "plugins": {
          const response = await requestCodexAppServer(dispatch.method, {
            cwds: [request.cwd ?? input.workingDirectory],
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Loaded Codex plugins.",
            response,
          })
        }
        case "model": {
          const response = await requestCodexAppServer(dispatch.method, {
            includeHidden: args.toLowerCase() === "all",
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Loaded Codex models.",
            response,
          })
        }
        case "permissions": {
          const response = await requestCodexAppServer(dispatch.method, {
            cwd: request.cwd ?? input.workingDirectory,
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Loaded Codex permission profiles.",
            response,
          })
        }
        case "experimental": {
          const response = await requestCodexAppServer(dispatch.method, {
            ...(threadId === null ? {} : { threadId }),
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Loaded Codex experimental features.",
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "keymap":
        case "pets": {
          if (args.length > 0) {
            const keyPath = command.command === "keymap" ? "tui.keymap" : "tui.pet"
            const value = command.command === "keymap" ? JSON.parse(args) : args
            attemptedMethod = "config/value/write"
            const response = await requestCodexAppServer("config/value/write", {
              keyPath,
              value,
              mergeStrategy: "replace",
            })
            return dispatchedSlashCommand({
              command: command.command,
              method: "config/value/write",
              message: `Updated Codex ${command.command === "keymap" ? "keymap" : "pet"} preference.`,
              response,
            })
          }
          const response = await requestCodexAppServer(dispatch.method, {
            cwd: request.cwd ?? input.workingDirectory,
            includeLayers: true,
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: `Loaded Codex ${command.command === "keymap" ? "keymap" : "pet"} preferences.`,
            response,
          })
        }
        case "vim":
        case "statusline":
        case "theme":
        case "personality": {
          const response = await requestCodexAppServer(dispatch.method, {
            cwd: request.cwd ?? input.workingDirectory,
            includeLayers: true,
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: `Loaded Codex /${command.command} preferences.`,
            response,
          })
        }
        case "usage":
        case "logout": {
          const response = await requestCodexAppServer(dispatch.method)
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: command.command === "usage"
              ? "Loaded Codex token usage."
              : "Requested Codex sign-out.",
            response,
          })
        }
        default: {
          const response = await requestCodexAppServer(dispatch.method)
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: `/${command.command} was sent to Codex.`,
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
      }
    } catch (error) {
      return blockedSlashCommand({
        command: command.command,
        message: error instanceof Error ? error.message : String(error),
        method: attemptedMethod,
        ...(threadId === null ? {} : { threadId }),
      })
    }
  }

  return {
    async appInfo() {
      return appInfo()
    },
    async appleFmReadiness() {
      return input.appleFmReadiness()
    },
    async codexAppServerRestart() {
      return input.codexAppServerHost?.restart() ?? {
        ok: false,
        action: "restart",
        changed: false,
        status: {
          ok: true,
          app: "Khala Code Desktop",
          adapterVersion: "unconfigured",
          codexCommand: "codex",
          codexHome: "",
          diagnostics: [],
          initialized: false,
          initializeResult: null,
          lastError: "Codex app-server host is not configured.",
          pendingRequestCount: 0,
          pid: null,
          state: "errored",
          transport: "stdio",
        },
        error: "Codex app-server host is not configured.",
      }
    },
    async codexAppServerStart() {
      const startedAt = performance.now()
      const result = input.codexAppServerHost === undefined
        ? {
          ok: false as const,
          action: "start" as const,
          changed: false,
          status: {
            ok: true as const,
            app: "Khala Code Desktop" as const,
            adapterVersion: "unconfigured",
            codexCommand: "codex",
            codexHome: "",
            diagnostics: [],
            initialized: false,
            initializeResult: null,
            lastError: "Codex app-server host is not configured.",
            pendingRequestCount: 0,
            pid: null,
            state: "errored" as const,
            transport: "stdio" as const,
          },
          error: "Codex app-server host is not configured.",
        }
        : await input.codexAppServerHost.start()
      if (
        result.ok &&
        result.changed &&
        result.status.state === "running" &&
        result.status.initialized
      ) {
        await recordQaTimerSample(input, "app_server.spawn_ready_ms", startedAt, {
          action: "start",
          transport: result.status.transport,
        })
      }
      return result
    },
    async codexAppServerStatus() {
      return input.codexAppServerHost?.status() ?? {
        ok: true,
        app: "Khala Code Desktop",
        adapterVersion: "unconfigured",
        codexCommand: "codex",
        codexHome: "",
        diagnostics: [],
        initialized: false,
        initializeResult: null,
        lastError: "Codex app-server host is not configured.",
        pendingRequestCount: 0,
        pid: null,
        state: "errored",
        transport: "stdio",
      }
    },
    async codexAppServerStop() {
      return input.codexAppServerHost?.stop() ?? {
        ok: true,
        action: "stop",
        changed: false,
        status: {
          ok: true,
          app: "Khala Code Desktop",
          adapterVersion: "unconfigured",
          codexCommand: "codex",
          codexHome: "",
          diagnostics: [],
          initialized: false,
          initializeResult: null,
          lastError: "Codex app-server host is not configured.",
          pendingRequestCount: 0,
          pid: null,
          state: "stopped",
          transport: "stdio",
        },
      }
    },
    async codexAccountsStatus() {
      return codexAccountsStatus()
    },
    async codexFleetDelegateRun(request): Promise<KhalaCodeDesktopFleetDelegateRunResult> {
      if (request.objective.trim().length === 0) {
        throw new Error("codexFleetDelegateRun requires an objective")
      }
      if (request.mode !== "fixture" && request.mode !== "real_work") {
        throw new Error("codexFleetDelegateRun requires mode fixture or real_work")
      }
      const missingPins = missingDelegateRunPins(request)
      if (request.mode === "real_work" && missingPins.length > 0) {
        throw new Error(`codexFleetDelegateRun real-work mode requires repo, claimRef, commit, and verify pins; missing ${missingPins.join(", ")}`)
      }
      const spawn = await spawnCodexInstances({
        accountRef: request.accountRef,
        branch: request.mode === "fixture" ? undefined : request.branch,
        claimRef: request.mode === "fixture" ? undefined : request.claimRef,
        commit: request.mode === "fixture" ? undefined : request.commit,
        count: request.count,
        fixture: request.mode === "fixture",
        noRun: request.noRun,
        prompt: `Khala Code role: coder\n\n${request.objective}`,
        repo: request.mode === "fixture" ? undefined : request.repo,
        timeoutMs: request.timeoutMs,
        verify: request.mode === "fixture" ? undefined : request.verify,
      }, {
        ...input.codexFleetToolOptions,
        env: input.env,
      })
      return projectFleetDelegateRunResult(request, spawn)
    },
    async codexFleetStatus(): Promise<KhalaCodeDesktopFleetStatus> {
      const fleet = await inspectCodexFleet(
        { includeProcesses: true, includeRateLimits: false, startPylon: false },
        { ...input.codexFleetToolOptions, env: input.env as NodeJS.ProcessEnv },
      )
      const emails = await collectCodexAccountEmails(
        fleet.accounts.map(account => account.accountRef),
        { env: input.env },
      )
      return {
        ok: fleet.ensure.ok,
        observedAt: fleet.observedAt,
        sessionLayers: sessionLayers(),
        pylon: {
          status: fleet.ensure.status,
          pylonRef: fleet.ensure.pylonRef,
          message: fleet.ensure.message,
        },
        availableCodexAssignments: fleet.availableCodexAssignments,
        maxCodexAssignments: fleet.maxCodexAssignments,
        accounts: fleet.accounts.map(account => ({
          accountRef: account.accountRef,
          provider: account.provider,
          readiness: account.readiness,
          quotaState: account.quotaState,
          accountKey: account.accountKey,
          capacity: account.capacity,
          paused: account.paused,
          ...(account.rateLimits === undefined ? {} : { rateLimits: account.rateLimits }),
          homeRole: accountHomeRole(account.accountRef),
          queuePolicy: fleetQueuePolicy(account.capacity, account.readiness),
          sessionRole: accountSessionRole(account.accountRef),
          email: emails[account.accountRef] ?? null,
        })),
        activeAssignments: fleet.activeAssignments.map(marker => ({
          assignmentRef: marker.assignmentRef,
          blockerRefs: marker.blockerRefs,
          closeoutStatus: marker.closeoutStatus,
          elapsedMs: marker.elapsedMs,
          issueRef: marker.issueRef,
          runRef: marker.runRef,
          tokenRate: marker.tokenRate,
          workerSession: workerSessionForAssignment(marker),
          updatedAt: marker.updatedAt,
        })),
        tokenRate: fleet.tokenRate,
        processes: fleet.processes.map(process => ({
          pid: process.pid,
          parentPid: process.parentPid,
          elapsed: process.elapsed,
        })),
      }
    },
    async codexFleetPromoteThread(request): Promise<KhalaCodeDesktopFleetPromotionResult> {
      if (request.sessionId.trim().length === 0) {
        throw new Error("codexFleetPromoteThread requires a sessionId")
      }
      if (request.threadId.trim().length === 0) {
        throw new Error("codexFleetPromoteThread requires a threadId")
      }
      if (request.objective.trim().length === 0) {
        throw new Error("codexFleetPromoteThread requires an explicit objective")
      }
      if (request.contextBoundary.includeTranscript !== false) {
        throw new Error("codexFleetPromoteThread requires includeTranscript: false")
      }
      const spawn = await spawnCodexInstances({
        accountRef: request.accountRef,
        branch: request.branch,
        claimRef: request.claimRef,
        commit: request.commit,
        count: request.count,
        fixture: request.fixture,
        noRun: request.noRun,
        prompt: `Khala Code role: coder\n\n${renderFleetPromotionObjective(request)}`,
        repo: request.repo,
        timeoutMs: request.timeoutMs,
        verify: request.verify,
      }, {
        ...input.codexFleetToolOptions,
        env: input.env,
      })
      return promoteThreadResult(request, spawn)
    },
    async fleetRunStart(request): Promise<KhalaCodeDesktopFleetRunStartResult> {
      requireNonEmpty("fleetRunStart", "objective", request.objective)
      if (!Number.isInteger(request.targetConcurrency) || request.targetConcurrency < 1) {
        throw new Error("fleetRunStart requires positive integer targetConcurrency")
      }
      const result = await requireFleetRunSupervisor().start(request)
      return {
        ok: true,
        run: result.run,
        supervisorStarted: result.supervisorStarted,
      }
    },
    async fleetRunStatus(request): Promise<KhalaCodeDesktopFleetRunStatusResult> {
      requireNonEmpty("fleetRunStatus", "runRef", request.runRef)
      const result = await requireFleetRunSupervisor().status(request)
      return {
        ok: true,
        run: result.run,
        supervisorActive: result.supervisorActive,
      }
    },
    async architectPlanRun(request): Promise<KhalaCodeDesktopArchitectPlanRunResult> {
      return runArchitectPlan(request)
    },
    async architectPlanDecision(request): Promise<KhalaCodeDesktopArchitectPlanDecisionResult> {
      return decideArchitectPlan(request)
    },
    async fleetRunControl(request): Promise<KhalaCodeDesktopFleetRunControlResult> {
      requireNonEmpty("fleetRunControl", "runRef", request.runRef)
      const result = await requireFleetRunSupervisor().control(request)
      return {
        ok: true,
        previousState: result.previousState,
        run: result.run,
        supervisorActive: result.supervisorActive,
        verb: request.verb,
      }
    },
    async fleetRunList(request): Promise<KhalaCodeDesktopFleetRunListResult> {
      const runs = await requireFleetRunSupervisor().list(request)
      return {
        ok: true,
        runs: [...runs],
      }
    },
    async fleetWorkerControl(request): Promise<KhalaCodeDesktopFleetWorkerControlResult> {
      requireNonEmpty("fleetWorkerControl", "workerRefHash", request.workerRefHash)
      if (request.verb !== "flag") {
        requireNonEmpty("fleetWorkerControl", "assignmentRef", request.assignmentRef ?? "")
      }
      const supervisor = requireFleetRunSupervisor()
      if (supervisor.workerControl === undefined) {
        throw new Error("fleetWorkerControl requires fleet-run supervisor worker control")
      }
      return supervisor.workerControl(request)
    },
    async forumRequest(request): Promise<KhalaCodeDesktopForumResponse> {
      return fetchOpenAgentsForum(request)
    },
    async khalaCodePlanCatalog(): Promise<KhalaCodeDesktopPlanCatalogResult> {
      const planFetch = input.fetch ?? fetch
      try {
        const response = await planFetch(
          khalaCodePlanRequestUrl(khalaCodePlanBaseUrl(input.env), KHALA_CODE_PLAN_CATALOG_PATH),
          { headers: { accept: "application/json" } },
        )
        if (!response.ok) return { ok: false, error: "catalog_unavailable" }
        const payload = await response.json().catch(() => null) as unknown
        const catalog = isRecord(payload) ? payload.catalog : undefined
        return {
          ok: true,
          catalog: S.decodeUnknownSync(KhalaCodeDesktopPlanCatalogSchema)(catalog),
        }
      } catch {
        return { ok: false, error: "catalog_unavailable" }
      }
    },
    async khalaCodePlanStatus(): Promise<KhalaCodeDesktopPlanStatusResult> {
      const token = khalaCodeAgentToken(input.env)
      // No configured agent token means we honestly do not know a server-side
      // plan; the UI treats this as "Free (default)" without fabricating one.
      if (token === null) return { state: "unauthenticated" }
      const planFetch = input.fetch ?? fetch
      try {
        const response = await planFetch(
          khalaCodePlanRequestUrl(khalaCodePlanBaseUrl(input.env), KHALA_CODE_PLAN_STATUS_PATH),
          {
            headers: {
              accept: "application/json",
              authorization: `Bearer ${token}`,
            },
          },
        )
        if (response.status === 401 || response.status === 403) {
          return { state: "unauthenticated" }
        }
        if (!response.ok) return { state: "unavailable" }
        const payload = await response.json().catch(() => null) as unknown
        if (!isRecord(payload) || payload.ok !== true) return { state: "unavailable" }
        return {
          state: "ok",
          plan: S.decodeUnknownSync(KhalaCodeDesktopPlanStatusPlanSchema)(payload.plan),
        }
      } catch {
        return { state: "unavailable" }
      }
    },
    async khalaCodePlanPurchase(
      request?: KhalaCodeDesktopPlanPurchaseRequest,
    ): Promise<KhalaCodeDesktopPlanPurchaseResult> {
      const token = khalaCodeAgentToken(input.env)
      if (token === null) return { ok: false, error: "unauthenticated" }
      const planFetch = input.fetch ?? fetch
      try {
        const response = await planFetch(
          khalaCodePlanRequestUrl(khalaCodePlanBaseUrl(input.env), KHALA_CODE_PLAN_PURCHASE_PATH),
          {
            method: "POST",
            headers: {
              accept: "application/json",
              authorization: `Bearer ${token}`,
              "content-type": "application/json",
            },
            body: JSON.stringify(
              request?.idempotencyKey === undefined
                ? {}
                : { idempotencyKey: request.idempotencyKey },
            ),
          },
        )
        const payload = await response.json().catch(() => null) as unknown
        if (response.status === 401 || response.status === 403) {
          return { ok: false, error: "unauthenticated" }
        }
        if (!response.ok) {
          const errorRef = isRecord(payload) ? stringValue(payload.error) : null
          return {
            ok: false,
            error: errorRef === "khala_code_paid_plans_not_enabled"
              ? "khala_code_paid_plans_not_enabled"
              : "purchase_unavailable",
          }
        }
        return S.decodeUnknownSync(KhalaCodeDesktopPlanPurchaseSuccessSchema)(payload)
      } catch {
        return { ok: false, error: "purchase_unavailable" }
      }
    },
    async khalaCodeOutsideUserRunReport(
      request?: KhalaCodeDesktopOutsideUserRunReportRequest,
    ): Promise<KhalaCodeDesktopOutsideUserRunReportResult> {
      const reportFetch = input.fetch ?? fetch
      try {
        const [harness, pylon] = await Promise.all([
          codexHarnessStatus(),
          pylonRuntimeStatus().catch(() =>
            runtimeStatus({
              available: false,
              capability: "pylon",
              reason: "Pylon status unavailable.",
              status: "unavailable",
            }),
          ),
        ])
        const response = await reportFetch(
          khalaCodePlanRequestUrl(khalaCodePlanBaseUrl(input.env), KHALA_CODE_OUTSIDE_USER_RUNS_PATH),
          {
            method: "POST",
            headers: {
              accept: "application/json",
              "content-type": "application/json",
            },
            body: JSON.stringify({
              schemaVersion: "openagents.khala_code.outside_user_run_intake.v1",
              consent: {
                publicReceipt: true,
                noPrivateDataIncluded: true,
              },
              appVersion: khalaCodeDesktopAppVersion(input.env),
              platform: khalaCodeOutsideUserPlatform(process.platform),
              arch: khalaCodeOutsideUserArch(process.arch),
              distributionChannel: khalaCodeDistributionChannel(input.env),
              harnessReadiness: {
                codexCli: codexCliRunEvidenceState(harness),
                codexAuth: codexAuthRunEvidenceState(harness),
                pylon: pylonRunEvidenceState(pylon),
              },
              ...(request?.idempotencyKey === undefined
                ? {}
                : { idempotencyKey: request.idempotencyKey }),
            }),
          },
        )
        const payload = await response.json().catch(() => null) as unknown
        if (!response.ok) {
          return { ok: false, error: "outside_user_run_receipt_unavailable" }
        }
        return S.decodeUnknownSync(KhalaCodeDesktopOutsideUserRunReportResultSchema)(payload)
      } catch {
        return { ok: false, error: "outside_user_run_receipt_unavailable" }
      }
    },
    async claudeApprovalPending() {
      return {
        ok: true,
        requests: claudeApprovalService.pending(),
      }
    },
    async claudeApprovalRespond(request) {
      try {
        const decision = request.decision as unknown as Parameters<ClaudeApprovalService["respond"]>[1]
        const ok = await claudeApprovalService.respond(request.requestId, decision)
        return {
          ok,
          requestId: request.requestId,
          decision,
          ...(ok ? {} : { error: "Claude approval request is not pending." }),
        }
      } catch (error) {
        return {
          ok: false,
          requestId: request.requestId,
          decision: request.decision,
          error: error instanceof Error ? error.message : String(error),
        }
      }
    },
    async claudeSettingsRead() {
      const runtime = requireClaudeChatRuntime()
      if (!("claudeSettingsRead" in runtime) || typeof runtime.claudeSettingsRead !== "function") {
        return {
          ok: false,
          observedAt: new Date().toISOString(),
          errors: ["Claude settings are unavailable for the injected runtime."],
          account: {
            apiProvider: null,
            apiKeySource: null,
            email: null,
            organization: null,
            subscriptionType: null,
            tokenSource: null,
          },
          init: {
            permissionMode: input.env.KHALA_CODE_DESKTOP_CLAUDE_PERMISSION_MODE ?? "acceptEdits",
            model: null,
            system: null,
          },
          models: {
            options: [],
            selected: null,
          },
        }
      }
      return runtime.claudeSettingsRead()
    },
    async codexHarnessStatus() {
      return codexHarnessStatus()
    },
    async codexApprovalRespond(request) {
      const approvalInput = {
        action: request.action,
        method: request.method,
        ...(request.execpolicyAmendment === undefined ? {} : { execpolicyAmendment: request.execpolicyAmendment }),
        ...(request.networkPolicyAmendment === undefined ? {} : { networkPolicyAmendment: request.networkPolicyAmendment }),
        ...(request.permissions === undefined ? {} : { permissions: request.permissions }),
      } as KhalaCodeDesktopCodexApprovalResponseInput
      const payload = khalaCodeDesktopCodexApprovalResponsePayload(approvalInput)
      try {
        input.codexAppServerHost?.respondToServerRequest(
          request.requestId as KhalaCodeDesktopJsonRpcId,
          payload,
        )
        if (input.codexAppServerHost === undefined) {
          throw new Error("Codex app-server host is not configured.")
        }
        return {
          ok: true,
          method: request.method,
          payload,
          requestId: request.requestId,
        }
      } catch (error) {
        return {
          ok: false,
          method: request.method,
          payload,
          requestId: request.requestId,
          error: error instanceof Error ? error.message : String(error),
        }
      }
    },
    async codexBackgroundTerminalsClean(request) {
      return codexBackgroundTerminalsClean(request)
    },
    async codexBackgroundTerminalsList(request) {
      return codexBackgroundTerminalsList(request)
    },
    async codexBackgroundTerminalsTerminate(request) {
      return codexBackgroundTerminalsTerminate(request)
    },
    async codexConfigValueWrite(request) {
      return writeCodexConfigValue(request)
    },
    async codexEcosystemRead(request = {}) {
      return readCodexEcosystem(request)
    },
    async codexExternalAgentConfigDetect(request = {}) {
      return codexAppServerAction("externalAgentConfig/detect", {
        ...(request.cwds === undefined ? {} : { cwds: request.cwds }),
        ...(request.includeHome === undefined ? {} : { includeHome: request.includeHome }),
      })
    },
    async codexExternalAgentConfigImport(request) {
      return codexAppServerAction("externalAgentConfig/import", {
        migrationItems: request.migrationItems,
        ...(request.source === undefined ? {} : { source: request.source }),
      })
    },
    async codexExternalAgentConfigImportHistoriesRead() {
      return codexAppServerAction("externalAgentConfig/import/readHistories")
    },
    async codexFsGetMetadata(request) {
      return codexAppServerAction("fs/getMetadata", request)
    },
    async codexFsReadFile(request) {
      return codexAppServerAction("fs/readFile", request)
    },
    async codexFsWriteFile(request) {
      return codexAppServerAction("fs/writeFile", request)
    },
    async codexMarketplaceAdd(request) {
      return codexAppServerAction("marketplace/add", {
        source: request.source,
        ...(request.refName === undefined ? {} : { refName: request.refName }),
        ...(request.sparsePaths === undefined ? {} : { sparsePaths: request.sparsePaths }),
      })
    },
    async codexMarketplaceRemove(request) {
      return codexAppServerAction("marketplace/remove", request)
    },
    async codexMarketplaceUpgrade(request = {}) {
      return codexAppServerAction("marketplace/upgrade", request)
    },
    async codexMentionCandidates(request = {}) {
      try {
        return await readCodexMentionCandidates(request)
      } catch (error) {
        return {
          ok: false,
          candidates: [],
          source: request.query?.trim() ? "fuzzyFileSearch" : "fs/readDirectory",
          truncated: false,
          error: error instanceof Error ? error.message : String(error),
        }
      }
    },
    async codexMcpOauthLogin(request) {
      return codexAppServerAction("mcpServer/oauth/login", {
        name: request.server,
        ...(request.threadId === undefined ? {} : { threadId: request.threadId }),
        ...(request.scopes === undefined ? {} : { scopes: request.scopes }),
        ...(request.timeoutSecs === undefined ? {} : { timeoutSecs: request.timeoutSecs }),
      })
    },
    async codexMcpResourceRead(request) {
      return codexAppServerAction("mcpServer/resource/read", request)
    },
    async codexMcpServerReload() {
      return codexAppServerAction("config/mcpServer/reload")
    },
    async codexMcpToolCall(request) {
      return codexAppServerAction("mcpServer/tool/call", {
        threadId: request.threadId,
        server: request.server,
        tool: request.tool,
        ...(request.arguments === undefined ? {} : { arguments: request.arguments }),
        ...(request.meta === undefined ? {} : { _meta: request.meta }),
      })
    },
    async codexPluginInstall(request) {
      return codexAppServerAction("plugin/install", request)
    },
    async codexPluginUninstall(request) {
      return codexAppServerAction("plugin/uninstall", request)
    },
    async codexModelRolePresetApply(request) {
      return applyCodexModelRolePreset(request)
    },
    async codexSettingsRead(request = {}) {
      return readCodexSettings(request)
    },
    async codexSkillsConfigWrite(request) {
      return codexAppServerAction("skills/config/write", request)
    },
    async codexSkillsExtraRootsSet(request) {
      return codexAppServerAction("skills/extraRoots/set", {
        extraRoots: request.extraRoots,
      })
    },
    async codexThreadArchive(request) {
      const selection = await selectChatRuntime()
      if (selection.kind === "legacy") return unsupportedLegacyThreadLifecycle()
      return selection.runtime.archiveThread(request)
    },
    async codexThreadCompact(request) {
      const selection = await selectChatRuntime()
      if (selection.kind === "legacy") return unsupportedLegacyThreadLifecycle()
      return selection.runtime.compactThread(request)
    },
    async codexThreadDelete(request) {
      const selection = await selectChatRuntime()
      if (selection.kind === "legacy") return unsupportedLegacyThreadLifecycle()
      return selection.runtime.deleteThread(request)
    },
    async codexThreadFork(request) {
      const selection = await selectChatRuntime()
      if (selection.kind === "legacy") return unsupportedLegacyThreadLifecycle()
      return selection.runtime.forkThread(request)
    },
    async codexThreadList(request) {
      const selection = await selectChatRuntime()
      if (selection.kind === "legacy") return unsupportedLegacyThreadLifecycle()
      return selection.runtime.listThreads(request)
    },
    async codexThreadRead(request) {
      const selection = await selectChatRuntime()
      if (selection.kind === "legacy") return unsupportedLegacyThreadLifecycle()
      return selection.runtime.readThread(request)
    },
    async codexThreadRename(request) {
      const selection = await selectChatRuntime()
      if (selection.kind === "legacy") return unsupportedLegacyThreadLifecycle()
      return selection.runtime.renameThread(request)
    },
    async codexThreadResume(request) {
      const selection = await selectChatRuntime()
      if (selection.kind === "legacy") return unsupportedLegacyThreadLifecycle()
      return selection.runtime.resumeThread(request)
    },
    async codexThreadStart(request = {}) {
      const selection = await selectChatRuntime()
      if (selection.kind === "legacy") return unsupportedLegacyThreadLifecycle()
      return selection.runtime.startThread({
        cwd: input.workingDirectory,
        ...request,
      })
    },
    async codexThreadUnarchive(request) {
      const selection = await selectChatRuntime()
      if (selection.kind === "legacy") return unsupportedLegacyThreadLifecycle()
      return selection.runtime.unarchiveThread(request)
    },
    async sessionCatalog(request = {}) {
      return readKhalaCodeDesktopSessionCatalog(request, {
        claudeRuntime: requireClaudeChatRuntime(),
        codexRuntime: codexChatRuntime,
        env: input.env,
        limit: 100,
      })
    },
    async codexTurnInterrupt(request) {
      const selection = await selectChatRuntime()
      if (selection.kind === "legacy") return unsupportedLegacyThreadLifecycle()
      return selection.runtime.interruptTurn(request)
    },
    async codexTurnStart(request) {
      const selection = await selectChatRuntime()
      if (selection.kind === "legacy") {
        return labelLegacyRuntimeResponse(await legacyChatTurn({
          env: input.env,
          ...(input.emitChatTurnEvent === undefined ? {} : { onEvent: input.emitChatTurnEvent }),
          request,
          workingDirectory: request.cwd ?? input.workingDirectory,
        }))
      }
      return selection.runtime.startTurn({
        ...request,
        cwd: request.cwd ?? input.workingDirectory,
        ...(selection.modelRole === undefined ? {} : { modelRole: selection.modelRole }),
      })
    },
    async codexTurnSteer(request) {
      const selection = await selectChatRuntime()
      if (selection.kind === "legacy") return unsupportedLegacyThreadLifecycle()
      return selection.runtime.steerTurn(request)
    },
    async codingStatus() {
      const { mode: runtimeMode } = await roleRuntimeMode("coder")
      if (runtimeMode === "claude_runtime") {
        const harness = await claudeHarnessStatus()
        return runtimeStatus({
          available: harness.available,
          capability: "coding",
          reason: harness.reason,
          status: harness.available ? "ready" : "unavailable",
        })
      }
      const harness = await codexHarnessStatus()
      if (!harness.available) {
        return runtimeStatus({
          available: false,
          capability: "coding",
          reason: harness.reason,
          status: harness.status,
        })
      }
      return runtimeStatus({
        available: true,
        capability: "coding",
        reason: "Khala Code coding is gated on the local Codex harness.",
        status: "ready",
      })
    },
    async onDeviceDeciderStatus() {
      return input.onDeviceDeciderStatus()
    },
    async connectCodexAccount(accountRef: string) {
      const result = await beginCodexConnect(accountRef, { env: input.env })
      if (result.verificationUrl !== null) openExternalUrl(result.verificationUrl)
      return result
    },
    async harnessSettingRead() {
      return readKhalaCodeDesktopHarnessSetting(input.env)
    },
    async harnessSettingWrite(request) {
      return writeKhalaCodeDesktopHarnessSetting(request.mode, input.env)
    },
    async modelRoleRegistryRead(): Promise<KhalaCodeDesktopModelRoleRegistryReadResult> {
      return readKhalaCodeDesktopModelRoleRegistry(input.env)
    },
    async modelRoleRegistryWrite(
      request: KhalaCodeDesktopModelRoleRegistryWriteRequest,
    ): Promise<KhalaCodeDesktopModelRoleRegistryWriteResult> {
      if ("registry" in request) return writeKhalaCodeDesktopModelRoleRegistry(request.registry, input.env)
      return writeKhalaCodeDesktopModelRoleEntry(request.entry, input.env)
    },
    async openExternalUrl(url: string) {
      return openExternalUrl(url)
    },
    async removeCodexAccount(accountRef: string) {
      return removeCodexAccount(accountRef, { env: input.env })
    },
    async setCodexAccountPaused(request) {
      return setCodexAccountPaused(request.accountRef, request.paused, { env: input.env })
    },
    async pylonStatus() {
      return pylonRuntimeStatus()
    },
    async qaMetricSample(sample): Promise<KhalaCodeDesktopQaMetricSampleResult> {
      await input.recordQaMetricSample?.(sample)
      return {
        ok: true,
        observedAt: new Date().toISOString(),
      }
    },
    async qaMetrics() {
      return input.qaMetrics?.() ?? emptyKhalaCodeQaMetricsSnapshot()
    },
    async slashCommandDispatch(request) {
      const selection = await selectChatRuntime()
      if (selection.kind === "claude") return selection.runtime.slashCommandDispatch(request)
      return dispatchSlashAppServerCommand(request)
    },
    async slashCommandList(request = {}) {
      const selection = await selectChatRuntime()
      if (selection.kind === "claude") return selection.runtime.slashCommandList(request)
      return {
        ok: true,
        commands: khalaCodeDesktopSlashCommandsWithAvailability({
          ...(request.activeTurn === undefined ? {} : { activeTurn: request.activeTurn }),
          ...(request.debug === undefined ? {} : { debug: request.debug }),
          ...(request.platform === undefined ? {} : { platform: request.platform }),
          ...(request.sideConversation === undefined ? {} : { sideConversation: request.sideConversation }),
        }),
      }
    },
    async submitChatMessage(request) {
      return withMaterializedChatAttachments(
        request,
        async materializedRequest => {
          const selection = await selectChatRuntime()
          if (selection.kind === "codex") {
            const bridge = await maybeEnsureFleetMcpBridge()
            return withFleetMcpBridgeNote(await labelCodexHarnessResponse(await selection.runtime.startTurn({
              ...materializedRequest,
              cwd: input.workingDirectory,
              ...(selection.modelRole === undefined ? {} : { modelRole: selection.modelRole }),
            })), bridge)
          }
          if (selection.kind === "claude") {
            return selection.runtime.startTurn({
              ...materializedRequest,
              cwd: input.workingDirectory,
              ...(selection.modelRole === undefined ? {} : { modelRole: selection.modelRole }),
            })
          }
          return labelLegacyRuntimeResponse(await legacyChatTurn({
            env: input.env,
            ...(input.emitChatTurnEvent === undefined ? {} : { onEvent: input.emitChatTurnEvent }),
            request: materializedRequest,
            workingDirectory: input.workingDirectory,
          }))
        },
      )
    },
    async consumeCodexRateLimitResetCredit(request): Promise<KhalaCodeDesktopCodexRateLimitResetResult> {
      const observedAt = new Date().toISOString()
      const idempotencyKey = randomUUID()
      try {
        const accountRef = request.accountRef.trim()
        if (accountRef.length === 0) throw new Error("Codex reset accountRef is required")
        const codexHomePath = await codexHomePathForResetCredit(accountRef)
        const outcome = await (input.consumeCodexRateLimitResetCredit?.({
          accountRef,
          codexHomePath,
          idempotencyKey,
        }) ??
          consumeKhalaCodexRateLimitResetCredit({
            codexHomePath,
            env: input.env as NodeJS.ProcessEnv,
            idempotencyKey,
          }))
        return {
          ok: true,
          observedAt,
          outcome,
          status: await codexAccountsStatus(),
        }
      } catch (error) {
        return {
          ok: false,
          observedAt,
          outcome: null,
          status: await codexAccountsStatus(),
          error: error instanceof Error ? error.message : String(error),
        }
      }
    },
    async tokenAccountingStatus() {
      const status = khalaCodeDesktopTokenUsageTelemetryStatus(input.env)
      const flags = [
        ...await readKhalaCodeDesktopTokenUsageInboxFlags({ env: input.env }),
        ...await readKhalaCodeDesktopClaudeTokenUsageInboxFlags({ env: input.env }),
      ]
      if (flags.length > 0) {
        return runtimeStatus({
          available: false,
          capability: "token_accounting",
          reason: `${flags.length} token usage reporting failure flag(s) need review; latest: ${flags[flags.length - 1]?.reason ?? "unknown failure"}`,
          status: "error",
        })
      }
      return runtimeStatus({
        available: true,
        capability: "token_accounting",
        reason: status.remoteConfigured
          ? "Codex app-server token accounting and message audit records are stored locally and mirrored to OpenAgents Stats."
          : "Codex app-server token accounting and message audit records are stored locally; set a token usage bearer to mirror counts to OpenAgents Stats.",
        status: "ready",
      })
    },
    async threadTokenSummary(request?: KhalaCodeDesktopThreadTokenSummaryRequest) {
      return readKhalaCodeDesktopThreadTokenSummary({
        env: input.env,
        threadId: request?.threadId ?? null,
      })
    },
    async toolCatalog() {
      const runtimeMode = await selectedRuntimeMode()
      return khalaCodeDesktopToolCatalog({
        runtimeMode,
      })
    },
  }
}
