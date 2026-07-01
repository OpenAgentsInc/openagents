import { createHash } from "node:crypto"
import { appendFile, mkdir } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join } from "node:path"

export type KhalaCodeDesktopCodexTokenUsageCounts = {
  readonly cachedInputTokens: number
  readonly inputTokens: number
  readonly outputTokens: number
  readonly reasoningOutputTokens: number
  readonly totalTokens: number
}

export type KhalaCodeDesktopCodexTokenUsageReport = {
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

export type KhalaCodeDesktopCodexTokenUsageReporter = (
  report: KhalaCodeDesktopCodexTokenUsageReport,
) => Promise<void>

export type KhalaCodeDesktopTokenUsageTelemetryStatus = {
  readonly localLedgerPath: string
  readonly remoteConfigured: boolean
  readonly remoteDisabled: boolean
}

type FetchLike = (url: URL, init: RequestInit) => Promise<Response>

type TokenUsageTelemetryConfig = {
  readonly baseUrl: string
  readonly bearerToken: string | null
  readonly localLedgerPath: string
  readonly remoteDisabled: boolean
}

export type CreateKhalaCodeDesktopCodexTokenUsageReporterOptions = {
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly fetch?: FetchLike
  readonly localLedgerPath?: string
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

const resolveConfig = (
  env: Readonly<Record<string, string | undefined>>,
  localLedgerPath?: string,
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
      nonEmpty(env.PROBE_OMEGA_BEARER_TOKEN),
    localLedgerPath: localLedgerPath ?? defaultLocalLedgerPath(env),
    remoteDisabled,
  }
}

export function khalaCodeDesktopTokenUsageTelemetryStatus(
  env: Readonly<Record<string, string | undefined>>,
): KhalaCodeDesktopTokenUsageTelemetryStatus {
  const config = resolveConfig(env)
  return {
    localLedgerPath: config.localLedgerPath,
    remoteConfigured: config.bearerToken !== null && !config.remoteDisabled,
    remoteDisabled: config.remoteDisabled,
  }
}

const eventRefs = (report: KhalaCodeDesktopCodexTokenUsageReport) => {
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
  const refs = eventRefs({ ...report, usage })
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
    privacy: { leaderboardEligible: false, privacyOptOut: true },
    producerSystem: "pylon",
    provider: "pylon-codex-direct-local",
    safeMetadata: {
      agentSurface: "khala_code_desktop",
      captureMethod: "thread_token_usage_updated",
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

const appendJsonLine = async (path: string, value: unknown): Promise<void> => {
  await mkdir(dirname(path), { recursive: true })
  await appendFile(path, `${JSON.stringify(value)}\n`, "utf8")
}

const appendFailure = async (
  localLedgerPath: string,
  value: Record<string, unknown>,
): Promise<void> => {
  const failurePath = join(dirname(localLedgerPath), "token-usage-report-failures.jsonl")
  await appendJsonLine(failurePath, value)
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

    if (config.remoteDisabled || config.bearerToken === null) return

    const endpoint = new URL("/api/stats/token-usage/events", config.baseUrl)
    try {
      const response = await fetchImpl(endpoint, {
        body: JSON.stringify(event),
        headers: {
          authorization: `Bearer ${config.bearerToken}`,
          "content-type": "application/json",
        },
        method: "POST",
      })
      if (!response.ok) {
        await appendFailure(config.localLedgerPath, {
          schemaVersion: "khala-code-desktop.codex-token-usage.remote-failure.v1",
          eventId: event.eventId,
          idempotencyKey: event.idempotencyKey,
          observedAt: report.observedAt,
          status: response.status,
        })
      }
    } catch (error) {
      await appendFailure(config.localLedgerPath, {
        schemaVersion: "khala-code-desktop.codex-token-usage.remote-failure.v1",
        eventId: event.eventId,
        idempotencyKey: event.idempotencyKey,
        observedAt: report.observedAt,
        reason: error instanceof Error ? error.message : String(error),
      })
    }
  }
}
