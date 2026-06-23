// The pluggable LANE SEAM the benchmark runner executes a cell against (book
// P1-5 / #6088).
//
// The book's gold standard (§4.5) is SHADOWING real production traffic. We
// cannot shadow real provider traffic from inside a deterministic test, and a
// real provider sweep COSTS MONEY and is owner/spend-gated. So the runner talks
// to a `BenchmarkLaneSeam` — an abstract "given a cell, produce a measured
// sample" interface — and there are two implementations:
//
//   1. `makeFixtureLaneSeam` (DEFAULT): a fully DETERMINISTIC, network-free,
//      spend-free lane. Given a scenario table, it returns the SAME measured
//      sample for the same cell every time. Its numbers are clearly labeled
//      ILLUSTRATIVE — they let us prove the harness, the telemetry plumbing, and
//      the report math without touching a provider. No clock, no randomness.
//
//   2. `makeRealLaneSeam` (FLAG/OWNER-GATED, default OFF): the seam that would
//      actually hit a live provider adapter and measure a real request. It is
//      constructed only when an explicit owner arming flag is set; absent the
//      flag it refuses to run (so a test or an un-armed environment can NEVER
//      issue a real, billable request). This module does NOT implement the live
//      provider calls — that is the owner-armed sweep — it implements the GATE
//      so the default path is provably spend-free.
//
// This file is PURE/Worker-safe: no Worker bindings, no Effect runtime, no
// network, no Date.now/Math.random. The fixture lane derives its numbers
// arithmetically from the cell + scenario so the whole harness is reproducible.
import type {
  KhalaExecutedVerdict,
  KhalaVerificationClass,
} from '../khala-telemetry'
import type { BenchmarkCell } from './matrix'

// ---------------------------------------------------------------------------
// The measured sample a seam returns for one execution of one cell.
// ---------------------------------------------------------------------------

// The raw measured outputs of executing a cell ONCE. These are the inputs the
// runner feeds into the canonical telemetry builder (`buildKhalaTelemetryRecord`),
// so the field names line up with the telemetry input contract. Every value is a
// concrete measured number here (the fixture lane always measures); the telemetry
// builder is what later collapses an absent value to the honest sentinel.
export type BenchmarkLaneSample = Readonly<{
  // Token counts (receipt-first from the provider usage, fixture-derived here).
  promptTokens: number
  completionTokens: number
  totalTokens: number
  cachedInputTokens: number
  // Latency split (ms).
  ttftMs: number
  totalWallClockMs: number
  generationWallClockMs: number
  providerTimeMs: number
  gatewayOverheadMs: number
  // Verification outcome (the headline of P1-5). The fixture lane assigns these
  // deterministically per scenario; a real lane would carry the executed runner's
  // verdict.
  verificationClass: KhalaVerificationClass
  executedVerdict: KhalaExecutedVerdict
  scalarReward: number
  // Verifier time when an executed verdict exists; 0 for a non-verified workload.
  verifierTimeMs: number
  // Cost basis in msat where known (the provider's unit cost for the request).
  // The benchmark records cost so the report can compute cost-per-accepted-outcome.
  costBasisMsat: number
  // The serving region the lane reports (public-safe coarse region string).
  region: string
}>

// The pluggable seam. Given a cell, return a measured sample. PURE for the
// fixture lane (same cell → same sample); the real lane would perform IO behind
// this same signature but is gated off by default.
export type BenchmarkLaneSeam = Readonly<{
  // Stable id of the seam implementation ("fixture" | "real").
  id: string
  // Whether this seam may issue real, billable provider requests. The fixture
  // seam is always `false`; the real seam is `true` ONLY when owner-armed.
  canSpend: boolean
  // Execute one sample of one cell. The runner calls this `samplesPerCell` times
  // (or once for a not-yet-available lane, which the seam reports as unexecuted).
  sample: (cell: BenchmarkCell, sampleIndex: number) => BenchmarkLaneSample
}>

// ---------------------------------------------------------------------------
// Fixture scenario table — the deterministic, illustrative numbers.
// ---------------------------------------------------------------------------

// A per-lane fixture profile: the synthetic-but-plausible performance shape used
// to PROVE the harness. These are NOT real measurements — they are illustrative
// constants chosen so the report math is exercised (different lanes get visibly
// different latency/cost so percentiles and rankings are non-degenerate). The
// report labels every fixture-sourced number as illustrative.
export type FixtureLaneProfile = Readonly<{
  // Baseline TTFT (ms) for a small streaming request on this lane.
  baseTtftMs: number
  // Mean per-output-token generation time (ms/token) — drives perceived TPS.
  msPerOutputToken: number
  // Fixed gateway overhead (ms) the edge adds regardless of lane.
  gatewayOverheadMs: number
  // Msat cost per 1000 prompt tokens and per 1000 completion tokens.
  costPerKPromptMsat: number
  costPerKCompletionMsat: number
  // Fraction (0..1) of the cacheable prefix this lane actually serves from cache
  // (book P0-2): a lane with a prompt cache discounts repeated prefix tokens.
  cacheHitFraction: number
  // Coarse region label this lane reports.
  region: string
}>

