import { createHash } from "node:crypto"
import { readFileSync } from "node:fs"
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join } from "node:path"
import { Effect } from "effect"

import type { KhalaCodeDesktopUsage } from "../shared/rpc.js"
import { khalaCodeConfigFromRuntimeEnv } from "./khala-code-config.js"
import { KhalaCodeDesktopTokenUsagePersistentFailure } from "./codex-token-usage-telemetry.js"
import type { KhalaCodeDesktopModelRoleRef } from "./codex-token-usage-telemetry.js"

type FetchLike = (url: URL, init: RequestInit) => Promise<Response>

export type KhalaCodeDesktopClaudeTokenUsageReport = {
  readonly claudeSessionId: string
  readonly desktopSessionId: string
  readonly desktopTurnId: string
  readonly model: string
  readonly observedAt: string
  readonly roleRef?: KhalaCodeDesktopModelRoleRef
  readonly sequence: number
  readonly totalCostUsd?: number
  readonly turnStatus?: string
  readonly usage: KhalaCodeDesktopUsage
}

export type KhalaCodeDesktopClaudeTokenUsageReporter = (
  report: KhalaCodeDesktopClaudeTokenUsageReport,
) => Effect.Effect<void, KhalaCodeDesktopTokenUsagePersistentFailure>

const DEFAULT_BASE_URL = "https://openagents.com"

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

const totalTokens = (usage: KhalaCodeDesktopUsage): number =>
  boundedCount(usage.input) +
  boundedCount(usage.cachedInput) +
  boundedCount(usage.output) +
  boundedCount(usage.reasoningOutput)

const nonEmpty = (value: string | undefined): string | null => {
  const trimmed = value?.trim()
  return trimmed === undefined || trimmed.length === 0 ? null : trimmed
}

const boolEnv = (value: string | undefined): boolean => {
  const normalized = value?.trim().toLowerCase()
  return normalized === "1" || normalized === "true" || normalized === "yes" || normalized === "on"
}

const secretTokenFromFile = (path: string | undefined): string | null => {
  const filePath = nonEmpty(path)
  if (filePath === null) return null
  try {
    const text = readFileSync(filePath, "utf8")
    const match = text.match(/(?:KHALA_CODE_TOKEN_USAGE_BEARER_TOKEN|OPENAGENTS_ADMIN_API_TOKEN)=([^\n\r]+)/u)
    return nonEmpty(match?.[1])
  } catch {
    return null
  }
}

const resolveConfig = (
  env: Readonly<Record<string, string | undefined>>,
  localLedgerPath: string | undefined,
) => {
  const root = join(homedir(), ".khala-code")
  return {
    baseUrl: nonEmpty(env.KHALA_CODE_TOKEN_USAGE_BASE_URL) ?? DEFAULT_BASE_URL,
    bearerToken:
      nonEmpty(env.KHALA_CODE_TOKEN_USAGE_BEARER_TOKEN) ??
      nonEmpty(env.OPENAGENTS_ADMIN_API_TOKEN) ??
      secretTokenFromFile(env.KHALA_CODE_TOKEN_USAGE_SECRET_PATH),
    localLedgerPath: localLedgerPath ?? join(root, "claude-token-usage-events.jsonl"),
    remoteDisabled: boolEnv(env.KHALA_CODE_TOKEN_USAGE_SECRET_DISABLED) ||
      boolEnv(env.KHALA_CODE_TOKEN_USAGE_REMOTE_DISABLED),
  }
}

