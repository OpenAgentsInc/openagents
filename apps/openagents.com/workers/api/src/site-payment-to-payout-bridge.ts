import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  BuyerPaymentReceiptRecord,
  BuyerPaymentReconciliationEventRecord,
  projectBuyerPaymentLedgerRecord,
} from './buyer-payment-ledger'
import {
  NexusTreasuryPayoutAdapterKind,
  NexusTreasuryPayoutAmount,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutLedgerProjection,
  projectNexusTreasuryPayoutLedgerRecord,
} from './nexus-treasury-payout-ledger'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import {
  OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_EVIDENCE_ONLY_AUTHORITY,
  OpenAgentsPylonSettlementBridgeProjection,
  projectOpenAgentsPylonSettlementBridge,
} from './pylon-settlement-bridge'
import {
  PylonV02OmegaReleaseGateProjection,
} from './pylon-v02-omega-release-gate'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'
import { OpenAgentsSiteCheckoutReturnProjection } from './site-checkout-return'
import { OpenAgentsSiteMdkCheckoutIntentRecord } from './site-mdk-checkout-intents'
import {
  TreasuryPaymentAuthorityRejectionReason,
  TreasuryPaymentAuthorityWalletReadiness,
  evaluateTreasuryPaymentAuthorityPolicy,
} from './treasury-payment-authority'

export const OpenAgentsSitePaymentToPayoutBridgeBlocker = S.Literals([
  'checkout_return_not_authority',
  'duplicate_buyer_payment_ref',
  'missing_accepted_work_ref',
  'missing_payout_target_approval',
  'missing_real_movement_gate',
  'missing_verified_buyer_payment',
  'release_gate_not_ready',
  'spend_cap_exceeded',
  'stale_or_absent_wallet_readiness',
])
export type OpenAgentsSitePaymentToPayoutBridgeBlocker =
  typeof OpenAgentsSitePaymentToPayoutBridgeBlocker.Type

export const OpenAgentsSitePaymentToPayoutBridgeState = S.Literals([
  'blocked',
  'payout_intent_ready',
])
export type OpenAgentsSitePaymentToPayoutBridgeState =
  typeof OpenAgentsSitePaymentToPayoutBridgeState.Type

export class OpenAgentsSitePaymentToPayoutBridgeRequest extends S.Class<OpenAgentsSitePaymentToPayoutBridgeRequest>(
  'OpenAgentsSitePaymentToPayoutBridgeRequest',
)({
  acceptedWorkRefs: S.Array(S.String),
  adapterKind: S.optionalKey(NexusTreasuryPayoutAdapterKind),
  amount: NexusTreasuryPayoutAmount,
  artanisDispatchRef: S.optionalKey(S.String),
  assignmentRef: S.optionalKey(S.String),
  checkoutIntentRef: S.String,
  metadataRefs: S.optionalKey(S.Array(S.String)),
  ownerUserId: S.optionalKey(S.NullOr(S.String)),
  payoutAttemptRefs: S.optionalKey(S.Array(S.String)),
  payoutTargetApprovalRef: S.NullOr(S.String),
  payoutTargetRef: S.String,
  payoutVerificationRefs: S.optionalKey(S.Array(S.String)),
  policySnapshotRef: S.String,
  providerRef: S.optionalKey(S.String),
  pylonJobRef: S.optionalKey(S.String),
  settlementRefs: S.optionalKey(S.Array(S.String)),
  spendCap: NexusTreasuryPayoutAmount,
  walletReadiness: TreasuryPaymentAuthorityWalletReadiness,
}) {}