// The default illustrative profiles. Deliberately distinct so the report can
// show a meaningful (illustrative) ranking. A real sweep replaces these with
// measured numbers; the SHAPE of the report does not change.
export const DEFAULT_FIXTURE_PROFILES: Readonly<
  Partial<Record<BenchmarkCell['lane'], FixtureLaneProfile>>
> = {
  fireworks: {
    baseTtftMs: 240,
    msPerOutputToken: 8,
    gatewayOverheadMs: 30,
    costPerKPromptMsat: 1500,
    costPerKCompletionMsat: 4500,
    cacheHitFraction: 0.8,
    region: 'us-central',
  },
  'vertex-anthropic': {
    baseTtftMs: 420,
    msPerOutputToken: 14,
    gatewayOverheadMs: 35,
    costPerKPromptMsat: 6000,
    costPerKCompletionMsat: 30000,
    cacheHitFraction: 0.9,
    region: 'us-central1',
  },
  'vertex-gemini': {
    baseTtftMs: 300,
    msPerOutputToken: 10,
    gatewayOverheadMs: 35,
    costPerKPromptMsat: 1200,
    costPerKCompletionMsat: 4800,
    cacheHitFraction: 0.75,
    region: 'us-central1',
  },
  'partner-passthrough': {
    baseTtftMs: 360,
    msPerOutputToken: 12,
    gatewayOverheadMs: 45,
    costPerKPromptMsat: 3000,
    costPerKCompletionMsat: 9000,
    cacheHitFraction: 0.5,
    region: 'partner',
  },
}

// A neutral fallback profile for a lane with no explicit fixture entry (e.g. a
// not-yet-available lane that is still expanded into the matrix). It is bland on
// purpose; the report flags the lane as unavailable regardless.
const FALLBACK_PROFILE: FixtureLaneProfile = {
  baseTtftMs: 500,
  msPerOutputToken: 16,
  gatewayOverheadMs: 50,
  costPerKPromptMsat: 2000,
  costPerKCompletionMsat: 6000,
  cacheHitFraction: 0,
  region: 'unknown',
}

// ---------------------------------------------------------------------------
// Deterministic fixture sample derivation.
// ---------------------------------------------------------------------------

// A tiny deterministic spread so repeated samples of one cell are not identical
// (so percentiles are meaningful) WITHOUT any randomness. The spread is a fixed
// function of the sample index: a small triangular jitter around the base.
const deterministicJitter = (sampleIndex: number): number => {
  // Cycles 0, +1, -1, +2, -2, ... as a fraction-of-percent multiplier.
  const magnitude = Math.ceil((sampleIndex + 1) / 2)
  const sign = sampleIndex % 2 === 0 ? 1 : -1
  return 1 + (sign * magnitude) / 100
}

// Derive the executed verification outcome for a fixture cell from its expected
// class. Artifact-gen / verifier-run cells "execute" and PASS in the fixture
// (scalarReward 1) so the report's accepted-outcome math has accepted outcomes;
// chat is unverified work (`none`, `not_executed`); a seeded long-context
// question is `seeded` with a partial reward. A real lane carries the real
// runner verdict instead.
const fixtureVerification = (
  cell: BenchmarkCell,
): Pick<
  BenchmarkLaneSample,
  'verificationClass' | 'executedVerdict' | 'scalarReward' | 'verifierTimeMs'
> => {
  switch (cell.verificationExpectation) {
    case 'none':
      return {
        verificationClass: 'none',
        executedVerdict: 'not_executed',
        scalarReward: 0,
        verifierTimeMs: 0,
      }
    case 'seeded':
      return {
        verificationClass: 'seeded',
        executedVerdict: 'not_executed',
        scalarReward: 0.6,
        verifierTimeMs: 0,
      }
    case 'test_passed':
      return {
        verificationClass: 'test_passed',
        executedVerdict: 'passed',
        scalarReward: 1,
        verifierTimeMs: 1800,
      }
    case 'exact_trace_replay':
      return {
        verificationClass: 'exact_trace_replay',
        executedVerdict: 'passed',
        scalarReward: 1,
        verifierTimeMs: 900,
      }
  }
}

