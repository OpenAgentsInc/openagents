// Exact usage recording for the hosted-Khala chat lane.
//
// THE GAP THIS CLOSES. `khala-hosted-runtime-dispatch.ts` drives REAL
// Gemma 4 inference on OpenAgents' own key for a mobile chat turn,
// but historically recorded NO usage and debited NO credits — so a user's
// credit balance never moved from chatting, contradicting the MVP contract
// (#8467: "$10 free per account, everything uses credits"). This module is the
// exact usage seam the dispatch calls on a completed turn. It writes one exact
// `token_usage_events` row (owner-attributed, lane `hosted_khala`) without
// pricing, charging, balance projection, or settlement.
//
// EXACT-ONLY. Token counts come straight from Gemini's `usageMetadata`
// receipt (`ArtanisMindUsage`); a turn with no usageMetadata records no row
// (the caller skips this path). `usage_truth` is always `exact`.
//
// FAIL-SOFT. Neither the token-usage insert nor the ledger charge may ever
// throw back into the dispatch loop (a recording failure must not wedge the
// turn or lose the assistant answer). Every failure is caught and surfaced as
// a typed, public-safe outcome the caller logs. The exact row is idempotent per
// turn, so a retried turn never duplicates usage.

import { Effect } from 'effect'

import type { ArtanisMindUsage } from './artanis-mind'
import { DEFAULT_GEMMA4_MODEL_ID } from './inference/gemma4-model'
import type { TokenUsageLedgerShape } from './token-usage-ledger'

/** The single lane this metering seam serves. */
export const HOSTED_KHALA_LANE = 'hosted_khala' as const

/** Exact model attribution for Sarah's hosted Gemma 4 lane. */
export const HOSTED_KHALA_PRICING_MODEL = DEFAULT_GEMMA4_MODEL_ID

/** Provider ref stamped on the exact token-usage row + metering context. */
export const HOSTED_KHALA_PROVIDER = 'google-ai-studio' as const

/** `token_usage_events.producer_system` — same as the org-cloud-runtime lane. */
export const HOSTED_KHALA_PRODUCER_SYSTEM = 'omega' as const

/** `token_usage_events.source_route` — the bounded hosted-Gemini route literal. */
export const HOSTED_KHALA_SOURCE_ROUTE = 'omega_hosted_gemini' as const

/** Demand attribution for the hosted mobile chat lane (external user demand
 * served on our own key), distinct from the org-cloud-runtime demand source. */
export const HOSTED_KHALA_DEMAND_KIND = 'external' as const
export const HOSTED_KHALA_DEMAND_SOURCE = 'khala_mobile_hosted_dispatch' as const
export const HOSTED_KHALA_DEMAND_CLIENT = 'khala-code-mobile' as const
export const HOSTED_KHALA_DEMAND_CHANNEL = 'khala_api' as const

/** The owner credit account ref convention (matches `agentRefForUser`). */
export const hostedKhalaOwnerCreditAccountRef = (ownerUserId: string): string =>
  `agent:${ownerUserId}`

const safeRefPart = (value: string): string => {
  const sanitized = value.replace(/[^A-Za-z0-9._:-]/g, '_').slice(0, 96)
  return sanitized === '' || !/^[A-Za-z0-9]/.test(sanitized) ? 'ref' : sanitized
}

// ---------------------------------------------------------------------------
// Exact usage normalization (Gemini usageMetadata -> bucketed token counts)
// ---------------------------------------------------------------------------

/** Bucketed exact token counts for one hosted turn. `outputTokens` folds the
 * thinking (reasoning) tokens into visible output, matching the org-cloud
 * runtime convention so both hosted lanes bucket usage identically. */
export type HostedTurnUsage = Readonly<{
  inputTokens: number
  outputTokens: number
  reasoningTokens: number
  cacheReadTokens: number
  totalTokens: number
}>

/** Normalize Gemini's raw `usageMetadata` counters into bucketed exact counts.
 * `null` when the counts carry no billable input/output tokens (the caller then
 * records no row — never a fabricated zero-token receipt). */
export const hostedTurnUsageFromArtanisMind = (
  usage: ArtanisMindUsage,
): HostedTurnUsage | null => {
  const inputTokens = Math.max(0, Math.trunc(usage.promptTokens))
  const reasoningTokens = Math.max(0, Math.trunc(usage.thoughtsTokens))
  const outputTokens =
    Math.max(0, Math.trunc(usage.candidatesTokens)) + reasoningTokens
  const cacheReadTokens = Math.max(0, Math.trunc(usage.cachedInputTokens))
  const minimumTotal = inputTokens + outputTokens
  if (minimumTotal <= 0) return null
  return {
    cacheReadTokens,
    inputTokens,
    outputTokens,
    reasoningTokens,
    totalTokens: Math.max(minimumTotal, Math.trunc(usage.totalTokens)),
  }
}

// ---------------------------------------------------------------------------
// Row + metering-context builders (owner-attributed, exact)
// ---------------------------------------------------------------------------

