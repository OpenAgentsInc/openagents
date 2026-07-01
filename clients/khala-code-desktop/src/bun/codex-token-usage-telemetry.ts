import { createHash } from "node:crypto"
import { readFileSync } from "node:fs"
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join } from "node:path"
import { Database } from "bun:sqlite"

import type {
  KhalaCodeDesktopMessage,
  KhalaCodeDesktopMessageRole,
  KhalaCodeDesktopThreadTokenSummary,
} from "../shared/rpc.js"

export type KhalaCodeDesktopCodexTokenUsageCounts = {
  readonly cachedInputTokens: number
  readonly inputTokens: number
  readonly outputTokens: number
  readonly reasoningOutputTokens: number
  readonly totalTokens: number
}

export type KhalaCodeDesktopCodexTokenUsageReport = {
  readonly clientUserMessageId?: string
  readonly codexThreadId: string
  readonly codexTurnId: string
  readonly desktopSessionId: string
  readonly desktopTurnId: string
  readonly model: string
  readonly observedAt: string
  readonly sequence: number
  readonly turnStatus?: string
  readonly usage: KhalaCodeDesktopCodexTokenUsageCounts
}

export type KhalaCodeDesktopCodexTokenUsageEventRefs = {
  readonly anonymizedSourceRef: string
  readonly eventId: string
  readonly idempotencyKey: string
}

export type KhalaCodeDesktopCodexTokenUsageReporter = (
  report: KhalaCodeDesktopCodexTokenUsageReport,
) => Promise<void>

export type KhalaCodeDesktopCodexMessageTokenAuditMessage = {
  readonly body: string
  readonly bodyChars: number
  readonly bodySha256: string
  readonly id: string
  readonly role: KhalaCodeDesktopMessageRole
  readonly source: "codex_app_server" | "khala_code_client"
}

export type KhalaCodeDesktopCodexMessageTokenAuditUsageEvent = {
  readonly eventId: string
  readonly idempotencyKey: string
  readonly observedAt: string
  readonly sequence: number
  readonly usage: KhalaCodeDesktopCodexTokenUsageCounts
}

export type KhalaCodeDesktopCodexMessageTokenAuditRecord = {
  readonly clientUserMessage: KhalaCodeDesktopCodexMessageTokenAuditMessage
  readonly codexMessages: readonly KhalaCodeDesktopCodexMessageTokenAuditMessage[]
  readonly codexThreadId: string
  readonly codexTurnId?: string
  readonly completedAt: string
  readonly desktopSessionId: string
  readonly desktopTurnId: string
  readonly error?: string
  readonly model: string
  readonly reconciliation: {
    readonly aggregateBackfillEventId?: string
    readonly aggregateBackfillEventIds?: readonly string[]
    readonly aggregateBackfillIdempotencyKey?: string
    readonly aggregateBackfillIdempotencyKeys?: readonly string[]
    readonly globalCountedTokens: number
    readonly globalCounterRoute: "/api/stats/token-usage/events"
    readonly status:
      | "global_count_backfilled_aggregate"
      | "global_count_event_recorded"
      | "missing_token_usage_update"
    readonly tokenAccountingRequired: true
    readonly tokenScope: "codex_turn_provider_reported"
    readonly usageTruth: "exact"
  }
  readonly submittedAt: string
  readonly turnStatus: string
  readonly usage: KhalaCodeDesktopCodexTokenUsageCounts
  readonly usageEvents: readonly KhalaCodeDesktopCodexMessageTokenAuditUsageEvent[]
}

export type KhalaCodeDesktopCodexMessageTokenAuditRecorder = (
  record: KhalaCodeDesktopCodexMessageTokenAuditRecord,
) => Promise<void>

export type KhalaCodeDesktopTokenUsageTelemetryStatus = {
  readonly localMessageAuditLedgerPath: string
  readonly localLedgerPath: string
  readonly remoteConfigured: boolean
  readonly remoteDisabled: boolean
}

type FetchLike = (url: URL, init: RequestInit) => Promise<Response>

