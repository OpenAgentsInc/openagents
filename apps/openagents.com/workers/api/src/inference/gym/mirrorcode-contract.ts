// Public-safe contract for the MirrorCode-as-a-service gym demo (#6378, epic
// #6376), per docs/benchmarks/2026-06-27-mirrorcode-khala-gym-integration-analysis.md.
//
// MirrorCode (Epoch Research) asks an agent to REIMPLEMENT a real tool from
// scratch in a sandbox over a long horizon, then scores it by running the
// agent's implementation against a held-out test suite it never saw. This module
// owns the shared, public-safe RESULT contract a MirrorCode run emits and the
// ingest boundary the demo surface stores.
//
// Honesty + contamination discipline (non-negotiable, from the analysis):
//   - PUBLIC tasks only. The private paper set is never run or reported here.
//   - NEVER echo task contents. The ingest boundary rejects anything that smells
//     like task source/test data (code fences) or carries the MirrorCode /
//     BIG-Bench canary strings, so raw benchmark content never reaches D1.
//   - A `smoke` (Phase-0) run is in-progress / endpoint-validation evidence and
//     is ALWAYS `decisionGrade: false`. Only an owner-armed `decision_grade` run
//     is a published frontier measurement.
//   - The paper-reference comparator list is ILLUSTRATIVE only. The upstream
//     comparator model ids in scripts/run_mirrorcode.py are forward-dated
//     placeholders; this is not a real head-to-head.
import { Schema as S } from 'effect'

export const MirrorCodeRunsSchemaVersion = 'openagents.gym.mirrorcode_runs.v1'
export type MirrorCodeRunsSchemaVersion = typeof MirrorCodeRunsSchemaVersion
export const MirrorCodeTokenBurnReportSchemaVersion =
  'openagents.gym.mirrorcode_token_burn_report.v1'
export type MirrorCodeTokenBurnReportSchemaVersion =
  typeof MirrorCodeTokenBurnReportSchemaVersion

// The Khala model id is fixed: there is one Khala model, `openagents/khala`.
export const KHALA_MODEL_ID = 'openagents/khala'

// MirrorCode size buckets (paper Table 3, scripts/run_mirrorcode.py).
export const MirrorCodeBucket = S.Literals(['S', 'M', 'L'])
export type MirrorCodeBucket = typeof MirrorCodeBucket.Type

export const MIRRORCODE_PUBLIC_TARGETS_BY_BUCKET = {
  S: [
    'qsv_select',
    'jq_simple',
    'gron',
    'bitwise',
    'hexyl',
    'uuidparse',
    'numfmt',
    'cal',
    'choose',
  ],
  M: [
    'giac',
    'tex',
    'gotree',
    'mailauth',
    'brotli',
    'wren_cli',
    'nonogrid',
    'sed',
    'tssql',
    'bib2json',
  ],
  L: ['ruff', 'pkl', 'cprepro'],
} as const

// Run lifecycle status. `queued`/`running` are in-progress; `passed`/`failed`
// are scored terminal states; `error` is an execution/harness failure.
export const MirrorCodeRunStatus = S.Literals([
  'queued',
  'running',
  'passed',
  'failed',
  'error',
])
export type MirrorCodeRunStatus = typeof MirrorCodeRunStatus.Type

// Grade: `smoke` = Phase-0 endpoint-validation run (never a published
// measurement); `decision_grade` = owner-armed real measurement.
export const MirrorCodeRunGrade = S.Literals(['smoke', 'decision_grade'])
export type MirrorCodeRunGrade = typeof MirrorCodeRunGrade.Type

// Demand attribution (#6298): MirrorCode traffic is internal gym demand and
// must yield to real external demand (#6318).
export const MIRRORCODE_DEMAND_KIND = 'internal'
export const MIRRORCODE_DEMAND_SOURCE = 'gym_mirrorcode'
export const MIRRORCODE_GENERALIZATION_SET_ID = 'mirrorcode_public_tasks_no_rag'
export const MIRRORCODE_MEMORY_POLICY = 'no_rag_public_tasks_only'
export const MIRRORCODE_TOKEN_ATTRIBUTION_TRUTH = 'exact_rows_as_proof'

