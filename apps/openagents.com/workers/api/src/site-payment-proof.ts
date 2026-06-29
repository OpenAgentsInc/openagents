import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  BuyerPaymentChallengeRecord,
  BuyerPaymentEntitlementRecord,
  BuyerPaymentLedgerProjection,
  BuyerPaymentReceiptRecord,
  BuyerPaymentReconciliationEventRecord,
  projectBuyerPaymentLedgerRecord,
} from './buyer-payment-ledger'
import { OpenAgentsHostedMdkCheckoutStatus } from './hosted-mdk-client'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'
import { OpenAgentsSiteMdkCheckoutIntentRecord } from './site-mdk-checkout-intents'

export const OpenAgentsSitePaymentProofState = S.Literals([
  'blocked',
  'pending_checkout',
  'pending_reconciliation',
  'payment_observed',
  'verified_entitlement',
])
export type OpenAgentsSitePaymentProofState =
  typeof OpenAgentsSitePaymentProofState.Type

export const OpenAgentsSitePaymentProofClaimState = S.Literals([
  'buyer_payment_observed',
  'buyer_payment_verified',
  'checkout_intent_recorded',
  'entitlement_active',
  'no_payment_claim',
])
export type OpenAgentsSitePaymentProofClaimState =
  typeof OpenAgentsSitePaymentProofClaimState.Type

export const OpenAgentsSitePaymentProofEntitlementState = S.Literals([
  'active',
  'blocked',
  'consumed',
  'expired',
  'none',
  'pending_reconciliation',
  'revoked',
])
export type OpenAgentsSitePaymentProofEntitlementState =
  typeof OpenAgentsSitePaymentProofEntitlementState.Type

export const OpenAgentsSitePaymentProofImplementationState = S.Literals([
  'live_provider',
  'sandbox',
])
export type OpenAgentsSitePaymentProofImplementationState =
  typeof OpenAgentsSitePaymentProofImplementationState.Type

export const OpenAgentsSitePaymentProofLabels = S.Struct({
  agentLabel: S.String,
  customerLabel: S.String,
  operatorLabel: S.String,
})
export type OpenAgentsSitePaymentProofLabels =
  typeof OpenAgentsSitePaymentProofLabels.Type

export class OpenAgentsSitePaymentProofProjection extends S.Class<OpenAgentsSitePaymentProofProjection>(
  'OpenAgentsSitePaymentProofProjection',
)({
  acceptedWorkPayoutAuthority: S.Literal(false),
  audience: OpenAgentsPaymentPolicyAudience,
  buyerPaymentChallenge: BuyerPaymentLedgerProjection,
  catalogRef: S.String,
  caveatRefs: S.Array(S.String),
  checkoutIntentRef: S.String,
  checkoutRef: S.String,
  checkoutStatus: OpenAgentsHostedMdkCheckoutStatus,
  checkoutUrlRef: S.String,
  claimState: OpenAgentsSitePaymentProofClaimState,
  entitlement: S.NullOr(BuyerPaymentLedgerProjection),
  entitlementState: OpenAgentsSitePaymentProofEntitlementState,
  environment: S.Literals(['production', 'sandbox']),
  finalSettlementClaim: S.Literal(false),
  implementationState: OpenAgentsSitePaymentProofImplementationState,
  labels: OpenAgentsSitePaymentProofLabels,
  payoutClaimAllowed: S.Literal(false),
  productId: S.String,
  proofRefs: S.Array(S.String),
  proofState: OpenAgentsSitePaymentProofState,
  providerPayoutAuthority: S.Literal(false),
  providerRef: S.String,
  receipt: S.NullOr(BuyerPaymentLedgerProjection),
  reconciliationEvent: S.NullOr(BuyerPaymentLedgerProjection),
  sandbox: S.Boolean,
  settlementClaimAllowed: S.Literal(false),
  siteId: S.String,
  siteVersionId: S.String,
}) {}

export const OpenAgentsSitePaymentProofInput = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  buyerPaymentChallenge: BuyerPaymentChallengeRecord,
  checkoutIntent: OpenAgentsSiteMdkCheckoutIntentRecord,
  entitlement: S.NullOr(BuyerPaymentEntitlementRecord),
  receipt: S.NullOr(BuyerPaymentReceiptRecord),
  reconciliationEvent: S.NullOr(BuyerPaymentReconciliationEventRecord),
})
export type OpenAgentsSitePaymentProofInput =
  typeof OpenAgentsSitePaymentProofInput.Type

export class OpenAgentsSitePaymentProofUnsafe extends S.TaggedErrorClass<OpenAgentsSitePaymentProofUnsafe>()(
  'OpenAgentsSitePaymentProofUnsafe',
  {
    reason: S.String,
  },
) {}

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const unsafeKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?body|full[_-]?destination|grant|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|source[_-]?archive|wallet[_-]?(config|mnemonic|secret|state)|webhook)/i
const unsafeValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)=|preimage|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?(config|mnemonic|secret|state))/i

