import { Effect, Schema as S } from 'effect'

import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import type { TreasuryPaymentAuthorityAdapter } from './treasury-payment-authority'

export const TreasuryPaymentSimulationDispatchState = S.Literals([
  'dispatch_accepted',
  'dispatch_rejected',
])
export type TreasuryPaymentSimulationDispatchState =
  typeof TreasuryPaymentSimulationDispatchState.Type

export const TreasuryPaymentSimulationReconciliationState = S.Literals([
  'confirmation_failed',
  'confirmation_pending',
  'confirmation_succeeded',
  'duplicate',
  'stale_pending',
])
export type TreasuryPaymentSimulationReconciliationState =
  typeof TreasuryPaymentSimulationReconciliationState.Type

const treasuryPaymentSimulationReceiptStages = [
  'dispatch',
  'confirmation',
  'verification',
  'settlement',
] as const

export const TreasuryPaymentSimulationReceiptStage = S.Literals(
  treasuryPaymentSimulationReceiptStages,
)
export type TreasuryPaymentSimulationReceiptStage =
  typeof TreasuryPaymentSimulationReceiptStage.Type

export type TreasuryPaymentSimulationAdapterOptions = Readonly<{
  dispatchStateByAttemptRef?: Readonly<
    Record<string, TreasuryPaymentSimulationDispatchState>
  >
  reconciliationStateByEventRef?: Readonly<
    Record<string, TreasuryPaymentSimulationReconciliationState>
  >
}>

export type TreasuryPaymentSimulationReceiptInput = Readonly<{
  attempt: NexusTreasuryPayoutAttemptRecord
  createdAt: string
  event: NexusTreasuryPayoutReconciliationEventRecord
  intent: NexusTreasuryPayoutIntentRecord
}>

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const stableSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:/-]/g, '_').slice(0, 120)

const simulationProjectionJson = (
  state: TreasuryPaymentSimulationDispatchState |
    TreasuryPaymentSimulationReconciliationState |
    TreasuryPaymentSimulationReceiptStage,
): string =>
  JSON.stringify({
    adapter: 'simulation',
    moneyMovement: 'none',
    policyProofOnly: true,
    simulation: true,
    state,
  })

const dispatchStateForAttempt = (
  attempt: NexusTreasuryPayoutAttemptRecord,
  options: TreasuryPaymentSimulationAdapterOptions,
): TreasuryPaymentSimulationDispatchState =>
  options.dispatchStateByAttemptRef?.[attempt.payoutAttemptRef] ??
  'dispatch_accepted'

const reconciliationStateForEvent = (
  event: NexusTreasuryPayoutReconciliationEventRecord,
  options: TreasuryPaymentSimulationAdapterOptions,
): TreasuryPaymentSimulationReconciliationState =>
  options.reconciliationStateByEventRef?.[event.eventRef] ??
  'confirmation_succeeded'

const attemptForDispatchState = (
  attempt: NexusTreasuryPayoutAttemptRecord,
  state: TreasuryPaymentSimulationDispatchState,
): NexusTreasuryPayoutAttemptRecord => ({
  ...attempt,
  adapterKind: 'simulation',
  metadataRefs: uniqueRefs([
    ...attempt.metadataRefs,
    `metadata.nexus.simulation.${state}`,
  ]),
  publicProjectionJson: simulationProjectionJson(state),
  redactedPaymentRef:
    state === 'dispatch_rejected'
      ? null
      : `payment.redacted.simulation.${stableSuffix(attempt.idempotencyKeyHash)}`,
  status: state === 'dispatch_rejected' ? 'rejected' : 'dispatched',
})

const reconciliationStatusByState:
  Readonly<
    Record<
      TreasuryPaymentSimulationReconciliationState,
      NexusTreasuryPayoutReconciliationEventRecord['status']
    >
  > = {
    confirmation_failed: 'rejected',
    confirmation_pending: 'observed',
    confirmation_succeeded: 'matched',
    duplicate: 'replayed',
    stale_pending: 'rejected',
  }

export const makeTreasuryPaymentSimulationAdapter = (
  options: TreasuryPaymentSimulationAdapterOptions = {},
): TreasuryPaymentAuthorityAdapter => {
  const attemptsByIdempotency = new Map<
    string,
    NexusTreasuryPayoutAttemptRecord
  >()

  return {
    adapterKind: 'simulation',
    dispatch: input =>
      Effect.sync(() => {
        const existing = attemptsByIdempotency.get(
          input.attempt.idempotencyKeyHash,
        )

        if (existing !== undefined) {
          return existing
        }

        const state = dispatchStateForAttempt(input.attempt, options)
        const attempt = attemptForDispatchState(input.attempt, state)
        attemptsByIdempotency.set(attempt.idempotencyKeyHash, attempt)

        return attempt
      }),
    preview: input =>
      Effect.succeed({
        adapterKind: 'simulation',
        amount: input.intent.amount,
        dispatchAllowed: true,
        payoutIntentRef: input.intent.payoutIntentRef,
        payoutTargetApprovalRef: input.intent.payoutTargetApprovalRef ?? '',
        policySnapshotRef: input.intent.policySnapshotRef,
        spendCap: input.intent.spendCap,
      }),
    reconcile: input =>
      Effect.sync(() => {
        const state = reconciliationStateForEvent(input.event, options)

        return {
          ...input.event,
          adapterKind: 'simulation',
          metadataRefs: uniqueRefs([
            ...input.event.metadataRefs,
            `metadata.nexus.simulation.${state}`,
          ]),
          publicProjectionJson: simulationProjectionJson(state),
          resultRef: `result.nexus_simulation.${state}`,
          status: reconciliationStatusByState[state],
        }
      }),
  }
}

const receiptKindByStage:
  Readonly<
    Record<
      TreasuryPaymentSimulationReceiptStage,
      NexusPaymentAuthorityReceiptRecord['receiptKind']
    >
  > = {
    confirmation: 'confirmation_recorded',
    dispatch: 'dispatch_recorded',
    settlement: 'settlement_recorded',
    verification: 'verification_recorded',
  }

export const buildTreasuryPaymentSimulationReceipts = (
  input: TreasuryPaymentSimulationReceiptInput,
): ReadonlyArray<NexusPaymentAuthorityReceiptRecord> =>
  treasuryPaymentSimulationReceiptStages.map(stage => ({
    archivedAt: null,
    audience: 'public',
    createdAt: input.createdAt,
    eventRef: stage === 'dispatch' ? null : input.event.eventRef,
    id: `nexus_simulation_receipt_${stage}_${stableSuffix(input.attempt.idempotencyKeyHash)}`,
    metadataRefs: uniqueRefs([
      `metadata.nexus.simulation_receipt.${stage}`,
      `metadata.nexus.simulation_receipt.policy_proof_only`,
    ]),
    payoutAttemptRef: input.attempt.payoutAttemptRef,
    payoutIntentRef: input.intent.payoutIntentRef,
    publicProjectionJson: simulationProjectionJson(stage),
    receiptKind: receiptKindByStage[stage],
    receiptRef: `receipt.nexus.simulation.${stage}.${stableSuffix(input.attempt.idempotencyKeyHash)}`,
  }))