export const khalaCodeDesktopClaudeTokenUsageEvent = (
  report: KhalaCodeDesktopClaudeTokenUsageReport,
): Record<string, unknown> => {
  const usage = {
    cachedInputTokens: boundedCount(report.usage.cachedInput),
    inputTokens: boundedCount(report.usage.input),
    outputTokens: boundedCount(report.usage.output),
    reasoningOutputTokens: boundedCount(report.usage.reasoningOutput),
    totalTokens: totalTokens(report.usage),
  }
  const eventDigest = digest({
    claudeSessionId: report.claudeSessionId,
    desktopTurnId: report.desktopTurnId,
    sequence: report.sequence,
    usage,
  }).slice(0, 24)
  const sourceDigest = digest({
    claudeSessionId: report.claudeSessionId,
    desktopSessionId: report.desktopSessionId,
  }).slice(0, 24)
  const model = report.model.trim().length === 0
    ? "openagents/claude-direct-local"
    : report.model.trim()
  const roleRef = report.roleRef ?? "coder"
  return {
    schemaVersion: "openagents.token_usage_event.v1",
    backendProfile: "claude-agent-sdk",
    demand: {
      demandChannel: "direct_local",
      demandKind: "own_capacity",
      demandSource: "direct_local_claude",
      demandClient: "khala_code_desktop",
    },
    eventId: `token_usage_event.khala_code_claude_direct_local.${eventDigest}`,
    idempotencyKey:
      `khala-code-desktop:direct-local-claude:${report.claudeSessionId}:${report.desktopTurnId}:${report.sequence}`,
    model,
    observedAt: report.observedAt,
    privacy: { leaderboardEligible: true, privacyOptOut: false },
    producerSystem: "pylon",
    provider: "pylon-claude-direct-local",
    roleRef,
    safeMetadata: {
      agentSurface: "khala_code_desktop",
      captureMethod: "claude_result_usage",
      claudeSessionId: report.claudeSessionId,
      desktopSessionId: report.desktopSessionId,
      desktopTurnId: report.desktopTurnId,
      role_ref: roleRef,
      roleRef,
      runtimeMode: "claude_agent_sdk",
      totalCostUsd: report.totalCostUsd ?? null,
      turnStatus: report.turnStatus ?? "completed",
      usageEventIndex: report.sequence,
    },
    sourceRefs: {
      anonymizedSourceRef: `claude_turn.${sourceDigest}`,
      runRef: `claude.turn.${report.desktopTurnId}`,
      sessionRef: `claude.session.${report.claudeSessionId}`,
      taskRef: `khala_code_desktop.turn.${report.desktopTurnId}`,
    },
    sourceRoute: "pylon_claude_direct_local",
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

export type KhalaCodeDesktopClaudeTokenUsageInboxFlag = {
  readonly eventId: string
  readonly idempotencyKey: string
  readonly inboxFlagRef: string
  readonly reason: string
}

export async function readKhalaCodeDesktopClaudeTokenUsageInboxFlags(
  options: {
    readonly env?: Readonly<Record<string, string | undefined>>
    readonly localLedgerPath?: string
  } = {},
): Promise<readonly KhalaCodeDesktopClaudeTokenUsageInboxFlag[]> {
  const env = options.env ?? khalaCodeConfigFromRuntimeEnv().env
  const config = resolveConfig(env, options.localLedgerPath)
  const failurePath = join(dirname(config.localLedgerPath), "claude-token-usage-report-failures.jsonl")
  try {
    const text = await readFile(failurePath, "utf8")
    return text.split(/\r?\n/u).flatMap(line => {
      if (line.trim().length === 0) return []
      try {
        const row = JSON.parse(line) as Record<string, unknown>
        const eventId = typeof row.eventId === "string" ? row.eventId : "unknown"
        const idempotencyKey = typeof row.idempotencyKey === "string" ? row.idempotencyKey : "unknown"
        const reason = typeof row.reason === "string" ? row.reason : "unknown failure"
        return [{
          eventId,
          idempotencyKey,
          inboxFlagRef: `inbox.token_usage_reporting.claude.${digest({ eventId, idempotencyKey }).slice(0, 16)}`,
          reason,
        }]
      } catch {
        return []
      }
    })
  } catch {
    return []
  }
}

export function createKhalaCodeDesktopClaudeTokenUsageReporter(
  options: {
    readonly env?: Readonly<Record<string, string | undefined>>
    readonly fetch?: FetchLike
    readonly localLedgerPath?: string
  } = {},
): KhalaCodeDesktopClaudeTokenUsageReporter {
  const env = options.env ?? khalaCodeConfigFromRuntimeEnv().env
  const config = resolveConfig(env, options.localLedgerPath)
  const fetchImpl: FetchLike = options.fetch ?? ((url, init) => globalThis.fetch(url, init))
  return report =>
    Effect.tryPromise({
      try: async () => {
        const event = khalaCodeDesktopClaudeTokenUsageEvent(report)
        await appendJsonLine(config.localLedgerPath, {
          schemaVersion: "khala-code-desktop.claude-token-usage.local.v1",
          event,
          submitted: false,
        })
        if (config.remoteDisabled || config.bearerToken === null) return
        const response = await fetchImpl(
          new URL("/api/stats/token-usage/events", config.baseUrl),
          {
            body: JSON.stringify(event),
            headers: {
              authorization: `Bearer ${config.bearerToken}`,
              "content-type": "application/json",
            },
            method: "POST",
          },
        )
        if (!response.ok) throw new Error(`Claude token usage reporting failed (${response.status})`)
        await appendJsonLine(join(dirname(config.localLedgerPath), "claude-token-usage-report-successes.jsonl"), {
          eventId: event.eventId,
          idempotencyKey: event.idempotencyKey,
          observedAt: new Date().toISOString(),
        })
      },
      catch: error => {
        const event = khalaCodeDesktopClaudeTokenUsageEvent(report)
        const reason = error instanceof Error ? error.message : String(error)
        void writeFile(
          join(dirname(config.localLedgerPath), "claude-token-usage-report-failures.jsonl"),
          `${JSON.stringify({ eventId: event.eventId, idempotencyKey: event.idempotencyKey, reason })}\n`,
          { flag: "a" },
        )
        return new KhalaCodeDesktopTokenUsagePersistentFailure({
          eventId: String(event.eventId),
          idempotencyKey: String(event.idempotencyKey),
          inboxFlagRef: `inbox.token_usage_reporting.${digest(event).slice(0, 16)}`,
          reason,
        })
      },
    })
}
