import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusReleaseGateRecord,
  NexusTreasuryPayoutAdapterKind,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutLedgerStore,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import {
  type TreasuryPaymentAuthorityAdapter,
  TreasuryPaymentAuthorityError,
  type TreasuryPaymentAuthorityRejectionReason,
  makeTreasuryPaymentAuthority,
} from './treasury-payment-authority'

const now = '2026-06-07T08:20:00.000Z'

export type TreasuryPaymentAdapterConformanceFixtures = Readonly<{
  attempt: NexusTreasuryPayoutAttemptRecord
  duplicateEvent: NexusTreasuryPayoutReconciliationEventRecord
  failedEvent: NexusTreasuryPayoutReconciliationEventRecord
  intent: NexusTreasuryPayoutIntentRecord
  pendingEvent: NexusTreasuryPayoutReconciliationEventRecord
  rejectedAttempt: NexusTreasuryPayoutAttemptRecord
  stalePendingEvent: NexusTreasuryPayoutReconciliationEventRecord
  succeededEvent: NexusTreasuryPayoutReconciliationEventRecord
}>

export type TreasuryPaymentAdapterConformanceSubject = Readonly<{
  adapter: TreasuryPaymentAuthorityAdapter
  expected?: Partial<TreasuryPaymentAdapterConformanceExpectations>
}>

export type TreasuryPaymentAdapterConformanceExpectations = Readonly<{
  acceptedDispatchStatus: NexusTreasuryPayoutAttemptRecord['status']
  duplicateReconciliationStatus: NexusTreasuryPayoutReconciliationEventRecord['status']
  failedReconciliationStatus: NexusTreasuryPayoutReconciliationEventRecord['status']
  pendingReconciliationStatus: NexusTreasuryPayoutReconciliationEventRecord['status']
  rejectedDispatchStatus: NexusTreasuryPayoutAttemptRecord['status']
  stalePendingReconciliationStatus: NexusTreasuryPayoutReconciliationEventRecord['status']
  succeededReconciliationStatus: NexusTreasuryPayoutReconciliationEventRecord['status']
}>

export type TreasuryPaymentAdapterConformanceSuiteOptions = Readonly<{
  adapterKind: NexusTreasuryPayoutAdapterKind
  makeSubject: (
    fixtures: TreasuryPaymentAdapterConformanceFixtures,
  ) => TreasuryPaymentAdapterConformanceSubject
  name: string
}>

class MemoryLedgerStore implements NexusTreasuryPayoutLedgerStore {
  attempts = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  attemptsByIdempotency = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  events = new Map<string, NexusTreasuryPayoutReconciliationEventRecord>()
  intents = new Map<string, NexusTreasuryPayoutIntentRecord>()
  intentsByIdempotency = new Map<string, NexusTreasuryPayoutIntentRecord>()
  receipts = new Map<string, NexusPaymentAuthorityReceiptRecord>()
  releaseGates = new Map<string, NexusReleaseGateRecord>()

  createPayoutAttempt = async (record: NexusTreasuryPayoutAttemptRecord) => {
    if (!this.intents.has(record.payoutIntentRef)) {
      throw new Error('intent missing')
    }

    if (!this.attemptsByIdempotency.has(record.idempotencyKeyHash)) {
      this.attempts.set(record.payoutAttemptRef, record)
      this.attemptsByIdempotency.set(record.idempotencyKeyHash, record)
    }
  }

  createPayoutIntent = async (record: NexusTreasuryPayoutIntentRecord) => {
    if (!this.intentsByIdempotency.has(record.idempotencyKeyHash)) {
      this.intents.set(record.payoutIntentRef, record)
      this.intentsByIdempotency.set(record.idempotencyKeyHash, record)
    }
  }

  createPayoutTargetApproval = async () => {}

  createPaymentAuthorityReceipt = async (
    record: NexusPaymentAuthorityReceiptRecord,
  ) => {
    this.receipts.set(record.receiptRef, record)
  }

  createReconciliationEvent = async (
    record: NexusTreasuryPayoutReconciliationEventRecord,
  ) => {
    this.events.set(record.eventRef, record)
  }

  createReleaseGate = async (record: NexusReleaseGateRecord) => {
    this.releaseGates.set(record.gateRef, record)
  }

  readPayoutAttemptByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.attemptsByIdempotency.get(idempotencyKeyHash)

  readPayoutAttemptByRef = async (payoutAttemptRef: string) =>
    this.attempts.get(payoutAttemptRef)

  readPayoutIntentByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.intentsByIdempotency.get(idempotencyKeyHash)

  readPayoutIntentByBuyerPaymentRef = async (buyerPaymentRef: string) =>
    [...this.intents.values()].find(
      intent => intent.buyerPaymentRef === buyerPaymentRef,
    )

  readPayoutIntentByRef = async (payoutIntentRef: string) =>
    this.intents.get(payoutIntentRef)
  listPaymentAuthorityReceipts = async (limit: number) =>
    [...this.receipts.values()].slice(0, limit)

  readPaymentAuthorityReceiptByRef = async (receiptRef: string) =>
    this.receipts.get(receiptRef)

