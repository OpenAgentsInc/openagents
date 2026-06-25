// Live Gym / Harbor run progress (#6261, epic #6253).
//
// A Harbor Terminal-Bench job is long-running: tasks complete one at a time, and
// an operator currently has to tail the local `result.json` / Harbor jobs dir to
// follow it. This module turns an in-progress Harbor job (or an already-ingested
// summary) into a PUBLIC-SAFE live progress object the `/gym` follow-along view
// and a scoped operator endpoint can render WITHOUT exposing any raw prompt,
// response, log, trajectory, key, or private endpoint.
//
// The boundary discipline mirrors the completed-run path: the
// `hydralisk.evals.terminal_bench.summary.v1` summarizer + the benchmark
// `checkReportPublicSafety` tripwire only ever surface counts / denominators /
// timings. Live progress is the SAME shape, one task-completion earlier: it
// carries completed/running/pending/error/cancelled COUNTS, an official
// denominator, a pass-rate over COMPLETED tasks only, token COUNTS when safe,
// public-safe serving-profile refs (never raw endpoint URLs), and freshness.
//
// HONESTY (the whole point of this issue):
//   - `inProgress: true` and `decisionGrade: false` are HARD literals on a
//     progress object. A partial denominator is NEVER a final benchmark claim.
//   - pass-rate is over COMPLETED tasks, with the official denominator carried
//     separately, so a reader can never mistake "12/12 completed passed" for
//     "12/89 official solve rate".
//   - publication state is explicit: a `local_only` run is honestly labeled as
//     not yet authorized for web publication rather than faked into a number.
//   - missing token/timing data is `not_measured` (null), never coerced to 0.
//
// PURE: no Worker, no D1, no network, no clock. Same input → same progress.
import { Schema as S } from 'effect'

import {
  GYM_HARBOR_TERMINAL_BENCH_INGEST_SCHEMA,
  GymTerminalBenchProfileRef,
  GymHarborTerminalBenchModelId,
  HarborTerminalBenchAgent,
  GYM_TERMINAL_BENCH_PROFILE_CATALOG,
} from './harbor-dispatch'

export const GYM_RUN_PROGRESS_SCHEMA = 'openagents.gym.run_progress.v1' as const

// The job's lifecycle phase as reflected by the live counts. `running` covers any
// job with at least one task still in flight; `completed`/`cancelled`/`errored`
// are terminal. A terminal progress object is STILL `decisionGrade: false`: a
// finished JOB is not a finished decision-grade REPORT — that gate lives on the
// owner-armed report path (Epic F / #6242), never here.
export const GymRunPhase = S.Literals([
  'queued',
  'running',
  'completed',
  'cancelled',
  'errored',
])
export type GymRunPhase = typeof GymRunPhase.Type

// Whether this run is authorized to be shown on the public web surface yet.
// `local_only` runs are visible to scoped operators but the public projection
// MUST degrade honestly instead of inventing numbers.
export const GymRunPublication = S.Literals([
  'local_only',
  'web_authorized',
])
export type GymRunPublication = typeof GymRunPublication.Type

// A measured-or-absent count/timing. null is the honest `not_measured` sentinel:
// it is NOT 0. A run with no token telemetry yet reports null, never 0 tokens.
const MaybeNumber = S.NullOr(S.Number)

// Public-safe serving-profile metadata. ONLY refs/labels/coarse hardware — never
// the raw `modelEndpointRef` URL or any private endpoint material.
export const GymRunProgressProfile = S.Struct({
  profileRef: GymTerminalBenchProfileRef,
  publicLabel: S.String,
  model: GymHarborTerminalBenchModelId,
  // The public attribution string (e.g. "Z.ai GLM-5.2 ..."), never a vendor
  // serving endpoint. The raw `modelEndpointRef` is intentionally dropped.
  attribution: S.String,
  // Coarse hardware profile label (e.g. "hydralisk-g4-4x-rtx-pro-6000"); a label,
  // not an address.
  hardwareProfile: S.String,
  contextWindowTokens: S.Number,
})
export type GymRunProgressProfile = typeof GymRunProgressProfile.Type

