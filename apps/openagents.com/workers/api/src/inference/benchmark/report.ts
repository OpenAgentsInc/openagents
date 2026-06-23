// The dereferenceable benchmark REPORT (book P1-5 / #6088).
//
// Aggregates a `BenchmarkRunSet` into per-lane / per-workload metrics — the
// book's "best is product-specific, MEASURED" framing turned into a typed
// artifact. The report answers "faster/cheaper/better at WHAT" with:
//   - latency PERCENTILES (P50/P90/P99 — book Ch.1 §1.4.1: a right-skewed
//     distribution where the mean lies; outliers are the product risk);
//   - perceived TPS (completion tokens / generation wall-clock);
//   - COST-PER-ACCEPTED-OUTCOME (msat cost / accepted outcomes — the only cost
//     metric that respects verification: a cheap lane that fails verification is
//     not cheap);
//   - VERIFICATION RATE (executed-passed / executed-attempted — P1-5's headline:
//     score on outcome, not raw token speed);
//   - cache hit rate (book P0-2).
//
// PUBLIC-SAFE (INVARIANTS: no raw prompt/account/secret in any projection). The
// report carries ONLY token COUNTS, durations, neutral lane/engine/workload
// classifiers, the coarse region, and the aggregate metrics. It NEVER carries a
// prompt, completion, account ref, raw cache key, price, or margin. A
// `publicSafety` self-check (below) asserts this structurally.
//
// HONESTY: the report header records the seam that produced it. When the seam is
// the fixture lane (`seamCanSpend: false`), every number is labeled ILLUSTRATIVE
// and `decisionGrade: false` — they prove the harness, not the lanes. Only an
// owner-armed real sweep over REALISTIC traffic yields `decisionGrade: true`. A
// group whose shapes are all synthetic is flagged `syntheticOnly: true`
// regardless of seam.
//
// PURE: no clock, no randomness, no IO. Same run set → same report.
import { type MeasuredNumber, isMeasured } from '../khala-telemetry'
import type { BenchmarkRun, BenchmarkRunSet } from './runner'
import type { BenchmarkLane, BenchmarkWorkload } from './matrix'

// ---------------------------------------------------------------------------
// Percentile helper (book Ch.1 §1.4.1).
// ---------------------------------------------------------------------------

// Nearest-rank percentile over a sample array. Returns null for an empty input
// (honest absence, never a fabricated 0). `p` is in [0,100]. PURE: sorts a copy.
export const percentile = (
  values: ReadonlyArray<number>,
  p: number,
): number | null => {
  if (values.length === 0) {
    return null
  }
  const sorted = [...values].sort((a, b) => a - b)
  if (sorted.length === 1) {
    return sorted[0] ?? null
  }
  // Nearest-rank: rank = ceil(p/100 * N), clamped to [1, N].
  const rank = Math.min(
    sorted.length,
    Math.max(1, Math.ceil((p / 100) * sorted.length)),
  )
  return sorted[rank - 1] ?? null
}

// Arithmetic mean, null for empty. PURE.
export const mean = (values: ReadonlyArray<number>): number | null => {
  if (values.length === 0) {
    return null
  }
  return values.reduce((sum, value) => sum + value, 0) / values.length
}

// Collect the measured numeric values of one telemetry field across runs,
// dropping the honest `not_measured` sentinel (so percentiles are over MEASURED
// samples only — never polluted by a sentinel coerced to a number).
const measuredValues = (
  runs: ReadonlyArray<BenchmarkRun>,
  pick: (record: NonNullable<BenchmarkRun['record']>) => MeasuredNumber,
): ReadonlyArray<number> => {
  const out: Array<number> = []
  for (const run of runs) {
    if (run.record === null) {
      continue
    }
    const value = pick(run.record)
    if (isMeasured(value)) {
      out.push(value)
    }
  }
  return out
}

// ---------------------------------------------------------------------------
// Per-group aggregate metrics.
// ---------------------------------------------------------------------------

// The latency distribution summary for one field (ms). Each percentile is null
// when no sample measured the field (e.g. TTFT on a pure batch group).
export type LatencySummary = Readonly<{
  p50: number | null
  p90: number | null
  p99: number | null
  mean: number | null
  // Number of MEASURED samples behind this summary (the read confidence).
  sampleCount: number
}>

const summarize = (values: ReadonlyArray<number>): LatencySummary => ({
  p50: percentile(values, 50),
  p90: percentile(values, 90),
  p99: percentile(values, 99),
  mean: mean(values),
  sampleCount: values.length,
})

