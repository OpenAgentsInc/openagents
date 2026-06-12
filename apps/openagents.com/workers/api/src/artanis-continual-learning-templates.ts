import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { TASSADAR_EXECUTOR_CAPABILITY_REF } from '@openagentsinc/tassadar-executor'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  PylonMarketplaceCreateJobIntakeRequest,
  PylonMarketplaceTriageJobIntakeRequest,
} from './pylon-marketplace-service'
import {
  type PylonMarketplaceJobKind,
} from './pylon-marketplace-jobs'
import { type PylonResourceMode } from './pylon-resource-mode-setup'

export const ArtanisContinualLearningTemplateKind = S.Literals([
  'adapter_validation',
  'benchmark_eval_rerun',
  'dataset_curation',
  'dspy_gepa_optimization',
  'executor_trace_replay',
  'lora_finetuning_training',
  'regression_analysis',
])
export type ArtanisContinualLearningTemplateKind =
  typeof ArtanisContinualLearningTemplateKind.Type

export const ArtanisContinualLearningTemplateState = S.Literals([
  'accepted',
  'blocked',
  'proposed',
  'rejected',
  'running',
])
export type ArtanisContinualLearningTemplateState =
  typeof ArtanisContinualLearningTemplateState.Type

export const ArtanisContinualLearningAudience = S.Literals([
  'operator',
  'public_artanis',
  'public_forum',
])
export type ArtanisContinualLearningAudience =
  typeof ArtanisContinualLearningAudience.Type

export const ArtanisContinualLearningRiskLabel = S.Literals([
  'low',
  'medium',
  'high',
])
export type ArtanisContinualLearningRiskLabel =
  typeof ArtanisContinualLearningRiskLabel.Type

export const ArtanisContinualLearningCostLabel = S.Literals([
  'low',
  'medium',
  'high',
  'unknown',
])
export type ArtanisContinualLearningCostLabel =
  typeof ArtanisContinualLearningCostLabel.Type

export class ArtanisContinualLearningAuthority extends S.Class<ArtanisContinualLearningAuthority>(
  'ArtanisContinualLearningAuthority',
)({
  adapterInstallAllowed: S.Boolean,
  benchmarkLaunchAllowed: S.Boolean,
  modelPromotionAllowed: S.Boolean,
  paymentSpendAllowed: S.Boolean,
  providerMutationAllowed: S.Boolean,
  pylonDispatchAllowed: S.Boolean,
  reportPublicationAllowed: S.Boolean,
  runtimePromotionAllowed: S.Boolean,
  trainingLaunchAllowed: S.Boolean,
}) {}

export class ArtanisContinualLearningTemplateRecord extends S.Class<ArtanisContinualLearningTemplateRecord>(
  'ArtanisContinualLearningTemplateRecord',
)({
  acceptanceCriteriaRefs: S.Array(S.String),
  approvalRequirementRefs: S.Array(S.String),
  benchmarkCloudRefs: S.Array(S.String),
  benchmarkTargetRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  costCaveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  dispatchPayloadSchemaRefs: S.Array(S.String),
  downstreamExecutorAuthorityRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  forumSummaryRefs: S.Array(S.String),
  kind: ArtanisContinualLearningTemplateKind,
  marketplaceIntakeRefs: S.Array(S.String),
  modelArtifactRefs: S.Array(S.String),
  modelLabEvidenceRefs: S.Array(S.String),
  operatorApprovalRefs: S.Array(S.String),
  operatorDetailRefs: S.Array(S.String),
  promotionDecisionRefs: S.Array(S.String),
  publicReportRefs: S.Array(S.String),
  requiredCapabilityRefs: S.Array(S.String),
  retainedFailureRefs: S.Array(S.String),
  riskLabel: ArtanisContinualLearningRiskLabel,
  rollbackPostureRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  spendLabel: ArtanisContinualLearningCostLabel,
  spendLimitRefs: S.Array(S.String),
  state: ArtanisContinualLearningTemplateState,
  templateRef: S.String,
  trainingRunRefs: S.Array(S.String),
  updatedAtIso: S.String,
  workloadRefs: S.Array(S.String),
}) {}