type TokenUsageTelemetryConfig = {
  readonly baseUrl: string
  readonly bearerToken: string | null
  readonly codexStateDbPath: string
  readonly localMessageAuditLedgerPath: string
  readonly localLedgerPath: string
  readonly remoteDisabled: boolean
}

export type KhalaCodeDesktopTokenUsageSyncResult = {
  readonly attempted: number
  readonly failed: number
  readonly ok: boolean
  readonly remoteConfigured: boolean
  readonly remoteDisabled: boolean
  readonly synced: number
}

export type CreateKhalaCodeDesktopCodexTokenUsageReporterOptions = {
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly fetch?: FetchLike
  readonly localLedgerPath?: string
}

export type SyncKhalaCodeDesktopPendingTokenUsageReportsOptions = {
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly fetch?: FetchLike
  readonly localLedgerPath?: string
}

export type CreateKhalaCodeDesktopCodexMessageTokenAuditRecorderOptions = {
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly localMessageAuditLedgerPath?: string
}

const DEFAULT_BASE_URL = "https://openagents.com"

const nonEmpty = (value: string | undefined): string | null => {
  const trimmed = value?.trim()
  return trimmed === undefined || trimmed.length === 0 ? null : trimmed
}

const boolEnv = (value: string | undefined): boolean => {
  const normalized = value?.trim().toLowerCase()
  return normalized === "1" ||
    normalized === "true" ||
    normalized === "yes" ||
    normalized === "on"
}

const stableJson = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${stableJson(item)}`)
      .join(",")}}`
  }
  return JSON.stringify(value)
}

const digest = (value: unknown): string =>
  createHash("sha256").update(stableJson(value)).digest("hex")

const textDigest = (value: string): string =>
  createHash("sha256").update(value).digest("hex")

const boundedCount = (value: number): number =>
  Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0

const normalizeUsage = (
  usage: KhalaCodeDesktopCodexTokenUsageCounts,
): KhalaCodeDesktopCodexTokenUsageCounts => ({
  cachedInputTokens: boundedCount(usage.cachedInputTokens),
  inputTokens: boundedCount(usage.inputTokens),
  outputTokens: boundedCount(usage.outputTokens),
  reasoningOutputTokens: boundedCount(usage.reasoningOutputTokens),
  totalTokens: boundedCount(usage.totalTokens),
})

const hasUsage = (usage: KhalaCodeDesktopCodexTokenUsageCounts): boolean =>
  usage.inputTokens > 0 ||
  usage.outputTokens > 0 ||
  usage.reasoningOutputTokens > 0 ||
  usage.totalTokens > 0

const defaultLocalLedgerPath = (
  env: Readonly<Record<string, string | undefined>>,
): string => nonEmpty(env.KHALA_CODE_TOKEN_USAGE_LOCAL_LEDGER_PATH) ??
  join(homedir(), ".khala-code", "token-usage-events.jsonl")

const defaultLocalMessageAuditLedgerPath = (
  env: Readonly<Record<string, string | undefined>>,
): string =>
  nonEmpty(env.KHALA_CODE_MESSAGE_TOKEN_AUDIT_LOCAL_LEDGER_PATH) ??
  nonEmpty(env.KHALA_CODE_MESSAGE_AUDIT_LOCAL_LEDGER_PATH) ??
  join(homedir(), ".khala-code", "message-token-audit.jsonl")

const defaultCodexStateDbPath = (
  env: Readonly<Record<string, string | undefined>>,
): string =>
  nonEmpty(env.KHALA_CODE_CODEX_STATE_DB_PATH) ??
  nonEmpty(env.CODEX_STATE_DB_PATH) ??
  join(homedir(), ".codex", "state_5.sqlite")

const unquoteEnvValue = (value: string): string => {
  const trimmed = value.trim()
  if (trimmed.length >= 2) {
    const first = trimmed[0]
    const last = trimmed[trimmed.length - 1]
    if ((first === "\"" && last === "\"") || (first === "'" && last === "'")) {
      return trimmed.slice(1, -1)
    }
  }
  return trimmed
}

