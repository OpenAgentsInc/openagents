// Khala request-telemetry scorecard (book P0-1 / Open Questions #1-2).
//
// This is the CANONICAL, public-safe Khala request-lifecycle telemetry schema —
// the "measure the request lifecycle before optimizing it" lesson from
// `docs/inference/inference-engineering-book/` turned into a typed contract.
//
// It is the ONE place the gateway records what the book's P0-1 field list asks
// for: token counts (incl. cached input where the provider exposes them), the
// latency split (TTFT, inter-token latency / perceived TPS, total wall-clock,
// and the provider/gateway/verifier/settlement time split where available),
// queue/batch wait, the request CLASS, the route/provider/served-model/region/
// cache-affinity HASH/fallback identity, the verification class + executed
// verdict + scalar reward, and the cost/price/margin/settlement disclosure.
//
// HONESTY CONTRACT (the load-bearing invariant of this whole module):
//   - Every numeric field is EITHER a real measured number OR the explicit
//     sentinel `not_measured`. A field is NEVER fabricated or defaulted to a
//     plausible-looking number. "We did not measure this" is a first-class,
//     typed value (`NOT_MEASURED`), not a missing key and not a fake `0`.
//   - This mirrors the M8 metric table discipline
//     (`docs/inference/2026-06-23-khala-head-to-head-m8-status.md`): a `0` means
//     "measured zero", `not_measured` means "no measurement exists". They are
//     different products.
//
// PUBLIC-SAFE PROJECTION (INVARIANTS: no secret/private leakage):
//   - The cache-affinity key is recorded ONLY as a hash (`cacheAffinityKeyHash`),
//     never the raw account/session/codebase key. The hash is FNV-1a (a stable,
//     non-cryptographic public digest) so two requests that share a cache lane
//     can be correlated WITHOUT exposing the raw key.
//   - No prompt, completion, chain-of-thought, account ref, session ref, amount,
//     destination, or payment material is ever a telemetry field. Only token
//     COUNTS, durations, neutral classifiers, public refs, and the margin BUCKET
//     (not the raw margin) appear.
//
// BLOCK-vs-RECEIPT SPLIT (Open Question #2, RESOLVED here):
//   - The IMMEDIATE `openagents` response block carries only the SMALL hot-path
//     telemetry summary (`KhalaTelemetryBlock`): the few fields a caller wants at
//     a glance — request class, tokens, TTFT, total wall-clock, the verification
//     class + verdict + reward, and a pointer (`detailRef`) to the full record.
//   - The FULL lifecycle record (`KhalaTelemetryRecord`) — every P0-1 field,
//     including the time split, queue/batch wait, region, cache-affinity hash,
//     fallback reason, cost basis / margin bucket / settlement state / blocker
//     refs — is dereferenceable from the public inference receipt detail. The
//     immediate block stays small; the receipt carries the depth.
//
// It is PURE + framework-agnostic: no Worker, no D1, no Effect runtime. The
// gateway builds a record from what it can measure NOW; everything it cannot is
// `not_measured`. The schema is the same whether a field is measured today or
// becomes measurable later, so adding a measurement never reshapes the contract.
import { Schema as S } from 'effect'

// ---------------------------------------------------------------------------
// The honest sentinel.
// ---------------------------------------------------------------------------

// The single explicit "no measurement exists" value. Typed, never a fake number.
export const NOT_MEASURED = 'not_measured' as const
export type NotMeasured = typeof NOT_MEASURED

// A scalar that is EITHER a finite non-negative number OR the honest sentinel.
// Used for every measurable quantity so an unmeasured field is structurally
// distinct from a measured zero.
const MeasuredNumber = S.Union([S.Number, S.Literal(NOT_MEASURED)])
export type MeasuredNumber = typeof MeasuredNumber.Type

// A measured count of tokens (>= 0) or the sentinel.
const MeasuredTokens = MeasuredNumber
// A measured duration in milliseconds (>= 0) or the sentinel.
const MeasuredMs = MeasuredNumber

