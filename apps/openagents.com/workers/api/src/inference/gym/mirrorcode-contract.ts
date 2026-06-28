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

// The Khala model id is fixed: there is one Khala model, `openagents/khala`.
export const KHALA_MODEL_ID = 'openagents/khala'

// MirrorCode size buckets (paper Table 3, scripts/run_mirrorcode.py).
export const MirrorCodeBucket = S.Literals(['S', 'M', 'L'])
export type MirrorCodeBucket = typeof MirrorCodeBucket.Type

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

// Field bounds enforced in `buildMirrorCodeRun` (the codebase avoids inline
// Schema length/number refinements; the build boundary is the single place that
// validates + re-asserts public safety before storage).
const MAX_RUN_ID_LENGTH = 128
const MAX_TASK_ID_LENGTH = 64
const MAX_LANGUAGE_LENGTH = 32
const MAX_SUMMARY_LENGTH = 400

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
  startedAt: S.String,
  finishedAt: S.optional(S.NullOr(S.String)),
  // A short, public-safe human summary. Bounded; no task contents.
  summary: S.String,
  grade: S.optional(MirrorCodeRunGrade),
})
export type MirrorCodeRunInput = typeof MirrorCodeRunInput.Type

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
  startedAt: S.String,
  finishedAt: S.NullOr(S.String),
  summary: S.String,
  grade: MirrorCodeRunGrade,
  // True only for an owner-armed decision_grade run that has a scored result.
  decisionGrade: S.Boolean,
  demandKind: S.Literal(MIRRORCODE_DEMAND_KIND),
  demandSource: S.Literal(MIRRORCODE_DEMAND_SOURCE),
})
export type MirrorCodeRun = typeof MirrorCodeRun.Type

// Paper-reference comparator (ILLUSTRATIVE — not a head-to-head).
export const MirrorCodeComparator = S.Struct({
  label: S.String,
  model: S.String,
  source: S.Literal('paper_reference_illustrative'),
  note: S.String,
})
export type MirrorCodeComparator = typeof MirrorCodeComparator.Type

export const MirrorCodeGeneralizationSet = S.Struct({
  setRef: S.Literal('gg.mirrorcode.public_tasks.no_rag.v1'),
  metric: S.Literal('generalization_gain'),
  scope: S.Literal('public_tasks_only'),
  privateSet: S.Literal('excluded'),
  taskContentExposure: S.Literal('never_store_or_echo'),
  retrievalPolicy: S.Literal('no_rag_on_tasks'),
  trainingPolicy: S.Literal('no_training_or_optimization_on_tasks'),
  decisionGradeRule: S.Literal('owner_armed_scored_terminal_runs_only'),
})
export type MirrorCodeGeneralizationSet =
  typeof MirrorCodeGeneralizationSet.Type

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
  assertPublicSafe(input)

  const grade = input.grade ?? 'smoke'
  const status = input.status
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
    startedAt: input.startedAt,
    finishedAt: input.finishedAt ?? null,
    summary: input.summary,
    grade,
    decisionGrade: decisionGradeFor(grade, status),
    demandKind: MIRRORCODE_DEMAND_KIND,
    demandSource: MIRRORCODE_DEMAND_SOURCE,
  }
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

export const MIRRORCODE_GENERALIZATION_SET: MirrorCodeGeneralizationSet = {
  decisionGradeRule: 'owner_armed_scored_terminal_runs_only',
  metric: 'generalization_gain',
  privateSet: 'excluded',
  retrievalPolicy: 'no_rag_on_tasks',
  scope: 'public_tasks_only',
  setRef: 'gg.mirrorcode.public_tasks.no_rag.v1',
  taskContentExposure: 'never_store_or_echo',
  trainingPolicy: 'no_training_or_optimization_on_tasks',
}