const readEnvFileValue = (path: string, key: string): string | null => {
  let text: string
  try {
    text = readFileSync(path, "utf8")
  } catch {
    return null
  }

  for (const line of text.split(/\r?\n/)) {
    const match = line.match(/^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$/)
    if (match === null || match[1] !== key) continue
    return nonEmpty(unquoteEnvValue(match[2] ?? ""))
  }
  return null
}

const tokenUsageSecretPaths = (
  env: Readonly<Record<string, string | undefined>>,
): readonly string[] => {
  const explicit = nonEmpty(env.KHALA_CODE_TOKEN_USAGE_SECRET_PATH)
  if (explicit !== null) return [explicit]
  if (boolEnv(env.KHALA_CODE_TOKEN_USAGE_SECRET_DISABLED)) return []
  return [join(homedir(), "work", ".secrets", "vortex-admin.env")]
}

const tokenUsageBearerFromLocalSecret = (
  env: Readonly<Record<string, string | undefined>>,
): string | null => {
  for (const path of tokenUsageSecretPaths(env)) {
    for (const key of [
      "KHALA_CODE_TOKEN_USAGE_BEARER_TOKEN",
      "OPENAGENTS_ADMIN_API_TOKEN",
      "PROBE_TOKEN_USAGE_BEARER_TOKEN",
    ]) {
      const value = readEnvFileValue(path, key)
      if (value !== null) return value
    }
  }
  return null
}

const resolveConfig = (
  env: Readonly<Record<string, string | undefined>>,
  localLedgerPath?: string,
  localMessageAuditLedgerPath?: string,
): TokenUsageTelemetryConfig => {
  const remoteDisabled = boolEnv(env.KHALA_CODE_TOKEN_USAGE_REMOTE_DISABLED) ||
    boolEnv(env.KHALA_CODE_TOKEN_USAGE_DISABLED)
  return {
    baseUrl: nonEmpty(env.KHALA_CODE_TOKEN_USAGE_BASE_URL) ??
      nonEmpty(env.PYLON_OPENAGENTS_BASE_URL) ??
      nonEmpty(env.OPENAGENTS_BASE_URL) ??
      nonEmpty(env.PROBE_OMEGA_BASE_URL) ??
      DEFAULT_BASE_URL,
    bearerToken: nonEmpty(env.KHALA_CODE_TOKEN_USAGE_BEARER_TOKEN) ??
      nonEmpty(env.OPENAGENTS_ADMIN_API_TOKEN) ??
      nonEmpty(env.PROBE_TOKEN_USAGE_BEARER_TOKEN) ??
      tokenUsageBearerFromLocalSecret(env),
    codexStateDbPath: defaultCodexStateDbPath(env),
    localLedgerPath: localLedgerPath ?? defaultLocalLedgerPath(env),
    localMessageAuditLedgerPath:
      localMessageAuditLedgerPath ?? defaultLocalMessageAuditLedgerPath(env),
    remoteDisabled,
  }
}

export function khalaCodeDesktopTokenUsageTelemetryStatus(
  env: Readonly<Record<string, string | undefined>>,
): KhalaCodeDesktopTokenUsageTelemetryStatus {
  const config = resolveConfig(env)
  return {
    localMessageAuditLedgerPath: config.localMessageAuditLedgerPath,
    localLedgerPath: config.localLedgerPath,
    remoteConfigured: config.bearerToken !== null && !config.remoteDisabled,
    remoteDisabled: config.remoteDisabled,
  }
}

type JsonRecord = Record<string, unknown>

const isJsonRecord = (value: unknown): value is JsonRecord =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const objectField = (
  value: JsonRecord,
  key: string,
): JsonRecord | null => isJsonRecord(value[key]) ? value[key] : null

const arrayField = (
  value: JsonRecord,
  key: string,
): readonly unknown[] => Array.isArray(value[key]) ? value[key] : []

const stringField = (
  value: JsonRecord,
  key: string,
): string | null => typeof value[key] === "string" ? value[key] : null