export class ArtanisContinualLearningTemplateProjection extends S.Class<ArtanisContinualLearningTemplateProjection>(
  'ArtanisContinualLearningTemplateProjection',
)({
  acceptanceCriteriaRefs: S.Array(S.String),
  approvalRequirementRefs: S.Array(S.String),
  benchmarkCloudRefs: S.Array(S.String),
  benchmarkTargetRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  costCaveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  dispatchPayloadSchemaRefs: S.Array(S.String),
  downstreamExecutorAuthorityRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  forumSummaryRefs: S.Array(S.String),
  forumSummaryState: ArtanisContinualLearningTemplateState,
  kind: ArtanisContinualLearningTemplateKind,
  marketplaceIntakeRefs: S.Array(S.String),
  modelArtifactRefs: S.Array(S.String),
  modelLabEvidenceRefs: S.Array(S.String),
  operatorApprovalRefs: S.Array(S.String),
  operatorDetailRefs: S.Array(S.String),
  operatorReadyProposal: S.Boolean,
  promotionDecisionRefs: S.Array(S.String),
  publicReportRefs: S.Array(S.String),
  requiredCapabilityRefs: S.Array(S.String),
  retainedFailureRefs: S.Array(S.String),
  riskLabel: ArtanisContinualLearningRiskLabel,
  rollbackPostureRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  spendLabel: ArtanisContinualLearningCostLabel,
  spendLimitRefs: S.Array(S.String),
  state: ArtanisContinualLearningTemplateState,
  templateExecutionAllowed: S.Boolean,
  templateRef: S.String,
  trainingRunRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  workloadRefs: S.Array(S.String),
}) {}

export class ArtanisContinualLearningTemplateLedgerRecord extends S.Class<ArtanisContinualLearningTemplateLedgerRecord>(
  'ArtanisContinualLearningTemplateLedgerRecord',
)({
  agentId: S.String,
  authority: ArtanisContinualLearningAuthority,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  ledgerRef: S.String,
  sourceRefs: S.Array(S.String),
  templates: S.Array(ArtanisContinualLearningTemplateRecord),
  updatedAtIso: S.String,
}) {}

export class ArtanisContinualLearningTemplateLedgerProjection extends S.Class<ArtanisContinualLearningTemplateLedgerProjection>(
  'ArtanisContinualLearningTemplateLedgerProjection',
)({
  acceptedCount: S.Number,
  agentId: S.String,
  audience: ArtanisContinualLearningAudience,
  authority: ArtanisContinualLearningAuthority,
  blockedCount: S.Number,
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  ledgerRef: S.String,
  operatorReadyProposalCount: S.Number,
  proposedCount: S.Number,
  rejectedCount: S.Number,
  runningCount: S.Number,
  sourceRefs: S.Array(S.String),
  templateCount: S.Number,
  templates: S.Array(ArtanisContinualLearningTemplateProjection),
  updatedAtDisplay: S.String,
}) {}

export class ArtanisContinualLearningTemplateUnsafe extends S.TaggedErrorClass<ArtanisContinualLearningTemplateUnsafe>()(
  'ArtanisContinualLearningTemplateUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_CONTINUAL_LEARNING_NO_EXECUTION_AUTHORITY:
  ArtanisContinualLearningAuthority = {
    adapterInstallAllowed: false,
    benchmarkLaunchAllowed: false,
    modelPromotionAllowed: false,
    paymentSpendAllowed: false,
    providerMutationAllowed: false,
    pylonDispatchAllowed: false,
    reportPublicationAllowed: false,
    runtimePromotionAllowed: false,
    trainingLaunchAllowed: false,
  }

export const ARTANIS_CONTINUAL_LEARNING_TEMPLATE_KINDS:
  ReadonlyArray<ArtanisContinualLearningTemplateKind> = [
    'benchmark_eval_rerun',
    'dspy_gepa_optimization',
    'dataset_curation',
    'adapter_validation',
    'executor_trace_replay',
    'lora_finetuning_training',
    'regression_analysis',
  ]

const templateToMarketplaceKind:
  Readonly<Record<ArtanisContinualLearningTemplateKind, PylonMarketplaceJobKind>> =
  {
    adapter_validation: 'validation',
    benchmark_eval_rerun: 'benchmark_evaluation',
    dataset_curation: 'embedding_data_prep',
    dspy_gepa_optimization: 'gepa_dspy_optimization',
    executor_trace_replay: 'validation',
    lora_finetuning_training: 'lora_finetuning',
    regression_analysis: 'artifact_review',
  }

const highRiskKinds = new Set<ArtanisContinualLearningTemplateKind>([
  'adapter_validation',
  'lora_finetuning_training',
])

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|customer[_-]?(email|name|phone|prompt|record|value)|dataset[._-]?(raw|private|secret|payload)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[._-]?(artifact|raw|secret|weights)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|model|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|benchmark|customer|dataset|email|fixture|input|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(authority\.operator|evidence\.private|operator\.|private\.|provider\.private|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new ArtanisContinualLearningTemplateUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ArtanisContinualLearningTemplateUnsafe({
      reason: `${label} contains raw prompts, raw datasets, raw weights, private repo/customer data, provider payloads, runner logs, wallet/payment material, secrets, or raw timestamps.`,
    })
  }
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: ArtanisContinualLearningAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  if (audience === 'operator') {
    return uniqueRefs(refs)
  }

  return uniqueRefs(refs).filter(ref => !publicUnsafeRefPattern.test(ref))
}