// The aggregate metrics for one (lane × workload) group.
export type BenchmarkGroupMetrics = Readonly<{
  lane: BenchmarkLane
  workload: BenchmarkWorkload
  laneAvailability: 'available' | 'not_yet_available'
  // True when EVERY shape in this group is synthetic (numbers are not
  // production-representative — book §4.5). Labeled prominently.
  syntheticOnly: boolean
  // How many samples were EXECUTED (telemetry record produced) vs SKIPPED (a
  // not-yet-available lane). Skipped samples carry no metrics.
  executedSamples: number
  skippedSamples: number
  // Latency distributions (book Ch.1 percentiles).
  ttftMs: LatencySummary
  totalWallClockMs: LatencySummary
  perceivedTps: LatencySummary
  interTokenLatencyMs: LatencySummary
  // Cache hit rate: cached input tokens / prompt tokens, averaged over executed
  // samples that measured both. null when unmeasured.
  cacheHitRate: number | null
  // VERIFICATION RATE: executed-passed / executed-attempted (a verdict that is
  // not `not_executed`). null when the group has no verification (e.g. chat).
  verificationRate: number | null
  acceptedOutcomes: number
  attemptedVerifications: number
  // COST-PER-ACCEPTED-OUTCOME in msat: total cost basis / accepted outcomes.
  // null when there were zero accepted outcomes (dividing would fabricate a
  // number — a lane with no accepted outcome has UNDEFINED cost-per-outcome,
  // which is itself a finding, not a 0).
  costPerAcceptedOutcomeMsat: number | null
  totalCostBasisMsat: number
}>

// Aggregate one group's runs into metrics. PURE.
const aggregateGroup = (
  lane: BenchmarkLane,
  workload: BenchmarkWorkload,
  runs: ReadonlyArray<BenchmarkRun>,
): BenchmarkGroupMetrics => {
  const executed = runs.filter(run => run.record !== null)
  const skipped = runs.filter(run => run.record === null)
  const laneAvailability =
    runs[0]?.cell.laneAvailability ?? 'available'
  const syntheticOnly =
    runs.length > 0 &&
    runs.every(run => run.cell.shape.provenance === 'synthetic')

  // Verification: count attempted (an executed verdict) and accepted (passed).
  let attemptedVerifications = 0
  let acceptedOutcomes = 0
  let totalCostBasisMsat = 0
  const cacheRates: Array<number> = []

  for (const run of executed) {
    const record = run.record
    if (record === null) {
      continue
    }
    if (record.executedVerdict !== 'not_executed') {
      attemptedVerifications += 1
      if (record.executedVerdict === 'passed') {
        acceptedOutcomes += 1
      }
    }
    if (isMeasured(record.costBasisMsat)) {
      totalCostBasisMsat += record.costBasisMsat
    }
    if (
      isMeasured(record.cachedInputTokens) &&
      isMeasured(record.promptTokens) &&
      record.promptTokens > 0
    ) {
      cacheRates.push(record.cachedInputTokens / record.promptTokens)
    }
  }

  return {
    lane,
    workload,
    laneAvailability,
    syntheticOnly,
    executedSamples: executed.length,
    skippedSamples: skipped.length,
    ttftMs: summarize(measuredValues(executed, r => r.ttftMs)),
    totalWallClockMs: summarize(
      measuredValues(executed, r => r.totalWallClockMs),
    ),
    perceivedTps: summarize(measuredValues(executed, r => r.perceivedTps)),
    interTokenLatencyMs: summarize(
      measuredValues(executed, r => r.interTokenLatencyMs),
    ),
    cacheHitRate: mean(cacheRates),
    verificationRate:
      attemptedVerifications === 0
        ? null
        : acceptedOutcomes / attemptedVerifications,
    acceptedOutcomes,
    attemptedVerifications,
    costPerAcceptedOutcomeMsat:
      acceptedOutcomes === 0
        ? null
        : totalCostBasisMsat / acceptedOutcomes,
    totalCostBasisMsat,
  }
}

// ---------------------------------------------------------------------------
// The full report artifact.
// ---------------------------------------------------------------------------

export type BenchmarkReport = Readonly<{
  schemaVersion: 'openagents.khala.benchmark-report.v1'
  configId: string
  seamId: string
  // True only when an owner-armed REAL sweep over REALISTIC traffic produced
  // this report. Fixture-lane reports are NOT decision-grade.
  decisionGrade: boolean
  // The honest illustrative banner the report carries when not decision-grade.
  // Empty string when decision-grade.
  illustrativeNotice: string
  cellsExpanded: number
  cellsExecuted: number
  cellsSkipped: number
  // Per (lane × workload) metrics, in deterministic (lane, workload) order.
  groups: ReadonlyArray<BenchmarkGroupMetrics>
}>

