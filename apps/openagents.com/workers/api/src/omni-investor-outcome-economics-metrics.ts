import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  type OmniAcceptedOutcomeWorkKind,
  OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema,
} from './omni-accepted-outcome-contracts'
import { OmniProjectionAudience } from './omni-data-classification'

export const OmniInvestorOutcomeEconomicsRevenueState = S.Literals([
  'none',
  'modeled',
  'accepted',
  'payable',
  'dispatched',
  'verified',
  'settled',
  'refunded',
  'blocked',
  'mixed',
])
export type OmniInvestorOutcomeEconomicsRevenueState =
  typeof OmniInvestorOutcomeEconomicsRevenueState.Type

export const OmniInvestorOutcomeEconomicsProviderSettlementState = S.Literals([
  'none',
  'payable',
  'dispatched',
  'verified',
  'settled',
  'failed',
  'blocked',
  'mixed',
])
export type OmniInvestorOutcomeEconomicsProviderSettlementState =
  typeof OmniInvestorOutcomeEconomicsProviderSettlementState.Type

export const OmniInvestorOutcomeEconomicsRefundState = S.Literals([
  'none',
  'exposure',
  'requested',
  'partial_refund',
  'refunded',
  'settled',
  'blocked',
  'mixed',
])
export type OmniInvestorOutcomeEconomicsRefundState =
  typeof OmniInvestorOutcomeEconomicsRefundState.Type

export const OmniInvestorOutcomeEconomicsAuthorityBoundary = S.Literals([
  'read_only_metrics_projection',
])
export type OmniInvestorOutcomeEconomicsAuthorityBoundary =
  typeof OmniInvestorOutcomeEconomicsAuthorityBoundary.Type

export class OmniInvestorOutcomeEconomicsAuthority extends S.Class<OmniInvestorOutcomeEconomicsAuthority>(
  'OmniInvestorOutcomeEconomicsAuthority',
)({
  authorityBoundary: OmniInvestorOutcomeEconomicsAuthorityBoundary,
  noBuyerChargeMutation: S.Boolean,
  noEconomicsLedgerMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noProviderSettlementMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noRefundMutation: S.Boolean,
}) {}

export class OmniInvestorOutcomeEconomicsMetricRecord extends S.Class<OmniInvestorOutcomeEconomicsMetricRecord>(
  'OmniInvestorOutcomeEconomicsMetricRecord',
)({
  acceptedOutcomeCount: S.Number,
  acceptedOutcomeRefs: S.Array(S.String),
  acceptedRevenueCents: S.Number,
  artifactCostCents: S.Number,
  authority: OmniInvestorOutcomeEconomicsAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  economicsRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  gradingCostCents: S.Number,
  gradingRefs: S.Array(S.String),
  id: S.String,
  providerPayableCents: S.Number,
  providerSettledCents: S.Number,
  providerSettlementRefs: S.Array(S.String),
  providerSettlementState: OmniInvestorOutcomeEconomicsProviderSettlementState,
  refundExposureCents: S.Number,
  refundRefs: S.Array(S.String),
  refundState: OmniInvestorOutcomeEconomicsRefundState,
  refundedCents: S.Number,
  revenueRefs: S.Array(S.String),
  revenueState: OmniInvestorOutcomeEconomicsRevenueState,
  reviewCostCents: S.Number,
  reviewMinutes: S.Number,
  reviewRefs: S.Array(S.String),
  retryCostCents: S.Number,
  retryCount: S.Number,
  retryRefs: S.Array(S.String),
  runnerCostCents: S.Number,
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomRefs: S.Array(S.String),
}) {}