const assertNoExecutionAuthority = (
  authority: ArtanisContinualLearningAuthority,
): void => {
  if (
    authority.adapterInstallAllowed ||
    authority.benchmarkLaunchAllowed ||
    authority.modelPromotionAllowed ||
    authority.paymentSpendAllowed ||
    authority.providerMutationAllowed ||
    authority.pylonDispatchAllowed ||
    authority.reportPublicationAllowed ||
    authority.runtimePromotionAllowed ||
    authority.trainingLaunchAllowed
  ) {
    throw new ArtanisContinualLearningTemplateUnsafe({
      reason:
        'Continual-learning templates do not grant benchmark, training, provider, Pylon dispatch, payment, report publication, model promotion, or runtime promotion authority.',
    })
  }
}

const assertRequiredRefs = (
  template: ArtanisContinualLearningTemplateRecord,
): void => {
  const missing = [
    ['benchmark target', template.benchmarkTargetRefs],
    ['acceptance criteria', template.acceptanceCriteriaRefs],
    ['dispatch payload schema', template.dispatchPayloadSchemaRefs],
    ['evidence', template.evidenceRefs],
    ['required capability', template.requiredCapabilityRefs],
    ['cost caveat', template.costCaveatRefs],
    ['rollback posture', template.rollbackPostureRefs],
    ['approval requirement', template.approvalRequirementRefs],
    ['spend limit', template.spendLimitRefs],
    ['workload', template.workloadRefs],
  ].find(([, refs]) => !hasAny(refs as ReadonlyArray<string>))

  if (missing !== undefined) {
    throw new ArtanisContinualLearningTemplateUnsafe({
      reason: `Continual-learning templates require ${missing[0]} refs.`,
    })
  }
}