export class OpenAgentsSitePaymentToPayoutBridgeProjection extends S.Class<OpenAgentsSitePaymentToPayoutBridgeProjection>(
  'OpenAgentsSitePaymentToPayoutBridgeProjection',
)({
  acceptedWorkRefs: S.Array(S.String),
  audience: OpenAgentsPaymentPolicyAudience,
  blockerRefs: S.Array(OpenAgentsSitePaymentToPayoutBridgeBlocker),
  buyerPayment: S.NullOr(S.Any),
  checkoutIntentRef: S.String,
  checkoutReturnAuthority: S.Literal(false),
  duplicateBuyerPaymentRef: S.NullOr(S.String),
  operatorRefs: S.Array(S.String),
  payoutIntent: S.NullOr(NexusTreasuryPayoutLedgerProjection),
  payoutIntentRef: S.NullOr(S.String),
  releaseGateRef: S.String,
  settlementBridge: S.NullOr(S.Any),
  settlementClaimAllowed: S.Boolean,
  state: OpenAgentsSitePaymentToPayoutBridgeState,
  stateLabel: S.String,
  verifiedBuyerPaymentRef: S.NullOr(S.String),
}) {}

export type OpenAgentsSitePaymentToPayoutBridgeInput = Readonly<{
  audience: typeof OpenAgentsPaymentPolicyAudience.Type
  existingPayoutIntentForBuyerPaymentRef: NexusTreasuryPayoutIntentRecord | null
  idempotencyKey: string
  nowIso: string
  receipt: BuyerPaymentReceiptRecord | null
  reconciliationEvent: BuyerPaymentReconciliationEventRecord | null
  releaseGate: PylonV02OmegaReleaseGateProjection
  request: OpenAgentsSitePaymentToPayoutBridgeRequest
  returnProjection: OpenAgentsSiteCheckoutReturnProjection | null
  siteCheckoutIntent: OpenAgentsSiteMdkCheckoutIntentRecord
}>

export type OpenAgentsSitePaymentToPayoutBridgeResult = Readonly<
  | {
      _tag: 'Blocked'
      blockers: ReadonlyArray<OpenAgentsSitePaymentToPayoutBridgeBlocker>
      projection: OpenAgentsSitePaymentToPayoutBridgeProjection
    }
  | {
      _tag: 'Ready'
      intent: NexusTreasuryPayoutIntentRecord
      projection: OpenAgentsSitePaymentToPayoutBridgeProjection
    }
>

export class OpenAgentsSitePaymentToPayoutBridgeUnsafe extends S.TaggedErrorClass<OpenAgentsSitePaymentToPayoutBridgeUnsafe>()(
  'OpenAgentsSitePaymentToPayoutBridgeUnsafe',
  {
    reason: S.String,
  },
) {}

const requiredRealMovementEvidenceRefs = [
  'receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507',
  'receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
] as const

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?body|full[_-]?destination|grant|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|source[_-]?archive|wallet[_-]?(config|mnemonic|secret|state)|webhook)/i
const unsafeValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)=|preimage|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?(config|mnemonic|secret|state))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const valueHasUnsafeMaterial = (value: unknown): boolean => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
  }

  if (Array.isArray(value)) {
    return value.some(valueHasUnsafeMaterial)
  }

  if (value !== null && typeof value === 'object') {
    return openAgentsRunnerGatewayPayloadHasPrivateMaterial(value) ||
      Object.entries(value).some(([key, item]) =>
        unsafeKeyPattern.test(key) || valueHasUnsafeMaterial(item),
      )
  }

  return false
}

const safeRef = (value: string): string | undefined =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  !rawTimestampPattern.test(value) &&
  !valueHasUnsafeMaterial(value)
    ? value
    : undefined

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)]
    .map(ref => safeRef(ref))
    .filter((ref): ref is string => ref !== undefined)

const segment = (value: string): string =>
  value.replaceAll(/[^A-Za-z0-9_-]+/g, '_').slice(0, 96)

const bridgeSuffix = (
  request: OpenAgentsSitePaymentToPayoutBridgeRequest,
): string => segment(request.checkoutIntentRef)

const amountWithinSpendCap = (
  amount: NexusTreasuryPayoutAmount,
  spendCap: NexusTreasuryPayoutAmount,
): boolean =>
  amount.asset === spendCap.asset &&
  amount.denomination === spendCap.denomination &&
  amount.amountMinorUnits <= spendCap.amountMinorUnits

const authorityBlocker = (
  reason: TreasuryPaymentAuthorityRejectionReason | null,
): OpenAgentsSitePaymentToPayoutBridgeBlocker | null =>
  reason === 'missing_accepted_work_ref' ||
  reason === 'missing_payout_target_approval' ||
  reason === 'spend_cap_exceeded' ||
  reason === 'stale_or_absent_wallet_readiness'
    ? reason
    : null