const numberField = (
  value: JsonRecord,
  key: string,
): number => typeof value[key] === "number" ? boundedCount(value[key]) : 0

const readJsonLines = async (path: string): Promise<readonly JsonRecord[]> => {
  let text = ""
  try {
    text = await readFile(path, "utf8")
  } catch (error) {
    if (
      isJsonRecord(error) &&
      typeof error.code === "string" &&
      error.code === "ENOENT"
    ) {
      return []
    }
    throw error
  }

  const rows: JsonRecord[] = []
  for (const line of text.split(/\r?\n/)) {
    if (line.trim().length === 0) continue
    try {
      const parsed = JSON.parse(line)
      if (isJsonRecord(parsed)) rows.push(parsed)
    } catch {
      // Keep the counter usable if a local ledger has a partial trailing write.
    }
  }
  return rows
}

const refSetFromRows = (rows: readonly JsonRecord[]): ReadonlySet<string> => {
  const refs = new Set<string>()
  for (const row of rows) {
    const eventId = stringField(row, "eventId")
    const idempotencyKey = stringField(row, "idempotencyKey")
    if (eventId !== null) refs.add(eventId)
    if (idempotencyKey !== null) refs.add(idempotencyKey)
  }
  return refs
}

const refsFromEventLike = (value: JsonRecord): readonly string[] => {
  const refs: string[] = []
  const eventId = stringField(value, "eventId")
  const idempotencyKey = stringField(value, "idempotencyKey")
  if (eventId !== null) refs.push(eventId)
  if (idempotencyKey !== null) refs.push(idempotencyKey)
  return refs
}

const usageLedgerCountedTokens = (tokenCounts: JsonRecord): number => {
  const inputTokens = numberField(tokenCounts, "inputTokens")
  const outputTokens = numberField(tokenCounts, "outputTokens")
  const countedTokens = inputTokens + outputTokens
  if (countedTokens > 0) return countedTokens
  return numberField(tokenCounts, "totalTokens")
}

const isSyncedUsageRef = (
  refs: readonly string[],
  successRefs: ReadonlySet<string>,
): boolean =>
  refs.length > 0 &&
  refs.every(ref => successRefs.has(ref))

const newerIso = (left: string | null, right: string | null): string | null => {
  if (right === null) return left
  if (left === null) return right
  const leftTime = Date.parse(left)
  const rightTime = Date.parse(right)
  if (!Number.isFinite(leftTime)) return right
  if (!Number.isFinite(rightTime)) return left
  return rightTime > leftTime ? right : left
}

const isoFromUnixMs = (value: unknown): string | null => {
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) return null
  return new Date(Math.trunc(value)).toISOString()
}

const isoFromUnixSeconds = (value: unknown): string | null => {
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) return null
  return new Date(Math.trunc(value * 1_000)).toISOString()
}

export const readKhalaCodeDesktopCodexStateThreadTokenSnapshot = (options: {
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly dbPath?: string
  readonly threadId: string
}): {
  readonly tokens: number
  readonly updatedAt: string | null
} => {
  const env = options.env ?? process.env
  const dbPath = options.dbPath ?? defaultCodexStateDbPath(env)
  let db: Database | null = null
  try {
    db = new Database(dbPath, { readonly: true })
    const row = db.query<{
      readonly tokens_used: number
      readonly updated_at: number | null
      readonly updated_at_ms: number | null
    }, [string]>(
      "select tokens_used, updated_at, updated_at_ms from threads where id = ?",
    ).get(options.threadId)
    if (row === null) return { tokens: 0, updatedAt: null }
    return {
      tokens: boundedCount(row.tokens_used),
      updatedAt: isoFromUnixMs(row.updated_at_ms) ?? isoFromUnixSeconds(row.updated_at),
    }
  } catch {
    return { tokens: 0, updatedAt: null }
  } finally {
    db?.close()
  }
}

