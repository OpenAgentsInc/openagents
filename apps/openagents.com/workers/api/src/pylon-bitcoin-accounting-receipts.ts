import { Schema as S } from 'effect'

import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const PylonBitcoinAccountingState = S.Literals([
  'accepted_work_reward_intent',
  'buyer_payment_evidence',
  'payout_confirmed',
  'payout_dispatched',
  'payout_eligible',
  'payout_verified',
  'settled',
])
export type PylonBitcoinAccountingState =
  typeof PylonBitcoinAccountingState.Type

export const PylonBitcoinAccountingProviderVisibility = S.Literals([
  'private',
  'public',
])
export type PylonBitcoinAccountingProviderVisibility =
  typeof PylonBitcoinAccountingProviderVisibility.Type

export class PylonBitcoinAccountingReceiptRecord extends S.Class<PylonBitcoinAccountingReceiptRecord>(
  'PylonBitcoinAccountingReceiptRecord',
)({
  acceptedWorkRefs: S.Array(S.String),
  amountReceiptRefs: S.Array(S.String),
  bitcoinAmountSats: S.Number,
  buyerPaymentEvidenceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  jobRef: S.String,
  payoutConfirmationRefs: S.Array(S.String),
  payoutDispatchRefs: S.Array(S.String),
  payoutEligibilityRefs: S.Array(S.String),
  payoutVerificationRefs: S.Array(S.String),
  providerRef: S.String,
  providerVisibility: PylonBitcoinAccountingProviderVisibility,
  receiptRef: S.String,
  rewardIntentRefs: S.Array(S.String),
  settlementRefs: S.Array(S.String),
  state: PylonBitcoinAccountingState,
  updatedAtIso: S.String,
}) {}

export class PylonBitcoinAccountingReceiptProjection extends S.Class<PylonBitcoinAccountingReceiptProjection>(
  'PylonBitcoinAccountingReceiptProjection',
)({
  acceptedWorkRefs: S.Array(S.String),
  amountReceiptRefs: S.Array(S.String),
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  bitcoinAmountDisplay: S.NullOr(S.String),
  bitcoinAmountSats: S.NullOr(S.Number),
  buyerPaymentEvidencePresent: S.Boolean,
  buyerPaymentEvidenceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  jobRef: S.String,
  payoutConfirmationClaimAllowed: S.Boolean,
  payoutConfirmationRefs: S.Array(S.String),
  payoutDispatchClaimAllowed: S.Boolean,
  payoutDispatchRefs: S.Array(S.String),
  payoutEligibilityClaimAllowed: S.Boolean,
  payoutEligibilityRefs: S.Array(S.String),
  payoutVerificationClaimAllowed: S.Boolean,
  payoutVerificationRefs: S.Array(S.String),
  providerRef: S.String,
  providerVisibility: PylonBitcoinAccountingProviderVisibility,
  receiptRef: S.String,
  rewardIntentClaimAllowed: S.Boolean,
  rewardIntentRefs: S.Array(S.String),
  settlementClaimAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  state: PylonBitcoinAccountingState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
}) {}

export class PylonBitcoinAccountingReceiptUnsafe extends S.TaggedErrorClass<PylonBitcoinAccountingReceiptUnsafe>()(
  'PylonBitcoinAccountingReceiptUnsafe',
  {
    reason: S.String,
  },
) {}

const stateRank: Record<PylonBitcoinAccountingState, number> = {
  accepted_work_reward_intent: 1,
  buyer_payment_evidence: 0,
  payout_confirmed: 4,
  payout_dispatched: 3,
  payout_eligible: 2,
  payout_verified: 5,
  settled: 6,
}