const verifiedBuyerPayment = (
  input: OpenAgentsSitePaymentToPayoutBridgeInput,
): boolean =>
  input.receipt !== null &&
  input.reconciliationEvent !== null &&
  input.receipt.status === 'issued' &&
  input.receipt.surface === 'site_checkout' &&
  input.siteCheckoutIntent.status === 'payment_received' &&
  input.receipt.challengeRef === input.siteCheckoutIntent.challengeRef &&
  input.receipt.productId === input.siteCheckoutIntent.productId &&
  input.reconciliationEvent.status === 'matched' &&
  input.reconciliationEvent.receiptRef === input.receipt.receiptRef &&
  input.reconciliationEvent.challengeRef === input.receipt.challengeRef &&
  input.reconciliationEvent.productId === input.receipt.productId

const releaseGateHasRealMovementEvidence = (
  gate: PylonV02OmegaReleaseGateProjection,
): boolean =>
  requiredRealMovementEvidenceRefs.every(ref => gate.evidenceRefs.includes(ref))

const preliminaryBlockers = (
  input: OpenAgentsSitePaymentToPayoutBridgeInput,
): ReadonlyArray<OpenAgentsSitePaymentToPayoutBridgeBlocker> => {
  const blockers: OpenAgentsSitePaymentToPayoutBridgeBlocker[] = []

  if (input.returnProjection !== null) {
    blockers.push('checkout_return_not_authority')
  }

  if (!verifiedBuyerPayment(input)) {
    blockers.push('missing_verified_buyer_payment')
  }

  if (input.existingPayoutIntentForBuyerPaymentRef !== null) {
    blockers.push('duplicate_buyer_payment_ref')
  }

  if (input.request.acceptedWorkRefs.length === 0) {
    blockers.push('missing_accepted_work_ref')
  }

  if (input.request.payoutTargetApprovalRef === null) {
    blockers.push('missing_payout_target_approval')
  }

  if (input.request.walletReadiness !== 'ready') {
    blockers.push('stale_or_absent_wallet_readiness')
  }

  if (!amountWithinSpendCap(input.request.amount, input.request.spendCap)) {
    blockers.push('spend_cap_exceeded')
  }

  if (
    input.releaseGate.state !== 'ready_for_operator_release_review' &&
    input.releaseGate.state !== 'limited_launcher_release_shipped'
  ) {
    blockers.push('release_gate_not_ready')
  }

  if (!releaseGateHasRealMovementEvidence(input.releaseGate)) {
    blockers.push('missing_real_movement_gate')
  }

  return [...new Set(blockers)]
}

const payoutIntentPublicProjectionJson = (
  input: OpenAgentsSitePaymentToPayoutBridgeInput,
): string =>
  JSON.stringify({
    bridgeState: 'payout_intent_ready',
    buyerPaymentRef: input.receipt?.receiptRef ?? null,
    checkoutIntentRef: input.request.checkoutIntentRef,
    siteId: input.siteCheckoutIntent.siteId,
    siteVersionId: input.siteCheckoutIntent.siteVersionId,
    settlementClaimRequiresReceipt: true,
  })