export class OmniInvestorOutcomeEconomicsAggregate extends S.Class<OmniInvestorOutcomeEconomicsAggregate>(
  'OmniInvestorOutcomeEconomicsAggregate',
)({
  acceptedGrossProfitCents: S.Number,
  acceptedOutcomeCount: S.Number,
  acceptedOutcomeRefs: S.Array(S.String),
  acceptedRevenueCents: S.Number,
  acceptedRevenueClaimAllowed: S.Boolean,
  artifactCostCents: S.Number,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  economicsRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  gradingCostCents: S.Number,
  gradingCount: S.Number,
  gradingRefs: S.Array(S.String),
  grossMarginBps: S.NullOr(S.Number),
  modeledOnly: S.Boolean,
  providerPayableCents: S.Number,
  providerPayableClaimAllowed: S.Boolean,
  providerSettledCents: S.Number,
  providerSettlementClaimAllowed: S.Boolean,
  providerSettlementRefs: S.Array(S.String),
  providerSettlementState: OmniInvestorOutcomeEconomicsProviderSettlementState,
  providerSettlementStateLabel: S.String,
  refundClaimAllowed: S.Boolean,
  refundExposureCents: S.Number,
  refundRefs: S.Array(S.String),
  refundState: OmniInvestorOutcomeEconomicsRefundState,
  refundStateLabel: S.String,
  refundedCents: S.Number,
  revenueRefs: S.Array(S.String),
  revenueState: OmniInvestorOutcomeEconomicsRevenueState,
  revenueStateLabel: S.String,
  reviewCostCents: S.Number,
  reviewCount: S.Number,
  reviewMinutes: S.Number,
  reviewRefs: S.Array(S.String),
  retryCostCents: S.Number,
  retryCount: S.Number,
  retryRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  workroomRefs: S.Array(S.String),
}) {}

export class OmniInvestorOutcomeEconomicsWorkClassMetric extends S.Class<OmniInvestorOutcomeEconomicsWorkClassMetric>(
  'OmniInvestorOutcomeEconomicsWorkClassMetric',
)({
  ...OmniInvestorOutcomeEconomicsAggregate.fields,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workKindLabel: S.String,
}) {}

export class OmniInvestorOutcomeEconomicsProjection extends S.Class<OmniInvestorOutcomeEconomicsProjection>(
  'OmniInvestorOutcomeEconomicsProjection',
)({
  audience: OmniProjectionAudience,
  authority: OmniInvestorOutcomeEconomicsAuthority,
  buyerChargeMutationAllowed: S.Boolean,
  economicsLedgerMutationAllowed: S.Boolean,
  generatedFromRecordCount: S.Number,
  liveWalletSpendAllowed: S.Boolean,
  payoutDispatchMutationAllowed: S.Boolean,
  providerSettlementMutationAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  refundMutationAllowed: S.Boolean,
  totals: OmniInvestorOutcomeEconomicsAggregate,
  updatedAtDisplay: S.NullOr(S.String),
  workClassMetrics: S.Array(OmniInvestorOutcomeEconomicsWorkClassMetric),
}) {}