const valueHasUnsafeMaterial = (
  value: unknown,
  options: Readonly<{ rejectRawTimestamps: boolean }> = {
    rejectRawTimestamps: true,
  },
): boolean => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value) ||
      (options.rejectRawTimestamps && rawTimestampPattern.test(value)) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
  }

  if (Array.isArray(value)) {
    return value.some(item => valueHasUnsafeMaterial(item, options))
  }

  if (value !== null && typeof value === 'object') {
    return openAgentsRunnerGatewayPayloadHasPrivateMaterial(value) ||
      Object.entries(value).some(([key, item]) =>
        unsafeKeyPattern.test(key) || valueHasUnsafeMaterial(item, options),
      )
  }

  return false
}

const refIsSafe = (value: string): boolean =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  !rawTimestampPattern.test(value) &&
  !valueHasUnsafeMaterial(value)

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(refIsSafe)

const refsMatch = (input: OpenAgentsSitePaymentProofInput): boolean => {
  const challenge = input.buyerPaymentChallenge
  const checkout = input.checkoutIntent
  const receiptMatches =
    input.receipt === null ||
    (
      input.receipt.challengeRef === challenge.challengeRef &&
      input.receipt.productId === checkout.productId &&
      input.receipt.surface === 'site_checkout'
    )
  const entitlementMatches =
    input.entitlement === null ||
    (
      input.entitlement.challengeRef === challenge.challengeRef &&
      input.entitlement.productId === checkout.productId &&
      input.entitlement.receiptRef === input.receipt?.receiptRef &&
      input.entitlement.surface === 'site_checkout'
    )
  const reconciliationMatches =
    input.reconciliationEvent === null ||
    (
      input.receipt !== null &&
      input.reconciliationEvent.status === 'matched' &&
      input.reconciliationEvent.challengeRef === challenge.challengeRef &&
      input.reconciliationEvent.productId === checkout.productId &&
      input.reconciliationEvent.receiptRef === input.receipt.receiptRef
    )

  return checkout.challengeRef === challenge.challengeRef &&
    checkout.productId === challenge.productId &&
    receiptMatches &&
    entitlementMatches &&
    reconciliationMatches
}

const entitlementState = (
  input: OpenAgentsSitePaymentProofInput,
  proofState: OpenAgentsSitePaymentProofState,
): OpenAgentsSitePaymentProofEntitlementState =>
  proofState === 'blocked'
    ? 'blocked'
    : input.entitlement !== null
      ? input.entitlement.status
      : input.receipt !== null || input.checkoutIntent.status === 'payment_received'
        ? 'pending_reconciliation'
        : 'none'

const proofState = (
  input: OpenAgentsSitePaymentProofInput,
): OpenAgentsSitePaymentProofState =>
  !refsMatch(input)
    ? 'blocked'
    : input.entitlement?.status === 'active' &&
        input.receipt?.status === 'issued' &&
        input.reconciliationEvent?.status === 'matched' &&
        input.checkoutIntent.status === 'payment_received'
      ? 'verified_entitlement'
      : input.receipt?.status === 'issued' &&
          input.reconciliationEvent?.status === 'matched'
        ? 'payment_observed'
        : input.receipt !== null ||
            input.checkoutIntent.status === 'payment_received'
          ? 'pending_reconciliation'
          : 'pending_checkout'

const claimState = (
  input: OpenAgentsSitePaymentProofInput,
  state: OpenAgentsSitePaymentProofState,
): OpenAgentsSitePaymentProofClaimState =>
  state === 'blocked'
    ? 'no_payment_claim'
    : state === 'verified_entitlement'
      ? 'entitlement_active'
      : input.receipt?.status === 'issued' &&
          input.reconciliationEvent?.status === 'matched'
        ? 'buyer_payment_verified'
        : input.receipt?.status === 'issued' ||
            input.checkoutIntent.status === 'payment_received'
          ? 'buyer_payment_observed'
          : input.checkoutIntent.status === 'created' ||
              input.checkoutIntent.status === 'pending_payment'
            ? 'checkout_intent_recorded'
            : 'no_payment_claim'

const labelsForState = (
  state: OpenAgentsSitePaymentProofState,
): OpenAgentsSitePaymentProofLabels =>
  state === 'verified_entitlement'
    ? {
        agentLabel: 'Buyer payment verified; entitlement active.',
        customerLabel: 'Payment verified.',
        operatorLabel: 'Receipt, reconciliation, and entitlement match.',
      }
    : state === 'payment_observed'
      ? {
          agentLabel: 'Buyer payment verified; entitlement pending.',
          customerLabel: 'Payment received.',
          operatorLabel: 'Receipt and reconciliation match.',
        }
      : state === 'pending_reconciliation'
        ? {
            agentLabel: 'Checkout payment evidence is waiting on reconciliation.',
            customerLabel: 'Payment review pending.',
            operatorLabel: 'Checkout status or receipt exists without full reconciliation.',
          }
        : state === 'pending_checkout'
          ? {
              agentLabel: 'Checkout has been created and is awaiting payment.',
              customerLabel: 'Awaiting payment.',
              operatorLabel: 'No verified buyer payment evidence yet.',
            }
          : {
              agentLabel: 'Payment proof is blocked by mismatched refs.',
              customerLabel: 'Payment proof unavailable.',
              operatorLabel: 'Checkout, receipt, entitlement, or reconciliation refs do not match.',
            }

