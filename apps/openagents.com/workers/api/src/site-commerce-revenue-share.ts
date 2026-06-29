import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  type AssetBoundaryAsset,
  validateAssetBoundary as validateSharedAssetBoundary,
} from './asset-bitcoin-boundary'

export const SiteCommerceRevenueAsset = S.Literals(['credits', 'sats', 'usd'])
export type SiteCommerceRevenueAsset = typeof SiteCommerceRevenueAsset.Type

// Map this projection's asset vocabulary (credits/sats/usd) onto the shared
// credit<->Bitcoin boundary asset vocabulary so the boundary stays the single
// source of truth (RL-3 #5460).
const toBoundaryAsset = (asset: SiteCommerceRevenueAsset): AssetBoundaryAsset =>
  asset === 'sats' ? 'bitcoin' : asset === 'usd' ? 'usd' : 'credit'

export const SiteCommerceRevenueEventKind = S.Literals([
  'signup_attributed',
  'checkout_paid',
  'l402_redeemed',
  'credit_spent',
  'accepted_work_closed',
  'refund_or_reversal',
])
export type SiteCommerceRevenueEventKind =
  typeof SiteCommerceRevenueEventKind.Type

export const SiteCommerceRevenueReceiptRefs = S.Struct({
  nexusReceiptRef: S.optionalKey(S.String),
  treasuryReceiptRef: S.optionalKey(S.String),
  ldkSettlementReceiptRef: S.optionalKey(S.String),
})
export type SiteCommerceRevenueReceiptRefs =
  typeof SiteCommerceRevenueReceiptRefs.Type

export const SiteCommerceRevenueLinkageInput = S.Struct({
  siteId: S.String,
  siteVersionId: S.optionalKey(S.String),
  softwareOrderId: S.optionalKey(S.String),
  productId: S.optionalKey(S.String),
  paidActionId: S.optionalKey(S.String),
  customerRef: S.optionalKey(S.String),
  referralSourceRef: S.optionalKey(S.String),
  acceptedWorkRef: S.optionalKey(S.String),
  paymentEvidenceRef: S.optionalKey(S.String),
  entitlementRef: S.optionalKey(S.String),
  publicReceiptRef: S.String,
  eventKind: SiteCommerceRevenueEventKind,
  amount: S.Number,
  asset: SiteCommerceRevenueAsset,
  requestedContributorAsset: SiteCommerceRevenueAsset,
  providerPayoutClaimed: S.Boolean,
  receiptRefs: SiteCommerceRevenueReceiptRefs,
})
export type SiteCommerceRevenueLinkageInput =
  typeof SiteCommerceRevenueLinkageInput.Type

export type SiteCommerceRevenueProjection = Readonly<{
  acceptedWork: Readonly<{
    ref: string | null
    status: 'absent' | 'present'
  }>
  entitlement: Readonly<{
    ref: string | null
    status: 'absent' | 'present'
  }>
  paymentEvidence: Readonly<{
    asset: SiteCommerceRevenueAsset
    ref: string | null
    status: 'absent' | 'present'
  }>
  providerPayoutEligibility: Readonly<{
    reason: string
    status: 'not_eligible' | 'eligible_pending_settlement_refs'
  }>
  publicReceiptRef: string
  referralAttribution: Readonly<{
    ref: string | null
    rewardTrigger: 'none' | 'paid_activity'
  }>
  settlement: Readonly<{
    asset: SiteCommerceRevenueAsset
    withdrawalPosture:
      | 'bitcoin_withdrawable_after_settlement'
      | 'internal_credit_only'
      | 'fiat_or_credit_policy_required'
  }>
}>

export class SiteCommerceRevenueLinkageError extends S.TaggedErrorClass<SiteCommerceRevenueLinkageError>()(
  'SiteCommerceRevenueLinkageError',
  {
    reason: S.String,
  },
) {}

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/
const PROHIBITED_VALUE_PATTERN =
  /\b(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|payment_preimage|mdk_access_token|wallet_secret|private_key|webhook_secret)/i

const optionalRefs = (
  input: SiteCommerceRevenueLinkageInput,
): ReadonlyArray<readonly [string, string | undefined]> => [
  ['siteId', input.siteId],
  ['siteVersionId', input.siteVersionId],
  ['softwareOrderId', input.softwareOrderId],
  ['productId', input.productId],
  ['paidActionId', input.paidActionId],
  ['customerRef', input.customerRef],
  ['referralSourceRef', input.referralSourceRef],
  ['acceptedWorkRef', input.acceptedWorkRef],
  ['paymentEvidenceRef', input.paymentEvidenceRef],
  ['entitlementRef', input.entitlementRef],
  ['publicReceiptRef', input.publicReceiptRef],
  ['nexusReceiptRef', input.receiptRefs.nexusReceiptRef],
  ['treasuryReceiptRef', input.receiptRefs.treasuryReceiptRef],
  ['ldkSettlementReceiptRef', input.receiptRefs.ldkSettlementReceiptRef],
]

const validateSafeRefs = (
  input: SiteCommerceRevenueLinkageInput,
): SiteCommerceRevenueLinkageError | undefined => {
  for (const [field, value] of optionalRefs(input)) {
    if (value === undefined) {
      continue
    }

    if (
      !SAFE_REF_PATTERN.test(value) ||
      containsProviderSecretMaterial(value) ||
      PROHIBITED_VALUE_PATTERN.test(value)
    ) {
      return new SiteCommerceRevenueLinkageError({
        reason: `${field} must be a public-safe receipt or entity ref.`,
      })
    }
  }

  return undefined
}

