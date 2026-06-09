import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const OmniTrainingRunAudience = S.Literals([
  'public',
  'agent',
  'customer',
  'team',
  'operator',
])
export type OmniTrainingRunAudience = typeof OmniTrainingRunAudience.Type

export const OmniTrainingRunKind = S.Literals([
  'adapter_tune',
  'benchmark_replay',
  'data_preparation',
  'distillation',
  'eval_only',
  'fine_tune',
  'optimizer',
])
export type OmniTrainingRunKind = typeof OmniTrainingRunKind.Type

export const OmniTrainingRunState = S.Literals([
  'archived',
  'blocked',
  'completed',
  'failed',
  'imported',
  'planned',
  'reviewed',
  'running',
  'superseded',
])
export type OmniTrainingRunState = typeof OmniTrainingRunState.Type

export const OmniTrainingRunReadiness = S.Literals([
  'archived',
  'blocked',
  'complete',
  'failed',
  'imported',
  'missing_evidence',
  'needs_review',
  'running',
])
export type OmniTrainingRunReadiness =
  typeof OmniTrainingRunReadiness.Type

export const OmniTrainingRunAuthorityBoundary = S.Literals([
  'read_only_training_run',
])
export type OmniTrainingRunAuthorityBoundary =
  typeof OmniTrainingRunAuthorityBoundary.Type

