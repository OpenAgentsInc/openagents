import { Schema as S } from 'effect'

import {
  type NexusPaymentAuthorityReceiptRecord,
  type NexusPayoutTargetApprovalRecord,
  NexusTreasuryPayoutAdapterKind,
  type NexusTreasuryPayoutAmount,
  type NexusTreasuryPayoutAttemptRecord,
  type NexusTreasuryPayoutIntentRecord,
  type NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import { TrainingPublicSafeRef } from './training-run-window-authority'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

/**
 * Tassadar earn-Bitcoin leg (openagents #5009, JUNE15_LAUNCH_PLAN §4.D).
 *
 * A `Verified` `exact_trace_replay` verification challenge for a training run
 * (Step C, #5008) is the only thing that becomes eligible for payout here. This
 * module turns one accepted executor-trace work item into the full
 * operator-approved settlement ledger chain — payout intent, attempt,
 * reconciliation event, and a provider-confirmed `settlement_recorded` receipt —
 * and the settlement receipt ref to link back onto the run.
 *
 * Payout itself is programmatic: the OpenAgents treasury wallet (the `/treasury`
 * MDK-backed wallet) makes the payout and Artanis is wired to dispatch from it
 * under bounded spend authority (`adapterKind: 'mdk_agent_wallet'`). The
 * `simulation` adapter records the same ledger chain with no money movement, for
 * proofs and tests. Every send is bounded by the run manifest `spendCapSats`
 * plus a hard per-payout ceiling, and is operator-approved (a required approval
 * ref). Nothing here dispatches unattended.
 *
 * The settlement receipt projection is the contract read back by
 * `settledSatsFromPaymentAuthorityReceipt`: `state: 'settled'` plus a positive
 * integer `amountSats`. No raw invoices, preimages, payment hashes, wallet
 * material, or payout-target addresses ever enter the projection — only
 * public-safe redacted refs.
 */

// Defense-in-depth ceiling independent of the run manifest cap. The launch is
// operator-approved small-sats; no single recorded settlement may exceed this.
export const TassadarRunSettlementHardPerPayoutCapSats = 100_000

export const TassadarRunSettlementPolicySnapshotRef =
  'policy.tassadar.operator_approved_small_sats.v1'

export const TassadarRunSettlementRequest = S.Struct({
  adapterKind: S.optionalKey(NexusTreasuryPayoutAdapterKind),
  amountSats: S.Number.check(
    S.isInt(),
    S.isBetween({ minimum: 1, maximum: TassadarRunSettlementHardPerPayoutCapSats }),
  ),
  challengeRef: TrainingPublicSafeRef,
  idempotencyRef: TrainingPublicSafeRef,
  leaseRef: TrainingPublicSafeRef,
  operatorApprovalRef: TrainingPublicSafeRef,
  payoutTargetApprovalRef: TrainingPublicSafeRef,
  payoutTargetRef: TrainingPublicSafeRef,
})
export type TassadarRunSettlementRequest =
  typeof TassadarRunSettlementRequest.Type

export class TassadarRunSettlementUnsafe extends S.TaggedErrorClass<TassadarRunSettlementUnsafe>()(
  'TassadarRunSettlementUnsafe',
  {
    kind: S.Literals(['conflict', 'validation_error']),
    reason: S.String,
  },
) {}

export const TassadarRunSettlementPayoutTargetApprovalPolicyRef =
  'policy.public.tassadar.run_settlement_payout'

export type TassadarRunSettlementRecords = Readonly<{
  amountSats: number
  attempt: NexusTreasuryPayoutAttemptRecord
  contributorRef: string
  intent: NexusTreasuryPayoutIntentRecord
  reconciliationEvent: NexusTreasuryPayoutReconciliationEventRecord
  settlementReceipt: NexusPaymentAuthorityReceiptRecord
  settlementReceiptRef: string
  targetApproval: NexusPayoutTargetApprovalRecord
}>

const stableSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:/-]/g, '_').slice(0, 120)

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const bitcoinAmount = (sats: number): NexusTreasuryPayoutAmount => ({
  amountMinorUnits: sats * 1000,
  asset: 'bitcoin',
  denomination: 'bitcoin_millisatoshi',
})

/**
 * Map a payout adapter to the public-projection `moneyMovement` label
 * (openagents #5232). `simulation` moves nothing (`none`); the proven Spark
 * treasury rail moves real sats (`real_bitcoin`), which is the only label the
 * public `realBitcoinMoved` derivation in `nexus-pylon-visibility.ts` keys on;
 * any other real adapter keeps the existing bounded-spend label. Pure.
 */
export const realSettlementMovementMode = (
  adapterKind: NexusTreasuryPayoutAdapterKind,
): 'none' | 'real_bitcoin' | 'treasury_mdk_bounded_spend' =>
  adapterKind === 'simulation'
    ? 'none'
    : adapterKind === 'spark_treasury'
      ? 'real_bitcoin'
      : 'treasury_mdk_bounded_spend'