// Narrow a possibly-undefined/negative number to a measured value or the
// sentinel. `undefined`, `null`, NaN, and negative inputs all collapse to the
// honest sentinel rather than a fabricated number.
export const measured = (value: number | undefined | null): MeasuredNumber => {
  if (value === undefined || value === null) {
    return NOT_MEASURED
  }
  if (!Number.isFinite(value) || value < 0) {
    return NOT_MEASURED
  }
  return value
}

export const isMeasured = (value: MeasuredNumber): value is number =>
  value !== NOT_MEASURED

// ---------------------------------------------------------------------------
// Classifiers (book P0-1: request class, verification class, settlement state).
// ---------------------------------------------------------------------------

// The request CLASS — the shape of the request, which determines its latency
// budget and which metrics matter (book Ch.1/Ch.7 + the 524 postmortem). An
// interactive stream optimizes TTFT/ITL; an async job optimizes total wall-clock
// + queue wait; a verifier run optimizes accepted-outcome; a batch job optimizes
// throughput/cost.
export const KhalaRequestClass = S.Literals([
  'interactive_stream',
  'async_job',
  'verifier_run',
  'batch',
])
export type KhalaRequestClass = typeof KhalaRequestClass.Type

// The verification class, mirrored from the Khala/Tassadar verification-class
// registry (khala.md §6) so telemetry never invents a parallel vocabulary.
export const KhalaVerificationClass = S.Literals([
  'none',
  'seeded',
  'test_passed',
  'exact_trace_replay',
  // Honest in-flight states from the executed-acceptance lane (EPIC #6017): an
  // executable artifact whose headless run has not happened yet, or an executed
  // run that did not fully pass.
  'unverified',
  'failed',
])
export type KhalaVerificationClass = typeof KhalaVerificationClass.Type

// The executed verifier verdict — distinct from the CLASS. `not_executed` is the
// honest default when no headless acceptance run produced a verdict (the hot
// Worker path cannot launch a browser); `passed`/`failed` require an EXECUTED
// run. This is the "a real failed beats a fake passed" discipline as a type.
export const KhalaExecutedVerdict = S.Literals([
  'not_executed',
  'passed',
  'failed',
])
export type KhalaExecutedVerdict = typeof KhalaExecutedVerdict.Type

// Settlement state of the accepted outcome (khala.md §7 / RL-2). `not_applicable`
// for an unverified or non-settling lane; `pending` once a verified outcome is
// awaiting the async settlement callback; `settled` once sats moved.
export const KhalaSettlementState = S.Literals([
  'not_applicable',
  'pending',
  'settled',
])
export type KhalaSettlementState = typeof KhalaSettlementState.Type

// Coarse margin BUCKET, never the raw margin (public-safe: the exact margin is
// private pricing material). `not_measured` when the metering hook did not price
// the request (e.g. a stream with no terminal usage frame).
export const KhalaMarginBucket = S.Literals([
  'not_measured',
  'negative',
  'zero',
  'thin',
  'standard',
  'rich',
])
export type KhalaMarginBucket = typeof KhalaMarginBucket.Type

// ---------------------------------------------------------------------------
// The IMMEDIATE block (small — rides on the `openagents` response block).
// ---------------------------------------------------------------------------

// The small hot-path telemetry summary embedded directly in the `openagents`
// response block. Resolves Open Question #2 on the SMALL side: just the
// at-a-glance lifecycle facts + a pointer to the full record. Every field is a
// measured value or the honest sentinel.
export const KhalaTelemetryBlock = S.Struct({
  schemaVersion: S.Literal('openagents.khala.telemetry.v1'),
  // The request class — the single most important shape classifier.
  requestClass: KhalaRequestClass,
  // Token counts from the provider usage (receipt-first). `not_measured` when no
  // usage frame was served.
  promptTokens: MeasuredTokens,
  completionTokens: MeasuredTokens,
  totalTokens: MeasuredTokens,
  // Cached input tokens where the provider exposes them (the headline metric of
  // the prefix-caching feature, book P0-2 / #6084: how much of the prompt hit
  // the provider prompt cache). `not_measured` when the provider does not report
  // a cached dimension. Promoted to the SMALL block (alongside token counts)
  // because cache hit-rate is an at-a-glance lifecycle fact for Khala traffic.
  cachedInputTokens: MeasuredTokens,
  // Time to first token (ms), measurable on the streaming path only.
  ttftMs: MeasuredMs,
  // Total wall-clock for the request (ms), gateway-edge measured.
  totalWallClockMs: MeasuredMs,
  // Verification class + executed verdict + scalar reward, reusing the existing
  // Khala/Tassadar values (never a parallel grader).
  verificationClass: KhalaVerificationClass,
  executedVerdict: KhalaExecutedVerdict,
  scalarReward: MeasuredNumber,
  // The dereferenceable pointer to the full lifecycle record (Open Question #2:
  // depth lives off the hot path). Null when no receipt was minted.
  detailRef: S.NullOr(S.String),
})
export type KhalaTelemetryBlock = typeof KhalaTelemetryBlock.Type