// Field bounds enforced in `buildMirrorCodeRun` (the codebase avoids inline
// Schema length/number refinements; the build boundary is the single place that
// validates + re-asserts public safety before storage).
const MAX_RUN_ID_LENGTH = 128
const MAX_TASK_ID_LENGTH = 64
const MAX_LANGUAGE_LENGTH = 32
const MAX_SUMMARY_LENGTH = 400

const PUBLIC_REF_PATTERN = /^[A-Za-z0-9_.:-]+$/

// The result contract the runner lane writes (and the POST ingest accepts). It
// is intentionally the small, public-safe shape from the design doc plus the
// honesty fields (grade/attribution). No raw task data, prompts, or trajectories.
export const MirrorCodeRunInput = S.Struct({
  runId: S.String,
  model: S.Literal(KHALA_MODEL_ID),
  taskId: S.String,
  bucket: MirrorCodeBucket,
  // The implementation language for this (target, language) sample. Optional.
  language: S.optional(S.NullOr(S.String)),
  status: MirrorCodeRunStatus,
  // Pass-rate over the scored test cases, a fraction in [0, 1]. Null until a
  // run reaches a scored terminal state.
  passRate: S.optional(S.NullOr(S.Number)),
  // Total tokens spent by this sample (the honest token-sink contribution).
  tokens: S.Struct({ total: S.Number }),
  // Exact downstream token rows backing `tokens.total`. Required before a
  // decision-grade run can publish into the gym ladder.
  exactTokenUsageEventRefs: S.optional(S.Array(S.String)),
  startedAt: S.String,
  finishedAt: S.optional(S.NullOr(S.String)),
  // A short, public-safe human summary. Bounded; no task contents.
  summary: S.String,
  grade: S.optional(MirrorCodeRunGrade),
})
export type MirrorCodeRunInput = typeof MirrorCodeRunInput.Type

// Owner-gated launch intent. This creates an honest queued run row only; the
// external MirrorCode/Inspect executor remains owner-operated and posts later
// status/result updates through `MirrorCodeRunInput`.
export const MirrorCodeLaunchRequest = S.Struct({
  kind: S.Literal('launch'),
  taskId: S.String,
  bucket: MirrorCodeBucket,
  language: S.optional(S.NullOr(S.String)),
  grade: S.optional(MirrorCodeRunGrade),
})
export type MirrorCodeLaunchRequest = typeof MirrorCodeLaunchRequest.Type

// The stored / served public-safe run object.
export const MirrorCodeRun = S.Struct({
  runId: S.String,
  model: S.Literal(KHALA_MODEL_ID),
  taskId: S.String,
  bucket: MirrorCodeBucket,
  language: S.NullOr(S.String),
  status: MirrorCodeRunStatus,
  passRate: S.NullOr(S.Number),
  tokensTotal: S.Number,
  exactTokenUsageEventRefs: S.Array(S.String),
  tokenAttributionTruth: S.Literal(MIRRORCODE_TOKEN_ATTRIBUTION_TRUTH),
  tokenAttributionProofRef: S.String,
  startedAt: S.String,
  finishedAt: S.NullOr(S.String),
  summary: S.String,
  grade: MirrorCodeRunGrade,
  // True only for an owner-armed decision_grade run that has a scored result.
  decisionGrade: S.Boolean,
  demandKind: S.Literal(MIRRORCODE_DEMAND_KIND),
  demandSource: S.Literal(MIRRORCODE_DEMAND_SOURCE),
  generalizationSet: S.Literal(MIRRORCODE_GENERALIZATION_SET_ID),
  memoryPolicy: S.Literal(MIRRORCODE_MEMORY_POLICY),
})
export type MirrorCodeRun = typeof MirrorCodeRun.Type

export const MirrorCodeTokenBurnBucketSummary = S.Struct({
  bucket: MirrorCodeBucket,
  runCount: S.Number,
  totalTokensBurned: S.Number,
  exactTokenBackedTokens: S.Number,
  exactTokenUsageEventRefs: S.Array(S.String),
})
export type MirrorCodeTokenBurnBucketSummary =
  typeof MirrorCodeTokenBurnBucketSummary.Type

export const MirrorCodeTokenBurnStatusSummary = S.Struct({
  status: MirrorCodeRunStatus,
  runCount: S.Number,
  totalTokensBurned: S.Number,
})
export type MirrorCodeTokenBurnStatusSummary =
  typeof MirrorCodeTokenBurnStatusSummary.Type

