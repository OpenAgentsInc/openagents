/**
 * W3 student-program eval report schema v0.1 (issue #4749).
 *
 * Mirrors the psionic `tassadar_student_eval_report.v0.1` JSON emitted
 * by `psionic-tassadar-student`'s eval binary. The monorepo side owns
 * the report schema types, typed validation, and the public projection;
 * training/eval execution truth lives in psionic.
 *
 * The metric is first divergence behind replay, never perplexity. A
 * report that cannot name its checkpoint, dataset, config, and eval
 * hashes is not evidence and fails validation here.
 */

export const TASSADAR_W3_EVAL_REPORT_VERSION =
  'tassadar_student_eval_report.v0.1'

export const TASSADAR_W3_BASELINES = [
  'baseline_a_next_token',
  'baseline_b_aux_state',
  'baseline_c_lookup_analytic',
  'baseline_c_lookup_random_init',
  'baseline_d_frozen_executor_learned_interface',
] as const
export type TassadarW3Baseline = (typeof TASSADAR_W3_BASELINES)[number]

export type TassadarW3SuiteReport = Readonly<{
  suite: string
  records: number
  exact_rollout_pass_at_1: number
  first_divergence_step_median: number
  first_divergence_step_p90: number
  first_divergence_step_median_diverged: number | null
  valid_prefix_tokens_median: number
  branch_accuracy: number | null
  memory_read_accuracy: number | null
  output_digest_match_rate: number
  replay_verifier_acceptance_rate: number
  tokens_per_sec: number
  divergence_causes: Readonly<Record<string, number>>
  divergence_step_histogram: Readonly<Record<string, number>>
}>

export type TassadarW3EvalReport = Readonly<{
  report_version: typeof TASSADAR_W3_EVAL_REPORT_VERSION
  baseline: TassadarW3Baseline
  corpus_id: string
  dataset_snapshot_digest: string
  eval_prep_sha256: string
  checkpoint_sha256: string
  config_digest: string
  suites: ReadonlyArray<TassadarW3SuiteReport>
  overall: TassadarW3SuiteReport
  wall_seconds: number
  threads: number
}>

export type TassadarW3ReportValidationFailure = Readonly<
  | { kind: 'not_an_object'; detail: string }
  | { kind: 'wrong_version'; detail: string }
  | { kind: 'unknown_baseline'; detail: string }
  | { kind: 'malformed_digest'; field: string; detail: string }
  | { kind: 'missing_field'; field: string }
  | { kind: 'invalid_rate'; field: string; detail: string }
  | { kind: 'empty_suites'; detail: string }
>

export type TassadarW3ReportParseResult =
  | Readonly<{ ok: true; report: TassadarW3EvalReport }>
  | Readonly<{ ok: false; failures: ReadonlyArray<TassadarW3ReportValidationFailure> }>

const HEX_64 = /^[0-9a-f]{64}$/

const isRate = (value: unknown): value is number =>
  typeof value === 'number' && value >= 0 && value <= 1

const suiteFailures = (
  suite: Record<string, unknown>,
  label: string,
): Array<TassadarW3ReportValidationFailure> => {
  const failures: Array<TassadarW3ReportValidationFailure> = []
  for (const field of [
    'suite',
    'records',
    'exact_rollout_pass_at_1',
    'first_divergence_step_median',
    'first_divergence_step_p90',
    'valid_prefix_tokens_median',
    'output_digest_match_rate',
    'replay_verifier_acceptance_rate',
    'tokens_per_sec',
    'divergence_causes',
    'divergence_step_histogram',
  ]) {
    if (!(field in suite)) {
      failures.push({ field: `${label}.${field}`, kind: 'missing_field' })
    }
  }
  for (const field of [
    'exact_rollout_pass_at_1',
    'output_digest_match_rate',
    'replay_verifier_acceptance_rate',
  ]) {
    if (field in suite && !isRate(suite[field])) {
      failures.push({
        detail: `${label}.${field} = ${JSON.stringify(suite[field])} is not a rate in [0, 1]`,
        field: `${label}.${field}`,
        kind: 'invalid_rate',
      })
    }
  }
  return failures
}

/**
 * Typed validation of one eval-report JSON value. Publication rule:
 * every checkpoint claim ships checkpoint/dataset/config/eval hashes,
 * or it ships nothing — malformed digests fail here.
 */