// Build the deterministic fixture lane seam. Given the profile table, it derives
// every sample arithmetically from the cell + scenario. Same inputs → same
// sample. `canSpend: false` always.
export const makeFixtureLaneSeam = (
  profiles: Readonly<
    Partial<Record<BenchmarkCell['lane'], FixtureLaneProfile>>
  > = DEFAULT_FIXTURE_PROFILES,
): BenchmarkLaneSeam => ({
  id: 'fixture',
  canSpend: false,
  sample: (cell, sampleIndex) => {
    const profile = profiles[cell.lane] ?? FALLBACK_PROFILE
    const jitter = deterministicJitter(sampleIndex)

    const promptTokens = cell.shape.inputTokens
    const completionTokens = cell.shape.outputTokens
    // Cached input tokens: the cacheable prefix served from cache at the lane's
    // hit fraction (book P0-2). Streaming-only effect on TTFT below.
    const cachedInputTokens = Math.round(
      cell.shape.cacheablePrefixTokens * profile.cacheHitFraction,
    )

    // Generation wall-clock scales with output length and the lane's per-token
    // cost; the deterministic jitter spreads repeated samples.
    const generationWallClockMs = Math.round(
      completionTokens * profile.msPerOutputToken * jitter,
    )
    // TTFT: a cache hit on the prefix shaves the prefill, so a higher cache hit
    // fraction lowers TTFT — the streaming path only. A batch transport has no
    // meaningful TTFT (the book: batch optimizes throughput, not first-token), so
    // we set it equal to the full wall-clock there to keep the field honest.
    const cacheTtftDiscount =
      cell.transport === 'streaming'
        ? 1 - 0.3 * profile.cacheHitFraction
        : 1
    const ttftMs =
      cell.transport === 'streaming'
        ? Math.round(profile.baseTtftMs * cacheTtftDiscount * jitter)
        : Math.round(
            (profile.baseTtftMs + generationWallClockMs) * jitter,
          )

    const providerTimeMs = ttftMs + generationWallClockMs
    const totalWallClockMs = providerTimeMs + profile.gatewayOverheadMs

    // Cost basis: prompt + completion priced per-1k; cached prefix tokens are
    // discounted 50% (the standard prompt-cache discount), matching the metering
    // hook's treatment of `cachedPromptTokens`.
    const billablePromptTokens = promptTokens - cachedInputTokens * 0.5
    const costBasisMsat = Math.round(
      (billablePromptTokens / 1000) * profile.costPerKPromptMsat +
        (completionTokens / 1000) * profile.costPerKCompletionMsat,
    )

    const totalTokens = promptTokens + completionTokens

    return {
      promptTokens,
      completionTokens,
      totalTokens,
      cachedInputTokens,
      ttftMs,
      totalWallClockMs,
      generationWallClockMs,
      providerTimeMs,
      gatewayOverheadMs: profile.gatewayOverheadMs,
      costBasisMsat,
      region: profile.region,
      ...fixtureVerification(cell),
    }
  },
})

// ---------------------------------------------------------------------------
// Real lane seam — FLAG/OWNER-GATED, default OFF.
// ---------------------------------------------------------------------------

// Raised when the real-lane seam is constructed without the owner arming flag.
// It is a typed refusal, not a silent fallback, so a caller can never
// accidentally believe it is hitting a real lane when it is not — and it can
// never issue a billable request unarmed.
export class RealLaneNotArmedError extends Error {
  readonly _tag = 'RealLaneNotArmedError'
  constructor() {
    super(
      'Real benchmark lane is owner/spend-gated and not armed. ' +
        'Set armRealSweep:true (owner-confirmed) to run a real, billable sweep.',
    )
    this.name = 'RealLaneNotArmedError'
  }
}

// The injected dependency the real lane would call to actually execute a cell
// against a live provider adapter and measure it. The benchmark module does NOT
// implement this (the live calls + spend live in the owner-armed sweep); it only
// types the seam and the gate, so the default code path is provably spend-free.
export type RealLaneExecutor = (
  cell: BenchmarkCell,
  sampleIndex: number,
) => BenchmarkLaneSample

export type RealLaneSeamOptions = Readonly<{
  // The owner arming flag. MUST be explicitly true to construct a spending seam.
  // Anything else (false / undefined) yields a refusing seam.
  armRealSweep: boolean
  // The injected live executor. Required only when armed.
  executor?: RealLaneExecutor | undefined
}>

// Construct the real lane seam. When `armRealSweep` is not exactly `true`, the
// returned seam's `sample` THROWS `RealLaneNotArmedError` and `canSpend` is
// false — so the default/test path can construct it and prove it refuses to
// spend. When armed (and given an executor), it delegates to the executor.
export const makeRealLaneSeam = (
  options: RealLaneSeamOptions,
): BenchmarkLaneSeam => {
  const armed = options.armRealSweep === true && options.executor !== undefined
  return {
    id: 'real',
    canSpend: armed,
    sample: (cell, sampleIndex) => {
      if (!armed || options.executor === undefined) {
        throw new RealLaneNotArmedError()
      }
      return options.executor(cell, sampleIndex)
    },
  }
}