const assertTemplate = (
  template: ArtanisContinualLearningTemplateRecord,
): void => {
  assertValidIso('template.createdAtIso', template.createdAtIso)
  assertValidIso('template.updatedAtIso', template.updatedAtIso)
  assertSafeRefs('template identity refs', [template.templateRef])
  assertSafeRefs('acceptance criteria refs', template.acceptanceCriteriaRefs)
  assertSafeRefs('approval requirement refs', template.approvalRequirementRefs)
  assertSafeRefs('benchmark Cloud refs', template.benchmarkCloudRefs)
  assertSafeRefs('benchmark target refs', template.benchmarkTargetRefs)
  assertSafeRefs('blocker refs', template.blockerRefs)
  assertSafeRefs('cost caveat refs', template.costCaveatRefs)
  assertSafeRefs(
    'dispatch payload schema refs',
    template.dispatchPayloadSchemaRefs,
  )
  assertSafeRefs(
    'downstream executor authority refs',
    template.downstreamExecutorAuthorityRefs,
  )
  assertSafeRefs('evidence refs', template.evidenceRefs)
  assertSafeRefs('Forum summary refs', template.forumSummaryRefs)
  assertSafeRefs('marketplace intake refs', template.marketplaceIntakeRefs)
  assertSafeRefs('model artifact refs', template.modelArtifactRefs)
  assertSafeRefs('Model Lab evidence refs', template.modelLabEvidenceRefs)
  assertSafeRefs('operator approval refs', template.operatorApprovalRefs)
  assertSafeRefs('operator detail refs', template.operatorDetailRefs)
  assertSafeRefs('promotion decision refs', template.promotionDecisionRefs)
  assertSafeRefs('public report refs', template.publicReportRefs)
  assertSafeRefs('required capability refs', template.requiredCapabilityRefs)
  assertSafeRefs('retained failure refs', template.retainedFailureRefs)
  assertSafeRefs('rollback posture refs', template.rollbackPostureRefs)
  assertSafeRefs('source refs', template.sourceRefs)
  assertSafeRefs('spend limit refs', template.spendLimitRefs)
  assertSafeRefs('training run refs', template.trainingRunRefs)
  assertSafeRefs('workload refs', template.workloadRefs)
  assertRequiredRefs(template)

  if (
    (template.state === 'blocked' || template.state === 'rejected') &&
    !hasAny(template.blockerRefs)
  ) {
    throw new ArtanisContinualLearningTemplateUnsafe({
      reason: 'Blocked or rejected continual-learning templates require blocker refs.',
    })
  }

  if (
    highRiskKinds.has(template.kind) &&
    (template.state === 'running' || template.state === 'accepted') &&
    (!hasAny(template.operatorApprovalRefs) ||
      !hasAny(template.downstreamExecutorAuthorityRefs))
  ) {
    throw new ArtanisContinualLearningTemplateUnsafe({
      reason:
        'Training, fine-tuning, adapter, provider, and promotion-sensitive templates cannot run or be accepted without operator approval refs and downstream executor authority refs.',
    })
  }
}

const assertLedger = (
  ledger: ArtanisContinualLearningTemplateLedgerRecord,
): void => {
  assertValidIso('ledger.createdAtIso', ledger.createdAtIso)
  assertValidIso('ledger.updatedAtIso', ledger.updatedAtIso)
  assertSafeRefs('ledger refs', [ledger.agentId, ledger.ledgerRef])
  assertSafeRefs('ledger caveat refs', ledger.caveatRefs)
  assertSafeRefs('ledger source refs', ledger.sourceRefs)
  assertNoExecutionAuthority(ledger.authority)

  if (ledger.agentId !== 'agent_artanis') {
    throw new ArtanisContinualLearningTemplateUnsafe({
      reason: 'Continual-learning template ledgers must be administered by agent_artanis.',
    })
  }

  if (!hasAny(ledger.templates)) {
    throw new ArtanisContinualLearningTemplateUnsafe({
      reason: 'Continual-learning template ledgers require at least one template.',
    })
  }

  ledger.templates.forEach(assertTemplate)
}

const operatorReadyProposal = (
  template: ArtanisContinualLearningTemplateRecord,
): boolean =>
  template.state === 'proposed' &&
  hasAny(template.acceptanceCriteriaRefs) &&
  hasAny(template.approvalRequirementRefs) &&
  hasAny(template.benchmarkTargetRefs) &&
  hasAny(template.costCaveatRefs) &&
  hasAny(template.evidenceRefs) &&
  hasAny(template.rollbackPostureRefs) &&
  !hasAny(template.blockerRefs)

const templateExecutionAllowed = (
  template: ArtanisContinualLearningTemplateRecord,
): boolean =>
  template.state === 'running' &&
  hasAny(template.operatorApprovalRefs) &&
  hasAny(template.downstreamExecutorAuthorityRefs)

