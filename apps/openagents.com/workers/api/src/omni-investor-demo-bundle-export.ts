import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema,
} from './omni-accepted-outcome-contracts'
import type {
  OmniInvestorOutcomeEconomicsProjection,
} from './omni-investor-outcome-economics-metrics'
import type {
  OmniOutcomePowerProductivityProjection,
} from './omni-outcome-power-productivity'
import {
  OmniPublicProofBundleStatus,
} from './omni-public-proof-bundles'
import {
  OmniRouteObservedResultKind,
  OmniRouteTrustTier,
} from './omni-route-scorecards'
import type {
  PylonCapacityFunnelAccountingProjection,
} from './pylon-capacity-funnel'

export const OmniInvestorDemoBundleAudience = S.Literals([
  'public',
  'investor',
  'team',
  'operator',
])
export type OmniInvestorDemoBundleAudience =
  typeof OmniInvestorDemoBundleAudience.Type

export const OmniInvestorDemoBundleReadiness = S.Literals([
  'ready',
  'needs_evidence',
  'blocked',
])
export type OmniInvestorDemoBundleReadiness =
  typeof OmniInvestorDemoBundleReadiness.Type

export const OmniInvestorDemoBundleSectionState = S.Literals([
  'complete',
  'partial',
  'missing',
  'blocked',
])
export type OmniInvestorDemoBundleSectionState =
  typeof OmniInvestorDemoBundleSectionState.Type

export const OmniInvestorDemoBundleMissingEvidenceKind = S.Literals([
  'accepted_revenue',
  'capacity_funnel',
  'fresh_capacity',
  'measured_power',
  'power_settlement',
  'proof_bundle_ready',
  'provider_settlement',
  'route_scorecard_success',
  'visible_capacity_settlement',
])
export type OmniInvestorDemoBundleMissingEvidenceKind =
  typeof OmniInvestorDemoBundleMissingEvidenceKind.Type

export const OmniInvestorDemoBundleAuthorityBoundary = S.Literals([
  'read_only_investor_demo_bundle',
])
export type OmniInvestorDemoBundleAuthorityBoundary =
  typeof OmniInvestorDemoBundleAuthorityBoundary.Type