const buildPayoutIntent = (
  input: OpenAgentsSitePaymentToPayoutBridgeInput,
): NexusTreasuryPayoutIntentRecord => {
  const suffix = bridgeSuffix(input.request)

  return {
    acceptedWorkRefs: safeRefs(input.request.acceptedWorkRefs),
    actorRef: 'agent.artanis',
    adapterKind: input.request.adapterKind ?? 'simulation',
    amount: input.request.amount,
    archivedAt: null,
    artanisDispatchRef: safeRef(input.request.artanisDispatchRef ?? '') ?? null,
    assignmentRef: safeRef(input.request.assignmentRef ?? '') ?? null,
    buyerPaymentRef: input.receipt?.receiptRef ?? null,
    createdAt: input.nowIso,
    id: `nexus_treasury_payout_intent_site_payment_${suffix}`,
    idempotencyKeyHash:
      `hash.site_payment_to_payout.${segment(input.siteCheckoutIntent.siteId)}.${segment(input.idempotencyKey)}`,
    metadataRefs: safeRefs([
      ...(input.request.metadataRefs ?? []),
      input.reconciliationEvent?.eventRef ?? '',
      input.releaseGate.gateRef,
      ...requiredRealMovementEvidenceRefs,
      'metadata.site_payment_to_payout.verified_buyer_payment',
    ]),
    ownerUserId: input.request.ownerUserId ?? null,
    payoutIntentRef: `payout_intent.site_payment_to_payout.${suffix}`,
    payoutTargetApprovalRef: input.request.payoutTargetApprovalRef,
    payoutTargetRef: input.request.payoutTargetRef,
    policySnapshotRef: input.request.policySnapshotRef,
    publicProjectionJson: payoutIntentPublicProjectionJson(input),
    pylonJobRef: safeRef(input.request.pylonJobRef ?? '') ?? null,
    sourceKind: 'pylon_marketplace_assignment',
    spendCap: input.request.spendCap,
    status: 'approved',
    updatedAt: input.nowIso,
  }
}

const settlementBridgeProjection = (
  input: OpenAgentsSitePaymentToPayoutBridgeInput,
  intent: NexusTreasuryPayoutIntentRecord,
): OpenAgentsPylonSettlementBridgeProjection => {
  const suffix = bridgeSuffix(input.request)
  const payoutAttemptRefs = safeRefs(input.request.payoutAttemptRefs ?? [])
  const payoutVerificationRefs = safeRefs(
    input.request.payoutVerificationRefs ?? [],
  )
  const settlementRefs = safeRefs(input.request.settlementRefs ?? [])
  const settled = payoutVerificationRefs.length > 0 && settlementRefs.length > 0

  return projectOpenAgentsPylonSettlementBridge(
    {
      acceptedWorkRefs: intent.acceptedWorkRefs,
      authority: OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_EVIDENCE_ONLY_AUTHORITY,
      blockerRefs: [],
      buyerPaymentEvidenceRefs: safeRefs([
        input.receipt?.receiptRef ?? '',
        input.reconciliationEvent?.eventRef ?? '',
      ]),
      capabilitySnapshotRefs: [
        `capability.public.site_payment_to_payout.${suffix}`,
      ],
      caveatRefs: ['caveat.public.product_payment_not_checkout_return_authority'],
      createdAtIso: input.nowIso,
      evidenceRefs: safeRefs([
        input.receipt?.receiptRef ?? '',
        input.reconciliationEvent?.eventRef ?? '',
        intent.payoutIntentRef,
        ...requiredRealMovementEvidenceRefs,
      ]),
      id: `pylon_settlement_bridge.site_payment_to_payout.${suffix}`,
      operatorDiagnosticRefs: [
        `diagnostic.operator.site_payment_to_payout.${suffix}`,
      ],
      payoutConfirmationRefs: [],
      payoutDispatchRefs: payoutAttemptRefs,
      payoutEligibilityRefs: [intent.payoutIntentRef],
      payoutVerificationRefs,
      providerAssignmentRefs: [
        intent.assignmentRef ?? `assignment.public.site_payment_to_payout.${suffix}`,
      ],
      providerJobRefs: [
        intent.pylonJobRef ?? `provider_job.public.site_payment_to_payout.${suffix}`,
      ],
      providerRef: input.request.providerRef ?? 'provider.openagents.site_payment_to_payout',
      providerVisibility: 'public',
      rewardIntentRefs: [`reward_intent.site_payment_to_payout.${suffix}`],
      settlementRefs,
      state: settled ? 'settled' : 'payout_eligible',
      updatedAtIso: input.nowIso,
      walletReadinessRefs: ['readiness.public.site_payment_to_payout.ready'],
      walletReadinessState: 'send_ready',
      workroomRefs: [`workroom.site_payment_to_payout.${suffix}`],
    },
    input.audience === 'public' ? 'public' : 'operator',
    input.nowIso,
  )
}