const projectTemplate = (
  template: ArtanisContinualLearningTemplateRecord,
  audience: ArtanisContinualLearningAudience,
  nowIso: string,
): ArtanisContinualLearningTemplateProjection =>
  new ArtanisContinualLearningTemplateProjection({
    acceptanceCriteriaRefs: refsForAudience(
      'acceptance criteria refs',
      template.acceptanceCriteriaRefs,
      audience,
    ),
    approvalRequirementRefs: refsForAudience(
      'approval requirement refs',
      template.approvalRequirementRefs,
      audience,
    ),
    benchmarkCloudRefs: refsForAudience(
      'benchmark Cloud refs',
      template.benchmarkCloudRefs,
      audience,
    ),
    benchmarkTargetRefs: refsForAudience(
      'benchmark target refs',
      template.benchmarkTargetRefs,
      audience,
    ),
    blockerRefs: refsForAudience('blocker refs', template.blockerRefs, audience),
    costCaveatRefs: refsForAudience(
      'cost caveat refs',
      template.costCaveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      template.createdAtIso,
      nowIso,
    ),
    dispatchPayloadSchemaRefs: refsForAudience(
      'dispatch payload schema refs',
      template.dispatchPayloadSchemaRefs,
      audience,
    ),
    downstreamExecutorAuthorityRefs: refsForAudience(
      'downstream executor authority refs',
      template.downstreamExecutorAuthorityRefs,
      audience,
    ),
    evidenceRefs: refsForAudience('evidence refs', template.evidenceRefs, audience),
    forumSummaryRefs: refsForAudience(
      'Forum summary refs',
      template.forumSummaryRefs,
      audience,
    ),
    forumSummaryState: template.state,
    kind: template.kind,
    marketplaceIntakeRefs: refsForAudience(
      'marketplace intake refs',
      template.marketplaceIntakeRefs,
      audience,
    ),
    modelArtifactRefs: refsForAudience(
      'model artifact refs',
      template.modelArtifactRefs,
      audience,
    ),
    modelLabEvidenceRefs: refsForAudience(
      'Model Lab evidence refs',
      template.modelLabEvidenceRefs,
      audience,
    ),
    operatorApprovalRefs: refsForAudience(
      'operator approval refs',
      template.operatorApprovalRefs,
      audience,
    ),
    operatorDetailRefs: refsForAudience(
      'operator detail refs',
      template.operatorDetailRefs,
      audience,
    ),
    operatorReadyProposal: operatorReadyProposal(template),
    promotionDecisionRefs: refsForAudience(
      'promotion decision refs',
      template.promotionDecisionRefs,
      audience,
    ),
    publicReportRefs: refsForAudience(
      'public report refs',
      template.publicReportRefs,
      audience,
    ),
    requiredCapabilityRefs: refsForAudience(
      'required capability refs',
      template.requiredCapabilityRefs,
      audience,
    ),
    retainedFailureRefs: refsForAudience(
      'retained failure refs',
      template.retainedFailureRefs,
      audience,
    ),
    riskLabel: template.riskLabel,
    rollbackPostureRefs: refsForAudience(
      'rollback posture refs',
      template.rollbackPostureRefs,
      audience,
    ),
    sourceRefs: refsForAudience('source refs', template.sourceRefs, audience),
    spendLabel: template.spendLabel,
    spendLimitRefs: refsForAudience(
      'spend limit refs',
      template.spendLimitRefs,
      audience,
    ),
    state: template.state,
    templateExecutionAllowed: templateExecutionAllowed(template),
    templateRef: refsForAudience(
      'template ref',
      [template.templateRef],
      audience,
    )[0] ?? 'template.redacted',
    trainingRunRefs: refsForAudience(
      'training run refs',
      template.trainingRunRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      template.updatedAtIso,
      nowIso,
    ),
    workloadRefs: refsForAudience(
      'workload refs',
      template.workloadRefs,
      audience,
    ),
  })

export const artanisContinualLearningTemplateProjectionHasPrivateMaterial = (
  projection: ArtanisContinualLearningTemplateLedgerProjection,
): boolean => {
  const values = stringValues(projection)

  return values.some(value =>
    containsProviderSecretMaterial(value) ||
    unsafeRefPattern.test(value) ||
    rawTimestampPattern.test(value) ||
    (
      projection.audience !== 'operator' &&
      publicUnsafeRefPattern.test(value)
    )
  )
}

const stringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(stringValues)
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value as Record<string, unknown>).flatMap(stringValues)
  }

  return []
}

