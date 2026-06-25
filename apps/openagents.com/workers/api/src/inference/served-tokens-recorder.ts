// Khala served-tokens recorder (issue #6227).
//
// The "Khala Tokens Served" public counter (GET /api/public/khala-tokens-served)
// is the running network-wide SUM of input + output tokens across ALL rows of the
// canonical token usage ledger (`token_usage_events`), summed by
// `readPublicTokensServed()` in `token-usage-ledger.ts`. Before this module the
// OpenAI-compatible inference gateway (`chat-completions-routes.ts`) recorded NO
// token usage of its own — it priced + metered the request (credit ledger) but
// never wrote a `token_usage_events` row — so real Khala completions never moved
// the counter (it was fed only by the unrelated omega provider-broker path).
//
// This module is the SINGLE typed point where a COMPLETED Khala gateway request's
// served tokens land in the canonical ledger. The gateway calls it once per
// served completion, AFTER the metering hook returns, on every completion path
// (non-streaming, buffered stream, and true pass-through stream). It runs
// INDEPENDENTLY of WHICH metering wrapper handled the request, so a FREE-TIER
// completion (which short-circuits the credit-ledger metering hook in
// `withFreeTierKhala`) STILL records its served tokens — the tokens were served
// and must count, even though no credit was debited.
//
// HONESTY + IDEMPOTENCY:
//   - Only a SUCCESSFUL completion that produced real provider usage is recorded.
//     A failed / 4xx / refused call never reaches here (the gateway only invokes
//     the recorder where it also settled metering, i.e. on a real terminal usage
//     frame / non-streaming success). A zero-token completion is skipped (nothing
//     was served).
//   - The row is keyed idempotently on the request id (one served completion =
//     one ledger row). A retry/replay for the SAME request id hits the ledger's
//     `idempotency_key` UNIQUE constraint and is a no-op insert (`inserted:false`)
//     — never double-counts.
//
// PUBLIC-SAFE: the row carries the account ref, model ids, adapter id, bounded
// integer token counts, and timestamps only — never prompts, completions, raw
// tokens, keys, wallet/payment material, or secrets. The recorder NEVER fails the
// customer's inference response: a persistence error logs a public-safe
// diagnostic and is swallowed (the counter is a projection, not the source of
// the served answer).
import { Effect } from 'effect'

import { workerLogEntry } from '../observability'
import { currentIsoTimestamp } from '../runtime-primitives'
import {
  type TokenUsageLedgerShape,
  makeD1TokenUsageLedger,
} from '../token-usage-ledger'
import { priceRequest } from './pricing'
import { type InferenceUsage } from './provider-adapter'

export type ServedTokensRequestAttribution = Readonly<{
  demandKind: 'external' | 'internal' | 'unlabeled'
  demandSource?: string | undefined
  demandClient?: string | undefined
}>

// The gateway-side recorder seam invoked by `chat-completions-routes.ts`.
// Effect-shaped so it stays in the route's Effect topology (the non-streaming and
// buffered-stream paths `yield*` it; the true pass-through path folds it into the
// SAME terminal `Effect.runPromise` as the metering hook). The Effect NEVER fails
// (it swallows its own errors into a public-safe log) so a recorder fault can
// never break the customer's already-delivered completion.
export type ServedTokensRecorderInput = Readonly<{
  accountRef: string
  requestedModel: string
  servedModel: string
  adapterId: string
  usage: InferenceUsage
  streamed: boolean
  // Stable per-request id (the gateway's response id). One served completion =
  // one ledger row; the idempotency key is derived from it so a retry/replay is
  // a no-op insert.
  requestId: string
  // Optional public-safe attribution from request headers. Used to distinguish
  // first-party dogfood such as qa-runner from external demand without storing
  // prompts, keys, customer data, or raw provider material.
  requestAttribution?: ServedTokensRequestAttribution | undefined
}>

export type ServedTokensRecorder = (
  input: ServedTokensRecorderInput,
) => Effect.Effect<void>

// The producer-system + source-route the Khala inference gateway records under.
// `omega` is the openagents.com Worker; `omega_hosted_gemini` is the hosted Khala
// gateway lane (own-infra Gemini Flash / GPT-OSS). Both are existing closed
// literals in the ledger schema — this reuses them, it does not widen the schema.
const KHALA_GATEWAY_PRODUCER_SYSTEM = 'omega' as const
const KHALA_GATEWAY_SOURCE_ROUTE = 'omega_hosted_gemini' as const

