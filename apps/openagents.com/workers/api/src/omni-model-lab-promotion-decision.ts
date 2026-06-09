import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const OmniPromotionDecisionAudience = S.Literals([
  'public',
  'agent',
  'customer',
  'team',
  'operator',
])
export type OmniPromotionDecisionAudience =
  typeof OmniPromotionDecisionAudience.Type

export const OmniPromotionDecisionState = S.Literals([
  'blocked',
  'failed',
  'passed',
  'superseded',
])
export type OmniPromotionDecisionState =
  typeof OmniPromotionDecisionState.Type

export const OmniPromotionDecisionTargetKind = S.Literals([
  'adapter',
  'artifact',
  'candidate',
  'route',
  'training_run',
])
export type OmniPromotionDecisionTargetKind =
  typeof OmniPromotionDecisionTargetKind.Type

export const OmniPromotionRiskLabel = S.Literals([
  'critical',
  'high',
  'medium',
  'low',
])
export type OmniPromotionRiskLabel = typeof OmniPromotionRiskLabel.Type

export const OmniPromotionRollbackPosture = S.Literals([
  'candidate',
  'missing',
  'ready',
  'verified',
])
export type OmniPromotionRollbackPosture =
  typeof OmniPromotionRollbackPosture.Type

export const OmniPromotionClaimState = S.Literals([
  'blocked',
  'failed_reviewed',
  'passed_not_deployed',
  'superseded',
])
export type OmniPromotionClaimState = typeof OmniPromotionClaimState.Type

export const OmniPromotionDecisionAuthorityBoundary = S.Literals([
  'read_only_model_lab_promotion_decision',
])
export type OmniPromotionDecisionAuthorityBoundary =
  typeof OmniPromotionDecisionAuthorityBoundary.Type