// The live task counts. They are independent buckets; `completed` is the sum of
// passed+failed (a completed task has a verdict), so pass-rate is over completed.
export const GymRunProgressCounts = S.Struct({
  // Official task-set denominator (e.g. 89 for terminal-bench@2.0). The fixed
  // total the run is measured against — carried separately from `completed` so a
  // partial denominator is never confused with the official one.
  officialDenominator: S.Number,
  completed: S.Number,
  // Of the completed tasks, how many passed verification (the verdict numerator).
  completedPassed: S.Number,
  completedFailed: S.Number,
  running: S.Number,
  pending: S.Number,
  error: S.Number,
  cancelled: S.Number,
})
export type GymRunProgressCounts = typeof GymRunProgressCounts.Type

export const GymRunProgressTokens = S.Struct({
  promptTokens: MaybeNumber,
  completionTokens: MaybeNumber,
  totalTokens: MaybeNumber,
})
export type GymRunProgressTokens = typeof GymRunProgressTokens.Type

export const GymRunProgress = S.Struct({
  schemaVersion: S.Literal(GYM_RUN_PROGRESS_SCHEMA),
  // Public-safe run ref + the originating job ref (both safe ref strings).
  runRef: S.String,
  jobRef: S.String,
  configId: S.String,
  environmentRef: S.Literal('terminal-bench'),
  datasetRef: S.Literal('terminal-bench@2.0'),
  runner: S.Literal('harbor'),
  agent: HarborTerminalBenchAgent,
  profile: GymRunProgressProfile,
  phase: GymRunPhase,
  // HARD honesty markers. A progress object is never decision-grade and is
  // always flagged in-progress for partial phases.
  decisionGrade: S.Literal(false),
  inProgress: S.Boolean,
  publication: GymRunPublication,
  counts: GymRunProgressCounts,
  // Pass-rate over COMPLETED tasks only (completedPassed / completed). null when
  // nothing has completed yet (honest absence, never 0% over an empty set).
  passRateOverCompleted: MaybeNumber,
  // The fraction of the official denominator that has completed so far — the
  // PROGRESS bar, explicitly NOT the solve rate.
  completionFraction: S.Number,
  tokens: GymRunProgressTokens,
  // Wall-clock ms elapsed for the run so far; null when unmeasured.
  elapsedMs: MaybeNumber,
  // ISO timestamp of the last observed update (freshness). Carried as a string;
  // the reader applies its own staleness contract.
  lastUpdatedAt: S.String,
  // Public-safe caveat / blocker refs (ref strings only, no free text leakage).
  caveatRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
})
export type GymRunProgress = typeof GymRunProgress.Type

// ---------------------------------------------------------------------------
// Ingest input: an in-progress Harbor job snapshot.
// ---------------------------------------------------------------------------
//
// This is the SHAPE the Hydralisk Harbor harness produces for a live job —
// essentially the in-progress projection of `result.json` plus the typed job
// context the Worker already holds. It is deliberately counts-only on the Worker
// boundary: the harness does the on-host parse of the Harbor jobs dir, and the
// Worker never imports Harbor or sees raw artifacts (the same boundary as
// `harbor-dispatch.ts`).
export const GymRunProgressInput = S.Struct({
  runRef: S.String,
  jobRef: S.String,
  configId: S.String,
  profileRef: GymTerminalBenchProfileRef,
  agent: HarborTerminalBenchAgent,
  phase: GymRunPhase,
  publication: GymRunPublication,
  officialDenominator: S.Number,
  completedPassed: S.Number,
  completedFailed: S.Number,
  running: S.Number,
  pending: S.Number,
  error: S.Number,
  cancelled: S.Number,
  promptTokens: MaybeNumber,
  completionTokens: MaybeNumber,
  elapsedMs: MaybeNumber,
  lastUpdatedAt: S.String,
  caveatRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
})
export type GymRunProgressInput = typeof GymRunProgressInput.Type

export class GymRunProgressError extends S.TaggedErrorClass<GymRunProgressError>()(
  'GymRunProgressError',
  {
    reason: S.Literals([
      'invalid_counts',
      'unsafe_progress',
      'unknown_profile',
    ]),
    message: S.String,
  },
) {}

const decodeInput = S.decodeUnknownSync(GymRunProgressInput)
const decodeProgress = S.decodeUnknownSync(GymRunProgress)