const stateLabelByState: Record<PylonBitcoinAccountingState, string> = {
  accepted_work_reward_intent: 'Accepted-work reward intent',
  buyer_payment_evidence: 'Buyer payment evidence',
  payout_confirmed: 'Payout confirmed',
  payout_dispatched: 'Payout dispatched',
  payout_eligible: 'Payout eligible',
  payout_verified: 'Payout verified',
  settled: 'Settled',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const universallyUnsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(buyer[_-]?payment|provider\.private|settlement\.private)/i
const customerUnsafeRefPattern =
  /(buyer[_-]?payment|provider\.private|settlement\.private)/i
const teamUnsafeRefPattern =
  /(provider\.private|settlement\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const stateAtLeast = (
  state: PylonBitcoinAccountingState,
  threshold: PylonBitcoinAccountingState,
): boolean => stateRank[state] >= stateRank[threshold]

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    universallyUnsafeRefPattern.test(ref) ||
    isoTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonBitcoinAccountingReceiptUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, payout target, invoice, preimage, customer, private repo, or raw timestamp material.`,
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

const providerRefForAudience = (
  record: PylonBitcoinAccountingReceiptRecord,
  audience: BlueprintMissionBriefingAudience,
): string => {
  if (record.providerVisibility === 'public' || audience === 'operator') {
    return safeRefsForAudience('bitcoin accounting provider ref', [
      record.providerRef,
    ], audience)[0] ?? 'provider.redacted'
  }

  return 'provider.redacted'
}

const bitcoinDisplay = (amountSats: number): string => {
  const whole = Math.trunc(amountSats / 100_000_000)
  const fractional = Math.abs(amountSats % 100_000_000)
    .toString()
    .padStart(8, '0')
  const sats = amountSats.toLocaleString('en-US')

  return `${whole}.${fractional} bitcoin (${sats} sats)`
}

const assertRecordSafe = (
  record: PylonBitcoinAccountingReceiptRecord,
): void => {
  assertSafeRefs('bitcoin accounting identity refs', [
    record.id,
    record.receiptRef,
    record.jobRef,
    record.providerRef,
  ])
  assertSafeRefs('bitcoin accounting accepted-work refs', record.acceptedWorkRefs)
  assertSafeRefs('bitcoin accounting amount receipt refs', record.amountReceiptRefs)
  assertSafeRefs(
    'bitcoin accounting buyer payment evidence refs',
    record.buyerPaymentEvidenceRefs,
  )
  assertSafeRefs('bitcoin accounting reward intent refs', record.rewardIntentRefs)
  assertSafeRefs(
    'bitcoin accounting payout eligibility refs',
    record.payoutEligibilityRefs,
  )
  assertSafeRefs(
    'bitcoin accounting payout dispatch refs',
    record.payoutDispatchRefs,
  )
  assertSafeRefs(
    'bitcoin accounting payout confirmation refs',
    record.payoutConfirmationRefs,
  )
  assertSafeRefs(
    'bitcoin accounting payout verification refs',
    record.payoutVerificationRefs,
  )
  assertSafeRefs('bitcoin accounting settlement refs', record.settlementRefs)
  assertSafeRefs('bitcoin accounting caveat refs', record.caveatRefs)
  assertSafeRefs('bitcoin accounting evidence refs', record.evidenceRefs)

  if (!Number.isInteger(record.bitcoinAmountSats) || record.bitcoinAmountSats < 0) {
    throw new PylonBitcoinAccountingReceiptUnsafe({
      reason: 'Bitcoin accounting amount must be a non-negative integer sats value.',
    })
  }

  if (
    stateAtLeast(record.state, 'accepted_work_reward_intent') &&
    record.rewardIntentRefs.length === 0
  ) {
    throw new PylonBitcoinAccountingReceiptUnsafe({
      reason: 'Accepted-work reward intent requires reward intent refs.',
    })
  }

  if (
    stateAtLeast(record.state, 'payout_eligible') &&
    record.payoutEligibilityRefs.length === 0
  ) {
    throw new PylonBitcoinAccountingReceiptUnsafe({
      reason: 'Payout eligibility requires payout eligibility refs.',
    })
  }

  if (
    stateAtLeast(record.state, 'payout_dispatched') &&
    record.payoutDispatchRefs.length === 0
  ) {
    throw new PylonBitcoinAccountingReceiptUnsafe({
      reason: 'Payout dispatch requires payout dispatch refs.',
    })
  }

  if (
    stateAtLeast(record.state, 'payout_confirmed') &&
    record.payoutConfirmationRefs.length === 0
  ) {
    throw new PylonBitcoinAccountingReceiptUnsafe({
      reason: 'Payout confirmation requires payout confirmation refs.',
    })
  }

  if (
    stateAtLeast(record.state, 'payout_verified') &&
    record.payoutVerificationRefs.length === 0
  ) {
    throw new PylonBitcoinAccountingReceiptUnsafe({
      reason: 'Payout verification requires payout verification refs.',
    })
  }

  if (record.state === 'settled' && record.settlementRefs.length === 0) {
    throw new PylonBitcoinAccountingReceiptUnsafe({
      reason: 'Settlement requires settlement refs.',
    })
  }
}

const projectionText = (
  projection: PylonBitcoinAccountingReceiptProjection,
): string =>
  [
    projection.id,
    projection.receiptRef,
    projection.jobRef,
    projection.providerRef,
    ...projection.acceptedWorkRefs,
    ...projection.amountReceiptRefs,
    ...projection.buyerPaymentEvidenceRefs,
    ...projection.rewardIntentRefs,
    ...projection.payoutEligibilityRefs,
    ...projection.payoutDispatchRefs,
    ...projection.payoutConfirmationRefs,
    ...projection.payoutVerificationRefs,
    ...projection.settlementRefs,
    ...projection.caveatRefs,
    ...projection.evidenceRefs,
  ].join(' ')

export const pylonBitcoinAccountingProjectionHasPrivateMaterial = (
  projection: PylonBitcoinAccountingReceiptProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return universallyUnsafeRefPattern.test(text) ||
    isoTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonBitcoinAccountingReceipt = (
  record: PylonBitcoinAccountingReceiptRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): PylonBitcoinAccountingReceiptProjection => {
  assertRecordSafe(record)

  const amountVisible = record.amountReceiptRefs.length > 0
  const projection: PylonBitcoinAccountingReceiptProjection = {
    acceptedWorkRefs: safeRefsForAudience(
      'bitcoin accounting accepted-work refs',
      record.acceptedWorkRefs,
      audience,
    ),
    amountReceiptRefs: safeRefsForAudience(
      'bitcoin accounting amount receipt refs',
      record.amountReceiptRefs,
      audience,
    ),
    audience,
    bitcoinAmountDisplay: amountVisible
      ? bitcoinDisplay(record.bitcoinAmountSats)
      : null,
    bitcoinAmountSats: amountVisible ? record.bitcoinAmountSats : null,
    buyerPaymentEvidencePresent: record.buyerPaymentEvidenceRefs.length > 0,
    buyerPaymentEvidenceRefs: audience === 'operator'
      ? safeRefsForAudience(
        'bitcoin accounting buyer payment evidence refs',
        record.buyerPaymentEvidenceRefs,
        audience,
      )
      : [],
    caveatRefs: safeRefsForAudience(
      'bitcoin accounting caveat refs',
      record.caveatRefs,
      audience,
    ),
    evidenceRefs: safeRefsForAudience(
      'bitcoin accounting evidence refs',
      record.evidenceRefs,
      audience,
    ),
    id: record.id,
    jobRef: record.jobRef,
    payoutConfirmationClaimAllowed:
      stateAtLeast(record.state, 'payout_confirmed') &&
      record.payoutConfirmationRefs.length > 0,
    payoutConfirmationRefs: safeRefsForAudience(
      'bitcoin accounting payout confirmation refs',
      record.payoutConfirmationRefs,
      audience,
    ),
    payoutDispatchClaimAllowed:
      stateAtLeast(record.state, 'payout_dispatched') &&
      record.payoutDispatchRefs.length > 0,
    payoutDispatchRefs: safeRefsForAudience(
      'bitcoin accounting payout dispatch refs',
      record.payoutDispatchRefs,
      audience,
    ),
    payoutEligibilityClaimAllowed:
      stateAtLeast(record.state, 'payout_eligible') &&
      record.payoutEligibilityRefs.length > 0,
    payoutEligibilityRefs: safeRefsForAudience(
      'bitcoin accounting payout eligibility refs',
      record.payoutEligibilityRefs,
      audience,
    ),
    payoutVerificationClaimAllowed:
      stateAtLeast(record.state, 'payout_verified') &&
      record.payoutVerificationRefs.length > 0,
    payoutVerificationRefs: safeRefsForAudience(
      'bitcoin accounting payout verification refs',
      record.payoutVerificationRefs,
      audience,
    ),
    providerRef: providerRefForAudience(record, audience),
    providerVisibility: record.providerVisibility,
    receiptRef: record.receiptRef,
    rewardIntentClaimAllowed:
      stateAtLeast(record.state, 'accepted_work_reward_intent') &&
      record.rewardIntentRefs.length > 0,
    rewardIntentRefs: safeRefsForAudience(
      'bitcoin accounting reward intent refs',
      record.rewardIntentRefs,
      audience,
    ),
    settlementClaimAllowed: record.state === 'settled' &&
      record.settlementRefs.length > 0 &&
      record.payoutVerificationRefs.length > 0,
    settlementRefs: safeRefsForAudience(
      'bitcoin accounting settlement refs',
      record.settlementRefs,
      audience,
    ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  }

  if (pylonBitcoinAccountingProjectionHasPrivateMaterial(projection)) {
    throw new PylonBitcoinAccountingReceiptUnsafe({
      reason: 'Bitcoin accounting receipt projection contains private material.',
    })
  }

  return projection
}

export const examplePylonBitcoinAccountingReceipt =
  (): PylonBitcoinAccountingReceiptRecord => ({
    acceptedWorkRefs: ['accepted_work.pylon_trace_summary'],
    amountReceiptRefs: ['amount_receipt.bitcoin.pylon_trace_summary'],
    bitcoinAmountSats: 1_500,
    buyerPaymentEvidenceRefs: ['buyer_payment_evidence.site_order_otec'],
    caveatRefs: ['caveat.bitcoin_accounting.receipt_bound'],
    evidenceRefs: ['evidence.bitcoin_accounting.pylon_trace_summary'],
    id: 'bitcoin_accounting_receipt_pylon_trace_summary',
    jobRef: 'pylon_job.trace_summary_1',
    payoutConfirmationRefs: ['payout_confirmation.trace_summary_1'],
    payoutDispatchRefs: ['payout_dispatch.trace_summary_1'],
    payoutEligibilityRefs: ['payout_eligibility.trace_summary_1'],
    payoutVerificationRefs: ['payout_verification.trace_summary_1'],
    providerRef: 'provider.pylon_public_demo',
    providerVisibility: 'public',
    receiptRef: 'receipt.bitcoin_accounting.pylon_trace_summary',
    rewardIntentRefs: ['reward_intent.trace_summary_1'],
    settlementRefs: ['settlement.public_receipt.trace_summary_1'],
    state: 'settled',
    updatedAtIso: '2026-06-06T21:50:00.000Z',
  })