export class OmniPromotionDecisionAuthority extends S.Class<OmniPromotionDecisionAuthority>(
  'OmniPromotionDecisionAuthority',
)({
  authorityBoundary: OmniPromotionDecisionAuthorityBoundary,
  noAdapterInstall: S.Boolean,
  noMarketplaceRankMutation: S.Boolean,
  noModelDeployment: S.Boolean,
  noPaymentSpend: S.Boolean,
  noPayoutMutation: S.Boolean,
  noProviderMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noRollbackExecution: S.Boolean,
  noRouteMutation: S.Boolean,
  noRuntimePromotion: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class OmniPromotionDecisionRecord extends S.Class<OmniPromotionDecisionRecord>(
  'OmniPromotionDecisionRecord',
)({
  adapterRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  benchmarkEvidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  candidateRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  decisionRef: S.String,
  evalEvidenceRefs: S.Array(S.String),
  marketplaceMemoryRefs: S.Array(S.String),
  outcomeAttributionRefs: S.Array(S.String),
  releaseGateRefs: S.Array(S.String),
  reviewerReceiptRefs: S.Array(S.String),
  riskLabels: S.Array(OmniPromotionRiskLabel),
  rollbackPosture: OmniPromotionRollbackPosture,
  rollbackRefs: S.Array(S.String),
  routeRefs: S.Array(S.String),
  state: OmniPromotionDecisionState,
  supersededByRefs: S.Array(S.String),
  supersedesRefs: S.Array(S.String),
  targetKind: OmniPromotionDecisionTargetKind,
  trainingRunRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class OmniPromotionDecisionProjectionRecord extends S.Class<OmniPromotionDecisionProjectionRecord>(
  'OmniPromotionDecisionProjectionRecord',
)({
  adapterRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  benchmarkEvidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  candidateRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  decisionRef: S.String,
  evalEvidenceRefs: S.Array(S.String),
  marketplaceMemoryRefs: S.Array(S.String),
  outcomeAttributionRefs: S.Array(S.String),
  releaseGateRefs: S.Array(S.String),
  reviewerReceiptRefs: S.Array(S.String),
  riskLabels: S.Array(OmniPromotionRiskLabel),
  rollbackPosture: OmniPromotionRollbackPosture,
  rollbackRefs: S.Array(S.String),
  routeRefs: S.Array(S.String),
  state: OmniPromotionDecisionState,
  supersededByRefs: S.Array(S.String),
  supersedesRefs: S.Array(S.String),
  targetKind: OmniPromotionDecisionTargetKind,
  trainingRunRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class OmniPromotionDecisionLedgerRecord extends S.Class<OmniPromotionDecisionLedgerRecord>(
  'OmniPromotionDecisionLedgerRecord',
)({
  adapterRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  authority: OmniPromotionDecisionAuthority,
  benchmarkEvidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  candidateRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  decisions: S.Array(OmniPromotionDecisionRecord),
  evalEvidenceRefs: S.Array(S.String),
  id: S.String,
  ledgerRef: S.String,
  marketplaceMemoryRefs: S.Array(S.String),
  outcomeAttributionRefs: S.Array(S.String),
  releaseGateRefs: S.Array(S.String),
  routeRefs: S.Array(S.String),
  trainingRunRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class OmniPromotionDecisionProjection extends S.Class<OmniPromotionDecisionProjection>(
  'OmniPromotionDecisionProjection',
)({
  adapterInstallAllowed: S.Boolean,
  adapterRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  audience: OmniPromotionDecisionAudience,
  authority: OmniPromotionDecisionAuthority,
  benchmarkEvidenceRefs: S.Array(S.String),
  blockedCount: S.Number,
  blockerRefs: S.Array(S.String),
  candidateRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  claimState: OmniPromotionClaimState,
  createdAtDisplay: S.String,
  decisionCount: S.Number,
  decisions: S.Array(OmniPromotionDecisionProjectionRecord),
  evalEvidenceRefs: S.Array(S.String),
  failedCount: S.Number,
  id: S.String,
  ledgerRef: S.String,
  marketplaceMemoryRefs: S.Array(S.String),
  marketplaceRankMutationAllowed: S.Boolean,
  modelDeploymentAllowed: S.Boolean,
  outcomeAttributionRefs: S.Array(S.String),
  passedCount: S.Number,
  paymentSpendAllowed: S.Boolean,
  payoutMutationAllowed: S.Boolean,
  providerMutationAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  releaseGateRefs: S.Array(S.String),
  rollbackExecutionAllowed: S.Boolean,
  routeMutationAllowed: S.Boolean,
  routeRefs: S.Array(S.String),
  runtimePromotionAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  supersededCount: S.Number,
  trainingRunRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class OmniPromotionDecisionUnsafe extends S.TaggedErrorClass<OmniPromotionDecisionUnsafe>()(
  'OmniPromotionDecisionUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_PROMOTION_DECISION_READ_ONLY_AUTHORITY:
  OmniPromotionDecisionAuthority = {
    authorityBoundary: 'read_only_model_lab_promotion_decision',
    noAdapterInstall: true,
    noMarketplaceRankMutation: true,
    noModelDeployment: true,
    noPaymentSpend: true,
    noPayoutMutation: true,
    noProviderMutation: true,
    noPublicClaimUpgrade: true,
    noRollbackExecution: true,
    noRouteMutation: true,
    noRuntimePromotion: true,
    noSettlementMutation: true,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafePromotionRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|benchmark|customer|dataset|email|fixture|input|invoice|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(adapter\.private|artifact\.private|benchmark\.private|blocker\.private|candidate\.private|caveat\.private|decision\.private|eval\.private|evidence\.private|gate\.private|ledger\.private|marketplace\.private|outcome\.private|promotion\.private|receipt\.private|reviewer\.private|rollback\.private|route\.private|source\.|training\.private)/i
const agentUnsafeRefPattern =
  /(adapter\.private|artifact\.private|benchmark\.private|blocker\.private|candidate\.private|decision\.private|eval\.private|gate\.private|ledger\.private|marketplace\.private|outcome\.private|receipt\.private|reviewer\.private|rollback\.private|route\.private|source\.private|training\.private)/i
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
      unsafePromotionRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniPromotionDecisionUnsafe({
      reason: `${label} contains private prompts, source archives, datasets, provider payloads, model weights, secrets, payment/wallet material, private repos, raw logs, raw traces, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniPromotionDecisionAudience,
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
  audience: OmniPromotionDecisionAudience,
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
  audience: OmniPromotionDecisionAudience,
  redactedRef: string,
): string => refsForAudience(label, [ref], audience)[0] ?? redactedRef

const assertReadOnlyAuthority = (
  authority: OmniPromotionDecisionAuthority,
): void => {
  if (
    authority.noAdapterInstall !== true ||
    authority.noMarketplaceRankMutation !== true ||
    authority.noModelDeployment !== true ||
    authority.noPaymentSpend !== true ||
    authority.noPayoutMutation !== true ||
    authority.noProviderMutation !== true ||
    authority.noPublicClaimUpgrade !== true ||
    authority.noRollbackExecution !== true ||
    authority.noRouteMutation !== true ||
    authority.noRuntimePromotion !== true ||
    authority.noSettlementMutation !== true
  ) {
    throw new OmniPromotionDecisionUnsafe({
      reason:
        'Promotion decision ledgers are read-only evidence and cannot promote runtime behavior, deploy models, install adapters, mutate routes, execute rollback, mutate provider state, spend money, mutate marketplace rank, pay out, settle, or upgrade public claims.',
    })
  }
}

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new OmniPromotionDecisionUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
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
    throw new OmniPromotionDecisionUnsafe({
      reason: `${label} must reference records in the same promotion ledger.`,
    })
  }
}

const targetRefsForDecision = (
  decision: OmniPromotionDecisionRecord,
): ReadonlyArray<string> => {
  if (decision.targetKind === 'adapter') {
    return decision.adapterRefs
  }

  if (decision.targetKind === 'artifact') {
    return decision.artifactRefs
  }

  if (decision.targetKind === 'candidate') {
    return decision.candidateRefs
  }

  if (decision.targetKind === 'route') {
    return decision.routeRefs
  }

  return decision.trainingRunRefs
}

const assertDecision = (
  decision: OmniPromotionDecisionRecord,
  ledger: OmniPromotionDecisionLedgerRecord,
): void => {
  assertValidIso('decision.createdAtIso', decision.createdAtIso)
  assertValidIso('decision.updatedAtIso', decision.updatedAtIso)
  assertSafeRefs('Promotion decision ref', [decision.decisionRef])
  assertSafeRefs('Promotion decision adapter refs', decision.adapterRefs)
  assertSafeRefs('Promotion decision artifact refs', decision.artifactRefs)
  assertSafeRefs(
    'Promotion decision benchmark evidence refs',
    decision.benchmarkEvidenceRefs,
  )
  assertSafeRefs('Promotion decision blocker refs', decision.blockerRefs)
  assertSafeRefs('Promotion decision candidate refs', decision.candidateRefs)
  assertSafeRefs('Promotion decision caveat refs', decision.caveatRefs)
  assertSafeRefs(
    'Promotion decision eval evidence refs',
    decision.evalEvidenceRefs,
  )
  assertSafeRefs(
    'Promotion decision marketplace memory refs',
    decision.marketplaceMemoryRefs,
  )
  assertSafeRefs(
    'Promotion decision outcome attribution refs',
    decision.outcomeAttributionRefs,
  )
  assertSafeRefs(
    'Promotion decision release gate refs',
    decision.releaseGateRefs,
  )
  assertSafeRefs(
    'Promotion decision reviewer receipt refs',
    decision.reviewerReceiptRefs,
  )
  assertSafeRefs('Promotion decision rollback refs', decision.rollbackRefs)
  assertSafeRefs('Promotion decision route refs', decision.routeRefs)
  assertSafeRefs(
    'Promotion decision superseded-by refs',
    decision.supersededByRefs,
  )
  assertSafeRefs(
    'Promotion decision supersedes refs',
    decision.supersedesRefs,
  )
  assertSafeRefs(
    'Promotion decision training run refs',
    decision.trainingRunRefs,
  )
  assertNoMissingRefs(
    'Promotion decision release gate refs',
    decision.releaseGateRefs,
    refSet(ledger.releaseGateRefs),
  )
  assertNoMissingRefs(
    'Promotion decision benchmark evidence refs',
    decision.benchmarkEvidenceRefs,
    refSet(ledger.benchmarkEvidenceRefs),
  )
  assertNoMissingRefs(
    'Promotion decision eval evidence refs',
    decision.evalEvidenceRefs,
    refSet(ledger.evalEvidenceRefs),
  )
  assertNoMissingRefs(
    'Promotion decision marketplace memory refs',
    decision.marketplaceMemoryRefs,
    refSet(ledger.marketplaceMemoryRefs),
  )
  assertNoMissingRefs(
    'Promotion decision attribution refs',
    decision.outcomeAttributionRefs,
    refSet(ledger.outcomeAttributionRefs),
  )

  if (!hasAny(targetRefsForDecision(decision))) {
    throw new OmniPromotionDecisionUnsafe({
      reason: 'Promotion decisions require target refs matching target kind.',
    })
  }

  if (!hasAny(decision.releaseGateRefs)) {
    throw new OmniPromotionDecisionUnsafe({
      reason: 'Promotion decisions require release gate refs.',
    })
  }

  if (
    !hasAny(decision.evalEvidenceRefs) &&
    !hasAny(decision.benchmarkEvidenceRefs)
  ) {
    throw new OmniPromotionDecisionUnsafe({
      reason: 'Promotion decisions require eval or benchmark evidence refs.',
    })
  }

  if (!hasAny(decision.riskLabels)) {
    throw new OmniPromotionDecisionUnsafe({
      reason: 'Promotion decisions require risk labels.',
    })
  }

  if (
    (decision.state === 'passed' || decision.state === 'failed') &&
    !hasAny(decision.reviewerReceiptRefs)
  ) {
    throw new OmniPromotionDecisionUnsafe({
      reason: 'Passed and failed promotion decisions require reviewer receipts.',
    })
  }

  if (
    decision.state === 'passed' &&
    (decision.rollbackPosture !== 'ready' &&
      decision.rollbackPosture !== 'verified')
  ) {
    throw new OmniPromotionDecisionUnsafe({
      reason:
        'Passed promotion decisions require ready or verified rollback posture.',
    })
  }

  if (decision.state === 'passed' && !hasAny(decision.rollbackRefs)) {
    throw new OmniPromotionDecisionUnsafe({
      reason: 'Passed promotion decisions require rollback refs.',
    })
  }

  if (
    decision.state === 'passed' &&
    (!hasAny(decision.marketplaceMemoryRefs) ||
      !hasAny(decision.outcomeAttributionRefs))
  ) {
    throw new OmniPromotionDecisionUnsafe({
      reason:
        'Passed promotion decisions require marketplace memory and outcome attribution refs.',
    })
  }

  if (
    decision.state === 'passed' &&
    decision.riskLabels.includes('critical')
  ) {
    throw new OmniPromotionDecisionUnsafe({
      reason: 'Critical-risk promotion decisions cannot pass without a later policy override contract.',
    })
  }

  if (
    (decision.state === 'failed' || decision.state === 'blocked') &&
    !hasAny(decision.blockerRefs)
  ) {
    throw new OmniPromotionDecisionUnsafe({
      reason: 'Failed and blocked promotion decisions require blocker refs.',
    })
  }

  if (decision.state === 'blocked' && !hasAny(decision.caveatRefs)) {
    throw new OmniPromotionDecisionUnsafe({
      reason: 'Blocked promotion decisions require caveat refs.',
    })
  }

  if (decision.state === 'superseded' && !hasAny(decision.supersededByRefs)) {
    throw new OmniPromotionDecisionUnsafe({
      reason: 'Superseded promotion decisions require superseded-by refs.',
    })
  }
}

const assertLedger = (ledger: OmniPromotionDecisionLedgerRecord): void => {
  assertReadOnlyAuthority(ledger.authority)
  assertValidIso('createdAtIso', ledger.createdAtIso)
  assertValidIso('updatedAtIso', ledger.updatedAtIso)
  assertSafeRefs('Promotion ledger id', [ledger.id])
  assertSafeRefs('Promotion ledger ref', [ledger.ledgerRef])
  assertSafeRefs('Promotion ledger adapter refs', ledger.adapterRefs)
  assertSafeRefs('Promotion ledger artifact refs', ledger.artifactRefs)
  assertSafeRefs(
    'Promotion ledger benchmark evidence refs',
    ledger.benchmarkEvidenceRefs,
  )
  assertSafeRefs('Promotion ledger blocker refs', ledger.blockerRefs)
  assertSafeRefs('Promotion ledger candidate refs', ledger.candidateRefs)
  assertSafeRefs('Promotion ledger caveat refs', ledger.caveatRefs)
  assertSafeRefs('Promotion ledger eval evidence refs', ledger.evalEvidenceRefs)
  assertSafeRefs(
    'Promotion ledger marketplace memory refs',
    ledger.marketplaceMemoryRefs,
  )
  assertSafeRefs(
    'Promotion ledger outcome attribution refs',
    ledger.outcomeAttributionRefs,
  )
  assertSafeRefs('Promotion ledger release gate refs', ledger.releaseGateRefs)
  assertSafeRefs('Promotion ledger route refs', ledger.routeRefs)
  assertSafeRefs('Promotion ledger training run refs', ledger.trainingRunRefs)

  if (!hasAny(ledger.decisions)) {
    throw new OmniPromotionDecisionUnsafe({
      reason: 'Promotion ledgers require decisions.',
    })
  }

  if (hasAny(duplicateRefs(ledger.decisions.map(decision => decision.decisionRef)))) {
    throw new OmniPromotionDecisionUnsafe({
      reason: 'Promotion ledgers cannot contain duplicate decision refs.',
    })
  }

  ledger.decisions.forEach(decision => assertDecision(decision, ledger))
}

const claimStateForLedger = (
  ledger: OmniPromotionDecisionLedgerRecord,
): OmniPromotionClaimState => {
  if (ledger.decisions.some(decision => decision.state === 'blocked')) {
    return 'blocked'
  }

  if (ledger.decisions.some(decision => decision.state === 'failed')) {
    return 'failed_reviewed'
  }

  if (ledger.decisions.some(decision => decision.state === 'passed')) {
    return 'passed_not_deployed'
  }

  return 'superseded'
}

const redactDecision = (
  decision: OmniPromotionDecisionRecord,
  audience: OmniPromotionDecisionAudience,
  nowIso: string,
): OmniPromotionDecisionProjectionRecord => ({
  adapterRefs: refsForAudience(
    'Promotion decision adapter refs',
    decision.adapterRefs,
    audience,
  ),
  artifactRefs: refsForAudience(
    'Promotion decision artifact refs',
    decision.artifactRefs,
    audience,
  ),
  benchmarkEvidenceRefs: refsForAudience(
    'Promotion decision benchmark evidence refs',
    decision.benchmarkEvidenceRefs,
    audience,
  ),
  blockerRefs: refsForAudience(
    'Promotion decision blocker refs',
    decision.blockerRefs,
    audience,
  ),
  candidateRefs: refsForAudience(
    'Promotion decision candidate refs',
    decision.candidateRefs,
    audience,
  ),
  caveatRefs: refsForAudience(
    'Promotion decision caveat refs',
    decision.caveatRefs,
    audience,
  ),
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    decision.createdAtIso,
    nowIso,
  ),
  decisionRef: refForAudience(
    'Promotion decision ref',
    decision.decisionRef,
    audience,
    'decision.redacted.promotion',
  ),
  evalEvidenceRefs: refsForAudience(
    'Promotion decision eval evidence refs',
    decision.evalEvidenceRefs,
    audience,
  ),
  marketplaceMemoryRefs: refsForAudience(
    'Promotion decision marketplace memory refs',
    decision.marketplaceMemoryRefs,
    audience,
  ),
  outcomeAttributionRefs: refsForAudience(
    'Promotion decision outcome attribution refs',
    decision.outcomeAttributionRefs,
    audience,
  ),
  releaseGateRefs: refsForAudience(
    'Promotion decision release gate refs',
    decision.releaseGateRefs,
    audience,
  ),
  reviewerReceiptRefs: refsForAudience(
    'Promotion decision reviewer receipt refs',
    decision.reviewerReceiptRefs,
    audience,
  ),
  riskLabels: decision.riskLabels,
  rollbackPosture: decision.rollbackPosture,
  rollbackRefs: refsForAudience(
    'Promotion decision rollback refs',
    decision.rollbackRefs,
    audience,
  ),
  routeRefs: refsForAudience(
    'Promotion decision route refs',
    decision.routeRefs,
    audience,
  ),
  state: decision.state,
  supersededByRefs: refsForAudience(
    'Promotion decision superseded-by refs',
    decision.supersededByRefs,
    audience,
  ),
  supersedesRefs: refsForAudience(
    'Promotion decision supersedes refs',
    decision.supersedesRefs,
    audience,
  ),
  targetKind: decision.targetKind,
  trainingRunRefs: refsForAudience(
    'Promotion decision training run refs',
    decision.trainingRunRefs,
    audience,
  ),
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    decision.updatedAtIso,
    nowIso,
  ),
})

export const projectOmniPromotionDecisionLedger = (
  ledger: OmniPromotionDecisionLedgerRecord,
  audience: OmniPromotionDecisionAudience,
  nowIso: string,
): OmniPromotionDecisionProjection => {
  assertLedger(ledger)

  return {
    adapterInstallAllowed: !ledger.authority.noAdapterInstall,
    adapterRefs: refsForAudience(
      'Promotion ledger adapter refs',
      ledger.adapterRefs,
      audience,
    ),
    artifactRefs: refsForAudience(
      'Promotion ledger artifact refs',
      ledger.artifactRefs,
      audience,
    ),
    audience,
    authority: ledger.authority,
    benchmarkEvidenceRefs: refsForAudience(
      'Promotion ledger benchmark evidence refs',
      ledger.benchmarkEvidenceRefs,
      audience,
    ),
    blockedCount: ledger.decisions.filter(decision => decision.state === 'blocked').length,
    blockerRefs: refsForAudience(
      'Promotion ledger blocker refs',
      ledger.blockerRefs,
      audience,
    ),
    candidateRefs: refsForAudience(
      'Promotion ledger candidate refs',
      ledger.candidateRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Promotion ledger caveat refs',
      ledger.caveatRefs,
      audience,
    ),
    claimState: claimStateForLedger(ledger),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      ledger.createdAtIso,
      nowIso,
    ),
    decisionCount: ledger.decisions.length,
    decisions: ledger.decisions.map(decision =>
      redactDecision(decision, audience, nowIso),
    ),
    evalEvidenceRefs: refsForAudience(
      'Promotion ledger eval evidence refs',
      ledger.evalEvidenceRefs,
      audience,
    ),
    failedCount: ledger.decisions.filter(decision => decision.state === 'failed').length,
    id: refForAudience(
      'Promotion ledger id',
      ledger.id,
      audience,
      'promotion-ledger.redacted',
    ),
    ledgerRef: refForAudience(
      'Promotion ledger ref',
      ledger.ledgerRef,
      audience,
      'ledger.redacted.promotion',
    ),
    marketplaceMemoryRefs: refsForAudience(
      'Promotion ledger marketplace memory refs',
      ledger.marketplaceMemoryRefs,
      audience,
    ),
    marketplaceRankMutationAllowed:
      !ledger.authority.noMarketplaceRankMutation,
    modelDeploymentAllowed: !ledger.authority.noModelDeployment,
    outcomeAttributionRefs: refsForAudience(
      'Promotion ledger outcome attribution refs',
      ledger.outcomeAttributionRefs,
      audience,
    ),
    passedCount: ledger.decisions.filter(decision => decision.state === 'passed').length,
    paymentSpendAllowed: !ledger.authority.noPaymentSpend,
    payoutMutationAllowed: !ledger.authority.noPayoutMutation,
    providerMutationAllowed: !ledger.authority.noProviderMutation,
    publicClaimUpgradeAllowed: !ledger.authority.noPublicClaimUpgrade,
    releaseGateRefs: refsForAudience(
      'Promotion ledger release gate refs',
      ledger.releaseGateRefs,
      audience,
    ),
    rollbackExecutionAllowed: !ledger.authority.noRollbackExecution,
    routeMutationAllowed: !ledger.authority.noRouteMutation,
    routeRefs: refsForAudience(
      'Promotion ledger route refs',
      ledger.routeRefs,
      audience,
    ),
    runtimePromotionAllowed: !ledger.authority.noRuntimePromotion,
    settlementMutationAllowed: !ledger.authority.noSettlementMutation,
    supersededCount: ledger.decisions.filter(
      decision => decision.state === 'superseded',
    ).length,
    trainingRunRefs: refsForAudience(
      'Promotion ledger training run refs',
      ledger.trainingRunRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      ledger.updatedAtIso,
      nowIso,
    ),
  }
}

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

export const omniPromotionDecisionProjectionHasPrivateMaterial = (
  projection: OmniPromotionDecisionProjection,
): boolean =>
  projectionStringValues(projection).some(
    value =>
      unsafePromotionRefPattern.test(value) ||
      rawTimestampPattern.test(value),
  )

export const exampleOmniPromotionDecisionLedger =
  (): OmniPromotionDecisionLedgerRecord => ({
    adapterRefs: ['adapter.public.autopilot_lora_v2'],
    artifactRefs: ['artifact.public.autopilot_lora_candidate_v2'],
    authority: OMNI_PROMOTION_DECISION_READ_ONLY_AUTHORITY,
    benchmarkEvidenceRefs: ['benchmark.public.autopilot_coding_cloud_v1'],
    blockerRefs: [],
    candidateRefs: ['candidate.public.autopilot_lora_v2'],
    caveatRefs: ['caveat.public.promotion_decision_evidence_only'],
    createdAtIso: '2026-06-07T00:05:00.000Z',
    decisions: [
      {
        adapterRefs: ['adapter.public.autopilot_lora_v2'],
        artifactRefs: ['artifact.public.autopilot_lora_candidate_v2'],
        benchmarkEvidenceRefs: ['benchmark.public.autopilot_coding_cloud_v1'],
        blockerRefs: [],
        candidateRefs: ['candidate.public.autopilot_lora_v2'],
        caveatRefs: ['caveat.public.promotion_does_not_deploy'],
        createdAtIso: '2026-06-07T00:08:00.000Z',
        decisionRef: 'decision.public.autopilot_lora_v2_passed',
        evalEvidenceRefs: ['eval.public.autopilot_candidate_cloud'],
        marketplaceMemoryRefs: ['marketplace.public.autopilot_margin_memory'],
        outcomeAttributionRefs: [
          'outcome.public.autopilot_site_revision_quality',
        ],
        releaseGateRefs: ['gate.public.model_lab_release_review'],
        reviewerReceiptRefs: ['receipt.public.operator_review_passed'],
        riskLabels: ['medium'],
        rollbackPosture: 'ready',
        rollbackRefs: ['rollback.public.autopilot_lora_v1_restore'],
        routeRefs: ['route.public.autopilot_model_lab_candidate'],
        state: 'passed',
        supersededByRefs: [],
        supersedesRefs: ['decision.public.autopilot_lora_v1_superseded'],
        targetKind: 'candidate',
        trainingRunRefs: ['training.public.autopilot_lora_v2_imported'],
        updatedAtIso: '2026-06-07T00:12:00.000Z',
      },
    ],
    evalEvidenceRefs: ['eval.public.autopilot_candidate_cloud'],
    id: 'promotion.public.autopilot_lora_v2',
    ledgerRef: 'ledger.public.model_lab_promotion_v1',
    marketplaceMemoryRefs: ['marketplace.public.autopilot_margin_memory'],
    outcomeAttributionRefs: ['outcome.public.autopilot_site_revision_quality'],
    releaseGateRefs: ['gate.public.model_lab_release_review'],
    routeRefs: ['route.public.autopilot_model_lab_candidate'],
    trainingRunRefs: ['training.public.autopilot_lora_v2_imported'],
    updatedAtIso: '2026-06-07T00:14:00.000Z',
  })