// Stable, public-safe idempotency key for one served Khala completion. One key
// per served request id, so a retried/replayed record (same request) hits the
// ledger's `idempotency_key` UNIQUE constraint and is a no-op insert.
export const servedTokensIdempotencyKey = (requestId: string): string =>
  `inference:served-tokens:${requestId}`

// Stable, public-safe ledger event id for one served Khala completion. Distinct
// namespace from the credit-charge / free-tier receipt refs so it never collides
// with a metering row.
export const servedTokensEventId = (requestId: string): string =>
  `event.inference.served-tokens.${requestId}`

// Build the canonical `token_usage_events` ingest body for one served completion.
// `usageTruth: 'exact'` — these are real provider-reported counts, not estimates.
// `leaderboardEligible: false` keeps the served-tokens row OUT of the per-actor
// leaderboards (it is the network-wide served counter, not a competitive tally);
// the public `readPublicTokensServed()` SUM ignores that flag, so the counter
// still reflects ALL served tokens.
// Our marginal COST (USD) to serve this completion, priced against the SERVED
// model on the real supply lane (so a Fireworks `deepseek-v4-flash` completion
// is costed at the Fireworks rate, GPT-OSS at the Hydralisk rate, etc.). This is
// the cost-side number the inference cost model + analytics read (issue #6232):
// the metering hook already charges the customer; this records what the request
// cost US so `token_usage_events.cost_amount` is complete going forward instead
// of NULL. `costUsd` is cost-only (before margin); `card` funding is irrelevant
// to cost so we pass it as a stable placeholder. Rounded to a stable 6 dp so the
// stored REAL carries no floating-point noise.
export const servedTokensCostUsd = (
  servedModel: string,
  usage: InferenceUsage,
): number => {
  const priced = priceRequest({
    model: servedModel,
    usage,
    fundingKind: 'card',
  })
  return Math.round(Math.max(0, priced.costUsd) * 1_000_000) / 1_000_000
}

export const buildServedTokensIngestBody = (
  input: Readonly<{
    accountRef: string
    requestedModel: string
    servedModel: string
    adapterId: string
    usage: InferenceUsage
    requestId: string
    requestAttribution?: ServedTokensRequestAttribution | undefined
    observedAt: string
  }>,
) => ({
  schemaVersion: 'openagents.token_usage_event.v1' as const,
  actor: { accountRef: input.accountRef },
  backendProfile: input.adapterId,
  // Our marginal cost to serve, in USD, so analytics can answer "what did it
  // cost" without re-deriving from the pricing table for every historical row.
  cost: {
    amount: servedTokensCostUsd(input.servedModel, input.usage),
    currency: 'USD' as const,
  },
  ...(input.requestAttribution === undefined
    ? {}
    : { demand: input.requestAttribution }),
  eventId: servedTokensEventId(input.requestId),
  idempotencyKey: servedTokensIdempotencyKey(input.requestId),
  model: input.servedModel,
  observedAt: input.observedAt,
  privacy: { leaderboardEligible: false, privacyOptOut: false },
  producerSystem: KHALA_GATEWAY_PRODUCER_SYSTEM,
  // The adapter id is the provider-capacity attribution (which served lane
  // produced these tokens), mirroring the metering hook's adapter attribution.
  provider: input.adapterId,
  safeMetadata: {
    requestedModel: input.requestedModel,
    ...(input.requestAttribution === undefined
      ? {}
      : {
          demandKind: input.requestAttribution.demandKind,
          ...(input.requestAttribution.demandSource === undefined
            ? {}
            : { demandSource: input.requestAttribution.demandSource }),
          ...(input.requestAttribution.demandClient === undefined
            ? {}
            : { demandClient: input.requestAttribution.demandClient }),
        }),
  },
  sourceRoute: KHALA_GATEWAY_SOURCE_ROUTE,
  tokenCounts: {
    cacheReadTokens: Math.max(
      0,
      Math.trunc(input.usage.cachedPromptTokens ?? 0),
    ),
    cacheWrite1hTokens: 0,
    cacheWrite5mTokens: 0,
    inputTokens: Math.max(0, Math.trunc(input.usage.promptTokens)),
    outputTokens: Math.max(0, Math.trunc(input.usage.completionTokens)),
    reasoningTokens: 0,
    totalTokens: Math.max(0, Math.trunc(input.usage.totalTokens)),
  },
  usageTruth: 'exact' as const,
})