export async function readKhalaCodeDesktopThreadTokenSummary(options: {
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly localLedgerPath?: string
  readonly localMessageAuditLedgerPath?: string
  readonly threadId?: string | null
}): Promise<KhalaCodeDesktopThreadTokenSummary> {
  const env = options.env ?? process.env
  const config = resolveConfig(
    env,
    options.localLedgerPath,
    options.localMessageAuditLedgerPath,
  )
  const threadId = nonEmpty(options.threadId ?? undefined)
  const remoteConfigured = config.bearerToken !== null && !config.remoteDisabled
  const emptySummary = (): KhalaCodeDesktopThreadTokenSummary => ({
    auditRows: 0,
    codexStateDbPath: config.codexStateDbPath,
    codexStateTokens: 0,
    leaderboardLabel: "OpenAgents Stats",
    leaderboardSyncedTokens: 0,
    localLedgerPath: config.localLedgerPath,
    localMessageAuditLedgerPath: config.localMessageAuditLedgerPath,
    missingUsageTurns: 0,
    ok: true,
    pendingSyncTokens: 0,
    remoteConfigured,
    remoteDisabled: config.remoteDisabled,
    threadId,
    totalTokens: 0,
    updatedAt: null,
    usageEventRows: 0,
  })
  if (threadId === null) return emptySummary()

  const [auditRows, usageRows, successRows] = await Promise.all([
    readJsonLines(config.localMessageAuditLedgerPath),
    readJsonLines(config.localLedgerPath),
    readJsonLines(join(dirname(config.localLedgerPath), "token-usage-report-successes.jsonl")),
  ])
  const successRefs = refSetFromRows(successRows)
  const auditedUsageRefs = new Set<string>()
  let auditRowCount = 0
  let usageEventRowCount = 0
  let missingUsageTurns = 0
  let totalTokens = 0
  let leaderboardSyncedTokens = 0
  let updatedAt: string | null = null
  const codexState = readKhalaCodeDesktopCodexStateThreadTokenSnapshot({
    dbPath: config.codexStateDbPath,
    threadId,
  })

  for (const row of auditRows) {
    const record = objectField(row, "record")
    if (record === null || stringField(record, "codexThreadId") !== threadId) continue

    auditRowCount += 1
    updatedAt = newerIso(
      updatedAt,
      stringField(record, "completedAt") ?? stringField(row, "recordedAt"),
    )

    const reconciliation = objectField(record, "reconciliation")
    const countedTokens = reconciliation === null
      ? 0
      : numberField(reconciliation, "globalCountedTokens")
    totalTokens += countedTokens

    const usageEventRefs = arrayField(record, "usageEvents")
      .filter(isJsonRecord)
      .flatMap(refsFromEventLike)
    for (const ref of usageEventRefs) auditedUsageRefs.add(ref)

    const status = reconciliation === null
      ? null
      : stringField(reconciliation, "status")
    if (status === "missing_token_usage_update") missingUsageTurns += 1
    if (status === "global_count_backfilled_aggregate") {
      leaderboardSyncedTokens += countedTokens
    } else if (isSyncedUsageRef(usageEventRefs, successRefs)) {
      leaderboardSyncedTokens += countedTokens
    }
  }

  for (const row of usageRows) {
    const event = objectField(row, "event")
    if (event === null) continue
    const metadata = objectField(event, "safeMetadata")
    if (metadata === null || stringField(metadata, "codexThreadId") !== threadId) continue

    usageEventRowCount += 1
    const refs = refsFromEventLike(event)
    if (refs.some(ref => auditedUsageRefs.has(ref))) continue

    const tokenCounts = objectField(event, "tokenCounts")
    if (tokenCounts === null) continue
    const countedTokens = usageLedgerCountedTokens(tokenCounts)
    if (countedTokens === 0) continue

    totalTokens += countedTokens
    updatedAt = newerIso(
      updatedAt,
      stringField(event, "observedAt") ?? stringField(row, "recordedAt"),
    )
    if (isSyncedUsageRef(refs, successRefs)) {
      leaderboardSyncedTokens += countedTokens
    }
  }

  // Codex state is global Codex history for the thread. It is useful as a
  // diagnostic, but it is not Khala Code provenance. Only rows that Khala
  // recorded locally may count as local/pending leaderboard usage.
  if (auditRowCount > 0 || usageEventRowCount > 0) {
    updatedAt = newerIso(updatedAt, codexState.updatedAt)
  }

  return {
    auditRows: auditRowCount,
    codexStateDbPath: config.codexStateDbPath,
    codexStateTokens: codexState.tokens,
    leaderboardLabel: "OpenAgents Stats",
    leaderboardSyncedTokens,
    localLedgerPath: config.localLedgerPath,
    localMessageAuditLedgerPath: config.localMessageAuditLedgerPath,
    missingUsageTurns,
    ok: true,
    pendingSyncTokens: Math.max(0, totalTokens - leaderboardSyncedTokens),
    remoteConfigured,
    remoteDisabled: config.remoteDisabled,
    threadId,
    totalTokens,
    updatedAt,
    usageEventRows: usageEventRowCount,
  }
}

