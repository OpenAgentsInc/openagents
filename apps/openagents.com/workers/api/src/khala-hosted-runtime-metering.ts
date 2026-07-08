// Exact usage metering + credit debit for the hosted-Khala chat lane (#8555).
//
// THE GAP THIS CLOSES. `khala-hosted-runtime-dispatch.ts` drives REAL
// `gemini-3.5-flash` inference on OpenAgents' own key for a mobile chat turn,
// but historically recorded NO usage and debited NO credits — so a user's
// credit balance never moved from chatting, contradicting the MVP contract
// (#8467: "$10 free per account, everything uses credits"). This module is the
// exact-only money seam the dispatch calls on a completed turn: it writes one
// exact `token_usage_events` row (owner-attributed, lane `hosted_khala`) AND
// debits the owner's credit balance through the SAME live metering hook the
// `/v1/chat/completions` and org-cloud-runtime lanes use, so the
// `scope.user.<userId>` `credit_balance` projection updates and fans out live.
//
// RATE SOURCE (no invented pricing). The charge is computed by the existing
// pure pricing engine (`priceRequest`) against the published `gemini-3.5-flash`
// row — the single Gemini-Flash rate the pricing table exposes for hosted Khala
// Flash inference (the same row the Autopilot Concierge / free-tier price
// against). The actual Gemini generation the dispatch calls is the Gemini Flash
// lane; `gemini-3.5-flash` is its published pricing/attribution alias. Nothing
// here defines a new rate; it reuses `makeLedgerMeteringHook` end to end.
//
// EXACT-ONLY. Token counts come straight from Gemini's `usageMetadata`
// receipt (`ArtanisMindUsage`); a turn with no usageMetadata records no row
// (the caller skips this path). `usage_truth` is always `exact`.
//
// FAIL-SOFT. Neither the token-usage insert nor the ledger charge may ever
// throw back into the dispatch loop (a metering failure must not wedge the
// turn or lose the assistant answer). Every failure is caught and surfaced as
// a typed, public-safe outcome the caller logs. The ledger charge is
// idempotent per turn (`inference:charge:khala-hosted.<turnId>`), so a retried
// turn never double-charges.

import { Effect } from 'effect'

import type { ArtanisMindUsage } from './artanis-mind'
import {
  inferenceChargeReceiptRef,
  type MeteringContext,
  type MeteringHook,
} from './inference/metering-hook'
import { priceRequest } from './inference/pricing'
import type { TokenUsageLedgerShape } from './token-usage-ledger'

/** The single lane this metering seam serves. */
export const HOSTED_KHALA_LANE = 'hosted_khala' as const

/** Published pricing/attribution alias for the hosted Gemini-Flash lane. The
 * ONE Gemini-Flash row the pricing table exposes; the charge re-solves from it,
 * never a new rate. */
export const HOSTED_KHALA_PRICING_MODEL = 'gemini-3.5-flash' as const

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

/** Stable public-safe refs for a hosted turn's exact usage row + charge. */
export const hostedKhalaUsageEventId = (turnId: string): string =>
  `event.inference.served-tokens.khala-hosted.${safeRefPart(turnId)}`
export const hostedKhalaUsageIdempotencyKey = (turnId: string): string =>
  `khala:hosted-turn:${safeRefPart(turnId)}`
/** Stable per-turn request id for the metering charge (feeds
 * `inferenceChargeIdempotencyKey`, so a retried turn never double-charges). */
export const hostedKhalaChargeRequestId = (turnId: string): string =>
  `khala-hosted.${safeRefPart(turnId)}`
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

/** Build the `MeteringContext` the live ledger hook charges + projects from. */
export const hostedKhalaMeteringContext = (
  input: HostedTurnMeteringInput,
): MeteringContext => ({
  accountRef: hostedKhalaOwnerCreditAccountRef(input.ownerUserId),
  adapterId: HOSTED_KHALA_PROVIDER,
  fundingKind: 'card',
  requestId: hostedKhalaChargeRequestId(input.turnId),
  requestedModel: HOSTED_KHALA_PRICING_MODEL,
  servedModel: HOSTED_KHALA_PRICING_MODEL,
  streamed: false,
  usage: {
    ...(input.usage.cacheReadTokens === 0
      ? {}
      : { cachedPromptTokens: input.usage.cacheReadTokens }),
    completionTokens: input.usage.outputTokens,
    promptTokens: input.usage.inputTokens,
    totalTokens: input.usage.totalTokens,
  },
})

