import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const OmniBenchmarkCloudAudience = S.Literals([
  'public',
  'agent',
  'customer',
  'team',
  'operator',
])
export type OmniBenchmarkCloudAudience =
  typeof OmniBenchmarkCloudAudience.Type

export const OmniBenchmarkCloudState = S.Literals([
  'blocked',
  'failed',
  'flaky',
  'imported',
  'passed',
  'planned',
  'running',
  'superseded',
])
export type OmniBenchmarkCloudState = typeof OmniBenchmarkCloudState.Type

export const OmniBenchmarkTaskInputClass = S.Literals([
  'private_fixture_ref',
  'public_fixture',
  'redacted_fixture',
  'synthetic_fixture',
])
export type OmniBenchmarkTaskInputClass =
  typeof OmniBenchmarkTaskInputClass.Type

export const OmniBenchmarkRegressionSeverity = S.Literals([
  'critical',
  'high',
  'medium',
  'low',
])
export type OmniBenchmarkRegressionSeverity =
  typeof OmniBenchmarkRegressionSeverity.Type

export const OmniBenchmarkCloudAuthorityBoundary = S.Literals([
  'read_only_benchmark_cloud_evidence',
])
export type OmniBenchmarkCloudAuthorityBoundary =
  typeof OmniBenchmarkCloudAuthorityBoundary.Type