// ---------------------------------------------------------------------------
// Public-safety tripwire (mirrors checkReportPublicSafety / the Hydralisk
// summary safety boundary). Counts/denominators/timings only — never a prompt,
// response, raw log, trajectory, key, or private endpoint URL.
// ---------------------------------------------------------------------------

// Value/marker shapes that indicate a raw leak, NOT bare field-name substrings
// (the object legitimately has `promptTokens`/`completionTokens` COUNT fields, so
// the markers reserve the leak SHAPES: `prompt:`, `prompt_text`, `completion_text`,
// etc.). This mirrors the value-based discipline of the trace tripwire and the
// benchmark report's reserved-substring guard.
const FORBIDDEN_PROGRESS_MARKERS: ReadonlyArray<string> = [
  'prompt:',
  'prompt_text',
  'rawprompt',
  'completion_text',
  'response_text',
  'rawresponse',
  'trajectory',
  'apikey',
  'api_key',
  'bearer ',
  'authorization:',
  'mnemonic',
  'secret',
  'http://',
  'https://',
  // private endpoint refs the catalog holds internally but the projection drops.
  '.private_openai_compat',
  'openai_api_base',
]

export type GymRunProgressPublicSafety = Readonly<{
  safe: boolean
  violations: ReadonlyArray<string>
}>

// Assert a built progress object is public-safe. The object is constructed only
// from counts/labels/refs, so this is a regression tripwire, not a scrubber. The
// token COUNT fields are named `promptTokens`/`completionTokens`/`totalTokens`, so
// the markers reserve the leak SHAPES (`prompt:`, `completion_text`, ...) rather
// than the bare count field names. PURE.
export const checkGymRunProgressPublicSafety = (
  progress: GymRunProgress,
): GymRunProgressPublicSafety => {
  const serialized = JSON.stringify(progress).toLowerCase()
  const violations: Array<string> = []
  for (const marker of FORBIDDEN_PROGRESS_MARKERS) {
    if (serialized.includes(marker)) {
      violations.push(marker)
    }
  }
  return { safe: violations.length === 0, violations }
}

// ---------------------------------------------------------------------------
// Build.
// ---------------------------------------------------------------------------

const profileMetadata = (
  profileRef: GymTerminalBenchProfileRef,
): GymRunProgressProfile => {
  const profile = GYM_TERMINAL_BENCH_PROFILE_CATALOG[profileRef]
  if (profile === undefined) {
    throw new GymRunProgressError({
      reason: 'unknown_profile',
      message: `Unknown Terminal-Bench serving profile ${profileRef}.`,
    })
  }
  // Public-safe subset ONLY — the raw `modelEndpointRef` (a private endpoint
  // address) and `sourceModelRef`/`sampler` internals are intentionally dropped.
  return {
    profileRef: profile.profileRef,
    publicLabel: profile.publicLabel,
    model: profile.model,
    attribution: profile.attribution,
    hardwareProfile: profile.hardwareProfile,
    contextWindowTokens: profile.contextWindowTokens,
  }
}

const isTerminalPhase = (phase: GymRunPhase): boolean =>
  phase === 'completed' || phase === 'cancelled' || phase === 'errored'

const tokenTotal = (
  prompt: number | null,
  completion: number | null,
): number | null => {
  if (prompt === null && completion === null) {
    return null
  }
  return (prompt ?? 0) + (completion ?? 0)
}