// ---------------------------------------------------------------------------
// The FULL record (dereferenceable — every P0-1 field).
// ---------------------------------------------------------------------------

// The full request-lifecycle telemetry record. This is the canonical P0-1
// scorecard. It is the dereferenceable depth behind the immediate block. Every
// field is a measured value, the honest sentinel, or a neutral public-safe
// classifier/ref — never a secret and never a fabricated number.
export const KhalaTelemetryRecord = S.Struct({
  schemaVersion: S.Literal('openagents.khala.telemetry.v1'),

  // --- identity / routing (P0-1: route, provider, served model, region,
  // cache-affinity key hash, fallback reason) ---
  requestId: S.String,
  requestedModel: S.String,
  servedModel: S.String,
  // The coordinator route lane (coding | cheap | long_context | default | ...).
  route: S.String,
  // The provider/adapter id that actually served (provider-capacity attribution).
  provider: S.String,
  // The serving region when the provider/lane exposes it; sentinel otherwise.
  region: S.Union([S.String, S.Literal(NOT_MEASURED)]),
  // Public-safe HASH of the cache-affinity key (account/session/codebase). NEVER
  // the raw key. Null when no affinity key applied to this request.
  cacheAffinityKeyHash: S.NullOr(S.String),
  // Why a fallback lane served (e.g. "rate_limited", "service_overloaded"). Null
  // when the primary lane served with no overflow.
  fallbackReason: S.NullOr(S.String),
  requestClass: KhalaRequestClass,

  // --- tokens (P0-1: prompt / completion / total; cached input where exposed) ---
  promptTokens: MeasuredTokens,
  completionTokens: MeasuredTokens,
  totalTokens: MeasuredTokens,
  // Cached input tokens where the provider exposes them (e.g. Fireworks prompt
  // cache). `not_measured` when the provider does not report a cached dimension.
  cachedInputTokens: MeasuredTokens,
  // The reconciliation of `totalTokens` against `promptTokens + completionTokens`
  // (book P0-2 / #6084). The served models bill tokens BEYOND the visible prompt
  // + completion — Gemini's `totalTokenCount` includes thinking/tool-use tokens,
  // reasoning lanes bill internal reasoning — so the live discrepancy (e.g. total
  // 679 vs prompt 347 + completion 20) is REAL, not a miscount. We record the
  // provider's authoritative `totalTokens` receipt-first and disclose the gap
  // here as `unaccountedTokens` rather than silently dropping it or recomputing
  // the total as prompt+completion (which would under-count billed tokens). `0`
  // when the total is exactly prompt+completion; `not_measured` when tokens are
  // unmeasured.
  unaccountedTokens: MeasuredTokens,

  // --- latency (P0-1: TTFT, ITL / perceived TPS, total wall-clock) ---
  ttftMs: MeasuredMs,
  // Mean inter-token latency (ms/token) on the streaming path.
  interTokenLatencyMs: MeasuredMs,
  // Perceived tokens/second (completion tokens / generation wall-clock).
  perceivedTps: MeasuredNumber,
  totalWallClockMs: MeasuredMs,

  // --- the time split (P0-1: provider / gateway / verifier / settlement time) ---
  // Each is measured where available, else the honest sentinel. These need not
  // sum to total wall-clock (overlap/gaps are normal); they are honest partials.
  providerTimeMs: MeasuredMs,
  gatewayOverheadMs: MeasuredMs,
  verifierTimeMs: MeasuredMs,
  settlementTimeMs: MeasuredMs,

  // --- queue / batch wait (P0-1) ---
  queueWaitMs: MeasuredMs,
  batchWaitMs: MeasuredMs,

  // --- verification (P0-1: class + executed verdict + scalar reward) ---
  verificationClass: KhalaVerificationClass,
  executedVerdict: KhalaExecutedVerdict,
  scalarReward: MeasuredNumber,
  // Public verifier receipt ref when an executed verdict exists; null otherwise.
  verifierReceiptRef: S.NullOr(S.String),

  // --- economics (P0-1: cost basis, price, margin bucket, settlement state,
  // blocker refs) ---
  // Cost basis + price in msat where the metering hook priced the request. Public
  // disclosure of the unit economics inputs; sentinel when unpriced.
  costBasisMsat: MeasuredNumber,
  priceMsat: MeasuredNumber,
  // Coarse margin bucket only — never the raw margin.
  marginBucket: KhalaMarginBucket,
  settlementState: KhalaSettlementState,
  // Public settlement receipt refs once a verified outcome settled.
  settlementReceiptRefs: S.Array(S.String),
  // Honest blockers preventing a full/green measurement (e.g.
  // "verifier_not_executed", "cost_not_measured"). Carries WHY a field is a
  // sentinel so a reader does not have to guess.
  blockerRefs: S.Array(S.String),
})
export type KhalaTelemetryRecord = typeof KhalaTelemetryRecord.Type