export const khalaCodeDesktopCodexTokenUsageEventRefs = (
  report: KhalaCodeDesktopCodexTokenUsageReport,
): KhalaCodeDesktopCodexTokenUsageEventRefs => {
  const eventDigest = digest({
    codexThreadId: report.codexThreadId,
    codexTurnId: report.codexTurnId,
    desktopTurnId: report.desktopTurnId,
    sequence: report.sequence,
    usage: report.usage,
  }).slice(0, 24)
  const sourceDigest = digest({
    codexThreadId: report.codexThreadId,
    codexTurnId: report.codexTurnId,
  }).slice(0, 24)

  return {
    anonymizedSourceRef: `codex_turn.${sourceDigest}`,
    eventId: `token_usage_event.khala_code_direct_local.${eventDigest}`,
    idempotencyKey:
      `khala-code-desktop:direct-local-codex:${report.codexThreadId}:${report.codexTurnId}:${report.sequence}`,
  }
}

export function khalaCodeDesktopCodexTokenUsageEvent(
  report: KhalaCodeDesktopCodexTokenUsageReport,
): Record<string, unknown> {
  const usage = normalizeUsage(report.usage)
  const refs = khalaCodeDesktopCodexTokenUsageEventRefs({ ...report, usage })
  return {
    schemaVersion: "openagents.token_usage_event.v1",
    backendProfile: "codex-app-server",
    demand: {
      demandChannel: "direct_local",
      demandKind: "own_capacity",
      demandSource: "direct_local_codex",
      demandClient: "khala_code_desktop",
    },
    eventId: refs.eventId,
    idempotencyKey: refs.idempotencyKey,
    model: report.model.trim().length === 0
      ? "openagents/codex-direct-local"
      : report.model.trim(),
    observedAt: report.observedAt,
    privacy: { leaderboardEligible: true, privacyOptOut: false },
    producerSystem: "pylon",
    provider: "pylon-codex-direct-local",
    safeMetadata: {
      agentSurface: "khala_code_desktop",
      captureMethod: "thread_token_usage_updated",
      ...(report.clientUserMessageId === undefined
        ? {}
        : { clientUserMessageId: report.clientUserMessageId }),
      codexThreadId: report.codexThreadId,
      codexTurnId: report.codexTurnId,
      desktopTurnId: report.desktopTurnId,
      runtimeMode: "codex_app_server",
      turnStatus: report.turnStatus ?? "inProgress",
      usageEventIndex: report.sequence,
    },
    sourceRefs: {
      anonymizedSourceRef: refs.anonymizedSourceRef,
      runRef: `codex.turn.${report.codexTurnId}`,
      sessionRef: `codex.thread.${report.codexThreadId}`,
      taskRef: `khala_code_desktop.turn.${report.desktopTurnId}`,
    },
    sourceRoute: "pylon_codex_direct_local",
    tokenCounts: {
      cacheReadTokens: usage.cachedInputTokens,
      cacheWrite1hTokens: 0,
      cacheWrite5mTokens: 0,
      inputTokens: usage.inputTokens,
      outputTokens: usage.outputTokens,
      reasoningTokens: usage.reasoningOutputTokens,
      totalTokens: usage.totalTokens,
    },
    usageTruth: "exact",
  }
}