export const MirrorCodeTokenBurnGradeSummary = S.Struct({
  grade: MirrorCodeRunGrade,
  runCount: S.Number,
  totalTokensBurned: S.Number,
  exactTokenBackedTokens: S.Number,
})
export type MirrorCodeTokenBurnGradeSummary =
  typeof MirrorCodeTokenBurnGradeSummary.Type

export const MirrorCodeTokenBurnTopRun = S.Struct({
  runId: S.String,
  taskId: S.String,
  bucket: MirrorCodeBucket,
  grade: MirrorCodeRunGrade,
  status: MirrorCodeRunStatus,
  tokensTotal: S.Number,
  exactTokenUsageEventRefs: S.Array(S.String),
  tokenAttributionProofRef: S.String,
})
export type MirrorCodeTokenBurnTopRun = typeof MirrorCodeTokenBurnTopRun.Type

export const MirrorCodeTokenBurnReport = S.Struct({
  schemaVersion: S.Literal(MirrorCodeTokenBurnReportSchemaVersion),
  reportRef: S.String,
  model: S.Literal(KHALA_MODEL_ID),
  benchmark: S.Struct({
    name: S.String,
    scope: S.String,
  }),
  runCount: S.Number,
  terminalRunCount: S.Number,
  decisionGradeRunCount: S.Number,
  totalTokensBurned: S.Number,
  exactTokenBackedTokens: S.Number,
  unprovenTokenTotal: S.Number,
  exactTokenUsageEventRefs: S.Array(S.String),
  tokenAttributionTruth: S.Literal(MIRRORCODE_TOKEN_ATTRIBUTION_TRUTH),
  tokenAttributionProofRefs: S.Array(S.String),
  demandKind: S.Literal(MIRRORCODE_DEMAND_KIND),
  demandSource: S.Literal(MIRRORCODE_DEMAND_SOURCE),
  generalizationSet: S.Literal(MIRRORCODE_GENERALIZATION_SET_ID),
  memoryPolicy: S.Literal(MIRRORCODE_MEMORY_POLICY),
  byBucket: S.Array(MirrorCodeTokenBurnBucketSummary),
  byStatus: S.Array(MirrorCodeTokenBurnStatusSummary),
  byGrade: S.Array(MirrorCodeTokenBurnGradeSummary),
  topRuns: S.Array(MirrorCodeTokenBurnTopRun),
  caveatRefs: S.Array(S.String),
})
export type MirrorCodeTokenBurnReport =
  typeof MirrorCodeTokenBurnReport.Type

// Paper-reference comparator (ILLUSTRATIVE — not a head-to-head).
export const MirrorCodeComparator = S.Struct({
  label: S.String,
  model: S.String,
  source: S.Literal('paper_reference_illustrative'),
  note: S.String,
})
export type MirrorCodeComparator = typeof MirrorCodeComparator.Type

// Typed ingest error so the route can map it to a 400 without leaking internals.
export class MirrorCodeRunError extends Error {
  readonly _tag = 'MirrorCodeRunError'
  constructor(reason: string) {
    super(reason)
    this.name = 'MirrorCodeRunError'
  }
}

// Public-safety boundary: reject anything that could echo task contents or
// carry a contamination canary. Operates on the serialized input so it catches
// leakage in any field.
const CONTAMINATION_MARKERS = [
  'mirrorcode:', // the MirrorCode canary string prefix
  'BIG-BENCH', // the BIG-Bench canary
  'BIG-bench',
  'canary GUID',
]