// Public-safe live-counter publish hook (openagents #6231). Called fire-and-
// forget ONLY when a served completion produced a REAL new ledger row (a fresh
// insert, never a duplicate/no-op or a failed completion), so the homepage
// odometer rolls up instantly without polling a per-second D1 SUM. It receives a
// bare integer delta + the recorder's stable per-request event ref + a
// timestamp; never any per-user/team/provider/model material. It must never
// throw — the counter is a projection, never the source of the served answer.
export type ServedTokensDeltaPublisher = (
  input: Readonly<{
    eventRef: string
    observedAt: string
    tokensServedDelta: number
  }>,
) => Effect.Effect<void>

// Build the served-tokens recorder over a token usage ledger. The recorder
// writes one canonical ledger row per served completion, idempotently, and never
// throws: any validation/persistence failure is logged public-safe and swallowed
// so the customer's already-delivered completion is never affected.
export const makeServedTokensRecorder = (
  deps: Readonly<{
    ledger: TokenUsageLedgerShape
    nowIso?: () => string
    // Optional live-counter publisher (#6231). Wired in production; omitted in
    // tests that only assert ledger behavior.
    publishDelta?: ServedTokensDeltaPublisher
  }>,
): ServedTokensRecorder => {
  const nowIso = deps.nowIso ?? currentIsoTimestamp
  return input =>
    Effect.gen(function* () {
      // Only a completion that actually served tokens moves the counter. A
      // zero-token result (e.g. an empty/no-usage frame) is skipped — nothing
      // was served, so there is nothing to count.
      const inputTokens = Math.max(0, Math.trunc(input.usage.promptTokens))
      const outputTokens = Math.max(0, Math.trunc(input.usage.completionTokens))
      if (inputTokens + outputTokens <= 0) {
        return
      }

      const observedAt = nowIso()
      const body = buildServedTokensIngestBody({
        accountRef: input.accountRef,
        adapterId: input.adapterId,
        observedAt,
        requestId: input.requestId,
        ...(input.requestAttribution === undefined
          ? {}
          : { requestAttribution: input.requestAttribution }),
        requestedModel: input.requestedModel,
        servedModel: input.servedModel,
        usage: input.usage,
      })

      yield* deps.ledger.ingestEvent(body).pipe(
        Effect.matchEffect({
          onFailure: error =>
            // Public-safe diagnostic only (refs + token counts, never prompt
            // or response content). Swallow: the served-tokens counter is a
            // projection, never the source of the served answer.
            Effect.logInfo(
              workerLogEntry('inference.served_tokens.record_failed', {
                accountRef: input.accountRef,
                adapterId: input.adapterId,
                reason: error._tag,
                requestId: input.requestId,
                servedModel: input.servedModel,
                streamed: input.streamed,
                totalTokens: input.usage.totalTokens,
              }),
            ),
          onSuccess: result =>
            Effect.logInfo(
              workerLogEntry('inference.served_tokens.recorded', {
                accountRef: input.accountRef,
                adapterId: input.adapterId,
                inserted: result.inserted,
                requestId: input.requestId,
                servedModel: input.servedModel,
                streamed: input.streamed,
                totalTokens: input.usage.totalTokens,
              }),
            ).pipe(
              // Live-counter push (#6231): publish the delta ONLY when this was a
              // REAL new ledger row (a fresh insert). A duplicate/no-op insert
              // (`inserted:false`) already counted, so re-publishing would double
              // count. The publisher is fail-soft (it swallows its own errors),
              // so it can never break the customer's completion.
              Effect.flatMap(() =>
                deps.publishDelta === undefined || !result.inserted
                  ? Effect.void
                  : deps.publishDelta({
                      eventRef: servedTokensEventId(input.requestId),
                      observedAt,
                      tokensServedDelta: inputTokens + outputTokens,
                    }),
              ),
            ),
        }),
      )
    })
}

// Build the production (D1-backed) served-tokens recorder for the Worker wiring.
// `env` carries the sync bindings (`OPENAGENTS_DB` + `SYNC_ROOM`) used to push
// the live tokens-served delta (#6231); when omitted (legacy callers), the
// recorder still records the ledger row but performs no live push.
export const makeD1ServedTokensRecorder = (
  db: D1Database,
  options: Readonly<{
    nowIso?: () => string
    publishDelta?: ServedTokensDeltaPublisher
  }> = {},
): ServedTokensRecorder =>
  makeServedTokensRecorder({
    ledger: makeD1TokenUsageLedger(db),
    ...(options.nowIso === undefined ? {} : { nowIso: options.nowIso }),
    ...(options.publishDelta === undefined
      ? {}
      : { publishDelta: options.publishDelta }),
  })