/** The priced charge for a hosted turn, in USD cents (display/logging only —
 * the authoritative debit is the ledger hook's msat decrement). */
export const hostedKhalaChargeUsdCents = (usage: HostedTurnUsage): number => {
  const chargeUsd = priceRequest({
    fundingKind: 'card',
    model: HOSTED_KHALA_PRICING_MODEL,
    usage: {
      ...(usage.cacheReadTokens === 0
        ? {}
        : { cachedPromptTokens: usage.cacheReadTokens }),
      completionTokens: usage.outputTokens,
      promptTokens: usage.inputTokens,
      totalTokens: usage.totalTokens,
    },
  }).chargeUsd
  return Math.max(0, Math.round(chargeUsd * 100))
}

// ---------------------------------------------------------------------------
// Composed recorder (exact usage row + credit debit + projection), fail-soft
// ---------------------------------------------------------------------------

export type HostedTurnMeteringDeps = Readonly<{
  /** The token ledger (writes the exact `token_usage_events` row + rollups). */
  ledger: TokenUsageLedgerShape
  /** The live ledger metering hook (debits credits + projects the balance). */
  meteringHook: MeteringHook
  /** Structured public-safe logger for per-turn metering outcomes. */
  log?: ((line: string, fields?: Record<string, unknown>) => void) | undefined
}>

export type HostedTurnMeteringOutcome = Readonly<{
  /** Public-safe ref to the exact usage row. */
  tokenUsageEventRef: string
  /** True when THIS call inserted the exact row (false on idempotent replay). */
  insertedTokenUsage: boolean
  /** input + output tokens (the public served-token contribution). */
  tokensServed: number
  /** True when the ledger hook priced + settled the charge (or $0). */
  metered: boolean
  /** True when the priced charge rounded to $0 (no ledger row). */
  zeroCharge: boolean
  /** Public-safe charge receipt ref, or null when not metered. */
  chargeReceiptRef: string | null
  /** Public-safe failure class when the debit could not settle. */
  failureReason?: 'insufficient_credit' | 'metering_storage_failed'
  /** The priced charge in USD cents (for the runtime usage event / logs). */
  chargeUsdCents: number
  /** Stable usage ref for the emitted `usage.recorded` runtime event. */
  usageRef: string
}>

/**
 * Record ONE hosted turn's exact usage + debit its owner's credits. FAIL-SOFT:
 * never throws. Writes the exact `token_usage_events` row, then runs the live
 * ledger metering hook (which decrements `agent_balances` and best-effort
 * mirrors the delta into the `scope.user.<userId>` `credit_balance`
 * projection). Both steps are guarded independently so a failure in one is a
 * typed diagnostic, never a wedged turn.
 */
export const recordHostedTurnUsageAndCharge = async (
  deps: HostedTurnMeteringDeps,
  input: HostedTurnMeteringInput,
): Promise<HostedTurnMeteringOutcome> => {
  const log = deps.log ?? (() => undefined)
  const tokensServed = input.usage.inputTokens + input.usage.outputTokens
  const chargeUsdCents = hostedKhalaChargeUsdCents(input.usage)

  // 1. Exact token-usage row (fail-soft: a lost row never blocks the charge).
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

  // 2. Debit credits + project the balance delta (the live ledger hook is
  // fail-soft by contract and returns a typed outcome, never throws).
  let metered = false
  let zeroCharge = false
  let chargeReceiptRef: string | null = null
  let failureReason: HostedTurnMeteringOutcome['failureReason']
  try {
    const outcome = await Effect.runPromise(
      deps.meteringHook(hostedKhalaMeteringContext(input)),
    )
    metered = outcome.metered
    zeroCharge = outcome.zeroCharge ?? false
    chargeReceiptRef = outcome.receiptRef
    failureReason = outcome.failureReason
  } catch (error) {
    failureReason = 'metering_storage_failed'
    log('hosted_runtime_metering_failed', {
      detail: error instanceof Error ? error.message : 'unknown',
      turnId: input.turnId,
    })
  }

  return {
    chargeReceiptRef:
      chargeReceiptRef ??
      (metered ? inferenceChargeReceiptRef(hostedKhalaChargeRequestId(input.turnId)) : null),
    chargeUsdCents,
    ...(failureReason === undefined ? {} : { failureReason }),
    insertedTokenUsage,
    metered,
    tokenUsageEventRef: hostedKhalaUsageEventId(input.turnId),
    tokensServed,
    usageRef: hostedKhalaUsageRef(input.turnId),
    zeroCharge,
  }
}