const assertBounds = (input: MirrorCodeRunInput): void => {
  if (input.runId.length < 1 || input.runId.length > MAX_RUN_ID_LENGTH) {
    throw new MirrorCodeRunError('runId must be 1..128 characters.')
  }
  if (input.taskId.length < 1 || input.taskId.length > MAX_TASK_ID_LENGTH) {
    throw new MirrorCodeRunError('taskId must be 1..64 characters.')
  }
  if (
    input.language !== undefined &&
    input.language !== null &&
    input.language.length > MAX_LANGUAGE_LENGTH
  ) {
    throw new MirrorCodeRunError('language must be <= 32 characters.')
  }
  if (input.summary.length > MAX_SUMMARY_LENGTH) {
    throw new MirrorCodeRunError('summary must be <= 400 characters.')
  }
  if (input.startedAt.length < 1) {
    throw new MirrorCodeRunError('startedAt is required.')
  }
  if (!Number.isFinite(input.tokens.total) || input.tokens.total < 0) {
    throw new MirrorCodeRunError('tokens.total must be a non-negative number.')
  }
  if (
    input.passRate !== undefined &&
    input.passRate !== null &&
    (!Number.isFinite(input.passRate) ||
      input.passRate < 0 ||
      input.passRate > 1)
  ) {
    throw new MirrorCodeRunError('passRate must be a fraction in [0, 1].')
  }
  for (const ref of input.exactTokenUsageEventRefs ?? []) {
    if (ref.length < 1 || ref.length > 160 || !PUBLIC_REF_PATTERN.test(ref)) {
      throw new MirrorCodeRunError(
        'exactTokenUsageEventRefs must be public-safe refs.',
      )
    }
  }
}

export const isMirrorCodePublicTargetForBucket = (
  bucket: MirrorCodeBucket,
  taskId: string,
): boolean =>
  (MIRRORCODE_PUBLIC_TARGETS_BY_BUCKET[bucket] as ReadonlyArray<string>).includes(
    taskId,
  )

const assertPublicTarget = (input: {
  readonly bucket: MirrorCodeBucket
  readonly taskId: string
}): void => {
  if (!isMirrorCodePublicTargetForBucket(input.bucket, input.taskId)) {
    throw new MirrorCodeRunError(
      'taskId must be a public MirrorCode target for the selected bucket.',
    )
  }
}

const assertPublicSafe = (input: MirrorCodeRunInput): void => {
  // No code fences anywhere — task source/tests would arrive that way.
  const serialized = JSON.stringify(input)
  if (serialized.includes('```')) {
    throw new MirrorCodeRunError(
      'A MirrorCode run record must not contain code blocks (possible task contents).',
    )
  }
  for (const marker of CONTAMINATION_MARKERS) {
    if (serialized.includes(marker)) {
      throw new MirrorCodeRunError(
        'A MirrorCode run record must not contain benchmark canary strings.',
      )
    }
  }
}

const decisionGradeFor = (
  grade: MirrorCodeRunGrade,
  status: MirrorCodeRunStatus,
): boolean =>
  grade === 'decision_grade' && (status === 'passed' || status === 'failed')

const tokenAttributionProofRefFor = (runId: string): string =>
  `proof.gym.mirrorcode.exact_token_rows.${runId.replace(/[^A-Za-z0-9_.:-]/g, '_')}`

// Build the public-safe stored run from a raw ingest body. Validates the shape,
// re-asserts the public-safety boundary, and derives the honesty fields. Throws
// MirrorCodeRunError on any rejection so nothing unsafe is ever stored.
export const buildMirrorCodeRun = (raw: unknown): MirrorCodeRun => {
  let input: MirrorCodeRunInput
  try {
    input = S.decodeUnknownSync(MirrorCodeRunInput)(raw)
  } catch (error) {
    throw new MirrorCodeRunError(
      error instanceof Error ? error.message : String(error),
    )
  }
  assertBounds(input)
  assertPublicTarget(input)
  assertPublicSafe(input)

  const grade = input.grade ?? 'smoke'
  const status = input.status
  const exactTokenUsageEventRefs = [...new Set(input.exactTokenUsageEventRefs ?? [])]
  if (
    decisionGradeFor(grade, status) &&
    exactTokenUsageEventRefs.length === 0
  ) {
    throw new MirrorCodeRunError(
      'decision_grade MirrorCode runs require exactTokenUsageEventRefs.',
    )
  }
  // A scored pass-rate is only meaningful for a scored terminal state.
  const passRate =
    status === 'passed' || status === 'failed'
      ? (input.passRate ?? null)
      : null

  return {
    runId: input.runId,
    model: KHALA_MODEL_ID,
    taskId: input.taskId,
    bucket: input.bucket,
    language: input.language ?? null,
    status,
    passRate,
    tokensTotal: input.tokens.total,
    exactTokenUsageEventRefs,
    tokenAttributionTruth: MIRRORCODE_TOKEN_ATTRIBUTION_TRUTH,
    tokenAttributionProofRef: tokenAttributionProofRefFor(input.runId),
    startedAt: input.startedAt,
    finishedAt: input.finishedAt ?? null,
    summary: input.summary,
    grade,
    decisionGrade: decisionGradeFor(grade, status),
    demandKind: MIRRORCODE_DEMAND_KIND,
    demandSource: MIRRORCODE_DEMAND_SOURCE,
    generalizationSet: MIRRORCODE_GENERALIZATION_SET_ID,
    memoryPolicy: MIRRORCODE_MEMORY_POLICY,
  }
}