export const projectArtanisContinualLearningTemplates = (
  ledger: ArtanisContinualLearningTemplateLedgerRecord,
  audience: ArtanisContinualLearningAudience,
  nowIso: string,
): ArtanisContinualLearningTemplateLedgerProjection => {
  assertLedger(ledger)

  const templates = ledger.templates.map(template =>
    projectTemplate(template, audience, nowIso)
  )
  const projection = new ArtanisContinualLearningTemplateLedgerProjection({
    acceptedCount: templates.filter(template => template.state === 'accepted')
      .length,
    agentId: ledger.agentId,
    audience,
    authority: ledger.authority,
    blockedCount: templates.filter(template => template.state === 'blocked')
      .length,
    caveatRefs: refsForAudience('ledger caveat refs', ledger.caveatRefs, audience),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      ledger.createdAtIso,
      nowIso,
    ),
    ledgerRef: refsForAudience('ledger ref', [ledger.ledgerRef], audience)[0] ??
      'ledger.redacted',
    operatorReadyProposalCount:
      templates.filter(template => template.operatorReadyProposal).length,
    proposedCount: templates.filter(template => template.state === 'proposed')
      .length,
    rejectedCount: templates.filter(template => template.state === 'rejected')
      .length,
    runningCount: templates.filter(template => template.state === 'running')
      .length,
    sourceRefs: refsForAudience('ledger source refs', ledger.sourceRefs, audience),
    templateCount: templates.length,
    templates,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      ledger.updatedAtIso,
      nowIso,
    ),
  })

  if (artanisContinualLearningTemplateProjectionHasPrivateMaterial(projection)) {
    throw new ArtanisContinualLearningTemplateUnsafe({
      reason:
        'Continual-learning template projection contains private material.',
    })
  }

  return projection
}

export const pylonMarketplaceIntakeRequestFromTemplate = (
  template: ArtanisContinualLearningTemplateRecord,
  resourceModePreference: PylonResourceMode,
): PylonMarketplaceCreateJobIntakeRequest => {
  assertTemplate(template)

  return new PylonMarketplaceCreateJobIntakeRequest({
    benchmarkRefs: template.benchmarkTargetRefs,
    budgetRefs: [`budget.public.continual_learning.${template.spendLabel}`],
    caveatRefs: [
      ...template.costCaveatRefs,
      'caveat.public.continual_learning_template_not_execution',
    ],
    dataRefs: uniqueRefs([...template.modelLabEvidenceRefs, ...template.workloadRefs]),
    eligibilityRequirementRefs: [
      'eligibility.public.pylon_provider_registered',
      'eligibility.public.model_lab_evidence_available',
      ...template.requiredCapabilityRefs.map(
        ref => `eligibility.public.${ref.replaceAll(/[^A-Za-z0-9]+/g, '_')}`,
      ),
    ],
    evidenceExpectationRefs: template.evidenceRefs,
    jobKind: templateToMarketplaceKind[template.kind],
    modelRefs: template.modelArtifactRefs,
    requesterRef: 'requester.public.openagents.artanis',
    resourceModePreference,
    resourceRequirementRefs: [`resource.public.pylon.${resourceModePreference}`],
    resultExpectationRefs: template.acceptanceCriteriaRefs,
    source: 'openagents_seeded',
    sourceRefs: template.sourceRefs,
    spendCaveatRefs: uniqueRefs([...template.costCaveatRefs, ...template.spendLimitRefs]),
  })
}

export const pylonMarketplaceTriageRequestFromTemplate = (
  template: ArtanisContinualLearningTemplateRecord,
  resourceMode: PylonResourceMode,
): PylonMarketplaceTriageJobIntakeRequest => {
  assertTemplate(template)

  return new PylonMarketplaceTriageJobIntakeRequest({
    assignment: {
      acceptanceCriteriaRefs: template.acceptanceCriteriaRefs,
      assignmentAuthorityRefs: template.approvalRequirementRefs,
      caveatRefs: [
        ...template.costCaveatRefs,
        'caveat.public.continual_learning_assignment_not_dispatch',
      ],
      payoutCaveatRefs: [
        'caveat.public.no_payout_before_model_lab_acceptance_receipts',
      ],
      providerEligibilityRefs: [
        'eligibility.public.provider.capability_snapshot_ok',
        'eligibility.public.model_lab_artifact_policy_ok',
        ...template.requiredCapabilityRefs.map(
          ref => `eligibility.public.${ref.replaceAll(/[^A-Za-z0-9]+/g, '_')}`,
        ),
      ],
      providerRefs: ['provider.public.pylon_eligible_pool'],
      resourceMode,
    },
    outcome: 'proposed_assignment',
  })
}