export class OmniBenchmarkCloudAuthority extends S.Class<OmniBenchmarkCloudAuthority>(
  'OmniBenchmarkCloudAuthority',
)({
  authorityBoundary: OmniBenchmarkCloudAuthorityBoundary,
  noBenchmarkLaunch: S.Boolean,
  noEvalExecution: S.Boolean,
  noPaymentSpend: S.Boolean,
  noPayoutMutation: S.Boolean,
  noProviderMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noRawBenchmarkInputCopy: S.Boolean,
  noRoutingMutation: S.Boolean,
  noRuntimePromotion: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class OmniBenchmarkSuiteRecord extends S.Class<OmniBenchmarkSuiteRecord>(
  'OmniBenchmarkSuiteRecord',
)({
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  policyRefs: S.Array(S.String),
  suiteRef: S.String,
  taskRefs: S.Array(S.String),
}) {}

export class OmniBenchmarkTaskRecord extends S.Class<OmniBenchmarkTaskRecord>(
  'OmniBenchmarkTaskRecord',
)({
  caveatRefs: S.Array(S.String),
  datasetRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  expectedOutputRefs: S.Array(S.String),
  inputClass: OmniBenchmarkTaskInputClass,
  suiteRefs: S.Array(S.String),
  taskRef: S.String,
}) {}

export class OmniBenchmarkEvalJobRecord extends S.Class<OmniBenchmarkEvalJobRecord>(
  'OmniBenchmarkEvalJobRecord',
)({
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  candidateRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  comparisonRefs: S.Array(S.String),
  evalJobRef: S.String,
  evidenceRefs: S.Array(S.String),
  flakeRefs: S.Array(S.String),
  providerRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  regressionRefs: S.Array(S.String),
  retainedFailureRefs: S.Array(S.String),
  runnerRefs: S.Array(S.String),
  scorecardRefs: S.Array(S.String),
  state: OmniBenchmarkCloudState,
  suiteRefs: S.Array(S.String),
  taskRefs: S.Array(S.String),
  trainingRunRefs: S.Array(S.String),
}) {}

export class OmniBenchmarkScorecardRecord extends S.Class<OmniBenchmarkScorecardRecord>(
  'OmniBenchmarkScorecardRecord',
)({
  caveatRefs: S.Array(S.String),
  evalJobRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  metricRefs: S.Array(S.String),
  observedScoreBps: S.Number,
  passThresholdBps: S.Number,
  receiptRefs: S.Array(S.String),
  scorecardRef: S.String,
  state: OmniBenchmarkCloudState,
}) {}

export class OmniBenchmarkRegressionRecord extends S.Class<OmniBenchmarkRegressionRecord>(
  'OmniBenchmarkRegressionRecord',
)({
  affectedTaskRefs: S.Array(S.String),
  baselineEvalRef: S.String,
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  promotionBlocking: S.Boolean,
  promotionGateRefs: S.Array(S.String),
  regressionRef: S.String,
  severity: OmniBenchmarkRegressionSeverity,
  sourceEvalRef: S.String,
}) {}

export class OmniBenchmarkFlakeRecord extends S.Class<OmniBenchmarkFlakeRecord>(
  'OmniBenchmarkFlakeRecord',
)({
  caveatRefs: S.Array(S.String),
  evalJobRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  flakeRateBps: S.Number,
  flakeRef: S.String,
  taskRefs: S.Array(S.String),
}) {}

export class OmniBenchmarkComparisonRecord extends S.Class<OmniBenchmarkComparisonRecord>(
  'OmniBenchmarkComparisonRecord',
)({
  baselineArtifactRefs: S.Array(S.String),
  baselineEvalRefs: S.Array(S.String),
  candidateArtifactRefs: S.Array(S.String),
  candidateEvalRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  comparisonRef: S.String,
  evidenceRefs: S.Array(S.String),
  regressionRefs: S.Array(S.String),
  scorecardRefs: S.Array(S.String),
}) {}

export class OmniBenchmarkCloudRecord extends S.Class<OmniBenchmarkCloudRecord>(
  'OmniBenchmarkCloudRecord',
)({
  artifactRefs: S.Array(S.String),
  authority: OmniBenchmarkCloudAuthority,
  benchmarkRef: S.String,
  blockerRefs: S.Array(S.String),
  candidateRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  comparisons: S.Array(OmniBenchmarkComparisonRecord),
  createdAtIso: S.String,
  evalJobs: S.Array(OmniBenchmarkEvalJobRecord),
  flakes: S.Array(OmniBenchmarkFlakeRecord),
  id: S.String,
  promotionGateRefs: S.Array(S.String),
  regressions: S.Array(OmniBenchmarkRegressionRecord),
  retainedFailureRefs: S.Array(S.String),
  scorecards: S.Array(OmniBenchmarkScorecardRecord),
  state: OmniBenchmarkCloudState,
  suites: S.Array(OmniBenchmarkSuiteRecord),
  tasks: S.Array(OmniBenchmarkTaskRecord),
  trainingRunRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class OmniBenchmarkCloudProjection extends S.Class<OmniBenchmarkCloudProjection>(
  'OmniBenchmarkCloudProjection',
)({
  artifactRefs: S.Array(S.String),
  audience: OmniBenchmarkCloudAudience,
  authority: OmniBenchmarkCloudAuthority,
  benchmarkLaunchAllowed: S.Boolean,
  benchmarkRef: S.String,
  blockerRefs: S.Array(S.String),
  candidateRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  comparisonCount: S.Number,
  comparisons: S.Array(OmniBenchmarkComparisonRecord),
  createdAtDisplay: S.String,
  evalExecutionAllowed: S.Boolean,
  evalJobCount: S.Number,
  evalJobs: S.Array(OmniBenchmarkEvalJobRecord),
  failedScorecardCount: S.Number,
  flakeCount: S.Number,
  flaky: S.Boolean,
  flakes: S.Array(OmniBenchmarkFlakeRecord),
  id: S.String,
  passedScorecardCount: S.Number,
  paymentSpendAllowed: S.Boolean,
  payoutMutationAllowed: S.Boolean,
  providerMutationAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  promotionBlocked: S.Boolean,
  promotionGateRefs: S.Array(S.String),
  rawBenchmarkInputCopyAllowed: S.Boolean,
  regressionCount: S.Number,
  regressions: S.Array(OmniBenchmarkRegressionRecord),
  retainedFailureRefs: S.Array(S.String),
  routingMutationAllowed: S.Boolean,
  runtimePromotionAllowed: S.Boolean,
  scorecardCount: S.Number,
  scorecards: S.Array(OmniBenchmarkScorecardRecord),
  settlementMutationAllowed: S.Boolean,
  state: OmniBenchmarkCloudState,
  stateLabel: S.String,
  suiteCount: S.Number,
  suites: S.Array(OmniBenchmarkSuiteRecord),
  taskCount: S.Number,
  tasks: S.Array(OmniBenchmarkTaskRecord),
  trainingRunRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class OmniBenchmarkCloudUnsafe extends S.TaggedErrorClass<OmniBenchmarkCloudUnsafe>()(
  'OmniBenchmarkCloudUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_BENCHMARK_CLOUD_READ_ONLY_AUTHORITY:
  OmniBenchmarkCloudAuthority = {
    authorityBoundary: 'read_only_benchmark_cloud_evidence',
    noBenchmarkLaunch: true,
    noEvalExecution: true,
    noPaymentSpend: true,
    noPayoutMutation: true,
    noProviderMutation: true,
    noPublicClaimUpgrade: true,
    noRawBenchmarkInputCopy: true,
    noRoutingMutation: true,
    noRuntimePromotion: true,
    noSettlementMutation: true,
  }

const stateLabelByState: Readonly<Record<OmniBenchmarkCloudState, string>> = {
  blocked: 'Blocked',
  failed: 'Failed',
  flaky: 'Flaky',
  imported: 'Imported evidence',
  passed: 'Passed evidence',
  planned: 'Planned evidence',
  running: 'Running evidence',
  superseded: 'Superseded',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeBenchmarkCloudRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|benchmark|customer|dataset|email|fixture|input|invoice|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(artifact\.private|benchmark\.private|blocker\.private|candidate\.private|caveat\.private|comparison\.private|dataset\.private|eval\.private|evidence\.private|expected_output\.private|fixture\.private|flake\.private|gate\.private|metric\.private|provider\.|regression\.private|retained_failure\.private|runner\.private|scorecard\.private|source\.|suite\.private|task\.private|training\.private)/i
const agentUnsafeRefPattern =
  /(artifact\.private|benchmark\.private|blocker\.private|candidate\.private|comparison\.private|dataset\.private|eval\.private|expected_output\.private|fixture\.private|flake\.private|gate\.private|metric\.private|provider\.private|regression\.private|retained_failure\.private|runner\.private|scorecard\.private|source\.private|suite\.private|task\.private|training\.private)/i
const customerUnsafeRefPattern = agentUnsafeRefPattern

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeBenchmarkCloudRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: `${label} contains private prompts, private benchmark inputs, source archives, datasets, provider payloads, model weights, secrets, payment/wallet material, private repos, raw logs, raw traces, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniBenchmarkCloudAudience,
): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'agent') {
    return agentUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  return null
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: OmniBenchmarkCloudAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const refForAudience = (
  label: string,
  ref: string,
  audience: OmniBenchmarkCloudAudience,
  redactedRef: string,
): string => refsForAudience(label, [ref], audience)[0] ?? redactedRef

const assertReadOnlyAuthority = (
  authority: OmniBenchmarkCloudAuthority,
): void => {
  if (
    authority.noBenchmarkLaunch !== true ||
    authority.noEvalExecution !== true ||
    authority.noPaymentSpend !== true ||
    authority.noPayoutMutation !== true ||
    authority.noProviderMutation !== true ||
    authority.noPublicClaimUpgrade !== true ||
    authority.noRawBenchmarkInputCopy !== true ||
    authority.noRoutingMutation !== true ||
    authority.noRuntimePromotion !== true ||
    authority.noSettlementMutation !== true
  ) {
    throw new OmniBenchmarkCloudUnsafe({
      reason:
        'Benchmark Cloud records are read-only evidence and cannot launch benchmark jobs, execute evals, mutate providers, copy raw benchmark inputs, spend money, promote runtime behavior, mutate routes, pay out, settle, or upgrade public claims.',
    })
  }
}

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertBps = (label: string, bps: number): void => {
  if (!Number.isFinite(bps) || bps < 0 || bps > 10_000) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: `${label} must be finite basis points from 0 to 10000.`,
    })
  }
}

const duplicateRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  uniqueRefs(refs.filter((ref, index) => refs.indexOf(ref) !== index))

const missingRefs = (
  refs: ReadonlyArray<string>,
  knownRefs: ReadonlySet<string>,
): ReadonlyArray<string> => uniqueRefs(refs.filter(ref => !knownRefs.has(ref)))

const refSet = (refs: ReadonlyArray<string>): ReadonlySet<string> =>
  new Set(refs)

const assertNoMissingRefs = (
  label: string,
  refs: ReadonlyArray<string>,
  knownRefs: ReadonlySet<string>,
): void => {
  if (hasAny(missingRefs(refs, knownRefs))) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: `${label} must reference records in the same Benchmark Cloud packet.`,
    })
  }
}

const assertSuite = (suite: OmniBenchmarkSuiteRecord): void => {
  assertSafeRefs('Benchmark suite ref', [suite.suiteRef])
  assertSafeRefs('Benchmark suite task refs', suite.taskRefs)
  assertSafeRefs('Benchmark suite policy refs', suite.policyRefs)
  assertSafeRefs('Benchmark suite evidence refs', suite.evidenceRefs)
  assertSafeRefs('Benchmark suite caveat refs', suite.caveatRefs)

  if (!hasAny(suite.taskRefs) || !hasAny(suite.evidenceRefs)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Benchmark suites require task refs and evidence refs.',
    })
  }
}

const assertTask = (task: OmniBenchmarkTaskRecord): void => {
  assertSafeRefs('Benchmark task ref', [task.taskRef])
  assertSafeRefs('Benchmark task suite refs', task.suiteRefs)
  assertSafeRefs('Benchmark task dataset refs', task.datasetRefs)
  assertSafeRefs('Benchmark task expected output refs', task.expectedOutputRefs)
  assertSafeRefs('Benchmark task evidence refs', task.evidenceRefs)
  assertSafeRefs('Benchmark task caveat refs', task.caveatRefs)

  if (!hasAny(task.suiteRefs) || !hasAny(task.evidenceRefs)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Benchmark tasks require suite refs and evidence refs.',
    })
  }

  if (
    task.inputClass !== 'synthetic_fixture' &&
    (!hasAny(task.datasetRefs) || !hasAny(task.expectedOutputRefs))
  ) {
    throw new OmniBenchmarkCloudUnsafe({
      reason:
        'Non-synthetic benchmark tasks require dataset and expected-output refs.',
    })
  }
}

const assertEvalJob = (
  evalJob: OmniBenchmarkEvalJobRecord,
  suiteRefs: ReadonlySet<string>,
  taskRefs: ReadonlySet<string>,
  scorecardRefs: ReadonlySet<string>,
  regressionRefs: ReadonlySet<string>,
  flakeRefs: ReadonlySet<string>,
  comparisonRefs: ReadonlySet<string>,
): void => {
  assertSafeRefs('Benchmark eval job ref', [evalJob.evalJobRef])
  assertSafeRefs('Benchmark eval suite refs', evalJob.suiteRefs)
  assertSafeRefs('Benchmark eval task refs', evalJob.taskRefs)
  assertSafeRefs('Benchmark eval runner refs', evalJob.runnerRefs)
  assertSafeRefs('Benchmark eval provider refs', evalJob.providerRefs)
  assertSafeRefs('Benchmark eval candidate refs', evalJob.candidateRefs)
  assertSafeRefs('Benchmark eval artifact refs', evalJob.artifactRefs)
  assertSafeRefs('Benchmark eval training run refs', evalJob.trainingRunRefs)
  assertSafeRefs(
    'Benchmark eval retained failure refs',
    evalJob.retainedFailureRefs,
  )
  assertSafeRefs('Benchmark eval scorecard refs', evalJob.scorecardRefs)
  assertSafeRefs('Benchmark eval regression refs', evalJob.regressionRefs)
  assertSafeRefs('Benchmark eval flake refs', evalJob.flakeRefs)
  assertSafeRefs('Benchmark eval comparison refs', evalJob.comparisonRefs)
  assertSafeRefs('Benchmark eval evidence refs', evalJob.evidenceRefs)
  assertSafeRefs('Benchmark eval receipt refs', evalJob.receiptRefs)
  assertSafeRefs('Benchmark eval caveat refs', evalJob.caveatRefs)
  assertSafeRefs('Benchmark eval blocker refs', evalJob.blockerRefs)

  assertNoMissingRefs('Benchmark eval suite refs', evalJob.suiteRefs, suiteRefs)
  assertNoMissingRefs('Benchmark eval task refs', evalJob.taskRefs, taskRefs)
  assertNoMissingRefs(
    'Benchmark eval scorecard refs',
    evalJob.scorecardRefs,
    scorecardRefs,
  )
  assertNoMissingRefs(
    'Benchmark eval regression refs',
    evalJob.regressionRefs,
    regressionRefs,
  )
  assertNoMissingRefs('Benchmark eval flake refs', evalJob.flakeRefs, flakeRefs)
  assertNoMissingRefs(
    'Benchmark eval comparison refs',
    evalJob.comparisonRefs,
    comparisonRefs,
  )

  if (!hasAny(evalJob.suiteRefs) || !hasAny(evalJob.taskRefs)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Benchmark eval jobs require suite and task refs.',
    })
  }

  if (
    evalJob.state === 'running' &&
    (!hasAny(evalJob.runnerRefs) || !hasAny(evalJob.providerRefs))
  ) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Running benchmark eval evidence requires runner and provider refs.',
    })
  }

  if (
    evalJob.state === 'passed' &&
    (!hasAny(evalJob.scorecardRefs) ||
      !hasAny(evalJob.evidenceRefs) ||
      !hasAny(evalJob.receiptRefs))
  ) {
    throw new OmniBenchmarkCloudUnsafe({
      reason:
        'Passed benchmark eval jobs require scorecard, evidence, and receipt refs.',
    })
  }

  if (evalJob.state === 'failed' && !hasAny(evalJob.regressionRefs)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Failed benchmark eval jobs require regression refs.',
    })
  }

  if (evalJob.state === 'flaky' && !hasAny(evalJob.flakeRefs)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Flaky benchmark eval jobs require flake refs.',
    })
  }

  if (evalJob.state === 'blocked' && !hasAny(evalJob.blockerRefs)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Blocked benchmark eval jobs require blocker refs.',
    })
  }
}

const assertScorecard = (
  scorecard: OmniBenchmarkScorecardRecord,
  evalJobRefs: ReadonlySet<string>,
): void => {
  assertSafeRefs('Benchmark scorecard ref', [scorecard.scorecardRef])
  assertSafeRefs('Benchmark scorecard eval job refs', scorecard.evalJobRefs)
  assertSafeRefs('Benchmark scorecard metric refs', scorecard.metricRefs)
  assertSafeRefs('Benchmark scorecard evidence refs', scorecard.evidenceRefs)
  assertSafeRefs('Benchmark scorecard receipt refs', scorecard.receiptRefs)
  assertSafeRefs('Benchmark scorecard caveat refs', scorecard.caveatRefs)
  assertBps('Benchmark scorecard pass threshold', scorecard.passThresholdBps)
  assertBps('Benchmark scorecard observed score', scorecard.observedScoreBps)
  assertNoMissingRefs(
    'Benchmark scorecard eval job refs',
    scorecard.evalJobRefs,
    evalJobRefs,
  )

  if (!hasAny(scorecard.evalJobRefs) || !hasAny(scorecard.metricRefs)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Benchmark scorecards require eval job and metric refs.',
    })
  }

  if (!hasAny(scorecard.evidenceRefs)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Benchmark scorecards require evidence refs.',
    })
  }

  if (
    scorecard.state === 'passed' &&
    (scorecard.observedScoreBps < scorecard.passThresholdBps ||
      !hasAny(scorecard.receiptRefs))
  ) {
    throw new OmniBenchmarkCloudUnsafe({
      reason:
        'Passed benchmark scorecards require observed score at or above threshold and receipt refs.',
    })
  }

  if (
    scorecard.state === 'failed' &&
    scorecard.observedScoreBps >= scorecard.passThresholdBps
  ) {
    throw new OmniBenchmarkCloudUnsafe({
      reason:
        'Failed benchmark scorecards must record a score below the pass threshold.',
    })
  }
}

const assertRegression = (
  regression: OmniBenchmarkRegressionRecord,
  evalJobRefs: ReadonlySet<string>,
  taskRefs: ReadonlySet<string>,
): void => {
  assertSafeRefs('Benchmark regression ref', [regression.regressionRef])
  assertSafeRefs('Benchmark regression eval refs', [
    regression.sourceEvalRef,
    regression.baselineEvalRef,
  ])
  assertSafeRefs(
    'Benchmark regression affected task refs',
    regression.affectedTaskRefs,
  )
  assertSafeRefs('Benchmark regression evidence refs', regression.evidenceRefs)
  assertSafeRefs(
    'Benchmark regression promotion gate refs',
    regression.promotionGateRefs,
  )
  assertSafeRefs('Benchmark regression caveat refs', regression.caveatRefs)
  assertNoMissingRefs(
    'Benchmark regression eval refs',
    [regression.sourceEvalRef, regression.baselineEvalRef],
    evalJobRefs,
  )
  assertNoMissingRefs(
    'Benchmark regression task refs',
    regression.affectedTaskRefs,
    taskRefs,
  )

  if (!hasAny(regression.affectedTaskRefs) || !hasAny(regression.evidenceRefs)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Benchmark regressions require affected task and evidence refs.',
    })
  }

  if (
    regression.promotionBlocking &&
    (!hasAny(regression.promotionGateRefs) ||
      (regression.severity !== 'critical' && regression.severity !== 'high'))
  ) {
    throw new OmniBenchmarkCloudUnsafe({
      reason:
        'Promotion-blocking benchmark regressions require promotion gate refs and high or critical severity.',
    })
  }
}

const assertFlake = (
  flake: OmniBenchmarkFlakeRecord,
  evalJobRefs: ReadonlySet<string>,
  taskRefs: ReadonlySet<string>,
): void => {
  assertSafeRefs('Benchmark flake ref', [flake.flakeRef])
  assertSafeRefs('Benchmark flake eval job refs', flake.evalJobRefs)
  assertSafeRefs('Benchmark flake task refs', flake.taskRefs)
  assertSafeRefs('Benchmark flake evidence refs', flake.evidenceRefs)
  assertSafeRefs('Benchmark flake caveat refs', flake.caveatRefs)
  assertBps('Benchmark flake rate', flake.flakeRateBps)
  assertNoMissingRefs(
    'Benchmark flake eval refs',
    flake.evalJobRefs,
    evalJobRefs,
  )
  assertNoMissingRefs('Benchmark flake task refs', flake.taskRefs, taskRefs)

  if (!hasAny(flake.evalJobRefs) || !hasAny(flake.taskRefs)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Benchmark flakes require eval job and task refs.',
    })
  }

  if (!hasAny(flake.evidenceRefs) || !hasAny(flake.caveatRefs)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Benchmark flakes require evidence and caveat refs.',
    })
  }
}

const assertComparison = (
  comparison: OmniBenchmarkComparisonRecord,
  evalJobRefs: ReadonlySet<string>,
  scorecardRefs: ReadonlySet<string>,
  regressionRefs: ReadonlySet<string>,
): void => {
  assertSafeRefs('Benchmark comparison ref', [comparison.comparisonRef])
  assertSafeRefs(
    'Benchmark comparison baseline artifact refs',
    comparison.baselineArtifactRefs,
  )
  assertSafeRefs(
    'Benchmark comparison candidate artifact refs',
    comparison.candidateArtifactRefs,
  )
  assertSafeRefs(
    'Benchmark comparison baseline eval refs',
    comparison.baselineEvalRefs,
  )
  assertSafeRefs(
    'Benchmark comparison candidate eval refs',
    comparison.candidateEvalRefs,
  )
  assertSafeRefs(
    'Benchmark comparison scorecard refs',
    comparison.scorecardRefs,
  )
  assertSafeRefs(
    'Benchmark comparison regression refs',
    comparison.regressionRefs,
  )
  assertSafeRefs('Benchmark comparison evidence refs', comparison.evidenceRefs)
  assertSafeRefs('Benchmark comparison caveat refs', comparison.caveatRefs)
  assertNoMissingRefs(
    'Benchmark comparison baseline eval refs',
    comparison.baselineEvalRefs,
    evalJobRefs,
  )
  assertNoMissingRefs(
    'Benchmark comparison candidate eval refs',
    comparison.candidateEvalRefs,
    evalJobRefs,
  )
  assertNoMissingRefs(
    'Benchmark comparison scorecard refs',
    comparison.scorecardRefs,
    scorecardRefs,
  )
  assertNoMissingRefs(
    'Benchmark comparison regression refs',
    comparison.regressionRefs,
    regressionRefs,
  )

  if (
    !hasAny(comparison.baselineEvalRefs) ||
    !hasAny(comparison.candidateEvalRefs) ||
    !hasAny(comparison.evidenceRefs)
  ) {
    throw new OmniBenchmarkCloudUnsafe({
      reason:
        'Benchmark comparisons require baseline eval, candidate eval, and evidence refs.',
    })
  }
}

const assertRecord = (record: OmniBenchmarkCloudRecord): void => {
  assertReadOnlyAuthority(record.authority)
  assertValidIso('createdAtIso', record.createdAtIso)
  assertValidIso('updatedAtIso', record.updatedAtIso)
  assertSafeRefs('Benchmark Cloud id', [record.id])
  assertSafeRefs('Benchmark Cloud ref', [record.benchmarkRef])
  assertSafeRefs('Benchmark Cloud artifact refs', record.artifactRefs)
  assertSafeRefs('Benchmark Cloud blocker refs', record.blockerRefs)
  assertSafeRefs('Benchmark Cloud candidate refs', record.candidateRefs)
  assertSafeRefs('Benchmark Cloud caveat refs', record.caveatRefs)
  assertSafeRefs(
    'Benchmark Cloud promotion gate refs',
    record.promotionGateRefs,
  )
  assertSafeRefs(
    'Benchmark Cloud retained failure refs',
    record.retainedFailureRefs,
  )
  assertSafeRefs('Benchmark Cloud training run refs', record.trainingRunRefs)

  if (
    !hasAny(record.suites) ||
    !hasAny(record.tasks) ||
    !hasAny(record.evalJobs) ||
    !hasAny(record.scorecards)
  ) {
    throw new OmniBenchmarkCloudUnsafe({
      reason:
        'Benchmark Cloud records require suites, tasks, eval jobs, and scorecards.',
    })
  }

  const suiteRefs = record.suites.map(suite => suite.suiteRef)
  const taskRefs = record.tasks.map(task => task.taskRef)
  const evalJobRefs = record.evalJobs.map(evalJob => evalJob.evalJobRef)
  const scorecardRefs = record.scorecards.map(scorecard => scorecard.scorecardRef)
  const regressionRefs = record.regressions.map(regression => regression.regressionRef)
  const flakeRefs = record.flakes.map(flake => flake.flakeRef)
  const comparisonRefs = record.comparisons.map(comparison => comparison.comparisonRef)
  const duplicated = [
    ...duplicateRefs(suiteRefs),
    ...duplicateRefs(taskRefs),
    ...duplicateRefs(evalJobRefs),
    ...duplicateRefs(scorecardRefs),
    ...duplicateRefs(regressionRefs),
    ...duplicateRefs(flakeRefs),
    ...duplicateRefs(comparisonRefs),
  ]

  if (hasAny(duplicated)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Benchmark Cloud records cannot contain duplicate record refs.',
    })
  }

  record.suites.forEach(assertSuite)
  record.tasks.forEach(assertTask)

  record.suites.forEach(suite =>
    assertNoMissingRefs(
      'Benchmark suite task refs',
      suite.taskRefs,
      refSet(taskRefs),
    ),
  )
  record.tasks.forEach(task =>
    assertNoMissingRefs(
      'Benchmark task suite refs',
      task.suiteRefs,
      refSet(suiteRefs),
    ),
  )
  record.scorecards.forEach(scorecard =>
    assertScorecard(scorecard, refSet(evalJobRefs)),
  )
  record.regressions.forEach(regression =>
    assertRegression(regression, refSet(evalJobRefs), refSet(taskRefs)),
  )
  record.flakes.forEach(flake =>
    assertFlake(flake, refSet(evalJobRefs), refSet(taskRefs)),
  )
  record.comparisons.forEach(comparison =>
    assertComparison(
      comparison,
      refSet(evalJobRefs),
      refSet(scorecardRefs),
      refSet(regressionRefs),
    ),
  )
  record.evalJobs.forEach(evalJob =>
    assertEvalJob(
      evalJob,
      refSet(suiteRefs),
      refSet(taskRefs),
      refSet(scorecardRefs),
      refSet(regressionRefs),
      refSet(flakeRefs),
      refSet(comparisonRefs),
    ),
  )

  const promotionBlockingRegressions = record.regressions.filter(
    regression => regression.promotionBlocking,
  )

  if (
    hasAny(promotionBlockingRegressions) &&
    !hasAny(record.promotionGateRefs)
  ) {
    throw new OmniBenchmarkCloudUnsafe({
      reason:
        'Benchmark Cloud promotion-blocking failures require top-level promotion gate refs.',
    })
  }

  if (
    record.state === 'passed' &&
    (hasAny(promotionBlockingRegressions) ||
      !record.scorecards.some(scorecard => scorecard.state === 'passed'))
  ) {
    throw new OmniBenchmarkCloudUnsafe({
      reason:
        'Passed Benchmark Cloud evidence requires a passed scorecard and no promotion-blocking regressions.',
    })
  }

  if (
    record.state === 'failed' &&
    (!hasAny(promotionBlockingRegressions) || !hasAny(record.blockerRefs))
  ) {
    throw new OmniBenchmarkCloudUnsafe({
      reason:
        'Failed Benchmark Cloud evidence requires promotion-blocking regression evidence and blocker refs.',
    })
  }

  if (record.state === 'flaky' && !hasAny(record.flakes)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Flaky Benchmark Cloud evidence requires flake records.',
    })
  }

  if (record.state === 'blocked' && !hasAny(record.blockerRefs)) {
    throw new OmniBenchmarkCloudUnsafe({
      reason: 'Blocked Benchmark Cloud evidence requires blocker refs.',
    })
  }
}

const redactSuite = (
  suite: OmniBenchmarkSuiteRecord,
  audience: OmniBenchmarkCloudAudience,
): OmniBenchmarkSuiteRecord => ({
  ...suite,
  caveatRefs: refsForAudience(
    'Benchmark suite caveat refs',
    suite.caveatRefs,
    audience,
  ),
  evidenceRefs: refsForAudience(
    'Benchmark suite evidence refs',
    suite.evidenceRefs,
    audience,
  ),
  policyRefs: refsForAudience(
    'Benchmark suite policy refs',
    suite.policyRefs,
    audience,
  ),
  suiteRef: refForAudience(
    'Benchmark suite ref',
    suite.suiteRef,
    audience,
    'suite.redacted.benchmark_cloud',
  ),
  taskRefs: refsForAudience(
    'Benchmark suite task refs',
    suite.taskRefs,
    audience,
  ),
})

const redactTask = (
  task: OmniBenchmarkTaskRecord,
  audience: OmniBenchmarkCloudAudience,
): OmniBenchmarkTaskRecord => ({
  ...task,
  caveatRefs: refsForAudience(
    'Benchmark task caveat refs',
    task.caveatRefs,
    audience,
  ),
  datasetRefs: refsForAudience(
    'Benchmark task dataset refs',
    task.datasetRefs,
    audience,
  ),
  evidenceRefs: refsForAudience(
    'Benchmark task evidence refs',
    task.evidenceRefs,
    audience,
  ),
  expectedOutputRefs: refsForAudience(
    'Benchmark task expected output refs',
    task.expectedOutputRefs,
    audience,
  ),
  suiteRefs: refsForAudience(
    'Benchmark task suite refs',
    task.suiteRefs,
    audience,
  ),
  taskRef: refForAudience(
    'Benchmark task ref',
    task.taskRef,
    audience,
    'task.redacted.benchmark_cloud',
  ),
})

const redactEvalJob = (
  evalJob: OmniBenchmarkEvalJobRecord,
  audience: OmniBenchmarkCloudAudience,
): OmniBenchmarkEvalJobRecord => ({
  ...evalJob,
  artifactRefs: refsForAudience(
    'Benchmark eval artifact refs',
    evalJob.artifactRefs,
    audience,
  ),
  blockerRefs: refsForAudience(
    'Benchmark eval blocker refs',
    evalJob.blockerRefs,
    audience,
  ),
  candidateRefs: refsForAudience(
    'Benchmark eval candidate refs',
    evalJob.candidateRefs,
    audience,
  ),
  caveatRefs: refsForAudience(
    'Benchmark eval caveat refs',
    evalJob.caveatRefs,
    audience,
  ),
  comparisonRefs: refsForAudience(
    'Benchmark eval comparison refs',
    evalJob.comparisonRefs,
    audience,
  ),
  evalJobRef: refForAudience(
    'Benchmark eval job ref',
    evalJob.evalJobRef,
    audience,
    'eval.redacted.benchmark_cloud',
  ),
  evidenceRefs: refsForAudience(
    'Benchmark eval evidence refs',
    evalJob.evidenceRefs,
    audience,
  ),
  flakeRefs: refsForAudience(
    'Benchmark eval flake refs',
    evalJob.flakeRefs,
    audience,
  ),
  providerRefs: refsForAudience(
    'Benchmark eval provider refs',
    evalJob.providerRefs,
    audience,
  ),
  receiptRefs: refsForAudience(
    'Benchmark eval receipt refs',
    evalJob.receiptRefs,
    audience,
  ),
  regressionRefs: refsForAudience(
    'Benchmark eval regression refs',
    evalJob.regressionRefs,
    audience,
  ),
  retainedFailureRefs: refsForAudience(
    'Benchmark eval retained failure refs',
    evalJob.retainedFailureRefs,
    audience,
  ),
  runnerRefs: refsForAudience(
    'Benchmark eval runner refs',
    evalJob.runnerRefs,
    audience,
  ),
  scorecardRefs: refsForAudience(
    'Benchmark eval scorecard refs',
    evalJob.scorecardRefs,
    audience,
  ),
  suiteRefs: refsForAudience(
    'Benchmark eval suite refs',
    evalJob.suiteRefs,
    audience,
  ),
  taskRefs: refsForAudience(
    'Benchmark eval task refs',
    evalJob.taskRefs,
    audience,
  ),
  trainingRunRefs: refsForAudience(
    'Benchmark eval training run refs',
    evalJob.trainingRunRefs,
    audience,
  ),
})

const redactScorecard = (
  scorecard: OmniBenchmarkScorecardRecord,
  audience: OmniBenchmarkCloudAudience,
): OmniBenchmarkScorecardRecord => ({
  ...scorecard,
  caveatRefs: refsForAudience(
    'Benchmark scorecard caveat refs',
    scorecard.caveatRefs,
    audience,
  ),
  evalJobRefs: refsForAudience(
    'Benchmark scorecard eval job refs',
    scorecard.evalJobRefs,
    audience,
  ),
  evidenceRefs: refsForAudience(
    'Benchmark scorecard evidence refs',
    scorecard.evidenceRefs,
    audience,
  ),
  metricRefs: refsForAudience(
    'Benchmark scorecard metric refs',
    scorecard.metricRefs,
    audience,
  ),
  receiptRefs: refsForAudience(
    'Benchmark scorecard receipt refs',
    scorecard.receiptRefs,
    audience,
  ),
  scorecardRef: refForAudience(
    'Benchmark scorecard ref',
    scorecard.scorecardRef,
    audience,
    'scorecard.redacted.benchmark_cloud',
  ),
})

const redactRegression = (
  regression: OmniBenchmarkRegressionRecord,
  audience: OmniBenchmarkCloudAudience,
): OmniBenchmarkRegressionRecord => ({
  ...regression,
  affectedTaskRefs: refsForAudience(
    'Benchmark regression task refs',
    regression.affectedTaskRefs,
    audience,
  ),
  baselineEvalRef: refForAudience(
    'Benchmark regression baseline eval ref',
    regression.baselineEvalRef,
    audience,
    'eval.redacted.benchmark_cloud',
  ),
  caveatRefs: refsForAudience(
    'Benchmark regression caveat refs',
    regression.caveatRefs,
    audience,
  ),
  evidenceRefs: refsForAudience(
    'Benchmark regression evidence refs',
    regression.evidenceRefs,
    audience,
  ),
  promotionGateRefs: refsForAudience(
    'Benchmark regression promotion gate refs',
    regression.promotionGateRefs,
    audience,
  ),
  regressionRef: refForAudience(
    'Benchmark regression ref',
    regression.regressionRef,
    audience,
    'regression.redacted.benchmark_cloud',
  ),
  sourceEvalRef: refForAudience(
    'Benchmark regression source eval ref',
    regression.sourceEvalRef,
    audience,
    'eval.redacted.benchmark_cloud',
  ),
})

const redactFlake = (
  flake: OmniBenchmarkFlakeRecord,
  audience: OmniBenchmarkCloudAudience,
): OmniBenchmarkFlakeRecord => ({
  ...flake,
  caveatRefs: refsForAudience(
    'Benchmark flake caveat refs',
    flake.caveatRefs,
    audience,
  ),
  evalJobRefs: refsForAudience(
    'Benchmark flake eval job refs',
    flake.evalJobRefs,
    audience,
  ),
  evidenceRefs: refsForAudience(
    'Benchmark flake evidence refs',
    flake.evidenceRefs,
    audience,
  ),
  flakeRef: refForAudience(
    'Benchmark flake ref',
    flake.flakeRef,
    audience,
    'flake.redacted.benchmark_cloud',
  ),
  taskRefs: refsForAudience(
    'Benchmark flake task refs',
    flake.taskRefs,
    audience,
  ),
})

const redactComparison = (
  comparison: OmniBenchmarkComparisonRecord,
  audience: OmniBenchmarkCloudAudience,
): OmniBenchmarkComparisonRecord => ({
  ...comparison,
  baselineArtifactRefs: refsForAudience(
    'Benchmark comparison baseline artifact refs',
    comparison.baselineArtifactRefs,
    audience,
  ),
  baselineEvalRefs: refsForAudience(
    'Benchmark comparison baseline eval refs',
    comparison.baselineEvalRefs,
    audience,
  ),
  candidateArtifactRefs: refsForAudience(
    'Benchmark comparison candidate artifact refs',
    comparison.candidateArtifactRefs,
    audience,
  ),
  candidateEvalRefs: refsForAudience(
    'Benchmark comparison candidate eval refs',
    comparison.candidateEvalRefs,
    audience,
  ),
  caveatRefs: refsForAudience(
    'Benchmark comparison caveat refs',
    comparison.caveatRefs,
    audience,
  ),
  comparisonRef: refForAudience(
    'Benchmark comparison ref',
    comparison.comparisonRef,
    audience,
    'comparison.redacted.benchmark_cloud',
  ),
  evidenceRefs: refsForAudience(
    'Benchmark comparison evidence refs',
    comparison.evidenceRefs,
    audience,
  ),
  regressionRefs: refsForAudience(
    'Benchmark comparison regression refs',
    comparison.regressionRefs,
    audience,
  ),
  scorecardRefs: refsForAudience(
    'Benchmark comparison scorecard refs',
    comparison.scorecardRefs,
    audience,
  ),
})

export const projectOmniBenchmarkCloud = (
  record: OmniBenchmarkCloudRecord,
  audience: OmniBenchmarkCloudAudience,
  nowIso: string,
): OmniBenchmarkCloudProjection => {
  assertRecord(record)

  const promotionBlockingRegressions = record.regressions.filter(
    regression => regression.promotionBlocking,
  )

  return {
    artifactRefs: refsForAudience(
      'Benchmark Cloud artifact refs',
      record.artifactRefs,
      audience,
    ),
    audience,
    authority: record.authority,
    benchmarkLaunchAllowed: !record.authority.noBenchmarkLaunch,
    benchmarkRef: refForAudience(
      'Benchmark Cloud ref',
      record.benchmarkRef,
      audience,
      'benchmark.redacted.cloud',
    ),
    blockerRefs: refsForAudience(
      'Benchmark Cloud blocker refs',
      record.blockerRefs,
      audience,
    ),
    candidateRefs: refsForAudience(
      'Benchmark Cloud candidate refs',
      record.candidateRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Benchmark Cloud caveat refs',
      record.caveatRefs,
      audience,
    ),
    comparisonCount: record.comparisons.length,
    comparisons: record.comparisons.map(comparison =>
      redactComparison(comparison, audience),
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    evalExecutionAllowed: !record.authority.noEvalExecution,
    evalJobCount: record.evalJobs.length,
    evalJobs: record.evalJobs.map(evalJob =>
      redactEvalJob(evalJob, audience),
    ),
    failedScorecardCount: record.scorecards.filter(
      scorecard => scorecard.state === 'failed',
    ).length,
    flakeCount: record.flakes.length,
    flaky: hasAny(record.flakes) || record.state === 'flaky',
    flakes: record.flakes.map(flake => redactFlake(flake, audience)),
    id: refForAudience(
      'Benchmark Cloud id',
      record.id,
      audience,
      'benchmark-cloud.redacted',
    ),
    passedScorecardCount: record.scorecards.filter(
      scorecard => scorecard.state === 'passed',
    ).length,
    paymentSpendAllowed: !record.authority.noPaymentSpend,
    payoutMutationAllowed: !record.authority.noPayoutMutation,
    providerMutationAllowed: !record.authority.noProviderMutation,
    publicClaimUpgradeAllowed: !record.authority.noPublicClaimUpgrade,
    promotionBlocked: hasAny(promotionBlockingRegressions),
    promotionGateRefs: refsForAudience(
      'Benchmark Cloud promotion gate refs',
      record.promotionGateRefs,
      audience,
    ),
    rawBenchmarkInputCopyAllowed: !record.authority.noRawBenchmarkInputCopy,
    regressionCount: record.regressions.length,
    regressions: record.regressions.map(regression =>
      redactRegression(regression, audience),
    ),
    retainedFailureRefs: refsForAudience(
      'Benchmark Cloud retained failure refs',
      record.retainedFailureRefs,
      audience,
    ),
    routingMutationAllowed: !record.authority.noRoutingMutation,
    runtimePromotionAllowed: !record.authority.noRuntimePromotion,
    scorecardCount: record.scorecards.length,
    scorecards: record.scorecards.map(scorecard =>
      redactScorecard(scorecard, audience),
    ),
    settlementMutationAllowed: !record.authority.noSettlementMutation,
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    suiteCount: record.suites.length,
    suites: record.suites.map(suite => redactSuite(suite, audience)),
    taskCount: record.tasks.length,
    tasks: record.tasks.map(task => redactTask(task, audience)),
    trainingRunRefs: refsForAudience(
      'Benchmark Cloud training run refs',
      record.trainingRunRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  }
}

export const omniBenchmarkCloudProjectionHasPrivateMaterial = (
  projection: OmniBenchmarkCloudProjection,
): boolean =>
  projectionStringValues(projection).some(
    value =>
      unsafeBenchmarkCloudRefPattern.test(value) ||
      rawTimestampPattern.test(value),
  )

const projectionStringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(projectionStringValues)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.values(value).flatMap(projectionStringValues)
  }

  return []
}

export const exampleOmniBenchmarkCloud = (): OmniBenchmarkCloudRecord => ({
  artifactRefs: ['artifact.public.autopilot_lora_candidate_v2'],
  authority: OMNI_BENCHMARK_CLOUD_READ_ONLY_AUTHORITY,
  benchmarkRef: 'benchmark.public.autopilot_coding_cloud_v1',
  blockerRefs: [],
  candidateRefs: ['candidate.public.autopilot_lora_v2'],
  caveatRefs: ['caveat.public.benchmark_cloud_evidence_only'],
  comparisons: [
    {
      baselineArtifactRefs: ['artifact.public.autopilot_baseline_v1'],
      baselineEvalRefs: ['eval.public.autopilot_baseline_cloud'],
      candidateArtifactRefs: ['artifact.public.autopilot_lora_candidate_v2'],
      candidateEvalRefs: ['eval.public.autopilot_candidate_cloud'],
      caveatRefs: ['caveat.public.benchmark_comparison_evidence_only'],
      comparisonRef: 'comparison.public.autopilot_candidate_vs_baseline',
      evidenceRefs: ['evidence.public.benchmark_comparison_summary'],
      regressionRefs: [],
      scorecardRefs: ['scorecard.public.autopilot_candidate_cloud'],
    },
  ],
  createdAtIso: '2026-06-06T23:40:00.000Z',
  evalJobs: [
    {
      artifactRefs: ['artifact.public.autopilot_lora_candidate_v2'],
      blockerRefs: [],
      candidateRefs: ['candidate.public.autopilot_lora_v2'],
      caveatRefs: ['caveat.public.eval_job_evidence_only'],
      comparisonRefs: ['comparison.public.autopilot_candidate_vs_baseline'],
      evalJobRef: 'eval.public.autopilot_candidate_cloud',
      evidenceRefs: ['evidence.public.eval_job_trace_summary'],
      flakeRefs: [],
      providerRefs: ['provider.public.benchmark_cloud'],
      receiptRefs: ['receipt.public.eval_job_completed'],
      regressionRefs: [],
      retainedFailureRefs: ['retained_failure.public.site_revision_images'],
      runnerRefs: ['runner.public.benchmark_cloud_worker_pool'],
      scorecardRefs: ['scorecard.public.autopilot_candidate_cloud'],
      state: 'passed',
      suiteRefs: ['suite.public.autopilot_sites_coding'],
      taskRefs: ['task.public.site_revision_image_grounding'],
      trainingRunRefs: ['training.public.autopilot_lora_v2_imported'],
    },
    {
      artifactRefs: ['artifact.public.autopilot_baseline_v1'],
      blockerRefs: [],
      candidateRefs: ['candidate.public.autopilot_baseline_v1'],
      caveatRefs: ['caveat.public.baseline_eval_imported'],
      comparisonRefs: ['comparison.public.autopilot_candidate_vs_baseline'],
      evalJobRef: 'eval.public.autopilot_baseline_cloud',
      evidenceRefs: ['evidence.public.baseline_eval_summary'],
      flakeRefs: [],
      providerRefs: ['provider.public.benchmark_cloud'],
      receiptRefs: ['receipt.public.baseline_eval_imported'],
      regressionRefs: [],
      retainedFailureRefs: ['retained_failure.public.site_revision_images'],
      runnerRefs: ['runner.public.benchmark_cloud_worker_pool'],
      scorecardRefs: ['scorecard.public.autopilot_baseline_cloud'],
      state: 'passed',
      suiteRefs: ['suite.public.autopilot_sites_coding'],
      taskRefs: ['task.public.site_revision_image_grounding'],
      trainingRunRefs: [],
    },
  ],
  flakes: [],
  id: 'benchmark_cloud.public.autopilot_candidate_cloud',
  promotionGateRefs: ['gate.public.model_lab_promotion_review'],
  regressions: [],
  retainedFailureRefs: ['retained_failure.public.site_revision_images'],
  scorecards: [
    {
      caveatRefs: ['caveat.public.scorecard_evidence_only'],
      evalJobRefs: ['eval.public.autopilot_candidate_cloud'],
      evidenceRefs: ['evidence.public.scorecard_candidate_summary'],
      metricRefs: ['metric.public.visual_grounding_pass_rate'],
      observedScoreBps: 9400,
      passThresholdBps: 9000,
      receiptRefs: ['receipt.public.scorecard_candidate_passed'],
      scorecardRef: 'scorecard.public.autopilot_candidate_cloud',
      state: 'passed',
    },
    {
      caveatRefs: ['caveat.public.baseline_scorecard_imported'],
      evalJobRefs: ['eval.public.autopilot_baseline_cloud'],
      evidenceRefs: ['evidence.public.scorecard_baseline_summary'],
      metricRefs: ['metric.public.visual_grounding_pass_rate'],
      observedScoreBps: 8500,
      passThresholdBps: 8000,
      receiptRefs: ['receipt.public.scorecard_baseline_imported'],
      scorecardRef: 'scorecard.public.autopilot_baseline_cloud',
      state: 'passed',
    },
  ],
  state: 'passed',
  suites: [
    {
      caveatRefs: ['caveat.public.suite_fixture_refs_only'],
      evidenceRefs: ['evidence.public.suite_definition_summary'],
      policyRefs: ['policy.public.model_lab_eval_redaction'],
      suiteRef: 'suite.public.autopilot_sites_coding',
      taskRefs: ['task.public.site_revision_image_grounding'],
    },
  ],
  tasks: [
    {
      caveatRefs: ['caveat.public.task_public_fixture_only'],
      datasetRefs: ['dataset.public.site_revision_fixture_set'],
      evidenceRefs: ['evidence.public.task_definition_summary'],
      expectedOutputRefs: ['expected_output.public.image_grounded_site_revision'],
      inputClass: 'public_fixture',
      suiteRefs: ['suite.public.autopilot_sites_coding'],
      taskRef: 'task.public.site_revision_image_grounding',
    },
  ],
  trainingRunRefs: ['training.public.autopilot_lora_v2_imported'],
  updatedAtIso: '2026-06-06T23:54:00.000Z',
})