const runIdPart = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 36)

export const buildMirrorCodeLaunchRun = (
  raw: unknown,
  nowIso: string,
): MirrorCodeRun => {
  let launch: MirrorCodeLaunchRequest
  try {
    launch = S.decodeUnknownSync(MirrorCodeLaunchRequest)(raw)
  } catch (error) {
    throw new MirrorCodeRunError(
      error instanceof Error ? error.message : String(error),
    )
  }

  const taskPart = runIdPart(launch.taskId)
  const languagePart = runIdPart(launch.language ?? 'default')
  const timestampPart = nowIso.replace(/\D/g, '').slice(0, 14)
  if (taskPart.length < 1) {
    throw new MirrorCodeRunError('taskId must contain a public-safe id.')
  }
  assertPublicTarget(launch)
  if (launch.grade === 'decision_grade') {
    throw new MirrorCodeRunError(
      'MirrorCode launch intents are smoke-only; post the scored decision_grade result with exactTokenUsageEventRefs after execution.',
    )
  }

  return buildMirrorCodeRun({
    runId: `mc-${launch.bucket.toLowerCase()}-${taskPart}-${languagePart}-${timestampPart}`,
    model: KHALA_MODEL_ID,
    taskId: launch.taskId,
    bucket: launch.bucket,
    language: launch.language ?? null,
    status: 'queued',
    passRate: null,
    tokens: { total: 0 },
    startedAt: nowIso,
    finishedAt: null,
    summary: `Owner-gated MirrorCode launch queued for ${launch.taskId} (${launch.bucket} bucket) through openagents/khala.`,
    grade: 'smoke',
  })
}

// The paper-reference comparators surfaced on the leaderboard scaffold. These
// are ILLUSTRATIVE: the upstream comparator model ids in
// scripts/run_mirrorcode.py are forward-dated placeholders, and we report only
// the public-task subset — so this is never a real head-to-head against the
// paper's private-set headline. Labeled accordingly on the wire and the page.
export const MIRRORCODE_PAPER_REFERENCE_COMPARATORS: ReadonlyArray<MirrorCodeComparator> =
  [
    {
      label: 'Claude (paper-reference)',
      model: 'anthropic/claude-opus-4-7',
      source: 'paper_reference_illustrative',
      note: 'Illustrative comparator id from upstream scripts/run_mirrorcode.py; forward-dated placeholder, not a head-to-head.',
    },
    {
      label: 'GPT (paper-reference)',
      model: 'openai/gpt-5.5',
      source: 'paper_reference_illustrative',
      note: 'Illustrative comparator id from upstream scripts/run_mirrorcode.py; forward-dated placeholder, not a head-to-head.',
    },
    {
      label: 'Gemini (paper-reference)',
      model: 'google/gemini-3.1-pro-preview',
      source: 'paper_reference_illustrative',
      note: 'Illustrative comparator id from upstream scripts/run_mirrorcode.py; forward-dated placeholder, not a head-to-head.',
    },
  ]

export const MIRRORCODE_BENCHMARK_LABEL = {
  name: 'Epoch Research MirrorCode',
  scope: 'public tasks only (private set excluded)',
} as const

const MIRRORCODE_BUCKET_ORDER: ReadonlyArray<MirrorCodeBucket> = ['S', 'M', 'L']
const MIRRORCODE_STATUS_ORDER: ReadonlyArray<MirrorCodeRunStatus> = [
  'queued',
  'running',
  'passed',
  'failed',
  'error',
]
const MIRRORCODE_GRADE_ORDER: ReadonlyArray<MirrorCodeRunGrade> = [
  'smoke',
  'decision_grade',
]
const MIRRORCODE_TOKEN_BURN_TOP_RUN_LIMIT = 5

const uniquePublicRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => [...new Set(refs)].sort()

const exactBackedTokensFor = (run: MirrorCodeRun): number =>
  run.exactTokenUsageEventRefs.length > 0 ? run.tokensTotal : 0