export class OmniInvestorOutcomeEconomicsUnsafe extends S.TaggedErrorClass<OmniInvestorOutcomeEconomicsUnsafe>()(
  'OmniInvestorOutcomeEconomicsUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_INVESTOR_OUTCOME_ECONOMICS_READ_ONLY_AUTHORITY:
  OmniInvestorOutcomeEconomicsAuthority = {
    authorityBoundary: 'read_only_metrics_projection',
    noBuyerChargeMutation: true,
    noEconomicsLedgerMutation: true,
    noLiveWalletSpend: true,
    noPayoutDispatch: true,
    noProviderSettlementMutation: true,
    noPublicClaimUpgrade: true,
    noRefundMutation: true,
  }

const workKindLabelByKind: Readonly<Record<OmniAcceptedOutcomeWorkKind, string>> = {
  adjustment: 'Adjustment',
  business: 'Business',
  coding: 'Coding',
  existing_project_import: 'Existing project import',
  legal_sensitive: 'Legal-sensitive',
  site: 'Site',
}

const revenueStateLabelByState:
  Readonly<Record<OmniInvestorOutcomeEconomicsRevenueState, string>> = {
    accepted: 'Accepted',
    blocked: 'Blocked',
    dispatched: 'Dispatch recorded',
    mixed: 'Mixed',
    modeled: 'Modeled',
    none: 'None',
    payable: 'Payable',
    refunded: 'Refunded',
    settled: 'Settled',
    verified: 'Verified',
  }

const providerSettlementStateLabelByState:
  Readonly<Record<OmniInvestorOutcomeEconomicsProviderSettlementState, string>> = {
    blocked: 'Blocked',
    dispatched: 'Dispatch recorded',
    failed: 'Failed',
    mixed: 'Mixed',
    none: 'None',
    payable: 'Payable',
    settled: 'Settled',
    verified: 'Verified',
  }

const refundStateLabelByState:
  Readonly<Record<OmniInvestorOutcomeEconomicsRefundState, string>> = {
    blocked: 'Blocked',
    exposure: 'Exposure',
    mixed: 'Mixed',
    none: 'None',
    partial_refund: 'Partial refund',
    refunded: 'Refunded',
    requested: 'Requested',
    settled: 'Settled',
  }

const revenueStateRank:
  Readonly<Record<OmniInvestorOutcomeEconomicsRevenueState, number>> = {
    accepted: 2,
    blocked: -1,
    dispatched: 4,
    mixed: 0,
    modeled: 1,
    none: 0,
    payable: 3,
    refunded: 7,
    settled: 6,
    verified: 5,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeInvestorEconomicsRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|channel[_-]?monitor|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|entropy|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(channel|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|channel|invoice|payment|payload|payout|prompt|provider|runner|run[_-]?log|state|target|telemetry|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(accepted_outcome\.private|blocker\.private|caveat\.private|customer\.|economics\.private|evidence\.private|grading\.private|provider\.private|refund\.private|revenue\.private|review\.private|retry\.private|settlement\.private|source\.private|workroom\.)/i
const teamUnsafeRefPattern =
  /(provider\.private|refund\.private|settlement\.private|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeInvestorEconomicsRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniInvestorOutcomeEconomicsUnsafe({
      reason: `${label} contains private customer data, provider secrets, wallet material, raw bitcoin payment material, invoices, preimages, raw payout targets, raw logs, private repo refs, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent' || audience === 'customer') {
    return publicUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const redactRefsForAudience = (
  audience: typeof OmniProjectionAudience.Type,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const pattern = audienceUnsafePattern(audience)
  const refsToProject = uniqueRefs(refs)

  if (pattern === null) {
    return refsToProject
  }

  return refsToProject.filter(ref => !pattern.test(ref))
}

const assertNonNegativeInteger = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new OmniInvestorOutcomeEconomicsUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

const assertMetricRecord = (
  record: OmniInvestorOutcomeEconomicsMetricRecord,
): void => {
  if (record.revenueState === 'mixed') {
    throw new OmniInvestorOutcomeEconomicsUnsafe({
      reason: 'Individual investor economics records cannot use mixed revenue state.',
    })
  }

  if (record.providerSettlementState === 'mixed') {
    throw new OmniInvestorOutcomeEconomicsUnsafe({
      reason:
        'Individual investor economics records cannot use mixed provider settlement state.',
    })
  }

  if (record.refundState === 'mixed') {
    throw new OmniInvestorOutcomeEconomicsUnsafe({
      reason: 'Individual investor economics records cannot use mixed refund state.',
    })
  }

  Object.entries({
    acceptedOutcomeCount: record.acceptedOutcomeCount,
    acceptedRevenueCents: record.acceptedRevenueCents,
    artifactCostCents: record.artifactCostCents,
    gradingCostCents: record.gradingCostCents,
    providerPayableCents: record.providerPayableCents,
    providerSettledCents: record.providerSettledCents,
    refundExposureCents: record.refundExposureCents,
    refundedCents: record.refundedCents,
    reviewCostCents: record.reviewCostCents,
    reviewMinutes: record.reviewMinutes,
    retryCostCents: record.retryCostCents,
    retryCount: record.retryCount,
    runnerCostCents: record.runnerCostCents,
  }).forEach(([label, value]) => assertNonNegativeInteger(label, value))

  if (
    record.acceptedRevenueCents > 0 &&
    revenueStateRank[record.revenueState] < revenueStateRank.accepted
  ) {
    throw new OmniInvestorOutcomeEconomicsUnsafe({
      reason:
        'Accepted revenue must use accepted, payable, dispatched, verified, settled, or refunded revenue state.',
    })
  }

  if (
    record.providerSettledCents > 0 &&
    record.providerSettlementState !== 'settled'
  ) {
    throw new OmniInvestorOutcomeEconomicsUnsafe({
      reason:
        'Provider settled cents can only be projected when provider settlement state is settled.',
    })
  }

  if (
    record.providerSettlementState === 'settled' &&
    (record.providerSettledCents <= 0 ||
      record.providerSettlementRefs.length === 0)
  ) {
    throw new OmniInvestorOutcomeEconomicsUnsafe({
      reason:
        'Settled provider economics require a positive providerSettledCents value and settlement refs.',
    })
  }

  if (record.providerSettledCents > record.providerPayableCents) {
    throw new OmniInvestorOutcomeEconomicsUnsafe({
      reason: 'Provider settled cents cannot exceed provider payable cents.',
    })
  }

  if (
    record.refundedCents > 0 &&
    !['partial_refund', 'refunded', 'settled'].includes(record.refundState)
  ) {
    throw new OmniInvestorOutcomeEconomicsUnsafe({
      reason:
        'Refunded cents require partial_refund, refunded, or settled refund state.',
    })
  }

  if (record.refundedCents > record.refundExposureCents) {
    throw new OmniInvestorOutcomeEconomicsUnsafe({
      reason: 'Refunded cents cannot exceed refund exposure cents.',
    })
  }

  if (record.refundedCents > 0 && record.refundRefs.length === 0) {
    throw new OmniInvestorOutcomeEconomicsUnsafe({
      reason: 'Refunded cents require refund refs.',
    })
  }

  if (
    record.authority.noBuyerChargeMutation !== true ||
    record.authority.noEconomicsLedgerMutation !== true ||
    record.authority.noLiveWalletSpend !== true ||
    record.authority.noPayoutDispatch !== true ||
    record.authority.noProviderSettlementMutation !== true ||
    record.authority.noPublicClaimUpgrade !== true ||
    record.authority.noRefundMutation !== true
  ) {
    throw new OmniInvestorOutcomeEconomicsUnsafe({
      reason:
        'Investor outcome economics records must remain read-only and cannot mutate charges, ledgers, wallets, payouts, provider settlement, public claims, or refunds.',
    })
  }

  assertSafeRefs('Investor economics accepted outcome refs', record.acceptedOutcomeRefs)
  assertSafeRefs('Investor economics blocker refs', record.blockerRefs)
  assertSafeRefs('Investor economics caveat refs', record.caveatRefs)
  assertSafeRefs('Investor economics economics refs', record.economicsRefs)
  assertSafeRefs('Investor economics evidence refs', record.evidenceRefs)
  assertSafeRefs('Investor economics grading refs', record.gradingRefs)
  assertSafeRefs(
    'Investor economics provider settlement refs',
    record.providerSettlementRefs,
  )
  assertSafeRefs('Investor economics refund refs', record.refundRefs)
  assertSafeRefs('Investor economics revenue refs', record.revenueRefs)
  assertSafeRefs('Investor economics review refs', record.reviewRefs)
  assertSafeRefs('Investor economics retry refs', record.retryRefs)
  assertSafeRefs('Investor economics source refs', record.sourceRefs)
  assertSafeRefs('Investor economics workroom refs', record.workroomRefs)
}

const combineState = <State extends string>(
  states: ReadonlyArray<State>,
  emptyState: State,
  mixedState: State,
): State => {
  const meaningfulStates = uniqueRefs(states).filter(state => state !== emptyState)

  if (meaningfulStates.length === 0) {
    return emptyState
  }

  if (meaningfulStates.length === 1) {
    return meaningfulStates[0] as State
  }

  return mixedState
}

const grossProfitCents = (
  records: ReadonlyArray<OmniInvestorOutcomeEconomicsMetricRecord>,
): number =>
  records.reduce(
    (sum, record) =>
      sum +
      record.acceptedRevenueCents -
      record.runnerCostCents -
      record.providerPayableCents -
      record.retryCostCents -
      record.reviewCostCents -
      record.gradingCostCents -
      record.artifactCostCents -
      record.refundExposureCents,
    0,
  )

const cents = (
  records: ReadonlyArray<OmniInvestorOutcomeEconomicsMetricRecord>,
  pick: (record: OmniInvestorOutcomeEconomicsMetricRecord) => number,
): number => records.reduce((sum, record) => sum + pick(record), 0)

const refs = (
  records: ReadonlyArray<OmniInvestorOutcomeEconomicsMetricRecord>,
  pick: (record: OmniInvestorOutcomeEconomicsMetricRecord) => ReadonlyArray<string>,
): ReadonlyArray<string> => uniqueRefs(records.flatMap(record => [...pick(record)]))

const aggregateRecords = (
  records: ReadonlyArray<OmniInvestorOutcomeEconomicsMetricRecord>,
  audience: typeof OmniProjectionAudience.Type,
): OmniInvestorOutcomeEconomicsAggregate => {
  const acceptedRevenueCents = cents(records, record => record.acceptedRevenueCents)
  const acceptedGrossProfitCents = grossProfitCents(records)
  const providerSettlementRefs = redactRefsForAudience(
    audience,
    refs(records, record => record.providerSettlementRefs),
  )
  const refundRefs = redactRefsForAudience(
    audience,
    refs(records, record => record.refundRefs),
  )
  const revenueState = combineState(
    records.map(record => record.revenueState),
    'none',
    'mixed',
  )
  const providerSettlementState = combineState(
    records.map(record => record.providerSettlementState),
    'none',
    'mixed',
  )
  const refundState = combineState(
    records.map(record => record.refundState),
    'none',
    'mixed',
  )

  return {
    acceptedGrossProfitCents,
    acceptedOutcomeCount: cents(records, record => record.acceptedOutcomeCount),
    acceptedOutcomeRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.acceptedOutcomeRefs),
    ),
    acceptedRevenueCents,
    acceptedRevenueClaimAllowed:
      acceptedRevenueCents > 0 &&
      !['blocked', 'modeled', 'mixed', 'none'].includes(revenueState),
    artifactCostCents: cents(records, record => record.artifactCostCents),
    blockerRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.blockerRefs),
    ),
    caveatRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.caveatRefs),
    ),
    economicsRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.economicsRefs),
    ),
    evidenceRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.evidenceRefs),
    ),
    gradingCostCents: cents(records, record => record.gradingCostCents),
    gradingCount: records.filter(record => record.gradingRefs.length > 0).length,
    gradingRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.gradingRefs),
    ),
    grossMarginBps:
      acceptedRevenueCents === 0
        ? null
        : Math.round((acceptedGrossProfitCents * 10000) / acceptedRevenueCents),
    modeledOnly:
      records.length > 0 &&
      records.every(record => record.revenueState === 'modeled'),
    providerPayableCents: cents(records, record => record.providerPayableCents),
    providerPayableClaimAllowed: records.some(record =>
      ['dispatched', 'payable', 'settled', 'verified'].includes(
        record.providerSettlementState,
      ),
    ),
    providerSettledCents: cents(records, record => record.providerSettledCents),
    providerSettlementClaimAllowed:
      providerSettlementRefs.length > 0 &&
      records.some(
        record =>
          record.providerSettlementState === 'settled' &&
          record.providerSettledCents > 0,
      ),
    providerSettlementRefs,
    providerSettlementState,
    providerSettlementStateLabel:
      providerSettlementStateLabelByState[providerSettlementState],
    refundClaimAllowed:
      refundRefs.length > 0 &&
      records.some(record => record.refundedCents > 0),
    refundExposureCents: cents(records, record => record.refundExposureCents),
    refundRefs,
    refundState,
    refundStateLabel: refundStateLabelByState[refundState],
    refundedCents: cents(records, record => record.refundedCents),
    revenueRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.revenueRefs),
    ),
    revenueState,
    revenueStateLabel: revenueStateLabelByState[revenueState],
    reviewCostCents: cents(records, record => record.reviewCostCents),
    reviewCount: records.filter(record => record.reviewRefs.length > 0).length,
    reviewMinutes: cents(records, record => record.reviewMinutes),
    reviewRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.reviewRefs),
    ),
    retryCostCents: cents(records, record => record.retryCostCents),
    retryCount: cents(records, record => record.retryCount),
    retryRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.retryRefs),
    ),
    sourceRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.sourceRefs),
    ),
    workroomRefs: redactRefsForAudience(
      audience,
      refs(records, record => record.workroomRefs),
    ),
  }
}

export const projectOmniInvestorOutcomeEconomicsMetrics = (
  records: ReadonlyArray<OmniInvestorOutcomeEconomicsMetricRecord>,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniInvestorOutcomeEconomicsProjection => {
  records.forEach(assertMetricRecord)

  const workKinds = uniqueRefs(
    records.map(record => record.workKind),
  ) as ReadonlyArray<OmniAcceptedOutcomeWorkKind>
  const workClassMetrics = workKinds.map(workKind => ({
    ...aggregateRecords(
      records.filter(record => record.workKind === workKind),
      audience,
    ),
    workKind,
    workKindLabel: workKindLabelByKind[workKind],
  }))
  const updatedAtIso =
    [...records]
      .map(record => record.updatedAtIso)
      .sort()
      .at(-1) ?? null

  return {
    audience,
    authority: OMNI_INVESTOR_OUTCOME_ECONOMICS_READ_ONLY_AUTHORITY,
    buyerChargeMutationAllowed: false,
    economicsLedgerMutationAllowed: false,
    generatedFromRecordCount: records.length,
    liveWalletSpendAllowed: false,
    payoutDispatchMutationAllowed: false,
    providerSettlementMutationAllowed: false,
    publicClaimUpgradeAllowed: false,
    refundMutationAllowed: false,
    totals: aggregateRecords(records, audience),
    updatedAtDisplay:
      updatedAtIso === null
        ? null
        : friendlyBlueprintMissionBriefingTime(updatedAtIso, nowIso),
    workClassMetrics,
  }
}
