import {
  validateAssetBoundary,
  type AssetBoundaryAsset,
} from './asset-bitcoin-boundary'
import {
  recordBusinessFunnelEvent,
  type BusinessFunnelEventRecord,
} from './business-funnel-dashboard'
import {
  createReferralPayoutEligibility,
  type CreateReferralPayoutEligibilityInput,
  type SiteReferralPayoutLedgerEntry,
  type SiteReferralPayoutLedgerStorageError,
  type SiteReferralPayoutLedgerValidationError,
} from './site-referral-payout-ledger'
import {
  referralRevenueAssetToBoundaryAsset,
  type SiteReferralRevenueAsset,
} from './site-referral-payout-feed'

export type BusinessReferralEngagementInput = Readonly<{
  businessSignupId: string
  idempotencyKey: string
  nowIso: string
  periodKey: string
  qualifyingAmountSats: number
  qualifyingEventKind: string
  qualifyingEventRef: string
  revenueAsset: SiteReferralRevenueAsset
  referredUserId?: string | null
}>

export type BusinessReferralEngagementResult =
  | Readonly<{ _tag: 'no_attribution' }>
  | Readonly<{ _tag: 'self_attribution' }>
  | Readonly<{
      _tag: 'boundary_refused'
      reasonRef: string
    }>
  | Readonly<{
      _tag: 'recorded'
      funnelEvent: BusinessFunnelEventRecord
      payout: SiteReferralPayoutLedgerEntry
    }>

export type BusinessReferralEngagementError =
  | SiteReferralPayoutLedgerStorageError
  | SiteReferralPayoutLedgerValidationError

type BusinessSignupAttributionRow = Readonly<{
  referral_attribution_id: string
  referral_invite_id: string | null
  referral_source_id: string
  referrer_user_id: string
}>

const SAFE_SIGNUP_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/

const readBusinessSignupAttribution = async (
  db: D1Database,
  businessSignupId: string,
): Promise<BusinessSignupAttributionRow | null> => {
  if (!SAFE_SIGNUP_ID_PATTERN.test(businessSignupId)) {
    return null
  }

  return db
    .prepare(
      `SELECT bsra.referral_attribution_id AS referral_attribution_id,
              bsra.referral_invite_id AS referral_invite_id,
              bsra.referral_source_id AS referral_source_id,
              src.referrer_user_id AS referrer_user_id
         FROM business_signup_referral_attributions AS bsra
         JOIN site_referral_sources AS src
           ON src.id = bsra.referral_source_id
        WHERE bsra.business_signup_request_id = ?
          AND bsra.archived_at IS NULL
          AND bsra.policy_state = 'active'
          AND src.archived_at IS NULL
          AND src.policy_state = 'active'
        LIMIT 1`,
    )
    .bind(businessSignupId)
    .first<BusinessSignupAttributionRow>()
}

const revshareContributorAssetFor = (
  revenueAsset: SiteReferralRevenueAsset,
): AssetBoundaryAsset => (revenueAsset === 'bitcoin' ? 'bitcoin' : 'credit')

export const recordBusinessReferralEngagement = async (
  db: D1Database,
  input: BusinessReferralEngagementInput,
): Promise<BusinessReferralEngagementResult> => {
  const attribution = await readBusinessSignupAttribution(
    db,
    input.businessSignupId,
  )

  if (attribution === null) {
    return { _tag: 'no_attribution' }
  }

  if (
    input.referredUserId !== undefined &&
    input.referredUserId !== null &&
    input.referredUserId === attribution.referrer_user_id
  ) {
    return { _tag: 'self_attribution' }
  }

  const boundaryViolation = validateAssetBoundary({
    contributorAsset: revshareContributorAssetFor(input.revenueAsset),
    movement: 'revshare',
    revenueAsset: referralRevenueAssetToBoundaryAsset(input.revenueAsset),
  })

  if (boundaryViolation !== null) {
    return { _tag: 'boundary_refused', reasonRef: boundaryViolation.reasonRef }
  }

  const createInput: CreateReferralPayoutEligibilityInput = {
    idempotencyKey: input.idempotencyKey,
    nowIso: input.nowIso,
    periodKey: input.periodKey,
    qualifyingAmountSats: input.qualifyingAmountSats,
    qualifyingEventKind: input.qualifyingEventKind,
    qualifyingEventRef: input.qualifyingEventRef,
    referredUserId: input.referredUserId ?? null,
    referralAttributionId: attribution.referral_attribution_id,
    referralInviteId: attribution.referral_invite_id,
    referralSourceId: attribution.referral_source_id,
    referrerUserId: attribution.referrer_user_id,
  }
  const payout = await createReferralPayoutEligibility(db, createInput)
  const funnelEvent = await recordBusinessFunnelEvent(db, {
    eventRef: `business_referral_engagement:${input.qualifyingEventRef}`,
    occurredAt: input.nowIso,
    sourceKind: 'referral',
    sourceRef: attribution.referral_source_id,
    stage: 'referred_engagement',
  })

  return { _tag: 'recorded', funnelEvent, payout }
}
