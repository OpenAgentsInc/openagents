import { createHash } from "node:crypto"

import type { BootstrapSummary } from "./bootstrap.js"
import {
  defaultDirectLocalCodexUsageReportStatus,
  loadAccountUsageStore,
  resolvePylonAccountUsageRefreshTargets,
  type PylonAccountsUsageArgs,
  type PylonDirectLocalCodexUsageReportStatus,
  type PylonLocalSessionUsageObservation,
} from "./account-usage.js"
import { resolveOpenAgentsAgentToken } from "./auth.js"
import { assertPublicProjectionSafe } from "./state.js"

export const PYLON_CODEX_DIRECT_LOCAL_USAGE_REPORT_SCHEMA =
  "openagents.pylon.codex_direct_local_usage_report.v0.1"
export const PYLON_CODEX_DIRECT_LOCAL_USAGE_INGEST_SCHEMA =
  "openagents.pylon.codex_direct_local_usage.v1"
export const PYLON_CODEX_DIRECT_LOCAL_USAGE_INGEST_PATH =
  "/api/pylon/codex/local-usage"

export type PylonCodexDirectLocalUsageReportFetcher = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>

type PylonCodexDirectLocalUsageReporterOptions = {
  env?: Record<string, string | undefined>
  fetcher?: PylonCodexDirectLocalUsageReportFetcher
  now?: Date
}

type UsageDelta = {
  accountRefHash: string
  observedAt: string
  sessionRef: string | null
  inputTokens: number
  outputTokens: number
  totalTokens: number
  usageTruth: "exact" | "estimated"
}

const trueValues = new Set(["1", "true", "yes", "on"])

const isEnvEnabled = (value: string | undefined): boolean =>
  value !== undefined && trueValues.has(value.trim().toLowerCase())

export const directLocalCodexUsageReportRequested = (
  args: Pick<PylonAccountsUsageArgs, "reportLocalCodexUsage">,
  env: Record<string, string | undefined>,
): boolean =>
  args.reportLocalCodexUsage || isEnvEnabled(env.PYLON_REPORT_LOCAL_CODEX_USAGE)

const baseUrlFromEnv = (env: Record<string, string | undefined>): string =>
  (env.PYLON_OPENAGENTS_BASE_URL ?? env.OPENAGENTS_BASE_URL ?? "https://openagents.com")
    .trim()
    .replace(/\/+$/, "")

const sha256 = (value: string): string =>
  createHash("sha256").update(value).digest("hex")

const idempotencyKeyForDelta = (delta: UsageDelta): string =>
  `pylon:codex-direct-local:${sha256([
    delta.accountRefHash,
    delta.sessionRef ?? "session.none",
    delta.observedAt,
    String(delta.inputTokens),
    String(delta.outputTokens),
    String(delta.totalTokens),
    delta.usageTruth,
  ].join(":")).slice(0, 48)}`

const nonNegativeInteger = (value: number): number =>
  Number.isFinite(value) ? Math.max(0, Math.round(value)) : 0

const stableSessionKey = (
  observation: PylonLocalSessionUsageObservation,
  index: number,
): string =>
  observation.usage.sessionRef ??
  `observation:${observation.observedAt}:${index}`

const directLocalUsageDeltasForHistory = (
  accountRefHash: string,
  observations: readonly PylonLocalSessionUsageObservation[],
): UsageDelta[] => {
  const sorted = observations
    .map((observation, index) => ({
      index,
      observation,
      observedMs: Date.parse(observation.observedAt),
    }))
    .filter(entry =>
      Number.isFinite(entry.observedMs) &&
      entry.observation.usage.provider === "codex",
    )
    .sort((left, right) =>
      left.observedMs - right.observedMs || left.index - right.index,
    )

  const previousBySession = new Map<
    string,
    { inputTokens: number; outputTokens: number; totalTokens: number }
  >()
  const deltas: UsageDelta[] = []

  for (const { index, observation } of sorted) {
    const sessionKey = stableSessionKey(observation, index)
    const current = {
      inputTokens: nonNegativeInteger(observation.usage.inputTokens),
      outputTokens: nonNegativeInteger(observation.usage.outputTokens),
      totalTokens: nonNegativeInteger(observation.usage.totalTokens),
    }
    const previous = previousBySession.get(sessionKey)
    const deltaInput = previous === undefined || current.inputTokens < previous.inputTokens
      ? current.inputTokens
      : current.inputTokens - previous.inputTokens
    const deltaOutput = previous === undefined || current.outputTokens < previous.outputTokens
      ? current.outputTokens
      : current.outputTokens - previous.outputTokens
    const deltaTotal = previous === undefined || current.totalTokens < previous.totalTokens
      ? current.totalTokens
      : current.totalTokens - previous.totalTokens
    const totalOnlyFallback = deltaInput + deltaOutput === 0 && deltaTotal > 0
    const inputTokens = totalOnlyFallback ? 0 : deltaInput
    const outputTokens = totalOnlyFallback ? deltaTotal : deltaOutput
    const servedTokens = inputTokens + outputTokens

    previousBySession.set(sessionKey, current)
    if (servedTokens <= 0) continue

    deltas.push({
      accountRefHash,
      inputTokens,
      observedAt: observation.observedAt,
      outputTokens,
      sessionRef: observation.usage.sessionRef,
      totalTokens: Math.max(deltaTotal, servedTokens),
      usageTruth: totalOnlyFallback ? "estimated" : "exact",
    })
  }

  return deltas
}