const caveatRefsForState = (
  state: OpenAgentsSitePaymentProofState,
): ReadonlyArray<string> =>
  safeRefs([
    `caveat.site_payment_proof.${state}`,
    'caveat.site_payment_proof.not_payout_authority',
    'caveat.site_payment_proof.not_settlement_authority',
  ])

const proofRefs = (
  input: OpenAgentsSitePaymentProofInput,
): ReadonlyArray<string> =>
  safeRefs([
    input.buyerPaymentChallenge.challengeRef,
    input.checkoutIntent.checkoutIntentRef,
    input.checkoutIntent.checkoutRef,
    input.receipt?.receiptRef ?? '',
    input.entitlement?.entitlementRef ?? '',
    input.reconciliationEvent?.eventRef ?? '',
    input.reconciliationEvent?.resultRef ?? '',
  ])

const projectionIsSafe = (
  projection: OpenAgentsSitePaymentProofProjection,
): boolean =>
  refIsSafe(projection.siteId) &&
  refIsSafe(projection.siteVersionId) &&
  refIsSafe(projection.catalogRef) &&
  refIsSafe(projection.checkoutIntentRef) &&
  refIsSafe(projection.checkoutRef) &&
  refIsSafe(projection.checkoutUrlRef) &&
  refIsSafe(projection.productId) &&
  refIsSafe(projection.providerRef) &&
  projection.caveatRefs.every(refIsSafe) &&
  projection.proofRefs.every(refIsSafe) &&
  !valueHasUnsafeMaterial(projection)

export const openAgentsSitePaymentProofHasPrivateMaterial =
  valueHasUnsafeMaterial

export const projectOpenAgentsSitePaymentProof = (
  input: OpenAgentsSitePaymentProofInput,
): OpenAgentsSitePaymentProofProjection => {
  if (valueHasUnsafeMaterial(input, { rejectRawTimestamps: false })) {
    throw new OpenAgentsSitePaymentProofUnsafe({
      reason:
        'Site payment proof input must not contain customer private data, raw checkout query state, raw payment material, wallet state, MDK credentials, provider grants, payout claims, raw timestamps, or secrets.',
    })
  }

  const state = proofState(input)
  const projection = new OpenAgentsSitePaymentProofProjection({
    acceptedWorkPayoutAuthority: false,
    audience: input.audience,
    buyerPaymentChallenge: projectBuyerPaymentLedgerRecord(
      'challenge',
      input.buyerPaymentChallenge,
      input.audience,
    ),
    catalogRef: input.checkoutIntent.catalogRef,
    caveatRefs: caveatRefsForState(state),
    checkoutIntentRef: input.checkoutIntent.checkoutIntentRef,
    checkoutRef: input.checkoutIntent.checkoutRef,
    checkoutStatus: input.checkoutIntent.status,
    checkoutUrlRef: input.checkoutIntent.checkoutUrlRef,
    claimState: claimState(input, state),
    entitlement: state === 'blocked' || input.entitlement === null
      ? null
      : projectBuyerPaymentLedgerRecord(
          'entitlement',
          input.entitlement,
          input.audience,
        ),
    entitlementState: entitlementState(input, state),
    environment: input.checkoutIntent.environment,
    finalSettlementClaim: false,
    implementationState: input.checkoutIntent.sandbox ? 'sandbox' : 'live_provider',
    labels: labelsForState(state),
    payoutClaimAllowed: false,
    productId: input.checkoutIntent.productId,
    proofRefs: proofRefs(input),
    proofState: state,
    providerPayoutAuthority: false,
    providerRef: input.checkoutIntent.providerRef,
    receipt: state === 'blocked' || input.receipt === null
      ? null
      : projectBuyerPaymentLedgerRecord(
          'receipt',
          input.receipt,
          input.audience,
        ),
    reconciliationEvent:
      state === 'blocked' || input.reconciliationEvent === null
        ? null
        : projectBuyerPaymentLedgerRecord(
            'reconciliation_event',
            input.reconciliationEvent,
            input.audience,
          ),
    sandbox: input.checkoutIntent.sandbox,
    settlementClaimAllowed: false,
    siteId: input.checkoutIntent.siteId,
    siteVersionId: input.checkoutIntent.siteVersionId,
  })

  if (!projectionIsSafe(projection)) {
    throw new OpenAgentsSitePaymentProofUnsafe({
      reason: 'Site payment proof projection is not public-safe.',
    })
  }

  return projection
}