const template = (
  input: Readonly<{
    benchmarkTargetRefs: ReadonlyArray<string>
    kind: ArtanisContinualLearningTemplateKind
    riskLabel: ArtanisContinualLearningRiskLabel
    spendLabel: ArtanisContinualLearningCostLabel
    state?: ArtanisContinualLearningTemplateState | undefined
    suffix: string
  }>,
): ArtanisContinualLearningTemplateRecord =>
  new ArtanisContinualLearningTemplateRecord({
    acceptanceCriteriaRefs: [
      `acceptance.public.continual_learning.${input.suffix}`,
    ],
    approvalRequirementRefs: [
      'approval.public.artanis.operator_required',
      `approval.public.artanis.executor_required.${input.suffix}`,
    ],
    benchmarkCloudRefs: ['benchmark_cloud.public.autopilot_coding_suite'],
    benchmarkTargetRefs: input.benchmarkTargetRefs,
    blockerRefs: [],
    costCaveatRefs: [`cost.public.continual_learning.${input.spendLabel}`],
    createdAtIso: '2026-06-07T06:00:00.000Z',
    dispatchPayloadSchemaRefs: [
      `payload_schema.public.continual_learning.${input.suffix}`,
    ],
    downstreamExecutorAuthorityRefs: [],
    evidenceRefs: [`evidence.public.continual_learning.${input.suffix}`],
    forumSummaryRefs: [`forum.public.artanis.continual_learning.${input.suffix}`],
    kind: input.kind,
    marketplaceIntakeRefs: [
      `intake.public.pylon_marketplace.continual_learning.${input.suffix}`,
    ],
    modelArtifactRefs: ['artifact.public.model_lab.autopilot_candidate'],
    modelLabEvidenceRefs: ['model_lab.public.evidence_graph.autopilot'],
    operatorApprovalRefs: [],
    operatorDetailRefs: [`operator.artanis.continual_learning.${input.suffix}`],
    promotionDecisionRefs: ['promotion.public.model_lab.autopilot_review'],
    publicReportRefs: ['report.public.model_lab_autopilot_v2'],
    requiredCapabilityRefs: [`capability.public.continual_learning.${input.suffix}`],
    retainedFailureRefs: ['retained_failure.public.autopilot_codegen'],
    riskLabel: input.riskLabel,
    rollbackPostureRefs: ['rollback.public.model_lab.autopilot_candidate'],
    sourceRefs: [
      'docs/artanis/2026-06-06-model-lab-context-bridge.md',
      'docs/artanis/2026-06-06-pylon-marketplace-job-contract.md',
    ],
    spendLabel: input.spendLabel,
    spendLimitRefs: [`spend_limit.public.continual_learning.${input.spendLabel}`],
    state: input.state ?? 'proposed',
    templateRef: `template.public.artanis.continual_learning.${input.suffix}`,
    trainingRunRefs: ['training_run.public.model_lab.autopilot_candidate'],
    updatedAtIso: '2026-06-07T06:05:00.000Z',
    workloadRefs: [`workload.public.continual_learning.${input.suffix}`],
  })

