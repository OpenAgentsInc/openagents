import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

export const SiteReferralRewardGateState = S.Literals([
  'attribution_only',
  'blocked_by_policy',
  'payout_pending',
  'settled',
])
export type SiteReferralRewardGateState =
  typeof SiteReferralRewardGateState.Type

export const SiteReferralRewardGate = S.Struct({
  attributionCaptured: S.Boolean,
  bitcoinWithdrawalCopyAllowed: S.Boolean,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  paidActivityRefs: S.Array(S.String),
  payoutPending: S.Boolean,
  publicCopyRefs: S.Array(S.String),
  rewardEligible: S.Boolean,
  settlementReceiptRefs: S.Array(S.String),
  settled: S.Boolean,
  state: SiteReferralRewardGateState,
})
export type SiteReferralRewardGate = typeof SiteReferralRewardGate.Type

export type SiteReferralRewardGateInput = Readonly<{
  attributionRefs: ReadonlyArray<string>
  paidActivityRefs: ReadonlyArray<string>
  policyBlockerRefs: ReadonlyArray<string>
  settlementReceiptRefs: ReadonlyArray<string>
}>

export class SiteReferralRewardGateUnsafe extends S.TaggedErrorClass<SiteReferralRewardGateUnsafe>()(
  'SiteReferralRewardGateUnsafe',
  {
    reason: S.String,
  },
) {}

const decodeRewardGate = S.decodeUnknownSync(SiteReferralRewardGate)

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const RAW_MATERIAL_PATTERN =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|balance[._-]?sats|bearer|bolt11|bolt12|checkout[_-]?secret|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|id|preimage|proof=|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|invoice|payment|payload|payout|target|webhook)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[._-]?(config|key|material|mnemonic|payment|preimage|secret|seed|state)|webhook[_-]?secret)/i
const RAW_TIMESTAMP_PATTERN = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const refIsSafe = (value: string): boolean =>
  SAFE_REF_PATTERN.test(value) &&
  !containsProviderSecretMaterial(value) &&
  !RAW_MATERIAL_PATTERN.test(value) &&
  !RAW_TIMESTAMP_PATTERN.test(value)

const safeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const cleaned = uniqueRefs(refs)

  for (const ref of cleaned) {
    if (!refIsSafe(ref)) {
      throw new SiteReferralRewardGateUnsafe({
        reason: `${label} must be public-safe refs without raw signup, customer, payment, wallet, payout, provider, secret, or timestamp material.`,
      })
    }
  }

  return cleaned
}

export const projectSiteReferralRewardGate = (
  input: SiteReferralRewardGateInput,
): SiteReferralRewardGate => {
  const attributionRefs = safeRefs(
    'Site referral attribution refs',
    input.attributionRefs,
  )
  const paidActivityRefs = safeRefs(
    'Site referral paid activity refs',
    input.paidActivityRefs,
  )
  const policyBlockerRefs = safeRefs(
    'Site referral policy blocker refs',
    input.policyBlockerRefs,
  )
  const settlementReceiptRefs = safeRefs(
    'Site referral settlement receipt refs',
    input.settlementReceiptRefs,
  )
  const attributionCaptured = attributionRefs.length > 0
  const hasPaidActivity = paidActivityRefs.length > 0
  const blocked = policyBlockerRefs.length > 0
  const settled = settlementReceiptRefs.length > 0 && !blocked
  const rewardEligible = attributionCaptured && hasPaidActivity && !blocked
  const payoutPending = rewardEligible && !settled
  const state: SiteReferralRewardGateState = blocked
    ? 'blocked_by_policy'
    : settled
      ? 'settled'
      : rewardEligible
        ? 'payout_pending'
        : 'attribution_only'

  return decodeRewardGate({
    attributionCaptured,
    bitcoinWithdrawalCopyAllowed: settled,
    blockerRefs: policyBlockerRefs,
    caveatRefs: [
      'caveat.public.site_referral.attribution_is_not_reward_eligibility',
      'caveat.public.site_referral.credits_do_not_create_bitcoin_liability',
      'caveat.public.site_referral.settlement_receipts_required_for_bitcoin_copy',
    ],
    paidActivityRefs,
    payoutPending,
    publicCopyRefs: settled
      ? ['copy.public.site_referral.bitcoin_settlement_receipts_visible']
      : ['copy.public.site_referral.bitcoin_stream_claim_blocked'],
    rewardEligible,
    settlementReceiptRefs,
    settled,
    state,
  })
}

export const siteReferralRewardGateHasPrivateMaterial = (
  gate: SiteReferralRewardGate,
): boolean => {
  const serialized = JSON.stringify(gate)

  return (
    containsProviderSecretMaterial(serialized) ||
    RAW_MATERIAL_PATTERN.test(serialized) ||
    RAW_TIMESTAMP_PATTERN.test(serialized)
  )
}