const responseTokenDelta = async (response: Response): Promise<number> => {
  const body = await response.json().catch(() => null) as unknown
  if (body === null || typeof body !== "object" || Array.isArray(body)) return 0
  const value = (body as Record<string, unknown>).tokensServedDelta
  return typeof value === "number" && Number.isFinite(value) ? value : 0
}

export async function reportDirectLocalCodexUsage(
  summary: Pick<BootstrapSummary, "paths">,
  args: PylonAccountsUsageArgs,
  options: PylonCodexDirectLocalUsageReporterOptions = {},
): Promise<PylonDirectLocalCodexUsageReportStatus> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const requested = directLocalCodexUsageReportRequested(args, env)
  if (!requested) return defaultDirectLocalCodexUsageReportStatus(false)

  const token = await resolveOpenAgentsAgentToken({ env, summary })
  if (token === null) {
    return defaultDirectLocalCodexUsageReportStatus(true, [
      "blocker.pylon.codex_direct_local_usage.openagents_agent_token_missing",
    ])
  }

  const targets = await resolvePylonAccountUsageRefreshTargets(
    summary,
    args,
    { env },
  )
  const store = await loadAccountUsageStore(summary)
  const fetcher = options.fetcher ?? fetch
  const baseUrl = baseUrlFromEnv(env)
  const blockerRefs = new Set<string>()
  let sentCount = 0
  let insertedCount = 0
  let duplicateCount = 0
  let skippedCount = 0

  for (const target of targets) {
    if (target.provider !== "codex") {
      skippedCount += 1
      continue
    }
    const entry = store.accounts[target.accountRefHash]
    const history =
      entry?.localSessionUsageHistory ??
      (entry?.localSessionTruth ? [entry.localSessionTruth] : [])
    const deltas = directLocalUsageDeltasForHistory(
      target.accountRefHash,
      history,
    )
    if (deltas.length === 0) {
      skippedCount += 1
      continue
    }

    for (const delta of deltas) {
      const payload = {
        schemaVersion: PYLON_CODEX_DIRECT_LOCAL_USAGE_INGEST_SCHEMA,
        accountRefHash: delta.accountRefHash,
        idempotencyKey: idempotencyKeyForDelta(delta),
        observedAt: delta.observedAt,
        ...(delta.sessionRef === null ? {} : { sessionRef: delta.sessionRef }),
        usage: {
          inputTokens: delta.inputTokens,
          outputTokens: delta.outputTokens,
          totalTokens: delta.totalTokens,
          usageTruth: delta.usageTruth,
        },
      }

      sentCount += 1
      const response = await fetcher(
        `${baseUrl}${PYLON_CODEX_DIRECT_LOCAL_USAGE_INGEST_PATH}`,
        {
          method: "POST",
          headers: {
            accept: "application/json",
            authorization: `Bearer ${token.token}`,
            "content-type": "application/json",
          },
          body: JSON.stringify(payload),
        },
      ).catch(() => null)

      if (response === null) {
        blockerRefs.add("blocker.pylon.codex_direct_local_usage.network_failed")
        continue
      }
      if (!response.ok) {
        blockerRefs.add(`blocker.pylon.codex_direct_local_usage.http_${response.status}`)
        continue
      }
      const deltaTokens = await responseTokenDelta(response)
      if (deltaTokens > 0) insertedCount += 1
      else duplicateCount += 1
    }
  }

  const status = {
    requested: true,
    performed: sentCount > 0,
    sentCount,
    insertedCount,
    duplicateCount,
    skippedCount,
    blockerRefs: [...blockerRefs].sort(),
  } satisfies PylonDirectLocalCodexUsageReportStatus
  assertPublicProjectionSafe({
    schema: PYLON_CODEX_DIRECT_LOCAL_USAGE_REPORT_SCHEMA,
    ...status,
  })
  return status
}