export const artanisExecutorTraceReplayTemplate =
  (): ArtanisContinualLearningTemplateRecord =>
    new ArtanisContinualLearningTemplateRecord({
      acceptanceCriteriaRefs: [
        'acceptance.public.tassadar_executor_trace.digest_match',
        'acceptance.public.tassadar_executor_trace.separate_replay_verdict',
      ],
      approvalRequirementRefs: [
        'approval.public.artanis.operator_required',
        'approval.public.artanis.tassadar_executor_paid_sample',
      ],
      benchmarkCloudRefs: ['benchmark_cloud.public.tassadar_executor_trace'],
      benchmarkTargetRefs: ['benchmark.public.tassadar.executor_trace_replay'],
      blockerRefs: [],
      costCaveatRefs: ['cost.public.tassadar_executor_trace.no_spend_default'],
      createdAtIso: '2026-06-10T16:30:00.000Z',
      dispatchPayloadSchemaRefs: [
        'openagents.tassadar_executor_trace_request.v1',
        'openagents.tassadar_executor_trace_output.v1',
      ],
      downstreamExecutorAuthorityRefs: [],
      evidenceRefs: [
        'evidence.public.tassadar_executor_trace.green_poc',
        'promise.public.compute.tassadar_executor_poc.v1',
      ],
      forumSummaryRefs: [
        'forum.public.artanis.continual_learning.executor_trace_replay',
      ],
      kind: 'executor_trace_replay',
      marketplaceIntakeRefs: [
        'intake.public.pylon_marketplace.continual_learning.executor_trace_replay',
      ],
      modelArtifactRefs: ['artifact.public.tassadar_executor_trace.fixture'],
      modelLabEvidenceRefs: [
        'model_lab.public.evidence_graph.tassadar_executor_trace',
      ],
      operatorApprovalRefs: [],
      operatorDetailRefs: [
        'operator.artanis.continual_learning.executor_trace_replay',
      ],
      promotionDecisionRefs: [
        'promotion.public.tassadar_executor_trace.no_runtime_promotion',
      ],
      publicReportRefs: [
        'report.public.product_promises.compute_tassadar_executor_poc',
      ],
      requiredCapabilityRefs: [TASSADAR_EXECUTOR_CAPABILITY_REF],
      retainedFailureRefs: [
        'retained_failure.public.tassadar_executor_trace.digest_mismatch',
      ],
      riskLabel: 'low',
      rollbackPostureRefs: [
        'rollback.public.tassadar_executor_trace.cancel_assignment',
      ],
      sourceRefs: [
        'docs/artanis/2026-06-10-executor-trace-loop-candidate.md',
        'docs/2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md',
        'apps/openagents.com/workers/api/scripts/tassadar-poc-dispatch.ts',
      ],
      spendLabel: 'low',
      spendLimitRefs: [
        'spend_limit.public.tassadar_executor_trace.zero_sats_default',
      ],
      state: 'proposed',
      templateRef:
        'template.public.artanis.continual_learning.executor_trace_replay',
      trainingRunRefs: ['training_run.public.tassadar_executor_trace.poc'],
      updatedAtIso: '2026-06-10T16:35:00.000Z',
      workloadRefs: ['workload.public.tassadar_poc.loop_sum_fixture'],
    })

export const exampleArtanisContinualLearningTemplateLedger = ():
  ArtanisContinualLearningTemplateLedgerRecord =>
  new ArtanisContinualLearningTemplateLedgerRecord({
    agentId: 'agent_artanis',
    authority: ARTANIS_CONTINUAL_LEARNING_NO_EXECUTION_AUTHORITY,
    caveatRefs: [
      'caveat.public.continual_learning_templates_are_proposals',
      'caveat.public.training_requires_operator_and_executor_authority',
    ],
    createdAtIso: '2026-06-07T06:00:00.000Z',
    ledgerRef: 'ledger.public.artanis.continual_learning_templates',
    sourceRefs: [
      'docs/artanis/2026-06-06-model-lab-context-bridge.md',
      'docs/omni/2026-06-06-model-lab-public-report-projection.md',
      'docs/omni/2026-06-06-benchmark-cloud-evidence-contract.md',
    ],
    templates: [
      template({
        benchmarkTargetRefs: ['benchmark.public.autopilot.eval_rerun'],
        kind: 'benchmark_eval_rerun',
        riskLabel: 'medium',
        spendLabel: 'low',
        suffix: 'benchmark_eval_rerun',
      }),
      template({
        benchmarkTargetRefs: ['benchmark.public.autopilot.gepa_dspy'],
        kind: 'dspy_gepa_optimization',
        riskLabel: 'medium',
        spendLabel: 'medium',
        suffix: 'dspy_gepa_optimization',
      }),
      template({
        benchmarkTargetRefs: ['benchmark.public.autopilot.dataset_quality'],
        kind: 'dataset_curation',
        riskLabel: 'low',
        spendLabel: 'low',
        suffix: 'dataset_curation',
      }),
      template({
        benchmarkTargetRefs: ['benchmark.public.autopilot.adapter_validation'],
        kind: 'adapter_validation',
        riskLabel: 'high',
        spendLabel: 'medium',
        suffix: 'adapter_validation',
      }),
      artanisExecutorTraceReplayTemplate(),
      template({
        benchmarkTargetRefs: ['benchmark.public.autopilot.lora_training'],
        kind: 'lora_finetuning_training',
        riskLabel: 'high',
        spendLabel: 'high',
        suffix: 'lora_finetuning_training',
      }),
      template({
        benchmarkTargetRefs: ['benchmark.public.autopilot.regression_analysis'],
        kind: 'regression_analysis',
        riskLabel: 'medium',
        spendLabel: 'low',
        suffix: 'regression_analysis',
      }),
    ],
    updatedAtIso: '2026-06-07T06:05:00.000Z',
  })