  readReconciliationEventByRef = async (eventRef: string) =>
    this.events.get(eventRef)
}

const event = (
  suffix: string,
  adapterKind: NexusTreasuryPayoutAdapterKind,
): NexusTreasuryPayoutReconciliationEventRecord => ({
  adapterKind,
  archivedAt: null,
  createdAt: now,
  eventRef: `reconciliation.nexus.${suffix}`,
  externalEventRef: `external_event.nexus.${suffix}`,
  id: `nexus_treasury_reconciliation_${suffix}`,
  idempotencyKeyHash: `hash.reconciliation.${suffix}`,
  metadataRefs: [`metadata.nexus.reconciliation.${suffix}`],
  payoutAttemptRef: 'payout_attempt.pylon_conformance_1',
  payoutIntentRef: 'payout_intent.pylon_conformance_1',
  providerRef: 'provider.simulation',
  publicProjectionJson: '{}',
  resultRef: `result.reconciliation.${suffix}`,
  status: 'observed',
})

export const treasuryPaymentAdapterConformanceFixtures = (
  adapterKind: NexusTreasuryPayoutAdapterKind = 'simulation',
): TreasuryPaymentAdapterConformanceFixtures => {
  const intent: NexusTreasuryPayoutIntentRecord = {
    acceptedWorkRefs: ['accepted_work.pylon_conformance_1'],
    actorRef: 'agent.artanis',
    adapterKind,
    amount: {
      amountMinorUnits: 1_000,
      asset: 'bitcoin',
      denomination: 'bitcoin_millisatoshi',
    },
    archivedAt: null,
    artanisDispatchRef: 'artanis.dispatch.pylon_conformance_1',
    assignmentRef: 'assignment.pylon_conformance_1',
    buyerPaymentRef: 'buyer_payment.receipt.pylon_conformance_1',
    createdAt: now,
    id: 'nexus_treasury_payout_intent_conformance_1',
    idempotencyKeyHash: 'hash.intent.pylon_conformance_1',
    metadataRefs: ['metadata.nexus.intent.conformance'],
    ownerUserId: 'user_owner_123',
    payoutIntentRef: 'payout_intent.pylon_conformance_1',
    payoutTargetApprovalRef: 'approval.nexus_payout_target.pylon_conformance_1',
    payoutTargetRef: 'payout_target.pylon_conformance_1',
    policySnapshotRef: 'policy_snapshot.nexus.spend_cap_conformance_1',
    publicProjectionJson: '{"kind":"conformance"}',
    pylonJobRef: 'pylon_job.conformance_1',
    sourceKind: 'pylon_marketplace_assignment',
    spendCap: {
      amountMinorUnits: 2_000,
      asset: 'bitcoin',
      denomination: 'bitcoin_millisatoshi',
    },
    status: 'approved',
    updatedAt: now,
  }

  const attempt: NexusTreasuryPayoutAttemptRecord = {
    adapterAttemptRef: 'adapter_attempt.simulation.pylon_conformance_1',
    adapterKind,
    amount: intent.amount,
    archivedAt: null,
    createdAt: now,
    id: 'nexus_treasury_payout_attempt_conformance_1',
    idempotencyKeyHash: 'hash.attempt.pylon_conformance_1',
    metadataRefs: ['metadata.nexus.attempt.conformance'],
    payoutAttemptRef: 'payout_attempt.pylon_conformance_1',
    payoutIntentRef: intent.payoutIntentRef,
    publicProjectionJson: '{}',
    redactedDestinationRef: 'destination.redacted.pylon_conformance_1',
    redactedPaymentRef: null,
    status: 'pending',
    updatedAt: now,
  }

  return {
    attempt,
    duplicateEvent: event('duplicate', adapterKind),
    failedEvent: event('confirmation_failed', adapterKind),
    intent,
    pendingEvent: event('confirmation_pending', adapterKind),
    rejectedAttempt: {
      ...attempt,
      adapterAttemptRef:
        'adapter_attempt.simulation.pylon_conformance_rejected',
      id: 'nexus_treasury_payout_attempt_conformance_rejected',
      idempotencyKeyHash: 'hash.attempt.pylon_conformance_rejected',
      payoutAttemptRef: 'payout_attempt.pylon_conformance_rejected',
    },
    stalePendingEvent: event('stale_pending', adapterKind),
    succeededEvent: event('confirmation_succeeded', adapterKind),
  }
}

const defaultExpectations: TreasuryPaymentAdapterConformanceExpectations = {
  acceptedDispatchStatus: 'dispatched',
  duplicateReconciliationStatus: 'replayed',
  failedReconciliationStatus: 'rejected',
  pendingReconciliationStatus: 'observed',
  rejectedDispatchStatus: 'rejected',
  stalePendingReconciliationStatus: 'rejected',
  succeededReconciliationStatus: 'matched',
}

const failureReason = async (
  effect: Effect.Effect<unknown, TreasuryPaymentAuthorityError>,
): Promise<TreasuryPaymentAuthorityRejectionReason> => {
  try {
    await Effect.runPromise(effect)
    throw new Error('Expected authority failure')
  } catch (error) {
    if (!(error instanceof TreasuryPaymentAuthorityError)) {
      throw new Error('Expected TreasuryPaymentAuthorityError')
    }

    return error.reason
  }
}

