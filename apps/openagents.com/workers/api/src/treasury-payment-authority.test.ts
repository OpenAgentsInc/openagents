import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type NexusPaymentAuthorityReceiptRecord,
  type NexusTreasuryPayoutAttemptRecord,
  type NexusTreasuryPayoutIntentRecord,
  type NexusTreasuryPayoutLedgerStore,
  type NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import {
  type TreasuryPaymentAuthorityAdapter,
  TreasuryPaymentAuthorityError,
  type TreasuryPaymentAuthorityRejectionReason,
  type TreasuryPaymentAuthoritySpendPolicy,
  evaluateTreasuryPaymentAuthorityPolicy,
  makeTreasuryPaymentAuthority,
  treasuryPaymentAuthorityPolicyDecisionReceipt,
  treasuryPaymentAuthorityReceiptProjection,
} from './treasury-payment-authority'

const now = '2026-06-07T07:45:00.000Z'

const intent: NexusTreasuryPayoutIntentRecord = {
  acceptedWorkRefs: ['accepted_work.pylon_smoke_1'],
  actorRef: 'agent.artanis',
  adapterKind: 'simulation',
  amount: {
    amountMinorUnits: 1_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
  archivedAt: null,
  artanisDispatchRef: 'artanis.dispatch.pylon_smoke_1',
  assignmentRef: 'assignment.pylon_smoke_1',
  buyerPaymentRef: 'buyer_payment.receipt.site_order_1',
  createdAt: now,
  id: 'nexus_treasury_payout_intent_1',
  idempotencyKeyHash: 'hash.intent.pylon_smoke_1',
  metadataRefs: ['metadata.nexus.intent.operator_test'],
  ownerUserId: 'user_owner_123',
  payoutIntentRef: 'payout_intent.pylon_smoke_1',
  payoutTargetApprovalRef: 'approval.nexus_payout_target.pylon_smoke_1',
  payoutTargetRef: 'payout_target.pylon_smoke_1',
  policySnapshotRef: 'policy_snapshot.nexus.spend_cap_1',
  publicProjectionJson: '{"kind":"operator_test"}',
  pylonJobRef: 'pylon_job.smoke_1',
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
  adapterAttemptRef: 'adapter_attempt.simulation.pylon_smoke_1',
  adapterKind: 'simulation',
  amount: intent.amount,
  archivedAt: null,
  createdAt: now,
  id: 'nexus_treasury_payout_attempt_1',
  idempotencyKeyHash: 'hash.attempt.pylon_smoke_1',
  metadataRefs: ['metadata.nexus.attempt.simulation'],
  payoutAttemptRef: 'payout_attempt.pylon_smoke_1',
  payoutIntentRef: intent.payoutIntentRef,
  publicProjectionJson: '{"adapter":"simulation"}',
  redactedDestinationRef: 'destination.redacted.pylon_smoke_1',
  redactedPaymentRef: 'payment.redacted.simulation_1',
  status: 'dispatched',
  updatedAt: now,
}

const reconciliationEvent: NexusTreasuryPayoutReconciliationEventRecord = {
  adapterKind: 'simulation',
  archivedAt: null,
  createdAt: now,
  eventRef: 'reconciliation.nexus.pylon_smoke_1',
  externalEventRef: 'external_event.simulation.pylon_smoke_1',
  id: 'nexus_treasury_reconciliation_1',
  idempotencyKeyHash: 'hash.reconciliation.pylon_smoke_1',
  metadataRefs: ['metadata.nexus.reconciliation.matched'],
  payoutAttemptRef: attempt.payoutAttemptRef,
  payoutIntentRef: intent.payoutIntentRef,
  providerRef: 'provider.simulation',
  publicProjectionJson: '{}',
  resultRef: 'result.reconciliation.matched',
  status: 'matched',
}

const receipt: NexusPaymentAuthorityReceiptRecord = {
  archivedAt: null,
  audience: 'public',
  createdAt: now,
  eventRef: reconciliationEvent.eventRef,
  id: 'nexus_payment_authority_receipt_1',
  metadataRefs: ['metadata.receipt.public'],
  payoutAttemptRef: attempt.payoutAttemptRef,
  payoutIntentRef: intent.payoutIntentRef,
  publicProjectionJson: '{"receipt":"public_safe"}',
  receiptKind: 'settlement_recorded',
  receiptRef: 'receipt.nexus.payment_authority.pylon_smoke_1',
}

class MemoryLedgerStore implements NexusTreasuryPayoutLedgerStore {
  attempts = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  attemptsByIdempotency = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  events = new Map<string, NexusTreasuryPayoutReconciliationEventRecord>()
  intents = new Map<string, NexusTreasuryPayoutIntentRecord>()
  intentsByIdempotency = new Map<string, NexusTreasuryPayoutIntentRecord>()
  receipts = new Map<string, NexusPaymentAuthorityReceiptRecord>()

  createPayoutAttempt = async (record: NexusTreasuryPayoutAttemptRecord) => {
    if (!this.intents.has(record.payoutIntentRef)) {
      throw new Error('intent missing')
    }

    this.attempts.set(record.payoutAttemptRef, record)
    this.attemptsByIdempotency.set(record.idempotencyKeyHash, record)
  }

  createPayoutIntent = async (record: NexusTreasuryPayoutIntentRecord) => {
    this.intents.set(record.payoutIntentRef, record)
    this.intentsByIdempotency.set(record.idempotencyKeyHash, record)
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

  createReleaseGate = async () => {}

  readPayoutAttemptByRef = async (payoutAttemptRef: string) =>
    this.attempts.get(payoutAttemptRef)

  readPayoutAttemptByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.attemptsByIdempotency.get(idempotencyKeyHash)

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

class FakeAdapter {
  dispatchCalls = 0
  previewCalls = 0
  reconcileCalls = 0

  adapter: TreasuryPaymentAuthorityAdapter = {
    adapterKind: 'simulation',
    dispatch: input =>
      Effect.sync(() => {
        this.dispatchCalls += 1

        return input.attempt
      }),
    preview: input =>
      Effect.sync(() => {
        this.previewCalls += 1

        return {
          adapterKind: input.intent.adapterKind,
          amount: input.intent.amount,
          dispatchAllowed: true,
          payoutIntentRef: input.intent.payoutIntentRef,
          payoutTargetApprovalRef: input.intent.payoutTargetApprovalRef ?? '',
          policySnapshotRef: input.intent.policySnapshotRef,
          spendCap: input.intent.spendCap,
        }
      }),
    reconcile: input =>
      Effect.sync(() => {
        this.reconcileCalls += 1

        return input.event
      }),
  }
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

describe('TreasuryPaymentAuthority', () => {
  test('previews payout without dispatching or writing ledger state', async () => {
    const ledger = new MemoryLedgerStore()
    const adapter = new FakeAdapter()
    const authority = makeTreasuryPaymentAuthority({
      adapters: [adapter.adapter],
      ledgerStore: ledger,
    })

    const preview = await Effect.runPromise(
      authority.previewPayout({
        intent,
        walletReadiness: 'ready',
      }),
    )

    expect(preview.dispatchAllowed).toBe(true)
    expect(adapter.previewCalls).toBe(1)
    expect(adapter.dispatchCalls).toBe(0)
    expect(ledger.intents.size).toBe(0)
    expect(ledger.attempts.size).toBe(0)
  })

  test('requires created payout intent before dispatch and records attempts after dispatch', async () => {
    const ledger = new MemoryLedgerStore()
    const adapter = new FakeAdapter()
    const authority = makeTreasuryPaymentAuthority({
      adapters: [adapter.adapter],
      ledgerStore: ledger,
    })

    expect(
      await failureReason(
        authority.dispatchPayout({
          attempt,
          payoutIntentRef: intent.payoutIntentRef,
        }),
      ),
    ).toBe('payout_intent_not_found')

    const creation = await Effect.runPromise(
      authority.createPayoutIntent({
        intent,
        walletReadiness: 'ready',
      }),
    )
    const dispatch = await Effect.runPromise(
      authority.dispatchPayout({
        attempt,
        payoutIntentRef: intent.payoutIntentRef,
      }),
    )

    expect(creation.intent.payoutIntentRef).toBe(intent.payoutIntentRef)
    expect(dispatch.attempt.payoutAttemptRef).toBe(attempt.payoutAttemptRef)
    expect(adapter.dispatchCalls).toBe(1)
    expect(ledger.attempts.get(attempt.payoutAttemptRef)).toEqual(attempt)
  })

  test('exposes typed rejection reasons for every required policy gate', async () => {
    const adapter = new FakeAdapter()
    const cases: ReadonlyArray<
      Readonly<{
        expected: TreasuryPaymentAuthorityRejectionReason
        service?: ReturnType<typeof makeTreasuryPaymentAuthority>
        value: NexusTreasuryPayoutIntentRecord
        walletReadiness?: 'absent' | 'ready' | 'stale'
      }>
    > = [
      {
        expected: 'missing_accepted_work_ref',
        value: { ...intent, acceptedWorkRefs: [] },
      },
      {
        expected: 'missing_payout_target_approval',
        value: { ...intent, payoutTargetApprovalRef: null },
      },
      {
        expected: 'stale_or_absent_wallet_readiness',
        value: intent,
        walletReadiness: 'stale',
      },
      {
        expected: 'spend_cap_exceeded',
        value: {
          ...intent,
          amount: {
            ...intent.amount,
            amountMinorUnits: intent.spendCap.amountMinorUnits + 1,
          },
        },
      },
      {
        expected: 'malformed_payout_amount',
        value: {
          ...intent,
          amount: {
            ...intent.amount,
            amountMinorUnits: 1.5,
          },
        },
      },
      {
        expected: 'malformed_payout_target',
        value: {
          ...intent,
          payoutTargetRef: 'lnbc2500n1rawinvoice',
        },
      },
      {
        expected: 'adapter_unavailable',
        service: makeTreasuryPaymentAuthority({
          adapters: [],
          ledgerStore: new MemoryLedgerStore(),
        }),
        value: intent,
      },
      {
        expected: 'paused_authority',
        service: makeTreasuryPaymentAuthority({
          adapters: [adapter.adapter],
          ledgerStore: new MemoryLedgerStore(),
          pauseState: {
            authorityPaused: true,
            pausedAdapters: [],
          },
        }),
        value: intent,
      },
      {
        expected: 'paused_adapter',
        service: makeTreasuryPaymentAuthority({
          adapters: [adapter.adapter],
          ledgerStore: new MemoryLedgerStore(),
          pauseState: {
            authorityPaused: false,
            pausedAdapters: ['simulation'],
          },
        }),
        value: intent,
      },
      {
        expected: 'paused_agent',
        service: makeTreasuryPaymentAuthority({
          adapters: [adapter.adapter],
          ledgerStore: new MemoryLedgerStore(),
          pauseState: {
            authorityPaused: false,
            pausedActorRefs: [intent.actorRef],
            pausedAdapters: [],
          },
        }),
        value: intent,
      },
      {
        expected: 'paused_payout_target',
        service: makeTreasuryPaymentAuthority({
          adapters: [adapter.adapter],
          ledgerStore: new MemoryLedgerStore(),
          pauseState: {
            authorityPaused: false,
            pausedAdapters: [],
            pausedPayoutTargetRefs: [intent.payoutTargetRef],
          },
        }),
        value: intent,
      },
      {
        expected: 'paused_pylon',
        service: makeTreasuryPaymentAuthority({
          adapters: [adapter.adapter],
          ledgerStore: new MemoryLedgerStore(),
          pauseState: {
            authorityPaused: false,
            pausedAdapters: [],
            pausedPylonRefs: [intent.pylonJobRef ?? ''],
          },
        }),
        value: intent,
      },
    ]

    for (const item of cases) {
      const service =
        item.service ??
        makeTreasuryPaymentAuthority({
          adapters: [adapter.adapter],
          ledgerStore: new MemoryLedgerStore(),
        })

      expect(
        await failureReason(
          service.previewPayout({
            intent: item.value,
            walletReadiness: item.walletReadiness ?? 'ready',
          }),
        ),
      ).toBe(item.expected)
    }

    const replayLedger = new MemoryLedgerStore()
    const replayAuthority = makeTreasuryPaymentAuthority({
      adapters: [adapter.adapter],
      ledgerStore: replayLedger,
    })

    await Effect.runPromise(
      replayAuthority.createPayoutIntent({
        intent,
        walletReadiness: 'ready',
      }),
    )

    expect(
      await failureReason(
        replayAuthority.createPayoutIntent({
          intent,
          walletReadiness: 'ready',
        }),
      ),
    ).toBe('replayed_idempotency_key')
  })

  test('requires explicit approval for large payouts and records policy rejection receipts', async () => {
    const ledger = new MemoryLedgerStore()
    const adapter = new FakeAdapter()
    const spendPolicy: TreasuryPaymentAuthoritySpendPolicy = {
      largePayoutApprovalRefs: ['approval.policy.large_payout.operator_1'],
      largePayoutThreshold: {
        amountMinorUnits: 500,
        asset: 'bitcoin',
        denomination: 'bitcoin_millisatoshi',
      },
      policyRef: 'policy.nexus.large_payout.operator_required',
    }
    const authority = makeTreasuryPaymentAuthority({
      adapters: [adapter.adapter],
      ledgerStore: ledger,
      spendPolicy,
    })

    await expect(
      failureReason(
        authority.createPayoutIntent({
          intent,
          walletReadiness: 'ready',
        }),
      ),
    ).resolves.toBe('large_payout_requires_approval')

    expect([...ledger.receipts.values()]).toHaveLength(1)
    expect([...ledger.receipts.values()][0]).toMatchObject({
      payoutIntentRef: intent.payoutIntentRef,
      receiptKind: 'policy_rejected',
    })

    const approvedIntent = {
      ...intent,
      id: 'nexus_treasury_payout_intent_large_approved',
      idempotencyKeyHash: 'hash.intent.large_approved',
      metadataRefs: [
        ...intent.metadataRefs,
        'approval.policy.large_payout.operator_1',
      ],
      payoutIntentRef: 'payout_intent.large_approved',
    }

    await expect(
      Effect.runPromise(
        authority.createPayoutIntent({
          intent: approvedIntent,
          walletReadiness: 'ready',
        }),
      ),
    ).resolves.toMatchObject({
      replayed: false,
    })

    const decision = evaluateTreasuryPaymentAuthorityPolicy(
      {
        intent,
        walletReadiness: 'ready',
      },
      {
        authorityPaused: false,
        pausedAdapters: [],
      },
      spendPolicy,
    )
    const receipt = treasuryPaymentAuthorityPolicyDecisionReceipt(
      intent,
      'large_payout_requires_approval',
    )

    expect(decision.allowed).toBe(false)
    expect(decision.receipt?.receiptKind).toBe('policy_rejected')
    expect(JSON.stringify(receipt)).not.toMatch(
      /lnbc|preimage|mnemonic|wallet_state|secret/i,
    )
  })

  test('reconciles through adapter boundary and projects public-safe receipts', async () => {
    const ledger = new MemoryLedgerStore()
    const adapter = new FakeAdapter()
    const authority = makeTreasuryPaymentAuthority({
      adapters: [adapter.adapter],
      ledgerStore: ledger,
    })

    await Effect.runPromise(
      authority.reconcilePayout({
        event: reconciliationEvent,
      }),
    )

    const projection = await Effect.runPromise(
      authority.projectReceipt({
        audience: 'public',
        record: receipt,
        recordKind: 'receipt',
      }),
    )
    const directProjection = treasuryPaymentAuthorityReceiptProjection(
      receipt,
      'public',
    )

    expect(adapter.reconcileCalls).toBe(1)
    expect(ledger.events.get(reconciliationEvent.eventRef)).toEqual(
      reconciliationEvent,
    )
    expect(projection.receiptRef).toBe(receipt.receiptRef)
    expect(directProjection.receiptRef).toBe(receipt.receiptRef)
    expect(JSON.stringify(projection)).not.toMatch(
      /lnbc|preimage|mnemonic|wallet_state|secret/i,
    )
  })
})
