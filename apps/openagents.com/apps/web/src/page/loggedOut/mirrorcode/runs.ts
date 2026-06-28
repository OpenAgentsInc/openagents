// Browser-side mirror of the public MirrorCode runs projection (#6378).
//
// MirrorCode is "powered by Khala": the `openagents/khala` model reimplements
// real tools from scratch in a sandbox and is scored by a held-out test suite
// (the Epoch Research MirrorCode benchmark, PUBLIC TASKS ONLY — the private set
// is excluded). The `/mirrorcode` page renders the LIVE run set from the Worker
// (`GET /api/gym/mirrorcode/runs`); the Worker is the authority and this
// file holds only the browser decode schema + small formatting helpers.
//
// Honesty discipline: a `grade: "smoke"` run is a Phase-0 smoke, never a
// published frontier measurement, and the paper-reference comparators are
// explicitly illustrative (not a head-to-head). `passRate` is a fraction 0..1
// rendered as a percentage; a null pass-rate is "not measured", never 0.
import { Schema as S } from 'effect'

export const MIRRORCODE_RUNS_SCHEMA =
  'openagents.gym.mirrorcode_runs.v1' as const

// The five run lifecycle states. Each gets a small colored marker in the view
// (passed=positive, failed/error=negative, running=info, queued=muted).
export const MirrorCodeRunStatus = S.Literals([
  'queued',
  'running',
  'passed',
  'failed',
  'error',
])
export type MirrorCodeRunStatus = typeof MirrorCodeRunStatus.Type

// Task size buckets, mirroring the benchmark's S/M/L grouping.
export const MirrorCodeBucket = S.Literals(['S', 'M', 'L'])
export type MirrorCodeBucket = typeof MirrorCodeBucket.Type

// A `smoke` run is a bounded Phase-0 machinery check; only a `decision_grade`
// run is a real measurement. The view labels smoke runs as "Phase-0 smoke" and
// never as a published frontier number.
export const MirrorCodeGrade = S.Literals(['smoke', 'decision_grade'])
export type MirrorCodeGrade = typeof MirrorCodeGrade.Type

const MaybeNumber = S.NullOr(S.Number)
const MaybeString = S.NullOr(S.String)

// One Khala run row. `model`/`taskId`/`language` stay loose strings so a new
// task or language the Worker adds never breaks the public render.
export const MirrorCodeRun = S.Struct({
  runId: S.String,
  model: S.String,
  taskId: S.String,
  bucket: MirrorCodeBucket,
  language: MaybeString,
  status: MirrorCodeRunStatus,
  passRate: MaybeNumber,
  tokensTotal: S.Number,
  exactTokenUsageEventRefs: S.Array(S.String),
  tokenAttributionTruth: S.String,
  tokenAttributionProofRef: S.String,
  startedAt: S.String,
  finishedAt: MaybeString,
  summary: S.String,
  grade: MirrorCodeGrade,
  decisionGrade: S.Boolean,
  demandKind: S.String,
  demandSource: S.String,
  generalizationSet: S.String,
  memoryPolicy: S.String,
})
export type MirrorCodeRun = typeof MirrorCodeRun.Type

// A paper-reference comparator. ALWAYS illustrative — `source` is
// `paper_reference_illustrative` and the view labels the whole table as "not a
// head-to-head".
export const MirrorCodeComparator = S.Struct({
  label: S.String,
  model: S.String,
  source: S.String,
  note: S.String,
})
export type MirrorCodeComparator = typeof MirrorCodeComparator.Type

export const MirrorCodeBenchmark = S.Struct({
  name: S.String,
  scope: S.String,
})
export type MirrorCodeBenchmark = typeof MirrorCodeBenchmark.Type

// The `GET /api/gym/mirrorcode/runs` envelope. Effect Schema ignores
// excess properties on decode, so the `staleness`/`scope` envelope fields are
// tolerated without being declared here; we render the fields below.
export const MirrorCodeRunsResponse = S.Struct({
  generatedAt: S.String,
  model: S.String,
  benchmark: MirrorCodeBenchmark,
  runs: S.Array(MirrorCodeRun),
  comparators: S.Array(MirrorCodeComparator),
})
export type MirrorCodeRunsResponse = typeof MirrorCodeRunsResponse.Type

// ---------------------------------------------------------------------------
// Formatting helpers.
// ---------------------------------------------------------------------------

export const formatMirrorCodePassRate = (value: number | null): string =>
  value === null ? 'not measured' : `${(value * 100).toFixed(1)}%`

// Compact token count, e.g. 12_300_000 -> "12.3M", 4_200 -> "4.2K".
export const formatMirrorCodeTokens = (value: number): string => {
  if (value < 1_000) {
    return value.toLocaleString('en-US')
  }
  if (value < 1_000_000) {
    return `${(value / 1_000).toFixed(1)}K`
  }
  if (value < 1_000_000_000) {
    return `${(value / 1_000_000).toFixed(1)}M`
  }
  return `${(value / 1_000_000_000).toFixed(1)}B`
}

export const mirrorCodeStatusLabel = (status: MirrorCodeRunStatus): string =>
  status === 'passed'
    ? 'passed'
    : status === 'failed'
      ? 'failed'
      : status === 'error'
        ? 'error'
        : status === 'running'
          ? 'running'
          : 'queued'

// The grade label shown on each run, keeping a smoke run honest.
export const mirrorCodeGradeLabel = (grade: MirrorCodeGrade): string =>
  grade === 'smoke' ? 'Phase-0 smoke' : 'decision-grade'