const validateAmount = (
  input: SiteCommerceRevenueLinkageInput,
): SiteCommerceRevenueLinkageError | undefined =>
  Number.isFinite(input.amount) && input.amount >= 0
    ? undefined
    : new SiteCommerceRevenueLinkageError({
        reason: 'amount must be finite and non-negative.',
      })

const validateSignupBoundary = (
  input: SiteCommerceRevenueLinkageInput,
): SiteCommerceRevenueLinkageError | undefined =>
  input.eventKind === 'signup_attributed' &&
  (input.amount > 0 || input.providerPayoutClaimed)
    ? new SiteCommerceRevenueLinkageError({
        reason:
          'signup attribution may record referral source only; it does not create payout eligibility.',
      })
    : undefined

const validateAssetBoundary = (
  input: SiteCommerceRevenueLinkageInput,
): SiteCommerceRevenueLinkageError | undefined => {
  // Delegate to the SHARED credit<->Bitcoin boundary guard (RL-3 #5460) so this
  // projection enforces the identical invariant as the live referral-dispatch
  // and firm-up-settlement paths: only Bitcoin (sats) revenue may fund a
  // withdrawable Bitcoin (sats) contributor share.
  const violation = validateSharedAssetBoundary({
    contributorAsset: toBoundaryAsset(input.requestedContributorAsset),
    movement: 'spend',
    revenueAsset: toBoundaryAsset(input.asset),
  })

  return violation === null
    ? undefined
    : new SiteCommerceRevenueLinkageError({
        reason:
          'credit spend may not silently create immediate Bitcoin withdrawal liability.',
      })
}

const validatePylonReceiptBoundary = (
  input: SiteCommerceRevenueLinkageInput,
): SiteCommerceRevenueLinkageError | undefined => {
  if (!input.providerPayoutClaimed) {
    return undefined
  }

  if (input.acceptedWorkRef === undefined) {
    return new SiteCommerceRevenueLinkageError({
      reason:
        'provider payout eligibility requires an accepted-work ref, not payment evidence alone.',
    })
  }

  const { ldkSettlementReceiptRef, nexusReceiptRef, treasuryReceiptRef } =
    input.receiptRefs

  return nexusReceiptRef !== undefined &&
    treasuryReceiptRef !== undefined &&
    ldkSettlementReceiptRef !== undefined
    ? undefined
    : new SiteCommerceRevenueLinkageError({
        reason:
          'Pylon accepted-work payout claims require Nexus, Treasury, and LDK settlement receipt refs.',
      })
}

const paymentEvidenceStatus = (
  input: SiteCommerceRevenueLinkageInput,
): 'absent' | 'present' =>
  input.paymentEvidenceRef === undefined ? 'absent' : 'present'

const rewardTrigger = (
  input: SiteCommerceRevenueLinkageInput,
): 'none' | 'paid_activity' =>
  input.referralSourceRef !== undefined &&
  input.eventKind !== 'signup_attributed' &&
  input.amount > 0
    ? 'paid_activity'
    : 'none'

const withdrawalPosture = (
  asset: SiteCommerceRevenueAsset,
): SiteCommerceRevenueProjection['settlement']['withdrawalPosture'] => {
  if (asset === 'sats') {
    return 'bitcoin_withdrawable_after_settlement'
  }

  if (asset === 'credits') {
    return 'internal_credit_only'
  }

  return 'fiat_or_credit_policy_required'
}

export const deriveSiteCommerceRevenueProjection = (
  input: SiteCommerceRevenueLinkageInput,
): SiteCommerceRevenueProjection => {
  const validationError =
    validateSafeRefs(input) ??
    validateAmount(input) ??
    validateSignupBoundary(input) ??
    validateAssetBoundary(input) ??
    validatePylonReceiptBoundary(input)

  if (validationError !== undefined) {
    throw validationError
  }

  return {
    acceptedWork: {
      ref: input.acceptedWorkRef ?? null,
      status: input.acceptedWorkRef === undefined ? 'absent' : 'present',
    },
    entitlement: {
      ref: input.entitlementRef ?? null,
      status: input.entitlementRef === undefined ? 'absent' : 'present',
    },
    paymentEvidence: {
      asset: input.asset,
      ref: input.paymentEvidenceRef ?? null,
      status: paymentEvidenceStatus(input),
    },
    providerPayoutEligibility: input.providerPayoutClaimed
      ? {
          reason:
            'Accepted-work and Nexus/Treasury/LDK receipt refs are present.',
          status: 'eligible_pending_settlement_refs',
        }
      : {
          reason:
            'No provider payout claim is backed by the current linkage event.',
          status: 'not_eligible',
        },
    publicReceiptRef: input.publicReceiptRef,
    referralAttribution: {
      ref: input.referralSourceRef ?? null,
      rewardTrigger: rewardTrigger(input),
    },
    settlement: {
      asset: input.requestedContributorAsset,
      withdrawalPosture: withdrawalPosture(input.requestedContributorAsset),
    },
  }
}