export const parseTassadarW3EvalReport = (
  value: unknown,
): TassadarW3ReportParseResult => {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    return {
      failures: [
        { detail: `expected object, found ${typeof value}`, kind: 'not_an_object' },
      ],
      ok: false,
    }
  }
  const report = value as Record<string, unknown>
  const failures: Array<TassadarW3ReportValidationFailure> = []
  if (report.report_version !== TASSADAR_W3_EVAL_REPORT_VERSION) {
    failures.push({
      detail: `report_version ${JSON.stringify(report.report_version)} is not ${TASSADAR_W3_EVAL_REPORT_VERSION}`,
      kind: 'wrong_version',
    })
  }
  if (
    !(TASSADAR_W3_BASELINES as ReadonlyArray<string>).includes(
      String(report.baseline),
    )
  ) {
    failures.push({
      detail: `baseline ${JSON.stringify(report.baseline)} is not in the W3 sweep`,
      kind: 'unknown_baseline',
    })
  }
  for (const field of [
    'dataset_snapshot_digest',
    'eval_prep_sha256',
    'checkpoint_sha256',
    'config_digest',
  ]) {
    const digest = report[field]
    if (typeof digest !== 'string' || !HEX_64.test(digest)) {
      failures.push({
        detail: `${field} must be 64 lowercase hex characters`,
        field,
        kind: 'malformed_digest',
      })
    }
  }
  if (typeof report.corpus_id !== 'string' || report.corpus_id.length === 0) {
    failures.push({ field: 'corpus_id', kind: 'missing_field' })
  }
  const suites = report.suites
  if (!Array.isArray(suites) || suites.length === 0) {
    failures.push({
      detail: 'a report with no suites is not evidence',
      kind: 'empty_suites',
    })
  } else {
    for (const suite of suites) {
      failures.push(
        ...suiteFailures(
          suite as Record<string, unknown>,
          `suites[${(suite as { suite?: string }).suite ?? '?'}]`,
        ),
      )
    }
  }
  if (report.overall === undefined || report.overall === null) {
    failures.push({ field: 'overall', kind: 'missing_field' })
  } else {
    failures.push(
      ...suiteFailures(report.overall as Record<string, unknown>, 'overall'),
    )
  }
  if (failures.length > 0) return { failures, ok: false }
  return { ok: true, report: value as TassadarW3EvalReport }
}

export type TassadarW3SweepProjection = Readonly<{
  projectionVersion: 'w3_student_sweep_projection.v0.1'
  corpusId: string
  datasetSnapshotDigest: string
  baselines: ReadonlyArray<{
    baseline: TassadarW3Baseline
    checkpointSha256: string
    configDigest: string
    exactRolloutPassAt1: number
    firstDivergenceStepMedian: number
    firstDivergenceStepP90: number
    replayVerifierAcceptanceRate: number
    topDivergenceCause: string | null
  }>
}>

export type TassadarW3ProjectionResult =
  | Readonly<{ ok: true; projection: TassadarW3SweepProjection }>
  | Readonly<{ ok: false; detail: string }>

/**
 * Rebuilds the public sweep projection from full validated reports —
 * a pure function of the report artifacts, so any consumer can rebuild
 * it on each report transition (the projection-rebuild rule from W2).
 */
export const rebuildTassadarW3SweepProjection = (
  reports: ReadonlyArray<TassadarW3EvalReport>,
): TassadarW3ProjectionResult => {
  if (reports.length === 0) {
    return { detail: 'no reports to project', ok: false }
  }
  const corpusIds = new Set(reports.map(report => report.corpus_id))
  const snapshots = new Set(reports.map(report => report.dataset_snapshot_digest))
  if (corpusIds.size !== 1 || snapshots.size !== 1) {
    return {
      detail: `reports span ${corpusIds.size} corpus ids and ${snapshots.size} snapshots; a sweep projection requires one snapshot`,
      ok: false,
    }
  }
  const baselines = [...reports]
    .sort((left, right) => (left.baseline < right.baseline ? -1 : 1))
    .map(report => {
      const causes = Object.entries(report.overall.divergence_causes).sort(
        (left, right) => right[1] - left[1],
      )
      return {
        baseline: report.baseline,
        checkpointSha256: report.checkpoint_sha256,
        configDigest: report.config_digest,
        exactRolloutPassAt1: report.overall.exact_rollout_pass_at_1,
        firstDivergenceStepMedian: report.overall.first_divergence_step_median,
        firstDivergenceStepP90: report.overall.first_divergence_step_p90,
        replayVerifierAcceptanceRate:
          report.overall.replay_verifier_acceptance_rate,
        topDivergenceCause: causes.length > 0 ? (causes[0]?.[0] ?? null) : null,
      }
    })
  return {
    ok: true,
    projection: {
      baselines,
      corpusId: reports[0]?.corpus_id ?? '',
      datasetSnapshotDigest: reports[0]?.dataset_snapshot_digest ?? '',
      projectionVersion: 'w3_student_sweep_projection.v0.1',
    },
  }
}