const conflict = (reason: string): TassadarRunSettlementUnsafe =>
  new TassadarRunSettlementUnsafe({ kind: 'conflict', reason })

const invalid = (reason: string): TassadarRunSettlementUnsafe =>
  new TassadarRunSettlementUnsafe({ kind: 'validation_error', reason })

/**
 * Build the settlement ledger chain for one accepted (Verified) executor-trace
 * work item. Throws `TassadarRunSettlementUnsafe` when the work item is not
 * settleable: a non-verified or wrong-class challenge, a challenge/lease that
 * does not belong to the run, a missing run spend cap, or an amount over the
 * cap. The caller maps `kind` to 409 (conflict) or 422 (validation_error).
 */
export const buildTassadarRunSettlement = (
  input: Readonly<{
    challenge: TrainingVerificationChallengeRecord
    lease: TrainingWindowLeaseRecord
    nowIso: string
    request: TassadarRunSettlementRequest
    run: TrainingRunRecord
  }>,
): TassadarRunSettlementRecords => {
  const { challenge, lease, nowIso, request, run } = input

  if (run.state === 'planned') {
    throw conflict(
      'Cannot settle accepted work on a planned run; activate the run first.',
    )
  }

  if (challenge.trainingRunRef !== run.trainingRunRef) {
    throw conflict(
      'Verification challenge does not belong to this training run.',
    )
  }

  if (challenge.state !== 'Verified') {
    throw conflict(
      `Only Verified work is settleable; challenge is ${challenge.state}.`,
    )
  }

  if (challenge.verificationClass !== 'exact_trace_replay') {
    throw conflict(
      'Only exact_trace_replay executor-trace work is settleable on this run.',
    )
  }

  if (lease.trainingRunRef !== run.trainingRunRef) {
    throw conflict('Lease does not belong to this training run.')
  }

  const contributorRef = lease.pylonRef.trim()

  if (contributorRef === '') {
    throw invalid('Lease is missing a contributor pylon ref.')
  }

  const capSats = run.manifest?.spendCapSats

  if (capSats === undefined) {
    throw invalid(
      'Run manifest must declare spendCapSats before settlement is allowed.',
    )
  }

  if (request.amountSats > capSats) {
    throw invalid(
      `Settlement amount ${request.amountSats} sats exceeds the run spend cap of ${capSats} sats.`,
    )
  }

  if (request.amountSats > TassadarRunSettlementHardPerPayoutCapSats) {
    throw invalid(
      `Settlement amount ${request.amountSats} sats exceeds the hard per-payout cap.`,
    )
  }

  const adapterKind = request.adapterKind ?? 'simulation'
  const moneyMovement = realSettlementMovementMode(adapterKind)
  const suffix = stableSuffix(request.idempotencyRef)
  const windowRef = challenge.windowRef ?? lease.windowRef
  const amount = bitcoinAmount(request.amountSats)
  const spendCap = bitcoinAmount(capSats)
  const acceptedWorkRefs = uniqueRefs([
    challenge.challengeRef,
    lease.leaseRef,
    windowRef,
  ])
  const metadataRefs = uniqueRefs([
    run.trainingRunRef,
    challenge.challengeRef,
    lease.leaseRef,
    windowRef,
    request.operatorApprovalRef,
    'metadata.tassadar.run_settlement.accepted_work',
  ])
  const redactedDestinationRef = `destination.redacted.tassadar_run_settlement.${suffix}`

  // The payout intent's payout_target_approval_ref is a foreign key into
  // nexus_payout_target_approvals. The training-run settlement path is
  // self-contained (no marketplace assignment), so it must materialize the
  // operator-approved payout-target approval row itself before the intent,
  // exactly as the accepted-work marketplace payout route does. Without this
  // the intent insert fails the foreign-key constraint.
  const targetApproval: NexusPayoutTargetApprovalRecord = {
    agentRef: 'agent.artanis',
    approvalPolicyRef: TassadarRunSettlementPayoutTargetApprovalPolicyRef,
    approvalRef: request.payoutTargetApprovalRef,
    approvedByRef: 'operator.openagents.tassadar_run_settlement',
    archivedAt: null,
    createdAt: nowIso,
    expiresAt: null,
    id: `nexus_payout_target_approval_tassadar_settlement_${suffix}`,
    idempotencyKeyHash: `hash.tassadar_run_settlement.approval.${suffix}`,
    ownerUserId: 'user_openagents_operator',
    payoutTargetRef: request.payoutTargetRef,
    publicProjectionJson: JSON.stringify({
      pylonRef: contributorRef,
      state: 'active',
      trainingRunRef: run.trainingRunRef,
    }),
    pylonRef: contributorRef,
    redactedDestinationRef,
    scopeRefs: uniqueRefs([
      run.trainingRunRef,
      challenge.challengeRef,
      lease.leaseRef,
    ]),
    status: 'active',
    updatedAt: nowIso,
  }

  const intent: NexusTreasuryPayoutIntentRecord = {
    acceptedWorkRefs,
    actorRef: 'agent.artanis',
    adapterKind,
    amount,
    archivedAt: null,
    artanisDispatchRef: `artanis_dispatch.tassadar_run_settlement.${suffix}`,
    assignmentRef: null,
    buyerPaymentRef: null,
    createdAt: nowIso,
    id: `nexus_treasury_payout_intent_tassadar_settlement_${suffix}`,
    idempotencyKeyHash: `hash.tassadar_run_settlement.intent.${suffix}`,
    metadataRefs,
    ownerUserId: null,
    payoutIntentRef: `payout_intent.tassadar_run_settlement.${suffix}`,
    payoutTargetApprovalRef: request.payoutTargetApprovalRef,
    payoutTargetRef: request.payoutTargetRef,
    policySnapshotRef: TassadarRunSettlementPolicySnapshotRef,
    publicProjectionJson: JSON.stringify({
      acceptedWork: true,
      adapter: adapterKind,
      amountSats: request.amountSats,
      moneyMovement,
      operatorApproved: true,
      state: 'approved',
      trainingRunRef: run.trainingRunRef,
    }),
    pylonJobRef: null,
    sourceKind: 'accepted_work',
    spendCap,
    status: 'approved',
    updatedAt: nowIso,
  }

  const attempt: NexusTreasuryPayoutAttemptRecord = {
    adapterAttemptRef: `adapter_attempt.tassadar_run_settlement.${adapterKind}.${suffix}`,
    adapterKind,
    amount,
    archivedAt: null,
    createdAt: nowIso,
    id: `nexus_treasury_payout_attempt_tassadar_settlement_${suffix}`,
    idempotencyKeyHash: `hash.tassadar_run_settlement.attempt.${suffix}`,
    metadataRefs,
    payoutAttemptRef: `payout_attempt.tassadar_run_settlement.${suffix}`,
    payoutIntentRef: intent.payoutIntentRef,
    publicProjectionJson: JSON.stringify({
      adapter: adapterKind,
      amountSats: request.amountSats,
      moneyMovement,
      trainingRunRef: run.trainingRunRef,
    }),
    redactedDestinationRef,
    redactedPaymentRef: null,
    status: 'confirmed',
    updatedAt: nowIso,
  }

  const reconciliationEvent: NexusTreasuryPayoutReconciliationEventRecord = {
    adapterKind,
    archivedAt: null,
    createdAt: nowIso,
    eventRef: `reconciliation.tassadar_run_settlement.${suffix}`,
    externalEventRef: `external_event.tassadar_run_settlement.${adapterKind}.${suffix}`,
    id: `nexus_treasury_reconciliation_tassadar_settlement_${suffix}`,
    idempotencyKeyHash: `hash.tassadar_run_settlement.reconciliation.${suffix}`,
    metadataRefs,
    payoutAttemptRef: attempt.payoutAttemptRef,
    payoutIntentRef: intent.payoutIntentRef,
    providerRef: `provider.${adapterKind}`,
    publicProjectionJson: JSON.stringify({
      adapter: adapterKind,
      amountSats: request.amountSats,
      moneyMovement,
      trainingRunRef: run.trainingRunRef,
    }),
    resultRef: `result.tassadar_run_settlement.${suffix}`,
    status: 'matched',
  }

  const settlementReceiptRef = `receipt.nexus.tassadar_run_settlement.${suffix}`
  const settlementReceipt: NexusPaymentAuthorityReceiptRecord = {
    archivedAt: null,
    audience: 'public',
    createdAt: nowIso,
    eventRef: reconciliationEvent.eventRef,
    id: `nexus_payment_authority_receipt_tassadar_settlement_${suffix}`,
    metadataRefs,
    payoutAttemptRef: attempt.payoutAttemptRef,
    payoutIntentRef: intent.payoutIntentRef,
    publicProjectionJson: JSON.stringify({
      adapter: adapterKind,
      amountSats: request.amountSats,
      asset: 'bitcoin',
      contributorRef,
      moneyMovement,
      state: 'settled',
      trainingRunRef: run.trainingRunRef,
      verificationChallengeRef: challenge.challengeRef,
      windowRef,
    }),
    receiptKind: 'settlement_recorded',
    receiptRef: settlementReceiptRef,
  }

  return {
    amountSats: request.amountSats,
    attempt,
    contributorRef,
    intent,
    reconciliationEvent,
    settlementReceipt,
    settlementReceiptRef,
    targetApproval,
  }
}