// ---------------------------------------------------------------------------
// Decoders (typed parse of an unknown projection back into the contract).
// ---------------------------------------------------------------------------

export const decodeKhalaTelemetryBlock = S.decodeUnknownOption(
  KhalaTelemetryBlock,
)
export const decodeKhalaTelemetryRecord = S.decodeUnknownOption(
  KhalaTelemetryRecord,
)
export const encodeKhalaTelemetryRecord = S.encodeSync(KhalaTelemetryRecord)
export const encodeKhalaTelemetryBlock = S.encodeSync(KhalaTelemetryBlock)

// ---------------------------------------------------------------------------
// Public-safe cache-affinity hashing.
// ---------------------------------------------------------------------------

// FNV-1a 32-bit, rendered as a stable lowercase hex digest with a neutral
// prefix. Non-cryptographic (it is a CORRELATION key, not a secret), but it is a
// ONE-WAY projection: the raw account/session/codebase key never leaves the
// gateway, only this digest. Two requests on the same cache lane share a digest.
export const hashCacheAffinityKey = (rawKey: string): string => {
  let hash = 0x811c9dc5
  for (let index = 0; index < rawKey.length; index += 1) {
    hash ^= rawKey.charCodeAt(index)
    // 32-bit FNV prime multiply via shifts (avoids BigInt; stays in uint32).
    hash = Math.imul(hash, 0x01000193) >>> 0
  }
  return `cacheaff:fnv1a32:${hash.toString(16).padStart(8, '0')}`
}

// ---------------------------------------------------------------------------
// Margin-bucket derivation (public-safe coarse-graining of the raw margin).
// ---------------------------------------------------------------------------

// Coarse-grain the raw (cost, price) msat into a public-safe bucket. The exact
// margin is private pricing material; the bucket is the only thing telemetry
// exposes. `not_measured` when either input is the sentinel.
export const deriveMarginBucket = (
  costBasisMsat: MeasuredNumber,
  priceMsat: MeasuredNumber,
): KhalaMarginBucket => {
  if (!isMeasured(costBasisMsat) || !isMeasured(priceMsat)) {
    return 'not_measured'
  }
  const margin = priceMsat - costBasisMsat
  if (margin < 0) {
    return 'negative'
  }
  if (margin === 0) {
    return 'zero'
  }
  // Ratio of margin to price (the public-safe coarse signal, not the amount).
  const ratio = priceMsat === 0 ? 0 : margin / priceMsat
  if (ratio < 0.1) {
    return 'thin'
  }
  if (ratio < 0.4) {
    return 'standard'
  }
  return 'rich'
}

