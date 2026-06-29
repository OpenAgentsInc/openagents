import { Schema as S } from 'effect'

import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const MarketplaceMarginMemoryReviewState = S.Literals([
  'draft',
  'promoted',
  'release_gate_ready',
  'reviewed',
  'unreviewed',
])
export type MarketplaceMarginMemoryReviewState =
  typeof MarketplaceMarginMemoryReviewState.Type

export const MarketplaceMarginMemorySettlementState = S.Literals([
  'accepted',
  'disputed',
  'modeled',
  'partially_settled',
  'payable',
  'refunded',
  'settled',
  'unknown',
])
export type MarketplaceMarginMemorySettlementState =
  typeof MarketplaceMarginMemorySettlementState.Type

export const MarketplaceMarginMemoryAuthorityBoundary = S.Literals([
  'evidence_only',
])
export type MarketplaceMarginMemoryAuthorityBoundary =
  typeof MarketplaceMarginMemoryAuthorityBoundary.Type

export class MarketplaceMarginMemoryAuthority extends S.Class<MarketplaceMarginMemoryAuthority>(
  'MarketplaceMarginMemoryAuthority',
)({
  authorityBoundary: MarketplaceMarginMemoryAuthorityBoundary,
  noAutomaticPublicRankMutation: S.Boolean,
  noModulePromotion: S.Boolean,
  noPayoutMutation: S.Boolean,
  noRoutingMutation: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class MarketplaceMarginMemoryRecord extends S.Class<MarketplaceMarginMemoryRecord>(
  'MarketplaceMarginMemoryRecord',
)({
  acceptedCount: S.Number,
  acceptedOutcomeRefs: S.Array(S.String),
  acceptedGrossProfitCents: S.Number,
  acceptedRevenueCents: S.Number,
  authority: MarketplaceMarginMemoryAuthority,
  capabilityRef: S.String,
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  grossMarginEvidenceRefs: S.Array(S.String),
  id: S.String,
  marketMemoryRef: S.String,
  modeledMarketplaceValueRefs: S.Array(S.String),
  moduleVersionRefs: S.Array(S.String),
  packageRefs: S.Array(S.String),
  programSignatureRefs: S.Array(S.String),
  providerRefs: S.Array(S.String),
  providerPayableCents: S.Number,
  refundCount: S.Number,
  refundedOutcomeRefs: S.Array(S.String),
  refundedRevenueCents: S.Number,
  rejectedCount: S.Number,
  rejectedOutcomeRefs: S.Array(S.String),
  repeatBuyerCount: S.Number,
  repeatBuyerSignalRefs: S.Array(S.String),
  retryCount: S.Number,
  retryEvidenceRefs: S.Array(S.String),
  revenueEvidenceRefs: S.Array(S.String),
  reviewBurdenRefs: S.Array(S.String),
  reviewBurdenScore: S.Number,
  reviewState: MarketplaceMarginMemoryReviewState,
  reviewerRefs: S.Array(S.String),
  routeRefs: S.Array(S.String),
  settledProviderCents: S.Number,
  settlementState: MarketplaceMarginMemorySettlementState,
  settlementStateRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  toolRefs: S.Array(S.String),
  totalBuyerCount: S.Number,
  updatedAtIso: S.String,
  workClassRefs: S.Array(S.String),
}) {}

export class MarketplaceMarginMemoryProjection extends S.Class<MarketplaceMarginMemoryProjection>(
  'MarketplaceMarginMemoryProjection',
)({
  acceptedCount: S.Number,
  acceptanceRateBps: S.Number,
  acceptedOutcomeClaimAllowed: S.Boolean,
  acceptedOutcomeRefs: S.Array(S.String),
  acceptedGrossProfitCents: S.Number,
  acceptedRevenueCents: S.Number,
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  authority: MarketplaceMarginMemoryAuthority,
  automaticPublicRankMutationAllowed: S.Boolean,
  capabilityRef: S.String,
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  grossMarginBps: S.Number,
  grossMarginClaimAllowed: S.Boolean,
  grossMarginEvidenceRefs: S.Array(S.String),
  id: S.String,
  marketMemoryRef: S.String,
  modeledMarketplaceValueClaimAllowed: S.Boolean,
  modeledMarketplaceValueRefs: S.Array(S.String),
  modulePromotionAllowed: S.Boolean,
  moduleVersionRefs: S.Array(S.String),
  packageRefs: S.Array(S.String),
  payoutMutationAllowed: S.Boolean,
  programSignatureRefs: S.Array(S.String),
  providerRefs: S.Array(S.String),
  providerPayableCents: S.Number,
  publicRankCandidateAllowed: S.Boolean,
  rankingScoreBps: S.Number,
  refundClaimAllowed: S.Boolean,
  refundCount: S.Number,
  refundedRevenueCents: S.Number,
  refundedOutcomeRefs: S.Array(S.String),
  refundRateBps: S.Number,
  rejectedCount: S.Number,
  rejectedOutcomeClaimAllowed: S.Boolean,
  rejectedOutcomeRefs: S.Array(S.String),
  repeatBuyerClaimAllowed: S.Boolean,
  repeatBuyerCount: S.Number,
  repeatBuyerRateBps: S.Number,
  repeatBuyerSignalRefs: S.Array(S.String),
  retryCount: S.Number,
  retryEvidenceRefs: S.Array(S.String),
  revenueClaimAllowed: S.Boolean,
  revenueEvidenceRefs: S.Array(S.String),
  reviewBurdenRefs: S.Array(S.String),
  reviewBurdenLabel: S.String,
  reviewBurdenScore: S.Number,
  reviewState: MarketplaceMarginMemoryReviewState,
  reviewerRefs: S.Array(S.String),
  routeRefs: S.Array(S.String),
  routingMutationAllowed: S.Boolean,
  settledProviderCents: S.Number,
  settlementClaimAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  settlementState: MarketplaceMarginMemorySettlementState,
  settlementStateLabel: S.String,
  settlementStateRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  toolRefs: S.Array(S.String),
  totalBuyerCount: S.Number,
  updatedAtDisplay: S.String,
  workClassRefs: S.Array(S.String),
}) {}

export class MarketplaceMarginMemoryUnsafe extends S.TaggedErrorClass<MarketplaceMarginMemoryUnsafe>()(
  'MarketplaceMarginMemoryUnsafe',
  {
    reason: S.String,
  },
) {}

export const MARKETPLACE_MARGIN_MEMORY_NO_AUTHORITY: MarketplaceMarginMemoryAuthority =
  {
    authorityBoundary: 'evidence_only',
    noAutomaticPublicRankMutation: true,
    noModulePromotion: true,
    noPayoutMutation: true,
    noRoutingMutation: true,
    noSettlementMutation: true,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeMarketplaceMemoryPattern =
  /(@|access[_-]?token|auth\.json|bearer|callback[_-]?token|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(email|invoice|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(provider\.private|reviewer\.private|revenue\.private|settlement\.private|source\.private)/i
const customerUnsafeRefPattern =
  /(provider\.private|reviewer\.private|revenue\.private|settlement\.private|source\.private)/i
const teamUnsafeRefPattern =
  /(provider\.private|settlement\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasRefs = (refs: ReadonlyArray<string>): boolean => refs.length > 0

const settlementStateLabelByState: Readonly<
  Record<MarketplaceMarginMemorySettlementState, string>
> = {
  accepted: 'Accepted',
  disputed: 'Disputed',
  modeled: 'Modeled',
  partially_settled: 'Partially settled',
  payable: 'Payable',
  refunded: 'Refunded',
  settled: 'Settled',
  unknown: 'Unknown',
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeMarketplaceMemoryPattern.test(ref) ||
    isoTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason: `${label} contains private customer data, raw source archives, tokens, provider payloads, wallet/payment material, payout targets, private repo refs, raw runner logs, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: BlueprintMissionBriefingAudience,
): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: BlueprintMissionBriefingAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const safeRefForAudience = (
  label: string,
  ref: string,
  audience: BlueprintMissionBriefingAudience,
): string =>
  safeRefsForAudience(label, [ref], audience)[0] ??
  `${label.replaceAll(' ', '_')}.redacted`

const assertNonNegativeInteger = (
  label: string,
  value: number,
): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

const assertRecordSafe = (record: MarketplaceMarginMemoryRecord): void => {
  if (
    marketplaceMarginMemoryAuthorityBoundaryUnsafe(record.authority) ||
    marketplaceMarginMemoryHasMutationAuthority(record)
  ) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason: 'Marketplace margin memory must remain evidence-only and cannot carry mutation authority.',
    })
  }

  assertSafeRefs('marketplace margin identity refs', [
    record.id,
    record.marketMemoryRef,
    record.capabilityRef,
  ])
  assertSafeRefs('marketplace margin accepted outcome refs', record.acceptedOutcomeRefs)
  assertSafeRefs('marketplace margin rejected outcome refs', record.rejectedOutcomeRefs)
  assertSafeRefs('marketplace margin refunded outcome refs', record.refundedOutcomeRefs)
  assertSafeRefs('marketplace margin retry evidence refs', record.retryEvidenceRefs)
  assertSafeRefs('marketplace margin review burden refs', record.reviewBurdenRefs)
  assertSafeRefs(
    'marketplace margin gross margin evidence refs',
    record.grossMarginEvidenceRefs,
  )
  assertSafeRefs(
    'marketplace margin modeled marketplace value refs',
    record.modeledMarketplaceValueRefs,
  )
  assertSafeRefs('marketplace margin revenue evidence refs', record.revenueEvidenceRefs)
  assertSafeRefs(
    'marketplace margin repeat buyer signal refs',
    record.repeatBuyerSignalRefs,
  )
  assertSafeRefs(
    'marketplace margin settlement state refs',
    record.settlementStateRefs,
  )
  assertSafeRefs('marketplace margin program signature refs', record.programSignatureRefs)
  assertSafeRefs('marketplace margin module version refs', record.moduleVersionRefs)
  assertSafeRefs('marketplace margin tool refs', record.toolRefs)
  assertSafeRefs('marketplace margin source refs', record.sourceRefs)
  assertSafeRefs('marketplace margin package refs', record.packageRefs)
  assertSafeRefs('marketplace margin provider refs', record.providerRefs)
  assertSafeRefs('marketplace margin reviewer refs', record.reviewerRefs)
  assertSafeRefs('marketplace margin route refs', record.routeRefs)
  assertSafeRefs('marketplace margin work class refs', record.workClassRefs)
  assertSafeRefs('marketplace margin caveat refs', record.caveatRefs)
  assertSafeRefs('marketplace margin evidence refs', record.evidenceRefs)

  assertNonNegativeInteger('acceptedCount', record.acceptedCount)
  assertNonNegativeInteger(
    'acceptedRevenueCents',
    record.acceptedRevenueCents,
  )
  assertNonNegativeInteger('rejectedCount', record.rejectedCount)
  assertNonNegativeInteger('refundCount', record.refundCount)
  assertNonNegativeInteger('refundedRevenueCents', record.refundedRevenueCents)
  assertNonNegativeInteger('repeatBuyerCount', record.repeatBuyerCount)
  assertNonNegativeInteger('retryCount', record.retryCount)
  assertNonNegativeInteger('providerPayableCents', record.providerPayableCents)
  assertNonNegativeInteger('settledProviderCents', record.settledProviderCents)
  assertNonNegativeInteger('totalBuyerCount', record.totalBuyerCount)

  if (record.reviewBurdenScore < 0) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason: 'reviewBurdenScore must be non-negative.',
    })
  }

  if (record.acceptedCount > 0 && !hasRefs(record.acceptedOutcomeRefs)) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason: 'Accepted outcome counts require accepted outcome refs.',
    })
  }

  if (record.rejectedCount > 0 && !hasRefs(record.rejectedOutcomeRefs)) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason: 'Rejected outcome counts require rejected outcome refs.',
    })
  }

  if (record.refundCount > 0 && !hasRefs(record.refundedOutcomeRefs)) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason: 'Refund counts require refunded outcome refs.',
    })
  }

  if (record.retryCount > 0 && !hasRefs(record.retryEvidenceRefs)) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason: 'Retry counts require retry evidence refs.',
    })
  }

  if (hasRefs(record.revenueEvidenceRefs) && !hasRefs(record.acceptedOutcomeRefs)) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason: 'Revenue evidence refs require accepted outcome refs.',
    })
  }

  if (
    record.acceptedRevenueCents > 0 &&
    (!hasRefs(record.revenueEvidenceRefs) ||
      !hasRefs(record.acceptedOutcomeRefs))
  ) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason:
        'Accepted revenue requires revenue evidence and accepted outcome refs.',
    })
  }

  if (
    record.acceptedGrossProfitCents !== 0 &&
    (!hasRefs(record.grossMarginEvidenceRefs) ||
      !hasRefs(record.revenueEvidenceRefs) ||
      !hasRefs(record.acceptedOutcomeRefs))
  ) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason:
        'Accepted gross profit requires gross margin evidence, revenue evidence, and accepted outcome refs.',
    })
  }

  if (
    hasRefs(record.grossMarginEvidenceRefs) &&
    (!hasRefs(record.revenueEvidenceRefs) ||
      !hasRefs(record.acceptedOutcomeRefs))
  ) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason:
        'Gross margin evidence refs require revenue evidence and accepted outcome refs.',
    })
  }

  if (
    record.providerPayableCents > 0 &&
    !['payable', 'partially_settled', 'settled'].includes(
      record.settlementState,
    )
  ) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason:
        'Provider payable cents require payable, partially settled, or settled settlement state.',
    })
  }

  if (
    record.settledProviderCents > 0 &&
    !['partially_settled', 'settled'].includes(record.settlementState)
  ) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason:
        'Settled provider cents require partially settled or settled settlement state.',
    })
  }

  if (record.settledProviderCents > record.providerPayableCents) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason: 'Settled provider cents cannot exceed provider payable cents.',
    })
  }

  if (
    record.settlementState === 'settled' &&
    (!hasRefs(record.settlementStateRefs) ||
      record.providerPayableCents === 0 ||
      record.settledProviderCents !== record.providerPayableCents)
  ) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason:
        'Settled marketplace margin memory requires settlement refs and fully settled provider payable cents.',
    })
  }

  if (
    record.refundedRevenueCents > 0 &&
    (!hasRefs(record.refundedOutcomeRefs) || record.refundCount === 0)
  ) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason:
        'Refunded revenue requires refund count and refunded outcome refs.',
    })
  }

  if (record.repeatBuyerCount > record.totalBuyerCount) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason: 'Repeat buyer count cannot exceed total buyer count.',
    })
  }

  if (
    record.repeatBuyerCount > 0 &&
    (!hasRefs(record.repeatBuyerSignalRefs) || record.totalBuyerCount === 0)
  ) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason:
        'Repeat buyer count requires repeat buyer signal refs and total buyers.',
    })
  }
}

export const marketplaceMarginMemoryHasMutationAuthority = (
  record: MarketplaceMarginMemoryRecord,
): boolean =>
  !record.authority.noAutomaticPublicRankMutation ||
  !record.authority.noModulePromotion ||
  !record.authority.noPayoutMutation ||
  !record.authority.noRoutingMutation ||
  !record.authority.noSettlementMutation

export const marketplaceMarginMemoryPublicRankCandidateAllowed = (
  record: MarketplaceMarginMemoryRecord,
): boolean =>
  !marketplaceMarginMemoryHasMutationAuthority(record) &&
  (record.reviewState === 'reviewed' ||
    record.reviewState === 'release_gate_ready' ||
    record.reviewState === 'promoted') &&
  record.acceptedCount > 0 &&
  hasRefs(record.acceptedOutcomeRefs) &&
  hasRefs(record.evidenceRefs)

const projectionText = (
  projection: MarketplaceMarginMemoryProjection,
): string => JSON.stringify(projection)

const ratioBps = (numerator: number, denominator: number): number =>
  denominator <= 0 ? 0 : Math.round((numerator / denominator) * 10_000)

const boundedRatioBps = (numerator: number, denominator: number): number =>
  Math.max(-10_000, Math.min(10_000, ratioBps(numerator, denominator)))

const reviewBurdenLabel = (score: number): string => {
  if (score <= 1) {
    return 'Low review burden'
  }

  if (score <= 3) {
    return 'Medium review burden'
  }

  return 'High review burden'
}

const rankingScoreBps = (
  acceptanceRateBps: number,
  grossMarginBps: number,
  refundRateBps: number,
  repeatBuyerRateBps: number,
  reviewBurdenScore: number,
): number =>
  Math.max(
    0,
    Math.min(
      10_000,
      Math.round(
        acceptanceRateBps * 0.35 +
          Math.max(0, grossMarginBps) * 0.3 +
          repeatBuyerRateBps * 0.15 -
          refundRateBps * 0.1 -
          reviewBurdenScore * 250,
      ),
    ),
  )

export const marketplaceMarginMemoryProjectionHasPrivateMaterial = (
  projection: MarketplaceMarginMemoryProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeMarketplaceMemoryPattern.test(text) ||
    isoTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectMarketplaceMarginMemory = (
  record: MarketplaceMarginMemoryRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): MarketplaceMarginMemoryProjection => {
  assertRecordSafe(record)

  const acceptanceRateBps = ratioBps(
    record.acceptedCount,
    record.acceptedCount + record.rejectedCount,
  )
  const grossMarginBps = boundedRatioBps(
    record.acceptedGrossProfitCents,
    record.acceptedRevenueCents,
  )
  const refundRateBps = ratioBps(record.refundCount, record.acceptedCount)
  const repeatBuyerRateBps = ratioBps(
    record.repeatBuyerCount,
    record.totalBuyerCount,
  )

  const projection: MarketplaceMarginMemoryProjection = {
    acceptanceRateBps,
    acceptedCount: record.acceptedCount,
    acceptedOutcomeClaimAllowed:
      record.acceptedCount > 0 && hasRefs(record.acceptedOutcomeRefs),
    acceptedOutcomeRefs: safeRefsForAudience(
      'marketplace margin accepted outcome refs',
      record.acceptedOutcomeRefs,
      audience,
    ),
    acceptedGrossProfitCents: record.acceptedGrossProfitCents,
    acceptedRevenueCents: record.acceptedRevenueCents,
    audience,
    authority: record.authority,
    automaticPublicRankMutationAllowed: false,
    capabilityRef: safeRefForAudience(
      'marketplace margin capability ref',
      record.capabilityRef,
      audience,
    ),
    caveatRefs: safeRefsForAudience(
      'marketplace margin caveat refs',
      record.caveatRefs,
      audience,
    ),
    evidenceRefs: safeRefsForAudience(
      'marketplace margin evidence refs',
      record.evidenceRefs,
      audience,
    ),
    grossMarginBps,
    grossMarginClaimAllowed:
      hasRefs(record.grossMarginEvidenceRefs) &&
      hasRefs(record.revenueEvidenceRefs) &&
      hasRefs(record.acceptedOutcomeRefs),
    grossMarginEvidenceRefs: safeRefsForAudience(
      'marketplace margin gross margin evidence refs',
      record.grossMarginEvidenceRefs,
      audience,
    ),
    id: safeRefForAudience('marketplace margin id', record.id, audience),
    marketMemoryRef: safeRefForAudience(
      'marketplace margin memory ref',
      record.marketMemoryRef,
      audience,
    ),
    modeledMarketplaceValueClaimAllowed:
      hasRefs(record.modeledMarketplaceValueRefs),
    modeledMarketplaceValueRefs: safeRefsForAudience(
      'marketplace margin modeled value refs',
      record.modeledMarketplaceValueRefs,
      audience,
    ),
    modulePromotionAllowed: false,
    moduleVersionRefs: safeRefsForAudience(
      'marketplace margin module version refs',
      record.moduleVersionRefs,
      audience,
    ),
    packageRefs: safeRefsForAudience(
      'marketplace margin package refs',
      record.packageRefs,
      audience,
    ),
    payoutMutationAllowed: false,
    programSignatureRefs: safeRefsForAudience(
      'marketplace margin program signature refs',
      record.programSignatureRefs,
      audience,
    ),
    providerRefs: safeRefsForAudience(
      'marketplace margin provider refs',
      record.providerRefs,
      audience,
    ),
    providerPayableCents: record.providerPayableCents,
    publicRankCandidateAllowed:
      marketplaceMarginMemoryPublicRankCandidateAllowed(record),
    rankingScoreBps: rankingScoreBps(
      acceptanceRateBps,
      grossMarginBps,
      refundRateBps,
      repeatBuyerRateBps,
      record.reviewBurdenScore,
    ),
    refundClaimAllowed:
      record.refundCount > 0 && hasRefs(record.refundedOutcomeRefs),
    refundCount: record.refundCount,
    refundedRevenueCents: record.refundedRevenueCents,
    refundedOutcomeRefs: safeRefsForAudience(
      'marketplace margin refunded outcome refs',
      record.refundedOutcomeRefs,
      audience,
    ),
    refundRateBps,
    rejectedCount: record.rejectedCount,
    rejectedOutcomeClaimAllowed:
      record.rejectedCount > 0 && hasRefs(record.rejectedOutcomeRefs),
    rejectedOutcomeRefs: safeRefsForAudience(
      'marketplace margin rejected outcome refs',
      record.rejectedOutcomeRefs,
      audience,
    ),
    repeatBuyerClaimAllowed: hasRefs(record.repeatBuyerSignalRefs),
    repeatBuyerCount: record.repeatBuyerCount,
    repeatBuyerRateBps,
    repeatBuyerSignalRefs: safeRefsForAudience(
      'marketplace margin repeat buyer refs',
      record.repeatBuyerSignalRefs,
      audience,
    ),
    retryCount: record.retryCount,
    retryEvidenceRefs: safeRefsForAudience(
      'marketplace margin retry evidence refs',
      record.retryEvidenceRefs,
      audience,
    ),
    revenueClaimAllowed:
      hasRefs(record.revenueEvidenceRefs) && hasRefs(record.acceptedOutcomeRefs),
    revenueEvidenceRefs: safeRefsForAudience(
      'marketplace margin revenue evidence refs',
      record.revenueEvidenceRefs,
      audience,
    ),
    reviewBurdenRefs: safeRefsForAudience(
      'marketplace margin review burden refs',
      record.reviewBurdenRefs,
      audience,
    ),
    reviewBurdenLabel: reviewBurdenLabel(record.reviewBurdenScore),
    reviewBurdenScore: record.reviewBurdenScore,
    reviewState: record.reviewState,
    reviewerRefs: safeRefsForAudience(
      'marketplace margin reviewer refs',
      record.reviewerRefs,
      audience,
    ),
    routeRefs: safeRefsForAudience(
      'marketplace margin route refs',
      record.routeRefs,
      audience,
    ),
    routingMutationAllowed: false,
    settledProviderCents: record.settledProviderCents,
    settlementClaimAllowed: hasRefs(record.settlementStateRefs),
    settlementMutationAllowed: false,
    settlementState: record.settlementState,
    settlementStateLabel: settlementStateLabelByState[record.settlementState],
    settlementStateRefs: safeRefsForAudience(
      'marketplace margin settlement refs',
      record.settlementStateRefs,
      audience,
    ),
    sourceRefs: safeRefsForAudience(
      'marketplace margin source refs',
      record.sourceRefs,
      audience,
    ),
    toolRefs: safeRefsForAudience(
      'marketplace margin tool refs',
      record.toolRefs,
      audience,
    ),
    totalBuyerCount: record.totalBuyerCount,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workClassRefs: safeRefsForAudience(
      'marketplace margin work class refs',
      record.workClassRefs,
      audience,
    ),
  }

  if (marketplaceMarginMemoryProjectionHasPrivateMaterial(projection)) {
    throw new MarketplaceMarginMemoryUnsafe({
      reason: 'Marketplace margin memory projection contains unsafe material.',
    })
  }

  return projection
}

const marketplaceMarginMemoryAuthorityBoundaryUnsafe = (
  authority: MarketplaceMarginMemoryAuthority,
): boolean =>
  authority.authorityBoundary !== 'evidence_only'

export const exampleMarketplaceMarginMemory = ():
  MarketplaceMarginMemoryRecord => ({
    acceptedCount: 3,
    acceptedOutcomeRefs: ['accepted.outcome.site_revision_4'],
    acceptedGrossProfitCents: 18_000,
    acceptedRevenueCents: 30_000,
    authority: MARKETPLACE_MARGIN_MEMORY_NO_AUTHORITY,
    capabilityRef: 'capability.autopilot_sites_revision',
    caveatRefs: ['caveat.margin_memory.evidence_only'],
    evidenceRefs: ['evidence.margin_memory.site_revision_4'],
    grossMarginEvidenceRefs: ['gross_margin.evidence.site_revision_4'],
    id: 'marketplace_margin_memory.site_revision_4',
    marketMemoryRef: 'market_memory.autopilot_sites_revision',
    modeledMarketplaceValueRefs: ['modeled.marketplace_value.site_revision'],
    moduleVersionRefs: ['module_version.autopilot.site_revision.v1'],
    packageRefs: ['developer_package.autopilot_sites.review'],
    programSignatureRefs: ['program_signature.autopilot_sites.review'],
    providerRefs: ['provider.public.openagents_runner'],
    providerPayableCents: 6_000,
    refundCount: 0,
    refundedOutcomeRefs: [],
    refundedRevenueCents: 0,
    rejectedCount: 1,
    rejectedOutcomeRefs: ['rejected.outcome.site_revision_attempt_2'],
    repeatBuyerCount: 2,
    repeatBuyerSignalRefs: ['repeat_buyer.signal.site_customer_returned'],
    retryCount: 2,
    retryEvidenceRefs: ['retry.evidence.site_revision_4'],
    revenueEvidenceRefs: ['revenue.evidence.site_revision_4'],
    reviewBurdenRefs: ['review_burden.operator_qa.site_revision_4'],
    reviewBurdenScore: 2,
    reviewState: 'reviewed',
    reviewerRefs: ['reviewer.public.openagents_operator'],
    routeRefs: ['route.scorecard.site_revision_4'],
    settledProviderCents: 0,
    settlementState: 'payable',
    settlementStateRefs: ['settlement.state.pending_payout_receipt'],
    sourceRefs: ['source.exa.site_revision_4'],
    toolRefs: ['tool.site_builder.static_export'],
    totalBuyerCount: 3,
    updatedAtIso: '2026-06-06T22:40:00.000Z',
    workClassRefs: ['work_class.autopilot_sites_review_build'],
  })