// Build a public-safe live progress object from an in-progress Harbor snapshot.
// Validates the counts are internally consistent, projects only public-safe
// serving-profile metadata, computes pass-rate over COMPLETED tasks (not the
// official denominator), and asserts public-safety before returning. PURE.
export const buildGymRunProgress = (
  raw: unknown,
): GymRunProgress => {
  const input = decodeInput(raw)
  const completed = input.completedPassed + input.completedFailed

  const negative = [
    input.officialDenominator,
    input.completedPassed,
    input.completedFailed,
    input.running,
    input.pending,
    input.error,
    input.cancelled,
  ].some(value => value < 0 || !Number.isFinite(value))
  if (negative) {
    throw new GymRunProgressError({
      reason: 'invalid_counts',
      message: 'Live progress counts must be finite and non-negative.',
    })
  }

  const accountedFor =
    completed + input.running + input.pending + input.error + input.cancelled
  if (input.officialDenominator > 0 && accountedFor > input.officialDenominator) {
    throw new GymRunProgressError({
      reason: 'invalid_counts',
      message:
        'Live progress accounted tasks must not exceed the official denominator.',
    })
  }

  const passRateOverCompleted =
    completed === 0 ? null : input.completedPassed / completed
  const completionFraction =
    input.officialDenominator <= 0
      ? 0
      : Math.min(1, completed / input.officialDenominator)

  const progress = decodeProgress({
    schemaVersion: GYM_RUN_PROGRESS_SCHEMA,
    runRef: input.runRef,
    jobRef: input.jobRef,
    configId: input.configId,
    environmentRef: 'terminal-bench',
    datasetRef: 'terminal-bench@2.0',
    runner: 'harbor',
    agent: input.agent,
    profile: profileMetadata(input.profileRef),
    phase: input.phase,
    decisionGrade: false,
    inProgress: !isTerminalPhase(input.phase),
    publication: input.publication,
    counts: {
      officialDenominator: input.officialDenominator,
      completed,
      completedPassed: input.completedPassed,
      completedFailed: input.completedFailed,
      running: input.running,
      pending: input.pending,
      error: input.error,
      cancelled: input.cancelled,
    },
    passRateOverCompleted,
    completionFraction,
    tokens: {
      promptTokens: input.promptTokens,
      completionTokens: input.completionTokens,
      totalTokens: tokenTotal(input.promptTokens, input.completionTokens),
    },
    elapsedMs: input.elapsedMs,
    lastUpdatedAt: input.lastUpdatedAt,
    caveatRefs: input.caveatRefs,
    blockerRefs: input.blockerRefs,
  })

  const safety = checkGymRunProgressPublicSafety(progress)
  if (!safety.safe) {
    throw new GymRunProgressError({
      reason: 'unsafe_progress',
      message: `Live progress failed the public-safety boundary: ${safety.violations.join(
        ', ',
      )}.`,
    })
  }

  return progress
}

// ---------------------------------------------------------------------------
// Projections.
// ---------------------------------------------------------------------------
//
// Scoped operators see the full progress object (still public-safe — there is no
// private tier here; "scoped" gates VISIBILITY of local_only runs, not extra
// fields). The PUBLIC projection degrades honestly: a `local_only` run is shown
// as awaiting authorization with no live numbers, so the web surface never fakes
// data for a run that is not yet authorized for publication.

export const GymRunProgressUnpublished = S.Struct({
  schemaVersion: S.Literal(GYM_RUN_PROGRESS_SCHEMA),
  runRef: S.String,
  publication: S.Literal('local_only'),
  inProgress: S.Boolean,
  decisionGrade: S.Literal(false),
  // Honest blocker copy ref instead of any number.
  blockerRefs: S.Array(S.String),
  lastUpdatedAt: S.String,
})
export type GymRunProgressUnpublished = typeof GymRunProgressUnpublished.Type

export const GymRunProgressPublicProjection = S.Union([
  GymRunProgress,
  GymRunProgressUnpublished,
])
export type GymRunProgressPublicProjection =
  typeof GymRunProgressPublicProjection.Type

const LOCAL_ONLY_BLOCKER =
  'blocker.gym.run_progress.not_authorized_for_web_publication'

// Project a progress object for the PUBLIC surface. A `web_authorized` run is
// returned as-is (already public-safe); a `local_only` run is degraded to an
// honest "awaiting authorization" marker with NO live counts. PURE.
export const projectPublicGymRunProgress = (
  progress: GymRunProgress,
): GymRunProgressPublicProjection => {
  if (progress.publication === 'web_authorized') {
    return progress
  }
  return {
    schemaVersion: GYM_RUN_PROGRESS_SCHEMA,
    runRef: progress.runRef,
    publication: 'local_only',
    inProgress: progress.inProgress,
    decisionGrade: false,
    blockerRefs: [LOCAL_ONLY_BLOCKER, ...progress.blockerRefs],
    lastUpdatedAt: progress.lastUpdatedAt,
  }
}

// Re-export the ingest schema id so callers can pin it without importing the
// dispatch module just for the constant.
export const GYM_RUN_PROGRESS_INGEST_FROM_SCHEMA =
  GYM_HARBOR_TERMINAL_BENCH_INGEST_SCHEMA