export class OmniTrainingRunAuthority extends S.Class<OmniTrainingRunAuthority>(
  'OmniTrainingRunAuthority',
)({
  authorityBoundary: OmniTrainingRunAuthorityBoundary,
  noAdapterInstall: S.Boolean,
  noModelTrainingLaunch: S.Boolean,
  noPaymentSpend: S.Boolean,
  noPayoutMutation: S.Boolean,
  noProviderMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noRawDatasetCopy: S.Boolean,
  noRoutingMutation: S.Boolean,
  noRuntimePromotion: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class OmniTrainingRunHyperparameterRecord extends S.Class<OmniTrainingRunHyperparameterRecord>(
  'OmniTrainingRunHyperparameterRecord',
)({
  evidenceRefs: S.Array(S.String),
  name: S.String,
  paramRef: S.String,
  valueSummary: S.String,
}) {}

export class OmniTrainingRunMetricRecord extends S.Class<OmniTrainingRunMetricRecord>(
  'OmniTrainingRunMetricRecord',
)({
  evidenceRefs: S.Array(S.String),
  metricRef: S.String,
  name: S.String,
  unit: S.String,
  value: S.Number,
}) {}

export class OmniTrainingRunBudgetRecord extends S.Class<OmniTrainingRunBudgetRecord>(
  'OmniTrainingRunBudgetRecord',
)({
  actualCostCents: S.NullOr(S.Number),
  budgetRef: S.String,
  caveatRefs: S.Array(S.String),
  creditRefs: S.Array(S.String),
  modeledCostCents: S.NullOr(S.Number),
  paymentSpendAllowed: S.Boolean,
}) {}

export class OmniTrainingRunRecord extends S.Class<OmniTrainingRunRecord>(
  'OmniTrainingRunRecord',
)({
  artifactRefs: S.Array(S.String),
  authority: OmniTrainingRunAuthority,
  benchmarkRefs: S.Array(S.String),
  budget: OmniTrainingRunBudgetRecord,
  candidateRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  dataPackageRefs: S.Array(S.String),
  evalRerunRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  failureRefs: S.Array(S.String),
  hyperparameters: S.Array(OmniTrainingRunHyperparameterRecord),
  id: S.String,
  kind: OmniTrainingRunKind,
  metrics: S.Array(OmniTrainingRunMetricRecord),
  modelLabLoopRefs: S.Array(S.String),
  operatorReviewReceiptRefs: S.Array(S.String),
  optimizerRefs: S.Array(S.String),
  providerRefs: S.Array(S.String),
  retainedFailureRefs: S.Array(S.String),
  runRef: S.String,
  runnerRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: OmniTrainingRunState,
  triggerWorkroomRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class OmniTrainingRunProjection extends S.Class<OmniTrainingRunProjection>(
  'OmniTrainingRunProjection',
)({
  adapterInstallAllowed: S.Boolean,
  artifactRefs: S.Array(S.String),
  audience: OmniTrainingRunAudience,
  authority: OmniTrainingRunAuthority,
  benchmarkRefs: S.Array(S.String),
  budget: OmniTrainingRunBudgetRecord,
  candidateRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  dataPackageRefs: S.Array(S.String),
  evalRerunRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  failureRefs: S.Array(S.String),
  hyperparameterCount: S.Number,
  hyperparameters: S.Array(OmniTrainingRunHyperparameterRecord),
  id: S.String,
  kind: OmniTrainingRunKind,
  metricCount: S.Number,
  metrics: S.Array(OmniTrainingRunMetricRecord),
  modelLabLoopRefs: S.Array(S.String),
  modelTrainingLaunchAllowed: S.Boolean,
  operatorReviewReceiptRefs: S.Array(S.String),
  optimizerRefs: S.Array(S.String),
  paymentSpendAllowed: S.Boolean,
  payoutMutationAllowed: S.Boolean,
  providerMutationAllowed: S.Boolean,
  providerRefs: S.Array(S.String),
  publicClaimUpgradeAllowed: S.Boolean,
  rawDatasetCopyAllowed: S.Boolean,
  readiness: OmniTrainingRunReadiness,
  readinessLabel: S.String,
  retainedFailureRefs: S.Array(S.String),
  routingMutationAllowed: S.Boolean,
  runRef: S.String,
  runnerRefs: S.Array(S.String),
  runtimePromotionAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  sourceRefs: S.Array(S.String),
  state: OmniTrainingRunState,
  stateLabel: S.String,
  triggerWorkroomRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class OmniTrainingRunUnsafe extends S.TaggedErrorClass<OmniTrainingRunUnsafe>()(
  'OmniTrainingRunUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_TRAINING_RUN_READ_ONLY_AUTHORITY:
  OmniTrainingRunAuthority = {
    authorityBoundary: 'read_only_training_run',
    noAdapterInstall: true,
    noModelTrainingLaunch: true,
    noPaymentSpend: true,
    noPayoutMutation: true,
    noProviderMutation: true,
    noPublicClaimUpgrade: true,
    noRawDatasetCopy: true,
    noRoutingMutation: true,
    noRuntimePromotion: true,
    noSettlementMutation: true,
  }

const stateLabelByState: Readonly<Record<OmniTrainingRunState, string>> = {
  archived: 'Archived',
  blocked: 'Blocked',
  completed: 'Completed evidence',
  failed: 'Failed',
  imported: 'Imported evidence',
  planned: 'Planned evidence',
  reviewed: 'Reviewed evidence',
  running: 'Running evidence',
  superseded: 'Superseded',
}

const readinessLabelByReadiness:
  Readonly<Record<OmniTrainingRunReadiness, string>> = {
    archived: 'Archived',
    blocked: 'Blocked',
    complete: 'Complete',
    failed: 'Failed',
    imported: 'Imported',
    missing_evidence: 'Missing evidence',
    needs_review: 'Needs review',
    running: 'Running',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeTrainingRunRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.(raw|private)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(artifact\.private|benchmark\.private|budget\.private|candidate\.private|caveat\.private|credit\.private|data_package\.private|eval\.private|evidence\.private|failure\.private|hyperparam\.private|loop\.private|metric\.private|operator_review\.private|optimizer\.private|provider\.|retained_failure\.private|run\.private|runner\.private|source\.|training\.private|workroom\.private)/i
const agentUnsafeRefPattern =
  /(artifact\.private|benchmark\.private|budget\.private|candidate\.private|credit\.private|data_package\.private|eval\.private|failure\.private|hyperparam\.private|metric\.private|operator_review\.private|provider\.private|run\.private|runner\.private|source\.private|training\.private|workroom\.private)/i
const customerUnsafeRefPattern =
  /(artifact\.private|benchmark\.private|budget\.private|candidate\.private|credit\.private|data_package\.private|eval\.private|failure\.private|hyperparam\.private|metric\.private|operator_review\.private|provider\.private|run\.private|runner\.private|source\.private|training\.private|workroom\.private)/i

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
      unsafeTrainingRunRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniTrainingRunUnsafe({
      reason: `${label} contains private prompts, source archives, datasets, provider payloads, model weights, secrets, payment/wallet material, private repos, raw logs, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniTrainingRunAudience,
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
  audience: OmniTrainingRunAudience,
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
  audience: OmniTrainingRunAudience,
  redactedRef: string,
): string => refsForAudience(label, [ref], audience)[0] ?? redactedRef

const assertReadOnlyAuthority = (authority: OmniTrainingRunAuthority): void => {
  if (
    authority.noAdapterInstall !== true ||
    authority.noModelTrainingLaunch !== true ||
    authority.noPaymentSpend !== true ||
    authority.noPayoutMutation !== true ||
    authority.noProviderMutation !== true ||
    authority.noPublicClaimUpgrade !== true ||
    authority.noRawDatasetCopy !== true ||
    authority.noRoutingMutation !== true ||
    authority.noRuntimePromotion !== true ||
    authority.noSettlementMutation !== true
  ) {
    throw new OmniTrainingRunUnsafe({
      reason:
        'Training runs are read-only evidence and cannot launch training, mutate providers, install adapters, spend money, promote runtime behavior, mutate routes, pay out, settle, or upgrade public claims.',
    })
  }
}

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new OmniTrainingRunUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertHyperparameter = (
  hyperparameter: OmniTrainingRunHyperparameterRecord,
): void => {
  assertSafeRefs('Training hyperparameter ref', [hyperparameter.paramRef])
  assertSafeRefs('Training hyperparameter name', [hyperparameter.name])
  assertSafeRefs(
    'Training hyperparameter value summary',
    [hyperparameter.valueSummary],
  )
  assertSafeRefs(
    'Training hyperparameter evidence refs',
    hyperparameter.evidenceRefs,
  )

  if (!hasAny(hyperparameter.evidenceRefs)) {
    throw new OmniTrainingRunUnsafe({
      reason: 'Training hyperparameters require evidence refs.',
    })
  }
}

const assertMetric = (metric: OmniTrainingRunMetricRecord): void => {
  assertSafeRefs('Training metric ref', [metric.metricRef])
  assertSafeRefs('Training metric name', [metric.name])
  assertSafeRefs('Training metric unit', [metric.unit])
  assertSafeRefs('Training metric evidence refs', metric.evidenceRefs)

  if (!hasAny(metric.evidenceRefs)) {
    throw new OmniTrainingRunUnsafe({
      reason: 'Training metrics require evidence refs.',
    })
  }

  if (!Number.isFinite(metric.value)) {
    throw new OmniTrainingRunUnsafe({
      reason: 'Training metric values must be finite.',
    })
  }
}

const assertBudget = (budget: OmniTrainingRunBudgetRecord): void => {
  assertSafeRefs('Training budget ref', [budget.budgetRef])
  assertSafeRefs('Training budget caveat refs', budget.caveatRefs)
  assertSafeRefs('Training credit refs', budget.creditRefs)

  if (budget.paymentSpendAllowed !== false) {
    throw new OmniTrainingRunUnsafe({
      reason: 'Training run budget records cannot grant payment spend.',
    })
  }

  if (
    (budget.modeledCostCents !== null && budget.modeledCostCents < 0) ||
    (budget.actualCostCents !== null && budget.actualCostCents < 0)
  ) {
    throw new OmniTrainingRunUnsafe({
      reason: 'Training run costs cannot be negative.',
    })
  }

  if (
    budget.actualCostCents !== null &&
    budget.actualCostCents > 0 &&
    !hasAny(budget.creditRefs)
  ) {
    throw new OmniTrainingRunUnsafe({
      reason: 'Observed training costs require credit or cost evidence refs.',
    })
  }
}

const trainingKindNeedsDataPackage = (
  kind: OmniTrainingRunKind,
): boolean =>
  kind === 'adapter_tune' ||
  kind === 'data_preparation' ||
  kind === 'distillation' ||
  kind === 'fine_tune'

const assertRecord = (record: OmniTrainingRunRecord): void => {
  assertReadOnlyAuthority(record.authority)
  assertValidIso('createdAtIso', record.createdAtIso)
  assertValidIso('updatedAtIso', record.updatedAtIso)

  assertSafeRefs('Training run id', [record.id])
  assertSafeRefs('Training run ref', [record.runRef])
  assertSafeRefs('Training artifact refs', record.artifactRefs)
  assertSafeRefs('Training benchmark refs', record.benchmarkRefs)
  assertSafeRefs('Training candidate refs', record.candidateRefs)
  assertSafeRefs('Training caveat refs', record.caveatRefs)
  assertSafeRefs('Training data package refs', record.dataPackageRefs)
  assertSafeRefs('Training eval rerun refs', record.evalRerunRefs)
  assertSafeRefs('Training evidence refs', record.evidenceRefs)
  assertSafeRefs('Training failure refs', record.failureRefs)
  assertSafeRefs('Training loop refs', record.modelLabLoopRefs)
  assertSafeRefs(
    'Training operator review receipt refs',
    record.operatorReviewReceiptRefs,
  )
  assertSafeRefs('Training optimizer refs', record.optimizerRefs)
  assertSafeRefs('Training provider refs', record.providerRefs)
  assertSafeRefs('Training retained failure refs', record.retainedFailureRefs)
  assertSafeRefs('Training runner refs', record.runnerRefs)
  assertSafeRefs('Training source refs', record.sourceRefs)
  assertSafeRefs('Training trigger workroom refs', record.triggerWorkroomRefs)
  record.hyperparameters.forEach(assertHyperparameter)
  record.metrics.forEach(assertMetric)
  assertBudget(record.budget)

  if (!hasAny(record.sourceRefs) || !hasAny(record.evidenceRefs)) {
    throw new OmniTrainingRunUnsafe({
      reason: 'Training runs require source and evidence refs.',
    })
  }

  if (trainingKindNeedsDataPackage(record.kind) && !hasAny(record.dataPackageRefs)) {
    throw new OmniTrainingRunUnsafe({
      reason: 'Fine-tune, adapter, distillation, and data-prep runs require data package refs.',
    })
  }

  if (record.state === 'running' && (!hasAny(record.providerRefs) || !hasAny(record.runnerRefs))) {
    throw new OmniTrainingRunUnsafe({
      reason: 'Running training evidence requires provider and runner refs.',
    })
  }

  if (record.state === 'failed' && !hasAny(record.failureRefs)) {
    throw new OmniTrainingRunUnsafe({
      reason: 'Failed training runs require failure refs.',
    })
  }

  if (record.state === 'blocked' && !hasAny(record.caveatRefs)) {
    throw new OmniTrainingRunUnsafe({
      reason: 'Blocked training runs require caveat refs.',
    })
  }

  if (
    (record.state === 'completed' || record.state === 'reviewed') &&
    (!hasAny(record.artifactRefs) ||
      !hasAny(record.metrics) ||
      (!hasAny(record.evalRerunRefs) && !hasAny(record.benchmarkRefs)))
  ) {
    throw new OmniTrainingRunUnsafe({
      reason:
        'Completed and reviewed training runs require artifact refs, metrics, and eval or benchmark evidence.',
    })
  }

  if (record.state === 'reviewed' && !hasAny(record.operatorReviewReceiptRefs)) {
    throw new OmniTrainingRunUnsafe({
      reason: 'Reviewed training runs require operator review receipt refs.',
    })
  }
}

const readinessForRecord = (
  record: OmniTrainingRunRecord,
): OmniTrainingRunReadiness => {
  if (record.state === 'archived' || record.state === 'superseded') {
    return 'archived'
  }

  if (record.state === 'blocked') {
    return 'blocked'
  }

  if (record.state === 'failed') {
    return 'failed'
  }

  if (record.state === 'running') {
    return 'running'
  }

  if (!hasAny(record.sourceRefs) || !hasAny(record.evidenceRefs)) {
    return 'missing_evidence'
  }

  if (record.state === 'reviewed') {
    return 'complete'
  }

  if (record.state === 'completed') {
    return 'needs_review'
  }

  return 'imported'
}

const hyperparameterForAudience = (
  hyperparameter: OmniTrainingRunHyperparameterRecord,
  audience: OmniTrainingRunAudience,
): OmniTrainingRunHyperparameterRecord => ({
  ...hyperparameter,
  evidenceRefs: refsForAudience(
    'Training hyperparameter evidence refs',
    hyperparameter.evidenceRefs,
    audience,
  ),
  paramRef: refForAudience(
    'Training hyperparameter ref',
    hyperparameter.paramRef,
    audience,
    'hyperparam.redacted.training_run',
  ),
})

const metricForAudience = (
  metric: OmniTrainingRunMetricRecord,
  audience: OmniTrainingRunAudience,
): OmniTrainingRunMetricRecord => ({
  ...metric,
  evidenceRefs: refsForAudience(
    'Training metric evidence refs',
    metric.evidenceRefs,
    audience,
  ),
  metricRef: refForAudience(
    'Training metric ref',
    metric.metricRef,
    audience,
    'metric.redacted.training_run',
  ),
})

const budgetForAudience = (
  budget: OmniTrainingRunBudgetRecord,
  audience: OmniTrainingRunAudience,
): OmniTrainingRunBudgetRecord => ({
  ...budget,
  budgetRef: refForAudience(
    'Training budget ref',
    budget.budgetRef,
    audience,
    'budget.redacted.training_run',
  ),
  caveatRefs: refsForAudience(
    'Training budget caveat refs',
    budget.caveatRefs,
    audience,
  ),
  creditRefs: refsForAudience('Training credit refs', budget.creditRefs, audience),
})

const stringValuesFromUnknown = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(stringValuesFromUnknown)
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value).flatMap(stringValuesFromUnknown)
  }

  return []
}

export const omniTrainingRunProjectionHasPrivateMaterial = (
  projection: OmniTrainingRunProjection,
): boolean => {
  const values = stringValuesFromUnknown(projection).join('\n')

  return unsafeTrainingRunRefPattern.test(values) ||
    rawTimestampPattern.test(values)
}

export const projectOmniTrainingRun = (
  record: OmniTrainingRunRecord,
  audience: OmniTrainingRunAudience,
  nowIso: string,
): OmniTrainingRunProjection => {
  assertRecord(record)
  assertValidIso('nowIso', nowIso)

  const readiness = readinessForRecord(record)
  const projection: OmniTrainingRunProjection = {
    adapterInstallAllowed: false,
    artifactRefs: refsForAudience(
      'Training artifact refs',
      record.artifactRefs,
      audience,
    ),
    audience,
    authority: record.authority,
    benchmarkRefs: refsForAudience(
      'Training benchmark refs',
      record.benchmarkRefs,
      audience,
    ),
    budget: budgetForAudience(record.budget, audience),
    candidateRefs: refsForAudience(
      'Training candidate refs',
      record.candidateRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Training caveat refs',
      record.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    dataPackageRefs: refsForAudience(
      'Training data package refs',
      record.dataPackageRefs,
      audience,
    ),
    evalRerunRefs: refsForAudience(
      'Training eval rerun refs',
      record.evalRerunRefs,
      audience,
    ),
    evidenceRefs: refsForAudience(
      'Training evidence refs',
      record.evidenceRefs,
      audience,
    ),
    failureRefs: refsForAudience(
      'Training failure refs',
      record.failureRefs,
      audience,
    ),
    hyperparameterCount: record.hyperparameters.length,
    hyperparameters: record.hyperparameters.map(hyperparameter =>
      hyperparameterForAudience(hyperparameter, audience),
    ),
    id: refForAudience('Training run id', record.id, audience, 'training.redacted'),
    kind: record.kind,
    metricCount: record.metrics.length,
    metrics: record.metrics.map(metric => metricForAudience(metric, audience)),
    modelLabLoopRefs: refsForAudience(
      'Training loop refs',
      record.modelLabLoopRefs,
      audience,
    ),
    modelTrainingLaunchAllowed: false,
    operatorReviewReceiptRefs: refsForAudience(
      'Training operator review receipt refs',
      record.operatorReviewReceiptRefs,
      audience,
    ),
    optimizerRefs: refsForAudience(
      'Training optimizer refs',
      record.optimizerRefs,
      audience,
    ),
    paymentSpendAllowed: false,
    payoutMutationAllowed: false,
    providerMutationAllowed: false,
    providerRefs: refsForAudience(
      'Training provider refs',
      record.providerRefs,
      audience,
    ),
    publicClaimUpgradeAllowed: false,
    rawDatasetCopyAllowed: false,
    readiness,
    readinessLabel: readinessLabelByReadiness[readiness],
    retainedFailureRefs: refsForAudience(
      'Training retained failure refs',
      record.retainedFailureRefs,
      audience,
    ),
    routingMutationAllowed: false,
    runRef: refForAudience(
      'Training run ref',
      record.runRef,
      audience,
      'run.redacted.training_run',
    ),
    runnerRefs: refsForAudience(
      'Training runner refs',
      record.runnerRefs,
      audience,
    ),
    runtimePromotionAllowed: false,
    settlementMutationAllowed: false,
    sourceRefs: refsForAudience(
      'Training source refs',
      record.sourceRefs,
      audience,
    ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    triggerWorkroomRefs: refsForAudience(
      'Training trigger workroom refs',
      record.triggerWorkroomRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  }

  if (omniTrainingRunProjectionHasPrivateMaterial(projection)) {
    throw new OmniTrainingRunUnsafe({
      reason:
        'Training run projection contains private prompt, source, dataset, provider, model, payment, wallet, raw log, or raw timestamp material.',
    })
  }

  return projection
}

export const exampleOmniTrainingRun = (): OmniTrainingRunRecord => ({
  artifactRefs: ['artifact.public.otect_layout_adapter_v1'],
  authority: OMNI_TRAINING_RUN_READ_ONLY_AUTHORITY,
  benchmarkRefs: ['benchmark.public.otect_revision_suite'],
  budget: {
    actualCostCents: 84,
    budgetRef: 'budget.public.model_lab_run_otel',
    caveatRefs: ['caveat.public.cost_imported_not_spend_authority'],
    creditRefs: ['credit.public.operator_lab_budget'],
    modeledCostCents: 120,
    paymentSpendAllowed: false,
  },
  candidateRefs: ['candidate.public.otect_adapter_candidate'],
  caveatRefs: ['caveat.public.imported_training_evidence_only'],
  createdAtIso: '2026-06-06T23:05:00.000Z',
  dataPackageRefs: ['data_package.public.otect_image_feedback_refs'],
  evalRerunRefs: ['eval.public.otect_revision_regression_pass'],
  evidenceRefs: ['evidence.public.training_run_manifest'],
  failureRefs: [],
  hyperparameters: [
    {
      evidenceRefs: ['evidence.public.hyperparameter_manifest'],
      name: 'learning_rate',
      paramRef: 'hyperparam.public.learning_rate',
      valueSummary: 'summary.public.low_rank_adapter_lr',
    },
  ],
  id: 'training_run.public.otect_adapter_tune',
  kind: 'adapter_tune',
  metrics: [
    {
      evidenceRefs: ['evidence.public.metric_manifest'],
      metricRef: 'metric.public.eval_pass_rate',
      name: 'eval_pass_rate',
      unit: 'ratio',
      value: 0.97,
    },
  ],
  modelLabLoopRefs: ['loop.public.otect_retained_failure_loop'],
  operatorReviewReceiptRefs: ['review.public.operator_model_lab_approved'],
  optimizerRefs: ['optimizer.public.prompt_signature_search'],
  providerRefs: ['provider.public.psionic_lab'],
  retainedFailureRefs: ['retained_failure.public.otect_revision_images'],
  runRef: 'run.public.otect_adapter_tune',
  runnerRefs: ['runner.public.model_lab_sandbox'],
  sourceRefs: ['source.public.otect_revision_training_brief'],
  state: 'reviewed',
  triggerWorkroomRefs: ['workroom.public.otect_revision_two'],
  updatedAtIso: '2026-06-06T23:25:00.000Z',
})