export class OmniInvestorDemoBundleAuthority extends S.Class<OmniInvestorDemoBundleAuthority>(
  'OmniInvestorDemoBundleAuthority',
)({
  authorityBoundary: OmniInvestorDemoBundleAuthorityBoundary,
  noDownloadRouteMutation: S.Boolean,
  noInvestorShareMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noRawDataCopy: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class OmniInvestorDemoProofBundleSummary extends S.Class<OmniInvestorDemoProofBundleSummary>(
  'OmniInvestorDemoProofBundleSummary',
)({
  acceptanceStateRef: S.String,
  artifactRefs: S.Array(S.String),
  economicsCaveatRef: S.String,
  legalCaveatRef: S.NullOr(S.String),
  noSettlementImplication: S.Boolean,
  privacyCaveatRef: S.String,
  publicReceiptRef: S.String,
  receiptRefs: S.Array(S.String),
  reviewStateRef: S.String,
  sourceRefs: S.Array(S.String),
  status: OmniPublicProofBundleStatus,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomId: S.String,
}) {}

export class OmniInvestorDemoRouteScorecardSummary extends S.Class<OmniInvestorDemoRouteScorecardSummary>(
  'OmniInvestorDemoRouteScorecardSummary',
)({
  observedResultKind: OmniRouteObservedResultKind,
  observedResultRef: S.String,
  postCloseoutScore: S.NullOr(S.Number),
  publicCaveatRef: S.String,
  selectedModelRef: S.String,
  selectedRuntimeRef: S.String,
  trustTier: OmniRouteTrustTier,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomId: S.String,
}) {}

export class OmniInvestorDemoEconomicsSummary extends S.Class<OmniInvestorDemoEconomicsSummary>(
  'OmniInvestorDemoEconomicsSummary',
)({
  acceptedGrossProfitCents: S.Number,
  acceptedOutcomeCount: S.Number,
  acceptedOutcomeRefs: S.Array(S.String),
  acceptedRevenueCents: S.Number,
  acceptedRevenueClaimAllowed: S.Boolean,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  economicsRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  grossMarginBps: S.NullOr(S.Number),
  modeledOnly: S.Boolean,
  providerPayableCents: S.Number,
  providerSettlementClaimAllowed: S.Boolean,
  providerSettlementRefs: S.Array(S.String),
  providerSettlementStateLabel: S.String,
  providerSettledCents: S.Number,
  refundStateLabel: S.String,
  revenueRefs: S.Array(S.String),
  revenueStateLabel: S.String,
}) {}

export class OmniInvestorDemoCapacitySummary extends S.Class<OmniInvestorDemoCapacitySummary>(
  'OmniInvestorDemoCapacitySummary',
)({
  acceptedCount: S.Number,
  darkCount: S.Number,
  darkReasonCount: S.Number,
  freshCount: S.Number,
  paidButNotSettledCount: S.Number,
  paidCount: S.Number,
  settledCount: S.Number,
  settledWithoutVisibleReceiptCount: S.Number,
  staleCapacityRefs: S.Array(S.String),
  staleCount: S.Number,
  totalCount: S.Number,
  visibleSettlementClaimAllowedCount: S.Number,
}) {}

export class OmniInvestorDemoPowerSummary extends S.Class<OmniInvestorDemoPowerSummary>(
  'OmniInvestorDemoPowerSummary',
)({
  acceptedOutcomeCount: S.Number,
  acceptedOutcomesPerMwh: S.NullOr(S.Number),
  acceptedRevenueCentsPerKwh: S.NullOr(S.Number),
  darkCapacityMwh: S.Number,
  energyMwh: S.NullOr(S.Number),
  measuredEnergyClaimAllowed: S.Boolean,
  powerDataStateLabel: S.String,
  providerPayableCentsPerKwh: S.NullOr(S.Number),
  settlementClaimAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  settlementStateLabel: S.String,
}) {}

export class OmniInvestorDemoBundleSection extends S.Class<OmniInvestorDemoBundleSection>(
  'OmniInvestorDemoBundleSection',
)({
  label: S.String,
  state: OmniInvestorDemoBundleSectionState,
}) {}

export class OmniInvestorDemoMissingEvidenceItem extends S.Class<OmniInvestorDemoMissingEvidenceItem>(
  'OmniInvestorDemoMissingEvidenceItem',
)({
  kind: OmniInvestorDemoBundleMissingEvidenceKind,
  label: S.String,
  reasonRef: S.String,
  requiredForRef: S.String,
}) {}

export class OmniInvestorDemoBundleExport extends S.Class<OmniInvestorDemoBundleExport>(
  'OmniInvestorDemoBundleExport',
)({
  acceptedOutcomeClaimAllowed: S.Boolean,
  audience: OmniInvestorDemoBundleAudience,
  authority: OmniInvestorDemoBundleAuthority,
  capacityFunnel: OmniInvestorDemoCapacitySummary,
  caveatRefs: S.Array(S.String),
  downloadRouteMutationAllowed: S.Boolean,
  economics: OmniInvestorDemoEconomicsSummary,
  generatedAtDisplay: S.String,
  id: S.String,
  investorShareMutationAllowed: S.Boolean,
  liveWalletSpendAllowed: S.Boolean,
  missingEvidence: S.Array(OmniInvestorDemoMissingEvidenceItem),
  powerProductivity: OmniInvestorDemoPowerSummary,
  proofBundles: S.Array(OmniInvestorDemoProofBundleSummary),
  publicClaimUpgradeAllowed: S.Boolean,
  rawDataCopyAllowed: S.Boolean,
  readiness: OmniInvestorDemoBundleReadiness,
  reviewBeforeSharing: S.Boolean,
  routeScorecards: S.Array(OmniInvestorDemoRouteScorecardSummary),
  sections: S.Array(OmniInvestorDemoBundleSection),
  settlementMutationAllowed: S.Boolean,
  settlementStateLabel: S.String,
  sourceRefs: S.Array(S.String),
  title: S.String,
}) {}

export class OmniInvestorDemoBundleUnsafe extends S.TaggedErrorClass<OmniInvestorDemoBundleUnsafe>()(
  'OmniInvestorDemoBundleUnsafe',
  {
    reason: S.String,
  },
) {}

export type ProjectOmniInvestorDemoBundleInput = Readonly<{
  audience: OmniInvestorDemoBundleAudience
  capacityFunnel: PylonCapacityFunnelAccountingProjection
  caveatRefs?: ReadonlyArray<string> | undefined
  economics: OmniInvestorOutcomeEconomicsProjection
  generatedAtIso: string
  id: string
  powerProductivity: OmniOutcomePowerProductivityProjection
  proofBundles: ReadonlyArray<OmniInvestorDemoProofBundleSummary>
  routeScorecards: ReadonlyArray<OmniInvestorDemoRouteScorecardSummary>
  sourceRefs?: ReadonlyArray<string> | undefined
  title: string
}>

export const OMNI_INVESTOR_DEMO_BUNDLE_READ_ONLY_AUTHORITY:
  OmniInvestorDemoBundleAuthority = {
    authorityBoundary: 'read_only_investor_demo_bundle',
    noDownloadRouteMutation: true,
    noInvestorShareMutation: true,
    noLiveWalletSpend: true,
    noPublicClaimUpgrade: true,
    noRawDataCopy: true,
    noSettlementMutation: true,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeInvestorDemoRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|channel[_-]?monitor|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hardware[_-]?telemetry|hostname|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(channel|hardware|key|wallet)|provider[_-]?(account|grant|payload|secret|telemetry|token)|raw[_-]?(auth|channel|customer|email|energy|export|host|invoice|market|meter|payment|payload|payout|power|prompt|provider|runner|run[_-]?log|source[_-]?archive|state|target|telemetry|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?archive|token|trading[_-]?(account|order)|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const audienceUnsafeRefPattern =
  /(accepted_outcome\.private|artifact\.private|blocker\.private|caveat\.private|capacity\.private|data_rights\.private|diligence\.private|economics\.private|energy\.private|evidence\.private|export\.private|meter\.private|model\.private|node\.private|proof\.private|provenance\.private|provider\.private|receipt\.private|refund\.private|revenue\.private|review\.private|retry\.private|route\.private|scenario\.private|settlement\.private|source\.private|workroom\.private)/i

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
      unsafeInvestorDemoRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniInvestorDemoBundleUnsafe({
      reason: `${label} contains private customer, provider, wallet, payment, trading, raw data, private repo, secret, or raw timestamp material.`,
    })
  }
}

const assertSafeTitle = (title: string): void => {
  if (
    title.trim() === '' ||
    unsafeInvestorDemoRefPattern.test(title) ||
    rawTimestampPattern.test(title)
  ) {
    throw new OmniInvestorDemoBundleUnsafe({
      reason:
        'Investor demo bundle title must not contain private customer, provider, wallet, payment, raw data, secret, or raw timestamp material.',
    })
  }
}

const redactRefsForAudience = (
  audience: OmniInvestorDemoBundleAudience,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  assertSafeRefs('Investor demo bundle refs', refs)

  if (audience === 'operator') {
    return uniqueRefs(refs)
  }

  return uniqueRefs(refs).filter(ref => !audienceUnsafeRefPattern.test(ref))
}

const routeScorecardRefs = (
  routeScorecards: ReadonlyArray<OmniInvestorDemoRouteScorecardSummary>,
): ReadonlyArray<string> =>
  routeScorecards.flatMap(route => [
    route.observedResultRef,
    route.publicCaveatRef,
    route.selectedModelRef,
    route.selectedRuntimeRef,
    route.workroomId,
  ])

const proofBundleRefs = (
  proofBundles: ReadonlyArray<OmniInvestorDemoProofBundleSummary>,
): ReadonlyArray<string> =>
  proofBundles.flatMap(bundle => [
    bundle.acceptanceStateRef,
    ...bundle.artifactRefs,
    bundle.economicsCaveatRef,
    bundle.legalCaveatRef ?? '',
    bundle.privacyCaveatRef,
    bundle.publicReceiptRef,
    ...bundle.receiptRefs,
    bundle.reviewStateRef,
    ...bundle.sourceRefs,
    bundle.workroomId,
  ])

const economicsSourceRefs = (
  economics: OmniInvestorOutcomeEconomicsProjection,
): ReadonlyArray<string> => [
  ...economics.totals.acceptedOutcomeRefs,
  ...economics.totals.economicsRefs,
  ...economics.totals.evidenceRefs,
  ...economics.totals.providerSettlementRefs,
  ...economics.totals.revenueRefs,
  ...economics.totals.sourceRefs,
]

const economicsCaveatRefs = (
  economics: OmniInvestorOutcomeEconomicsProjection,
): ReadonlyArray<string> => [
  ...economics.totals.blockerRefs,
  ...economics.totals.caveatRefs,
]

const powerSourceRefs = (
  power: OmniOutcomePowerProductivityProjection,
): ReadonlyArray<string> => [
  ...power.totals.acceptedOutcomeRefs,
  ...power.totals.energyEvidenceRefs,
  ...power.totals.energyModelRefs,
  ...power.totals.measuredEnergyRefs,
  ...power.totals.settlementRefs,
  ...power.totals.sourceRefs,
]

const powerCaveatRefs = (
  power: OmniOutcomePowerProductivityProjection,
): ReadonlyArray<string> => [
  ...power.totals.caveatRefs,
  ...power.totals.darkCapacityReasonRefs,
]

const capacityCaveatRefs = (
  capacity: PylonCapacityFunnelAccountingProjection,
): ReadonlyArray<string> => [
  ...capacity.claimBoundaryCaveatRefs,
  ...capacity.byDarkCapacityReason.flatMap(summary => [
    summary.reasonRef,
    ...summary.caveatRefs,
  ]),
]

const capacitySourceRefs = (
  capacity: PylonCapacityFunnelAccountingProjection,
): ReadonlyArray<string> => [
  ...capacity.staleCapacityRefs,
  ...capacity.byDarkCapacityReason.flatMap(summary => [
    ...summary.capacityRefs,
    ...summary.evidenceRefs,
    ...summary.workClassRefs,
  ]),
]

const missingItem = (
  kind: OmniInvestorDemoBundleMissingEvidenceKind,
  label: string,
  reasonRef: string,
  requiredForRef: string,
): OmniInvestorDemoMissingEvidenceItem => ({
  kind,
  label,
  reasonRef,
  requiredForRef,
})

const missingEvidenceForBundle = (
  input: ProjectOmniInvestorDemoBundleInput,
): ReadonlyArray<OmniInvestorDemoMissingEvidenceItem> => {
  const missing: Array<OmniInvestorDemoMissingEvidenceItem> = []
  const readyProofCount = input.proofBundles.filter(
    bundle => bundle.status === 'ready',
  ).length
  const successfulRouteCount = input.routeScorecards.filter(
    route => route.observedResultKind === 'success',
  ).length

  if (readyProofCount === 0) {
    missing.push(missingItem(
      'proof_bundle_ready',
      'Ready public proof bundle',
      'missing.proof_bundle.ready',
      'investor_share',
    ))
  }

  if (successfulRouteCount === 0) {
    missing.push(missingItem(
      'route_scorecard_success',
      'Successful route scorecard',
      'missing.route_scorecard.success',
      'investor_share',
    ))
  }

  if (!input.economics.totals.acceptedRevenueClaimAllowed) {
    missing.push(missingItem(
      'accepted_revenue',
      'Accepted revenue evidence',
      'missing.economics.accepted_revenue',
      'accepted_outcome_claim',
    ))
  }

  if (
    input.economics.totals.providerPayableCents > 0 &&
    !input.economics.totals.providerSettlementClaimAllowed
  ) {
    missing.push(missingItem(
      'provider_settlement',
      'Provider settlement receipt',
      'missing.economics.provider_settlement',
      'settlement_claim',
    ))
  }

  if (input.capacityFunnel.totalCount === 0) {
    missing.push(missingItem(
      'capacity_funnel',
      'Capacity funnel rows',
      'missing.capacity_funnel.rows',
      'capacity_claim',
    ))
  }

  if (input.capacityFunnel.staleCount > 0) {
    missing.push(missingItem(
      'fresh_capacity',
      'Fresh capacity records',
      'missing.capacity_funnel.freshness',
      'capacity_claim',
    ))
  }

  if (input.capacityFunnel.settledWithoutVisibleReceiptCount > 0) {
    missing.push(missingItem(
      'visible_capacity_settlement',
      'Visible capacity settlement receipt',
      'missing.capacity_funnel.visible_settlement',
      'settlement_claim',
    ))
  }

  if (!input.powerProductivity.totals.measuredEnergyClaimAllowed) {
    missing.push(missingItem(
      'measured_power',
      'Measured power evidence',
      'missing.power.measured_energy',
      'power_productivity_claim',
    ))
  }

  if (
    input.powerProductivity.totals.providerPayableCents > 0 &&
    !input.powerProductivity.totals.settlementClaimAllowed
  ) {
    missing.push(missingItem(
      'power_settlement',
      'Power-linked settlement receipt',
      'missing.power.settlement',
      'settlement_claim',
    ))
  }

  return missing
}

const sectionState = (
  blocked: boolean,
  complete: boolean,
  partial: boolean,
): OmniInvestorDemoBundleSectionState => {
  if (blocked) {
    return 'blocked'
  }

  if (complete) {
    return 'complete'
  }

  return partial ? 'partial' : 'missing'
}

const bundleSections = (
  input: ProjectOmniInvestorDemoBundleInput,
): ReadonlyArray<OmniInvestorDemoBundleSection> => [
  {
    label: 'Proof bundles',
    state: sectionState(
      input.proofBundles.some(bundle => bundle.status === 'blocked'),
      input.proofBundles.some(bundle => bundle.status === 'ready'),
      input.proofBundles.length > 0,
    ),
  },
  {
    label: 'Route scorecards',
    state: sectionState(
      input.routeScorecards.some(route => route.observedResultKind === 'failure'),
      input.routeScorecards.some(route => route.observedResultKind === 'success'),
      input.routeScorecards.length > 0,
    ),
  },
  {
    label: 'Outcome economics',
    state: sectionState(
      input.economics.totals.blockerRefs.length > 0,
      input.economics.totals.acceptedRevenueClaimAllowed,
      input.economics.totals.acceptedRevenueCents > 0,
    ),
  },
  {
    label: 'Capacity funnel',
    state: sectionState(
      false,
      input.capacityFunnel.totalCount > 0 &&
        input.capacityFunnel.staleCount === 0,
      input.capacityFunnel.totalCount > 0,
    ),
  },
  {
    label: 'Power productivity',
    state: sectionState(
      false,
      input.powerProductivity.totals.measuredEnergyClaimAllowed,
      input.powerProductivity.totals.energyMwh !== null,
    ),
  },
]

const readinessForBundle = (
  input: ProjectOmniInvestorDemoBundleInput,
  missingEvidence: ReadonlyArray<OmniInvestorDemoMissingEvidenceItem>,
): OmniInvestorDemoBundleReadiness => {
  if (
    input.proofBundles.some(bundle => bundle.status === 'blocked') ||
    input.economics.totals.blockerRefs.length > 0
  ) {
    return 'blocked'
  }

  return missingEvidence.length === 0 ? 'ready' : 'needs_evidence'
}

const assertBundleSafe = (bundle: OmniInvestorDemoBundleExport): void => {
  const text = JSON.stringify(bundle)

  if (
    unsafeInvestorDemoRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (bundle.audience !== 'operator' && audienceUnsafeRefPattern.test(text))
  ) {
    throw new OmniInvestorDemoBundleUnsafe({
      reason:
        'Investor demo bundle projection contains private customer, provider, wallet, payment, trading, raw data, private repo, secret, raw timestamp, or audience-inappropriate material.',
    })
  }
}

export const projectOmniInvestorDemoBundleExport = (
  input: ProjectOmniInvestorDemoBundleInput,
  nowIso: string,
): OmniInvestorDemoBundleExport => {
  assertSafeTitle(input.title)
  assertSafeRefs('Investor demo bundle identity refs', [input.id])
  assertSafeRefs('Investor demo bundle proof refs', proofBundleRefs(input.proofBundles))
  assertSafeRefs(
    'Investor demo bundle route scorecard refs',
    routeScorecardRefs(input.routeScorecards),
  )

  const sourceRefs = redactRefsForAudience(input.audience, [
    ...(input.sourceRefs ?? []),
    ...proofBundleRefs(input.proofBundles),
    ...routeScorecardRefs(input.routeScorecards),
    ...economicsSourceRefs(input.economics),
    ...powerSourceRefs(input.powerProductivity),
    ...capacitySourceRefs(input.capacityFunnel),
  ])
  const caveatRefs = redactRefsForAudience(input.audience, [
    ...(input.caveatRefs ?? []),
    ...input.proofBundles.flatMap(bundle => [
      bundle.economicsCaveatRef,
      bundle.legalCaveatRef ?? '',
      bundle.privacyCaveatRef,
    ]),
    ...input.routeScorecards.map(route => route.publicCaveatRef),
    ...economicsCaveatRefs(input.economics),
    ...powerCaveatRefs(input.powerProductivity),
    ...capacityCaveatRefs(input.capacityFunnel),
  ])
  const proofBundles = input.proofBundles.map(bundle => ({
    ...bundle,
    artifactRefs: redactRefsForAudience(input.audience, bundle.artifactRefs),
    legalCaveatRef:
      bundle.legalCaveatRef === null
        ? null
        : redactRefsForAudience(input.audience, [bundle.legalCaveatRef])[0] ?? null,
    receiptRefs: redactRefsForAudience(input.audience, bundle.receiptRefs),
    sourceRefs: redactRefsForAudience(input.audience, bundle.sourceRefs),
  }))
  const missingEvidence = missingEvidenceForBundle(input)
  const readiness = readinessForBundle(input, missingEvidence)
  const bundle: OmniInvestorDemoBundleExport = {
    acceptedOutcomeClaimAllowed:
      input.economics.totals.acceptedRevenueClaimAllowed &&
      input.powerProductivity.totals.acceptedOutcomeCount > 0,
    audience: input.audience,
    authority: OMNI_INVESTOR_DEMO_BUNDLE_READ_ONLY_AUTHORITY,
    capacityFunnel: {
      acceptedCount: input.capacityFunnel.acceptedCount,
      darkCount: input.capacityFunnel.darkCount,
      darkReasonCount: input.capacityFunnel.darkReasonCount,
      freshCount: input.capacityFunnel.freshCount,
      paidButNotSettledCount: input.capacityFunnel.paidButNotSettledCount,
      paidCount: input.capacityFunnel.paidCount,
      settledCount: input.capacityFunnel.settledCount,
      settledWithoutVisibleReceiptCount:
        input.capacityFunnel.settledWithoutVisibleReceiptCount,
      staleCapacityRefs: redactRefsForAudience(
        input.audience,
        input.capacityFunnel.staleCapacityRefs,
      ),
      staleCount: input.capacityFunnel.staleCount,
      totalCount: input.capacityFunnel.totalCount,
      visibleSettlementClaimAllowedCount:
        input.capacityFunnel.visibleSettlementClaimAllowedCount,
    },
    caveatRefs,
    downloadRouteMutationAllowed: false,
    economics: {
      acceptedGrossProfitCents:
        input.economics.totals.acceptedGrossProfitCents,
      acceptedOutcomeCount: input.economics.totals.acceptedOutcomeCount,
      acceptedOutcomeRefs: redactRefsForAudience(
        input.audience,
        input.economics.totals.acceptedOutcomeRefs,
      ),
      acceptedRevenueCents: input.economics.totals.acceptedRevenueCents,
      acceptedRevenueClaimAllowed:
        input.economics.totals.acceptedRevenueClaimAllowed,
      blockerRefs: redactRefsForAudience(
        input.audience,
        input.economics.totals.blockerRefs,
      ),
      caveatRefs: redactRefsForAudience(
        input.audience,
        input.economics.totals.caveatRefs,
      ),
      economicsRefs: redactRefsForAudience(
        input.audience,
        input.economics.totals.economicsRefs,
      ),
      evidenceRefs: redactRefsForAudience(
        input.audience,
        input.economics.totals.evidenceRefs,
      ),
      grossMarginBps: input.economics.totals.grossMarginBps,
      modeledOnly: input.economics.totals.modeledOnly,
      providerPayableCents: input.economics.totals.providerPayableCents,
      providerSettlementClaimAllowed:
        input.economics.totals.providerSettlementClaimAllowed,
      providerSettlementRefs: redactRefsForAudience(
        input.audience,
        input.economics.totals.providerSettlementRefs,
      ),
      providerSettlementStateLabel:
        input.economics.totals.providerSettlementStateLabel,
      providerSettledCents: input.economics.totals.providerSettledCents,
      refundStateLabel: input.economics.totals.refundStateLabel,
      revenueRefs: redactRefsForAudience(
        input.audience,
        input.economics.totals.revenueRefs,
      ),
      revenueStateLabel: input.economics.totals.revenueStateLabel,
    },
    generatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      input.generatedAtIso,
      nowIso,
    ),
    id: input.id,
    investorShareMutationAllowed: false,
    liveWalletSpendAllowed: false,
    missingEvidence,
    powerProductivity: {
      acceptedOutcomeCount: input.powerProductivity.totals.acceptedOutcomeCount,
      acceptedOutcomesPerMwh:
        input.powerProductivity.totals.acceptedOutcomesPerMwh,
      acceptedRevenueCentsPerKwh:
        input.powerProductivity.totals.acceptedRevenueCentsPerKwh,
      darkCapacityMwh: input.powerProductivity.totals.darkCapacityMwh,
      energyMwh: input.powerProductivity.totals.energyMwh,
      measuredEnergyClaimAllowed:
        input.powerProductivity.totals.measuredEnergyClaimAllowed,
      powerDataStateLabel: input.powerProductivity.totals.powerDataStateLabel,
      providerPayableCentsPerKwh:
        input.powerProductivity.totals.providerPayableCentsPerKwh,
      settlementClaimAllowed:
        input.powerProductivity.totals.settlementClaimAllowed,
      settlementRefs: redactRefsForAudience(
        input.audience,
        input.powerProductivity.totals.settlementRefs,
      ),
      settlementStateLabel:
        input.powerProductivity.totals.settlementStateLabel,
    },
    proofBundles,
    publicClaimUpgradeAllowed: false,
    rawDataCopyAllowed: false,
    readiness,
    reviewBeforeSharing:
      readiness !== 'ready' ||
      proofBundles.some(bundle => bundle.noSettlementImplication),
    routeScorecards: input.routeScorecards,
    sections: bundleSections(input),
    settlementMutationAllowed: false,
    settlementStateLabel:
      input.economics.totals.providerSettlementStateLabel ===
        input.powerProductivity.totals.settlementStateLabel
        ? input.economics.totals.providerSettlementStateLabel
        : 'Mixed',
    sourceRefs,
    title: input.title.trim(),
  }

  assertBundleSafe(bundle)

  return bundle
}