const projection = (
  input: OpenAgentsSitePaymentToPayoutBridgeInput,
  blockers: ReadonlyArray<OpenAgentsSitePaymentToPayoutBridgeBlocker>,
  intent: NexusTreasuryPayoutIntentRecord | null,
): OpenAgentsSitePaymentToPayoutBridgeProjection => {
  const buyerPayment =
    input.receipt === null
      ? null
      : projectBuyerPaymentLedgerRecord('receipt', input.receipt, input.audience)
  const payoutIntent =
    intent === null
      ? null
      : projectNexusTreasuryPayoutLedgerRecord(
          'intent',
          intent,
          input.audience,
        )
  const bridge =
    intent === null ? null : settlementBridgeProjection(input, intent)

  return new OpenAgentsSitePaymentToPayoutBridgeProjection({
    acceptedWorkRefs:
      input.audience === 'operator'
        ? safeRefs(input.request.acceptedWorkRefs)
        : [],
    audience: input.audience,
    blockerRefs: [...blockers],
    buyerPayment,
    checkoutIntentRef: input.request.checkoutIntentRef,
    checkoutReturnAuthority: false,
    duplicateBuyerPaymentRef:
      input.existingPayoutIntentForBuyerPaymentRef?.payoutIntentRef ?? null,
    operatorRefs:
      input.audience === 'operator'
        ? safeRefs([
            input.siteCheckoutIntent.checkoutIntentRef,
            input.siteCheckoutIntent.checkoutRef,
            input.receipt?.receiptRef ?? '',
            input.reconciliationEvent?.eventRef ?? '',
            input.request.payoutTargetApprovalRef ?? '',
            input.request.payoutTargetRef,
            input.request.policySnapshotRef,
            ...input.releaseGate.evidenceRefs,
          ])
        : [],
    payoutIntent,
    payoutIntentRef: intent?.payoutIntentRef ?? null,
    releaseGateRef: input.releaseGate.gateRef,
    settlementBridge: bridge,
    settlementClaimAllowed: bridge?.settlementClaimAllowed ?? false,
    state: blockers.length === 0 ? 'payout_intent_ready' : 'blocked',
    stateLabel:
      blockers.length === 0
        ? 'Verified Site buyer payment is ready to become a payout intent'
        : 'Site buyer payment cannot become a payout intent yet',
    verifiedBuyerPaymentRef: verifiedBuyerPayment(input)
      ? input.receipt?.receiptRef ?? null
      : null,
  })
}

const assertInputSafe = (
  input: OpenAgentsSitePaymentToPayoutBridgeInput,
): void => {
  if (valueHasUnsafeMaterial(input)) {
    throw new OpenAgentsSitePaymentToPayoutBridgeUnsafe({
      reason:
        'Site payment-to-payout bridge input must not contain raw invoices, payment hashes, preimages, wallet state, customer private data, provider grants, payout destinations, raw webhooks, MDK credentials, or secrets.',
    })
  }
}

export const openAgentsSitePaymentToPayoutBridgeHasPrivateMaterial =
  valueHasUnsafeMaterial

export const buildOpenAgentsSitePaymentToPayoutBridge = (
  input: OpenAgentsSitePaymentToPayoutBridgeInput,
): OpenAgentsSitePaymentToPayoutBridgeResult => {
  assertInputSafe(input)

  const firstBlockers = preliminaryBlockers(input)

  if (firstBlockers.length > 0) {
    return {
      _tag: 'Blocked',
      blockers: firstBlockers,
      projection: projection(input, firstBlockers, null),
    }
  }

  const intent = buildPayoutIntent(input)
  const policy = evaluateTreasuryPaymentAuthorityPolicy(
    {
      intent,
      walletReadiness: input.request.walletReadiness,
    },
    {
      authorityPaused: false,
      pausedAdapters: [],
    },
  )
  const policyBlocker = authorityBlocker(policy.reason)

  if (policyBlocker !== null) {
    const blockers = [policyBlocker]

    return {
      _tag: 'Blocked',
      blockers,
      projection: projection(input, blockers, intent),
    }
  }

  return {
    _tag: 'Ready',
    intent,
    projection: projection(input, [], intent),
  }
}