export function khalaCodeDesktopCodexMessageTokenAuditMessage(
  message: KhalaCodeDesktopMessage,
  source: KhalaCodeDesktopCodexMessageTokenAuditMessage["source"],
): KhalaCodeDesktopCodexMessageTokenAuditMessage {
  return {
    body: message.body,
    bodyChars: message.body.length,
    bodySha256: textDigest(message.body),
    id: message.id,
    role: message.role,
    source,
  }
}

const appendJsonLine = async (path: string, value: unknown): Promise<void> => {
  await mkdir(dirname(path), { recursive: true })
  await appendFile(path, `${JSON.stringify(value)}\n`, "utf8")
}

const failureLedgerPath = (localLedgerPath: string): string =>
  join(dirname(localLedgerPath), "token-usage-report-failures.jsonl")

const successLedgerPath = (localLedgerPath: string): string =>
  join(dirname(localLedgerPath), "token-usage-report-successes.jsonl")

const appendSuccess = async (
  localLedgerPath: string,
  value: Record<string, unknown>,
): Promise<void> => {
  await appendJsonLine(successLedgerPath(localLedgerPath), value)
}

const postTokenUsageEvent = async (
  fetchImpl: FetchLike,
  endpoint: URL,
  bearerToken: string,
  event: JsonRecord,
): Promise<Response> =>
  fetchImpl(endpoint, {
    body: JSON.stringify(event),
    headers: {
      authorization: `Bearer ${bearerToken}`,
      "content-type": "application/json",
    },
    method: "POST",
  })

const tokenUsageEventForRemotePost = (event: JsonRecord): JsonRecord => ({
  ...event,
  privacy: { leaderboardEligible: true, privacyOptOut: false },
})

const rewriteJsonLines = async (
  path: string,
  rows: readonly JsonRecord[],
): Promise<void> => {
  await mkdir(dirname(path), { recursive: true })
  await writeFile(
    path,
    rows.length === 0 ? "" : `${rows.map(row => JSON.stringify(row)).join("\n")}\n`,
    "utf8",
  )
}

const compactFailureRows = (
  rows: readonly JsonRecord[],
  successRefs: ReadonlySet<string>,
): readonly JsonRecord[] => {
  const byKey = new Map<string, JsonRecord>()
  for (const row of rows) {
    const refs = refsFromEventLike(row)
    if (refs.length === 0 || refs.some(ref => successRefs.has(ref))) continue
    byKey.set(refs.join("\0"), row)
  }
  return [...byKey.values()]
}