// ---------------------------------------------------------------------------
// Builder — assemble a full record from what the gateway measured NOW.
// ---------------------------------------------------------------------------

// The measurable inputs the gateway can collect on the hot path TODAY. Anything
// absent collapses to the honest sentinel via `measured(...)`; nothing is
// fabricated.
export type KhalaTelemetryInput = Readonly<{
  requestId: string
  requestedModel: string
  servedModel: string
  route: string
  provider: string
  requestClass: KhalaRequestClass

  // Tokens (receipt-first from provider usage).
  promptTokens?: number | undefined
  completionTokens?: number | undefined
  totalTokens?: number | undefined
  cachedInputTokens?: number | undefined

  // Latency (gateway-edge measured).
  ttftMs?: number | undefined
  totalWallClockMs?: number | undefined
  // Generation wall-clock used to derive perceived TPS + mean ITL. When omitted
  // these derived metrics are the sentinel (we do not guess them from total).
  generationWallClockMs?: number | undefined

  // Time split (each optional; sentinel when absent).
  providerTimeMs?: number | undefined
  gatewayOverheadMs?: number | undefined
  verifierTimeMs?: number | undefined
  settlementTimeMs?: number | undefined
  queueWaitMs?: number | undefined
  batchWaitMs?: number | undefined

  // Routing identity.
  region?: string | undefined
  // The RAW cache-affinity key. It is hashed here and NEVER stored raw.
  cacheAffinityKeyRaw?: string | undefined
  fallbackReason?: string | undefined

  // Verification.
  verificationClass: KhalaVerificationClass
  executedVerdict: KhalaExecutedVerdict
  scalarReward?: number | undefined
  verifierReceiptRef?: string | undefined

  // Economics.
  costBasisMsat?: number | undefined
  priceMsat?: number | undefined
  settlementState: KhalaSettlementState
  settlementReceiptRefs?: ReadonlyArray<string> | undefined
  blockerRefs?: ReadonlyArray<string> | undefined
}>

// Derive mean inter-token latency from completion tokens + generation
// wall-clock. Sentinel unless BOTH are measured and there is >1 token (ITL is
// undefined for a single token). Never fabricated.
const deriveInterTokenLatencyMs = (
  completionTokens: number | undefined,
  generationWallClockMs: number | undefined,
): MeasuredNumber => {
  if (
    completionTokens === undefined ||
    generationWallClockMs === undefined ||
    completionTokens <= 1 ||
    !Number.isFinite(generationWallClockMs) ||
    generationWallClockMs < 0
  ) {
    return NOT_MEASURED
  }
  // Inter-token gaps = (tokens - 1).
  return generationWallClockMs / (completionTokens - 1)
}

// Derive perceived tokens/second from completion tokens + generation wall-clock.
// Sentinel unless both are measured and wall-clock > 0.
const derivePerceivedTps = (
  completionTokens: number | undefined,
  generationWallClockMs: number | undefined,
): MeasuredNumber => {
  if (
    completionTokens === undefined ||
    generationWallClockMs === undefined ||
    !Number.isFinite(generationWallClockMs) ||
    generationWallClockMs <= 0 ||
    completionTokens < 0
  ) {
    return NOT_MEASURED
  }
  return completionTokens / (generationWallClockMs / 1000)
}

// Reconcile `totalTokens` against `promptTokens + completionTokens` (book P0-2 /
// #6084). The provider's `totalTokens` is AUTHORITATIVE (receipt-first; never
// recompute it as prompt+completion, which would under-count billed
// reasoning/thinking/tool-use tokens). The unaccounted delta is the hidden
// billed dimension: `max(0, total - (prompt + completion))`. Floors at 0 so a
// degenerate/malformed total below prompt+completion never yields a negative.
// `not_measured` unless ALL THREE token counts are measured (we never fabricate a
// reconciliation from a partial usage frame).
const deriveUnaccountedTokens = (
  promptTokens: number | undefined,
  completionTokens: number | undefined,
  totalTokens: number | undefined,
): MeasuredNumber => {
  const prompt = measured(promptTokens)
  const completion = measured(completionTokens)
  const total = measured(totalTokens)
  if (!isMeasured(prompt) || !isMeasured(completion) || !isMeasured(total)) {
    return NOT_MEASURED
  }
  return Math.max(0, total - (prompt + completion))
}