export const buildMirrorCodeTokenBurnReport = (
  runs: ReadonlyArray<MirrorCodeRun>,
): MirrorCodeTokenBurnReport => {
  const totalTokensBurned = runs.reduce(
    (sum, run) => sum + run.tokensTotal,
    0,
  )
  const exactTokenBackedTokens = runs.reduce(
    (sum, run) => sum + exactBackedTokensFor(run),
    0,
  )
  const exactTokenUsageEventRefs = uniquePublicRefs(
    runs.flatMap(run => run.exactTokenUsageEventRefs),
  )
  const tokenAttributionProofRefs = uniquePublicRefs(
    runs
      .filter(run => run.exactTokenUsageEventRefs.length > 0)
      .map(run => run.tokenAttributionProofRef),
  )

  return {
    schemaVersion: MirrorCodeTokenBurnReportSchemaVersion,
    reportRef: 'report.gym.mirrorcode.token_burn.live',
    model: KHALA_MODEL_ID,
    benchmark: MIRRORCODE_BENCHMARK_LABEL,
    runCount: runs.length,
    terminalRunCount: runs.filter(run =>
      ['passed', 'failed', 'error'].includes(run.status),
    ).length,
    decisionGradeRunCount: runs.filter(run => run.decisionGrade).length,
    totalTokensBurned,
    exactTokenBackedTokens,
    unprovenTokenTotal: totalTokensBurned - exactTokenBackedTokens,
    exactTokenUsageEventRefs,
    tokenAttributionTruth: MIRRORCODE_TOKEN_ATTRIBUTION_TRUTH,
    tokenAttributionProofRefs,
    demandKind: MIRRORCODE_DEMAND_KIND,
    demandSource: MIRRORCODE_DEMAND_SOURCE,
    generalizationSet: MIRRORCODE_GENERALIZATION_SET_ID,
    memoryPolicy: MIRRORCODE_MEMORY_POLICY,
    byBucket: MIRRORCODE_BUCKET_ORDER.map(bucket => {
      const bucketRuns = runs.filter(run => run.bucket === bucket)
      return {
        bucket,
        runCount: bucketRuns.length,
        totalTokensBurned: bucketRuns.reduce(
          (sum, run) => sum + run.tokensTotal,
          0,
        ),
        exactTokenBackedTokens: bucketRuns.reduce(
          (sum, run) => sum + exactBackedTokensFor(run),
          0,
        ),
        exactTokenUsageEventRefs: uniquePublicRefs(
          bucketRuns.flatMap(run => run.exactTokenUsageEventRefs),
        ),
      }
    }),
    byStatus: MIRRORCODE_STATUS_ORDER.map(status => {
      const statusRuns = runs.filter(run => run.status === status)
      return {
        status,
        runCount: statusRuns.length,
        totalTokensBurned: statusRuns.reduce(
          (sum, run) => sum + run.tokensTotal,
          0,
        ),
      }
    }),
    byGrade: MIRRORCODE_GRADE_ORDER.map(grade => {
      const gradeRuns = runs.filter(run => run.grade === grade)
      return {
        grade,
        runCount: gradeRuns.length,
        totalTokensBurned: gradeRuns.reduce(
          (sum, run) => sum + run.tokensTotal,
          0,
        ),
        exactTokenBackedTokens: gradeRuns.reduce(
          (sum, run) => sum + exactBackedTokensFor(run),
          0,
        ),
      }
    }),
    topRuns: [...runs]
      .sort((left, right) => right.tokensTotal - left.tokensTotal)
      .slice(0, MIRRORCODE_TOKEN_BURN_TOP_RUN_LIMIT)
      .map(run => ({
        runId: run.runId,
        taskId: run.taskId,
        bucket: run.bucket,
        grade: run.grade,
        status: run.status,
        tokensTotal: run.tokensTotal,
        exactTokenUsageEventRefs: run.exactTokenUsageEventRefs,
        tokenAttributionProofRef: run.tokenAttributionProofRef,
      })),
    caveatRefs: [
      'caveat.public.gym.mirrorcode.public_tasks_only_private_set_excluded',
      'caveat.public.gym.mirrorcode.token_burn_internal_demand',
      'caveat.public.gym.mirrorcode.exact_token_rows_required_for_proof',
    ],
  }
}