const syncPendingTokenUsageReports = async (
  config: TokenUsageTelemetryConfig,
  fetchImpl: FetchLike,
  endpoint: URL,
): Promise<KhalaCodeDesktopTokenUsageSyncResult> => {
  const remoteConfigured = config.bearerToken !== null && !config.remoteDisabled
  if (!remoteConfigured) {
    return {
      attempted: 0,
      failed: 0,
      ok: false,
      remoteConfigured,
      remoteDisabled: config.remoteDisabled,
      synced: 0,
    }
  }

  const usageRows = await readJsonLines(config.localLedgerPath)
  const existingSuccessRows = await readJsonLines(successLedgerPath(config.localLedgerPath))
  const successRefs = new Set(refSetFromRows(existingSuccessRows))
  const failurePath = failureLedgerPath(config.localLedgerPath)
  const failureRows = await readJsonLines(failurePath)
  const nextFailureRows: JsonRecord[] = [...failureRows]
  let attempted = 0
  let failed = 0
  let synced = 0

  for (const row of usageRows) {
    const event = objectField(row, "event")
    if (event === null) continue
    const refs = refsFromEventLike(event)
    if (refs.length === 0 || refs.every(ref => successRefs.has(ref))) continue

    attempted += 1
    try {
      const response = await postTokenUsageEvent(
        fetchImpl,
        endpoint,
        config.bearerToken,
        tokenUsageEventForRemotePost(event),
      )
      if (response.ok) {
        await appendSuccess(config.localLedgerPath, {
          schemaVersion: "khala-code-desktop.codex-token-usage.remote-success.v1",
          eventId: stringField(event, "eventId"),
          idempotencyKey: stringField(event, "idempotencyKey"),
          observedAt: stringField(event, "observedAt") ?? stringField(row, "recordedAt"),
          status: response.status,
          syncedAt: new Date().toISOString(),
        })
        for (const ref of refs) successRefs.add(ref)
        synced += 1
      } else {
        failed += 1
        nextFailureRows.push({
          schemaVersion: "khala-code-desktop.codex-token-usage.remote-failure.v1",
          eventId: stringField(event, "eventId"),
          idempotencyKey: stringField(event, "idempotencyKey"),
          observedAt: stringField(event, "observedAt") ?? stringField(row, "recordedAt"),
          status: response.status,
        })
      }
    } catch (error) {
      failed += 1
      nextFailureRows.push({
        schemaVersion: "khala-code-desktop.codex-token-usage.remote-failure.v1",
        eventId: stringField(event, "eventId"),
        idempotencyKey: stringField(event, "idempotencyKey"),
        observedAt: stringField(event, "observedAt") ?? stringField(row, "recordedAt"),
        reason: error instanceof Error ? error.message : String(error),
      })
    }
  }

  await rewriteJsonLines(
    failurePath,
    compactFailureRows(nextFailureRows, successRefs),
  )
  return {
    attempted,
    failed,
    ok: failed === 0,
    remoteConfigured,
    remoteDisabled: config.remoteDisabled,
    synced,
  }
}

export async function syncKhalaCodeDesktopPendingTokenUsageReports(
  options: SyncKhalaCodeDesktopPendingTokenUsageReportsOptions = {},
): Promise<KhalaCodeDesktopTokenUsageSyncResult> {
  const env = options.env ?? process.env
  const config = resolveConfig(env, options.localLedgerPath)
  const fetchImpl: FetchLike = options.fetch ?? ((url, init) => globalThis.fetch(url, init))
  const endpoint = new URL("/api/stats/token-usage/events", config.baseUrl)
  return syncPendingTokenUsageReports(config, fetchImpl, endpoint)
}

export function createKhalaCodeDesktopCodexTokenUsageReporter(
  options: CreateKhalaCodeDesktopCodexTokenUsageReporterOptions = {},
): KhalaCodeDesktopCodexTokenUsageReporter {
  const env = options.env ?? process.env
  const config = resolveConfig(env, options.localLedgerPath)
  const fetchImpl: FetchLike = options.fetch ?? ((url, init) => globalThis.fetch(url, init))

  return async report => {
    const usage = normalizeUsage(report.usage)
    if (!hasUsage(usage)) return
    const normalizedReport = { ...report, usage }
    const event = khalaCodeDesktopCodexTokenUsageEvent(normalizedReport)
    await appendJsonLine(config.localLedgerPath, {
      schemaVersion: "khala-code-desktop.codex-token-usage.local.v1",
      recordedAt: new Date().toISOString(),
      event,
    })

    const endpoint = new URL("/api/stats/token-usage/events", config.baseUrl)
    await syncPendingTokenUsageReports(config, fetchImpl, endpoint)
  }
}

export function createKhalaCodeDesktopCodexMessageTokenAuditRecorder(
  options: CreateKhalaCodeDesktopCodexMessageTokenAuditRecorderOptions = {},
): KhalaCodeDesktopCodexMessageTokenAuditRecorder {
  const env = options.env ?? process.env
  const config = resolveConfig(
    env,
    undefined,
    options.localMessageAuditLedgerPath,
  )
  return async record => {
    await appendJsonLine(config.localMessageAuditLedgerPath, {
      schemaVersion: "khala-code-desktop.codex-message-token-audit.local.v1",
      recordedAt: new Date().toISOString(),
      record,
    })
  }
}