// Assemble the canonical full lifecycle record from measured inputs. PURE: same
// inputs => same record. Unmeasured inputs become the honest sentinel; the raw
// cache-affinity key is hashed; the margin is coarse-grained to a bucket.
export const buildKhalaTelemetryRecord = (
  input: KhalaTelemetryInput,
): KhalaTelemetryRecord => {
  const costBasisMsat = measured(input.costBasisMsat)
  const priceMsat = measured(input.priceMsat)
  return {
    schemaVersion: 'openagents.khala.telemetry.v1',

    requestId: input.requestId,
    requestedModel: input.requestedModel,
    servedModel: input.servedModel,
    route: input.route,
    provider: input.provider,
    region:
      input.region === undefined || input.region.trim() === ''
        ? NOT_MEASURED
        : input.region,
    cacheAffinityKeyHash:
      input.cacheAffinityKeyRaw === undefined ||
      input.cacheAffinityKeyRaw.trim() === ''
        ? null
        : hashCacheAffinityKey(input.cacheAffinityKeyRaw),
    fallbackReason: input.fallbackReason ?? null,
    requestClass: input.requestClass,

    promptTokens: measured(input.promptTokens),
    completionTokens: measured(input.completionTokens),
    totalTokens: measured(input.totalTokens),
    cachedInputTokens: measured(input.cachedInputTokens),
    unaccountedTokens: deriveUnaccountedTokens(
      input.promptTokens,
      input.completionTokens,
      input.totalTokens,
    ),

    ttftMs: measured(input.ttftMs),
    interTokenLatencyMs: deriveInterTokenLatencyMs(
      input.completionTokens,
      input.generationWallClockMs,
    ),
    perceivedTps: derivePerceivedTps(
      input.completionTokens,
      input.generationWallClockMs,
    ),
    totalWallClockMs: measured(input.totalWallClockMs),

    providerTimeMs: measured(input.providerTimeMs),
    gatewayOverheadMs: measured(input.gatewayOverheadMs),
    verifierTimeMs: measured(input.verifierTimeMs),
    settlementTimeMs: measured(input.settlementTimeMs),

    queueWaitMs: measured(input.queueWaitMs),
    batchWaitMs: measured(input.batchWaitMs),

    verificationClass: input.verificationClass,
    executedVerdict: input.executedVerdict,
    scalarReward: measured(input.scalarReward),
    verifierReceiptRef: input.verifierReceiptRef ?? null,

    costBasisMsat,
    priceMsat,
    marginBucket: deriveMarginBucket(costBasisMsat, priceMsat),
    settlementState: input.settlementState,
    settlementReceiptRefs: input.settlementReceiptRefs ?? [],
    blockerRefs: input.blockerRefs ?? [],
  }
}

// Project the small immediate block out of the full record (the SMALL side of
// Open Question #2). `detailRef` points back to the dereferenceable full record.
export const khalaTelemetryBlockFromRecord = (
  record: KhalaTelemetryRecord,
  detailRef: string | null,
): KhalaTelemetryBlock => ({
  schemaVersion: 'openagents.khala.telemetry.v1',
  requestClass: record.requestClass,
  promptTokens: record.promptTokens,
  completionTokens: record.completionTokens,
  totalTokens: record.totalTokens,
  cachedInputTokens: record.cachedInputTokens,
  ttftMs: record.ttftMs,
  totalWallClockMs: record.totalWallClockMs,
  verificationClass: record.verificationClass,
  executedVerdict: record.executedVerdict,
  scalarReward: record.scalarReward,
  detailRef,
})

// Convenience: build the immediate block directly from measured inputs +
// detail ref (the common gateway path).
export const buildKhalaTelemetryBlock = (
  input: KhalaTelemetryInput,
  detailRef: string | null,
): KhalaTelemetryBlock =>
  khalaTelemetryBlockFromRecord(buildKhalaTelemetryRecord(input), detailRef)