export const defineTreasuryPaymentAdapterConformanceSuite = (
  options: TreasuryPaymentAdapterConformanceSuiteOptions,
): void => {
  describe(`${options.name} TreasuryPaymentAuthority adapter conformance`, () => {
    test('runs preview, dispatch replay, rejection, and reconciliation states through the authority', async () => {
      const fixtures = treasuryPaymentAdapterConformanceFixtures(
        options.adapterKind,
      )
      const subject = options.makeSubject(fixtures)
      const expectations = {
        ...defaultExpectations,
        ...subject.expected,
      }
      const ledger = new MemoryLedgerStore()
      const authority = makeTreasuryPaymentAuthority({
        adapters: [subject.adapter],
        ledgerStore: ledger,
      })

      const preview = await Effect.runPromise(
        authority.previewPayout({
          intent: fixtures.intent,
          walletReadiness: 'ready',
        }),
      )
      await Effect.runPromise(
        authority.createPayoutIntent({
          intent: fixtures.intent,
          walletReadiness: 'ready',
        }),
      )
      const dispatch = await Effect.runPromise(
        authority.dispatchPayout({
          attempt: fixtures.attempt,
          payoutIntentRef: fixtures.intent.payoutIntentRef,
        }),
      )
      const replay = await Effect.runPromise(
        authority.dispatchPayout({
          attempt: fixtures.attempt,
          payoutIntentRef: fixtures.intent.payoutIntentRef,
        }),
      )
      const rejectedDispatch = await Effect.runPromise(
        authority.dispatchPayout({
          attempt: fixtures.rejectedAttempt,
          payoutIntentRef: fixtures.intent.payoutIntentRef,
        }),
      )

      expect(preview.dispatchAllowed).toBe(true)
      expect(dispatch.attempt.status).toBe(expectations.acceptedDispatchStatus)
      expect(replay.attempt).toEqual(dispatch.attempt)
      expect(ledger.attemptsByIdempotency.size).toBe(2)
      expect(rejectedDispatch.attempt.status).toBe(
        expectations.rejectedDispatchStatus,
      )

      const reconciliations = [
        [fixtures.pendingEvent, expectations.pendingReconciliationStatus],
        [fixtures.succeededEvent, expectations.succeededReconciliationStatus],
        [fixtures.failedEvent, expectations.failedReconciliationStatus],
        [fixtures.duplicateEvent, expectations.duplicateReconciliationStatus],
        [
          fixtures.stalePendingEvent,
          expectations.stalePendingReconciliationStatus,
        ],
      ] as const

      for (const [eventRecord, expectedStatus] of reconciliations) {
        const result = await Effect.runPromise(
          authority.reconcilePayout({ event: eventRecord }),
        )

        expect(result.event.status).toBe(expectedStatus)
      }

      expect(ledger.events.size).toBe(5)
    })

    test('keeps spend, approval, readiness, pause, and intent gates above the adapter', async () => {
      const fixtures = treasuryPaymentAdapterConformanceFixtures(
        options.adapterKind,
      )
      const subject = options.makeSubject(fixtures)
      const ledger = new MemoryLedgerStore()
      const authority = makeTreasuryPaymentAuthority({
        adapters: [subject.adapter],
        ledgerStore: ledger,
      })

      await expect(
        failureReason(
          authority.createPayoutIntent({
            intent: {
              ...fixtures.intent,
              amount: {
                ...fixtures.intent.amount,
                amountMinorUnits: fixtures.intent.spendCap.amountMinorUnits + 1,
              },
            },
            walletReadiness: 'ready',
          }),
        ),
      ).resolves.toBe('spend_cap_exceeded')

      await expect(
        failureReason(
          authority.previewPayout({
            intent: {
              ...fixtures.intent,
              payoutTargetApprovalRef: null,
            },
            walletReadiness: 'ready',
          }),
        ),
      ).resolves.toBe('missing_payout_target_approval')

      await expect(
        failureReason(
          authority.previewPayout({
            intent: fixtures.intent,
            walletReadiness: 'stale',
          }),
        ),
      ).resolves.toBe('stale_or_absent_wallet_readiness')

      await expect(
        failureReason(
          authority.dispatchPayout({
            attempt: fixtures.attempt,
            payoutIntentRef: fixtures.intent.payoutIntentRef,
          }),
        ),
      ).resolves.toBe('payout_intent_not_found')

      const pausedAuthority = makeTreasuryPaymentAuthority({
        adapters: [subject.adapter],
        ledgerStore: new MemoryLedgerStore(),
        pauseState: {
          authorityPaused: true,
          pausedAdapters: [],
        },
      })

      await expect(
        failureReason(
          pausedAuthority.previewPayout({
            intent: fixtures.intent,
            walletReadiness: 'ready',
          }),
        ),
      ).resolves.toBe('paused_authority')

      expect(ledger.intents.size).toBe(0)
      expect(ledger.attempts.size).toBe(0)
    })
  })
}