const ILLUSTRATIVE_NOTICE =
  'ILLUSTRATIVE ONLY: produced by the deterministic FIXTURE lane (no network, ' +
  'no spend). These numbers exercise the harness and report math; they are NOT ' +
  'measurements of any real lane. Decision-grade numbers require an owner-armed ' +
  'real sweep over REALISTIC Khala traffic (real input/output lengths, real ' +
  'cacheable prefixes, real concurrency). Synthetic-only traffic is labeled as ' +
  'such per group and is never a basis for a product claim.'

// Stable ordering of lanes + workloads so the report group list is byte-stable.
const LANE_ORDER: ReadonlyArray<BenchmarkLane> = [
  'vertex-anthropic',
  'vertex-gemini',
  'fireworks',
  'partner-passthrough',
  'pylon-whole-small',
  'psionic-shard-wan',
]
const WORKLOAD_ORDER: ReadonlyArray<BenchmarkWorkload> = [
  'chat',
  'khala-code-artifact-gen',
  'verifier-run',
  'long-context-codebase-question',
]

// Build the report from a run set. A report is decision-grade ONLY when the seam
// can spend (a real sweep) AND no group is synthetic-only. PURE.
export const buildBenchmarkReport = (
  runSet: BenchmarkRunSet,
): BenchmarkReport => {
  // Bucket runs by (lane, workload).
  const buckets = new Map<string, Array<BenchmarkRun>>()
  for (const run of runSet.runs) {
    const key = `${run.cell.lane}::${run.cell.workload}`
    const existing = buckets.get(key)
    if (existing === undefined) {
      buckets.set(key, [run])
    } else {
      existing.push(run)
    }
  }

  const groups: Array<BenchmarkGroupMetrics> = []
  for (const lane of LANE_ORDER) {
    for (const workload of WORKLOAD_ORDER) {
      const runs = buckets.get(`${lane}::${workload}`)
      if (runs === undefined || runs.length === 0) {
        continue
      }
      groups.push(aggregateGroup(lane, workload, runs))
    }
  }

  const anySynthetic = groups.some(group => group.syntheticOnly)
  const decisionGrade = runSet.seamCanSpend && !anySynthetic

  return {
    schemaVersion: 'openagents.khala.benchmark-report.v1',
    configId: runSet.configId,
    seamId: runSet.seamId,
    decisionGrade,
    illustrativeNotice: decisionGrade ? '' : ILLUSTRATIVE_NOTICE,
    cellsExpanded: runSet.cellsExpanded,
    cellsExecuted: runSet.cellsExecuted,
    cellsSkipped: runSet.cellsSkipped,
    groups,
  }
}

// ---------------------------------------------------------------------------
// Public-safety self-check (INVARIANTS: no raw prompt/account/secret leakage).
// ---------------------------------------------------------------------------

// The forbidden substrings a public benchmark report must NEVER contain. This is
// a STRUCTURAL guard, not intent routing: it scans the serialized report for
// field names / values that would indicate a prompt, account, raw cache key,
// price, or margin leaked in. The report is built only from counts/durations/
// classifiers, so this should always pass — it is a regression tripwire.
const FORBIDDEN_REPORT_KEYS: ReadonlyArray<string> = [
  'prompt',
  'completion',
  'message',
  'content',
  'account',
  'apikey',
  'api_key',
  'token:', // a raw token value, not the "...Tokens" count fields
  'cacheaffinitykey', // the raw key (the hash never appears in the report)
  'pricemsat',
  'margin',
  'mnemonic',
  'secret',
]

export type ReportPublicSafety = Readonly<{
  safe: boolean
  violations: ReadonlyArray<string>
}>

// Assert a report is public-safe. Serializes it and checks no forbidden key
// appears. Returns the violations rather than throwing so a test can assert the
// empty list and a caller can decide. PURE.
export const checkReportPublicSafety = (
  report: BenchmarkReport,
): ReportPublicSafety => {
  const serialized = JSON.stringify(report).toLowerCase()
  const violations: Array<string> = []
  for (const key of FORBIDDEN_REPORT_KEYS) {
    if (serialized.includes(key)) {
      violations.push(key)
    }
  }
  return { safe: violations.length === 0, violations }
}