export type HostedTurnMeteringInput = Readonly<{
  ownerUserId: string
  threadId: string
  turnId: string
  usage: HostedTurnUsage
  observedAt: string
}>

/** Stable public-safe refs for a hosted turn's exact usage row. */
export const hostedKhalaUsageEventId = (turnId: string): string =>
  `event.inference.served-tokens.khala-hosted.${safeRefPart(turnId)}`
export const hostedKhalaUsageIdempotencyKey = (turnId: string): string =>
  `khala:hosted-turn:${safeRefPart(turnId)}`
export const hostedKhalaUsageRef = (turnId: string): string =>
  `usage.khala-hosted.${safeRefPart(turnId)}`

/** Build the exact `token_usage_events` ingest body for one hosted turn:
 * owner-attributed, lane `hosted_khala`, `usage_truth='exact'`, no prompt or
 * completion material. */
export const hostedKhalaTokenUsageEventBody = (
  input: HostedTurnMeteringInput,
): Record<string, unknown> => ({
  actor: {
    accountRef: hostedKhalaOwnerCreditAccountRef(input.ownerUserId),
    userId: input.ownerUserId,
  },
  backendProfile: HOSTED_KHALA_PROVIDER,
  demand: {
    demandChannel: HOSTED_KHALA_DEMAND_CHANNEL,
    demandClient: HOSTED_KHALA_DEMAND_CLIENT,
    demandKind: HOSTED_KHALA_DEMAND_KIND,
    demandSource: HOSTED_KHALA_DEMAND_SOURCE,
  },
  eventId: hostedKhalaUsageEventId(input.turnId),
  idempotencyKey: hostedKhalaUsageIdempotencyKey(input.turnId),
  model: HOSTED_KHALA_PRICING_MODEL,
  observedAt: input.observedAt,
  privacy: { leaderboardEligible: false, privacyOptOut: false },
  producerSystem: HOSTED_KHALA_PRODUCER_SYSTEM,
  provider: HOSTED_KHALA_PROVIDER,
  roleRef: 'assistant',
  safeMetadata: {
    lane: HOSTED_KHALA_LANE,
    model: HOSTED_KHALA_PRICING_MODEL,
    provider: HOSTED_KHALA_PROVIDER,
    threadId: input.threadId,
    turnId: input.turnId,
    usageBasis: 'gemini_usage_metadata',
  },
  schemaVersion: 'openagents.token_usage_event.v1',
  sourceRefs: {
    runRef: input.threadId,
    sessionRef: input.threadId,
    taskRef: input.turnId,
  },
  sourceRoute: HOSTED_KHALA_SOURCE_ROUTE,
  tokenCounts: {
    cacheReadTokens: input.usage.cacheReadTokens,
    cacheWrite1hTokens: 0,
    cacheWrite5mTokens: 0,
    inputTokens: input.usage.inputTokens,
    outputTokens: input.usage.outputTokens,
    reasoningTokens: input.usage.reasoningTokens,
    totalTokens: input.usage.totalTokens,
  },
  usageTruth: 'exact',
})

// ---------------------------------------------------------------------------
// Exact usage recorder, fail-soft
// ---------------------------------------------------------------------------

export type HostedTurnMeteringDeps = Readonly<{
  /** The token ledger (writes the exact `token_usage_events` row + rollups). */
  ledger: TokenUsageLedgerShape
  /** Structured public-safe logger for per-turn recording outcomes. */
  log?: ((line: string, fields?: Record<string, unknown>) => void) | undefined
}>

export type HostedTurnMeteringOutcome = Readonly<{
  /** Public-safe ref to the exact usage row. */
  tokenUsageEventRef: string
  /** True when THIS call inserted the exact row (false on idempotent replay). */
  insertedTokenUsage: boolean
  /** input + output tokens (the public served-token contribution). */
  tokensServed: number
  /** Stable usage ref for the emitted `usage.recorded` runtime event. */
  usageRef: string
}>

/**
 * Record ONE hosted turn's exact usage. FAIL-SOFT: never throws and never
 * prices, debits, settles, or projects a balance.
 */
export const recordHostedTurnUsage = async (
  deps: HostedTurnMeteringDeps,
  input: HostedTurnMeteringInput,
): Promise<HostedTurnMeteringOutcome> => {
  const log = deps.log ?? (() => undefined)
  const tokensServed = input.usage.inputTokens + input.usage.outputTokens
  // Exact token-usage row (fail-soft: a lost row never blocks the turn).
  let insertedTokenUsage = false
  try {
    const result = await Effect.runPromise(
      deps.ledger.ingestEvent(hostedKhalaTokenUsageEventBody(input)),
    )
    insertedTokenUsage = result.inserted
  } catch (error) {
    log('hosted_runtime_usage_ingest_failed', {
      detail: error instanceof Error ? error.message : 'unknown',
      turnId: input.turnId,
    })
  }

  return {
    insertedTokenUsage,
    tokenUsageEventRef: hostedKhalaUsageEventId(input.turnId),
    tokensServed,
    usageRef: hostedKhalaUsageRef(input.turnId),
  }
}
