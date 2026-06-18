import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { nexusPylonPublicReceiptDetailFromLedger } from './nexus-pylon-visibility'
import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutLedgerStore,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import { settledSatsFromPaymentAuthorityReceipt } from './training-leaderboards'
import { buildTrainingRunRecord } from './training-run-window-authority'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
} from './training-run-window-authority'
import {
  buildTassadarRunSettlement,
  realSettlementMovementMode,
} from './tassadar-run-settlement'
import {
  TassadarRunSettlementHardDailyCapSats,
  decideTassadarDailyBudget,
  disabledTassadarRealSettlementGate,
  parseTassadarRealSettlementGate,
  readTassadarRealSettlementGate,
  resolveTassadarSettlementAdapter,
  type TassadarRealSettlementGate,
  tassadarRealSettledSatsForDay,
  tassadarRealSettlementUtcDayKey,
} from './tassadar-run-settlement-gate'
import {
  type TreasuryPaymentAuthorityAdapter,
  makeTreasuryPaymentAuthority,
} from './treasury-payment-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

const nowIso = '2026-06-17T10:05:00.000Z'

const enabledGate: TassadarRealSettlementGate = {
  enabled: true,
  allowedAdapterKind: 'spark_treasury',
  allowedContributorRefs: ['pylon.contributor.stranger'],
  allowedRunRefs: ['run.tassadar.executor.20260615'],
  maxPayoutSats: 50,
}

const activeRun = (
  overrides: Partial<TrainingRunRecord> = {},
): TrainingRunRecord => ({
  ...buildTrainingRunRecord({
    makeId: () => 'run',
    nowIso: '2026-06-17T10:00:00.000Z',
    request: {
      manifest: { artifactDigestRefs: [], blockerRefs: [], spendCapSats: 100 },
      promiseRef: 'training.decentralized_training_launch.v1',
      trainingRunRef: 'run.tassadar.executor.20260615',
    },
  }),
  state: 'active',
  ...overrides,
})

const verifiedChallenge = (): TrainingVerificationChallengeRecord => ({
  challengeRef: 'challenge.tassadar.5232',
  commitmentRefs: ['commitment.tassadar.5232'],
  contributionRef: 'contribution.tassadar.5232',
  createdAt: '2026-06-17T10:02:00.000Z',
  failureCodes: [],
  homeworkKind: 'admin_dispatched_homework',
  id: 'challenge_5232',
  leaseExpiresAt: null,
  leaseRef: null,
  leasedToRef: null,
  maxAttempts: 3,
  payloadJson: '{}',
  publicProjectionJson: '{}',
  rejectedAt: null,
  samplingPolicy: 'per_contribution',
  state: 'Verified',
  timedOutAt: null,
  trainingRunRef: 'run.tassadar.executor.20260615',
  updatedAt: '2026-06-17T10:03:00.000Z',
  verdictRefs: ['verdict.tassadar.5232'],
  verificationClass: 'exact_trace_replay',
  verifiedAt: '2026-06-17T10:03:00.000Z',
  windowRef: 'training.window.tassadar.executor.20260615.w1',
})

const activeLease = (): TrainingWindowLeaseRecord => ({
  claimedAt: '2026-06-17T10:01:00.000Z',
  id: 'lease5232',
  leaseExpiresAt: '2026-06-17T12:00:00.000Z',
  leaseRef: 'lease.tassadar.5232',
  publicProjectionJson: '{}',
  pylonRef: 'pylon.contributor.stranger',
  receiptRefs: [],
  state: 'active',
  trainingRunRef: 'run.tassadar.executor.20260615',
  windowRef: 'training.window.tassadar.executor.20260615.w1',
})

const baseRequest = {
  amountSats: 21,
  challengeRef: 'challenge.tassadar.5232',
  idempotencyRef: 'idem.tassadar.5232',
  leaseRef: 'lease.tassadar.5232',
  operatorApprovalRef: 'operator.approval.5232',
  payoutTargetApprovalRef: 'payout.target.approval.5232',
  payoutTargetRef: 'payout.target.5232',
} as const

describe('readTassadarRealSettlementGate / parse (fail-closed)', () => {
  it('defaults to disabled when the env value is absent', () => {
    expect(readTassadarRealSettlementGate({})).toEqual(
      disabledTassadarRealSettlementGate,
    )
  })

  it('defaults to disabled on malformed JSON', () => {
    expect(parseTassadarRealSettlementGate('{not json')).toEqual(
      disabledTassadarRealSettlementGate,
    )
  })

  it('defaults to disabled when the decoded gate has enabled:false', () => {
    expect(
      parseTassadarRealSettlementGate(
        JSON.stringify({ ...enabledGate, enabled: false }),
      ).enabled,
    ).toBe(false)
  })

  it('rejects a cap over the hard per-payout ceiling (decode fails closed)', () => {
    expect(
      parseTassadarRealSettlementGate(
        JSON.stringify({ ...enabledGate, maxPayoutSats: 1_000_000 }),
      ),
    ).toEqual(disabledTassadarRealSettlementGate)
  })

  it('accepts a well-formed enabled gate from the env', () => {
    const gate = readTassadarRealSettlementGate({
      OPENAGENTS_REAL_SETTLEMENT_GATE: JSON.stringify(enabledGate),
    })

    expect(gate.enabled).toBe(true)
    expect(gate.allowedAdapterKind).toBe('spark_treasury')
    expect(gate.maxPayoutSats).toBe(50)
  })
})

describe('resolveTassadarSettlementAdapter (default OFF = simulation)', () => {
  it('returns simulation when the gate is disabled, even if real is requested', () => {
    const decision = resolveTassadarSettlementAdapter({
      amountSats: 21,
      contributorRef: 'pylon.contributor.stranger',
      gate: disabledTassadarRealSettlementGate,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: 'run.tassadar.executor.20260615',
    })

    expect(decision.adapterKind).toBe('simulation')
    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('gate_disabled')
  })

  it('returns simulation when the caller does not request the real adapter', () => {
    const decision = resolveTassadarSettlementAdapter({
      amountSats: 21,
      contributorRef: 'pylon.contributor.stranger',
      gate: enabledGate,
      requestedAdapterKind: 'simulation',
      trainingRunRef: 'run.tassadar.executor.20260615',
    })

    expect(decision.adapterKind).toBe('simulation')
    expect(decision.blockedReason).toBe('requested_adapter_mismatch')
  })

  it('fails closed to simulation when the amount is over the gate cap', () => {
    const decision = resolveTassadarSettlementAdapter({
      amountSats: 51,
      contributorRef: 'pylon.contributor.stranger',
      gate: enabledGate,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: 'run.tassadar.executor.20260615',
    })

    expect(decision.adapterKind).toBe('simulation')
    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('amount_over_gate_cap')
  })

  it('fails closed when the contributor is not allowlisted', () => {
    const decision = resolveTassadarSettlementAdapter({
      amountSats: 21,
      contributorRef: 'pylon.contributor.other',
      gate: enabledGate,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: 'run.tassadar.executor.20260615',
    })

    expect(decision.adapterKind).toBe('simulation')
    expect(decision.blockedReason).toBe('contributor_not_allowlisted')
  })

  it('fails closed when the run is not allowlisted', () => {
    const decision = resolveTassadarSettlementAdapter({
      amountSats: 21,
      contributorRef: 'pylon.contributor.stranger',
      gate: enabledGate,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: 'run.tassadar.executor.other',
    })

    expect(decision.adapterKind).toBe('simulation')
    expect(decision.blockedReason).toBe('run_not_allowlisted')
  })

  it('authorizes the real adapter only when every bound holds', () => {
    const decision = resolveTassadarSettlementAdapter({
      amountSats: 21,
      contributorRef: 'pylon.contributor.stranger',
      gate: enabledGate,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: 'run.tassadar.executor.20260615',
    })

    expect(decision.adapterKind).toBe('spark_treasury')
    expect(decision.realAuthorized).toBe(true)
    expect(decision.blockedReason).toBeNull()
  })
})

describe('movement mode + realBitcoinMoved derivation', () => {
  it('maps adapters to the right public moneyMovement label', () => {
    expect(realSettlementMovementMode('simulation')).toBe('none')
    expect(realSettlementMovementMode('spark_treasury')).toBe('real_bitcoin')
    expect(realSettlementMovementMode('mdk_agent_wallet')).toBe(
      'treasury_mdk_bounded_spend',
    )
  })

  it('default (simulation) settlement derives realBitcoinMoved:false', () => {
    const settlement = buildTassadarRunSettlement({
      challenge: verifiedChallenge(),
      lease: activeLease(),
      nowIso,
      request: baseRequest,
      run: activeRun(),
    })

    const detail = nexusPylonPublicReceiptDetailFromLedger({
      appUrl: 'https://openagents.com',
      attempt: settlement.attempt,
      event: settlement.reconciliationEvent,
      intent: settlement.intent,
      nowIso,
      receipt: settlement.settlementReceipt,
    })

    expect(detail.movementMode).toBe('simulation')
    expect(detail.realBitcoinMoved).toBe(false)
    // Settled-sats read is unchanged for the simulation path.
    expect(
      settledSatsFromPaymentAuthorityReceipt(settlement.settlementReceipt),
    ).toBe(21)
  })

  it('real (spark_treasury) settlement derives realBitcoinMoved:true', () => {
    const settlement = buildTassadarRunSettlement({
      challenge: verifiedChallenge(),
      lease: activeLease(),
      nowIso,
      request: { ...baseRequest, adapterKind: 'spark_treasury' },
      run: activeRun(),
    })

    const detail = nexusPylonPublicReceiptDetailFromLedger({
      appUrl: 'https://openagents.com',
      attempt: settlement.attempt,
      event: settlement.reconciliationEvent,
      intent: settlement.intent,
      nowIso,
      receipt: settlement.settlementReceipt,
    })

    expect(detail.movementMode).toBe('real_bitcoin')
    expect(detail.realBitcoinMoved).toBe(true)
    expect(detail.caveatRefs).toContain(
      'caveat.public.nexus_pylon.real_bitcoin_receipt',
    )
  })
})

// A minimal in-memory ledger store + counting Spark adapter to prove the
// no-double-pay idempotency contract end-to-end through TreasuryPaymentAuthority.
class MemoryLedgerStore implements NexusTreasuryPayoutLedgerStore {
  attempts = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  attemptsByIdempotency = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  events = new Map<string, NexusTreasuryPayoutReconciliationEventRecord>()
  intents = new Map<string, NexusTreasuryPayoutIntentRecord>()
  intentsByIdempotency = new Map<string, NexusTreasuryPayoutIntentRecord>()
  receipts = new Map<string, NexusPaymentAuthorityReceiptRecord>()

  createPayoutAttempt = async (record: NexusTreasuryPayoutAttemptRecord) => {
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

class CountingSparkAdapter {
  dispatchCalls = 0

  adapter: TreasuryPaymentAuthorityAdapter = {
    adapterKind: 'spark_treasury',
    dispatch: input =>
      Effect.sync(() => {
        this.dispatchCalls += 1

        return {
          ...input.attempt,
          publicProjectionJson: JSON.stringify({
            adapter: 'spark_treasury',
            moneyMovement: 'real_bitcoin',
            rawMaterialStored: false,
            state: 'dispatch_reported',
          }),
          redactedPaymentRef: 'payment.redacted.spark_treasury.test',
          status: 'dispatched',
        }
      }),
    preview: input =>
      Effect.succeed({
        adapterKind: 'spark_treasury',
        amount: input.intent.amount,
        dispatchAllowed: true,
        payoutIntentRef: input.intent.payoutIntentRef,
        payoutTargetApprovalRef: input.intent.payoutTargetApprovalRef ?? '',
        policySnapshotRef: input.intent.policySnapshotRef,
        spendCap: input.intent.spendCap,
      }),
    reconcile: input => Effect.succeed(input.event),
  }
}

describe('idempotency: a retry never double-pays (mocked Spark dispatch)', () => {
  it('returns the existing attempt on a replayed idempotency key without a second dispatch', async () => {
    const settlement = buildTassadarRunSettlement({
      challenge: verifiedChallenge(),
      lease: activeLease(),
      nowIso,
      request: { ...baseRequest, adapterKind: 'spark_treasury' },
      run: activeRun(),
    })
    const ledger = new MemoryLedgerStore()
    const spark = new CountingSparkAdapter()
    const authority = makeTreasuryPaymentAuthority({
      adapters: [spark.adapter],
      ledgerStore: ledger,
    })

    await ledger.createPayoutTargetApproval()
    await Effect.runPromise(
      authority.createPayoutIntent({
        intent: settlement.intent,
        walletReadiness: 'ready',
      }),
    )

    const first = await Effect.runPromise(
      authority.dispatchPayout({
        attempt: settlement.attempt,
        payoutIntentRef: settlement.intent.payoutIntentRef,
      }),
    )
    const second = await Effect.runPromise(
      authority.dispatchPayout({
        attempt: settlement.attempt,
        payoutIntentRef: settlement.intent.payoutIntentRef,
      }),
    )

    expect(spark.dispatchCalls).toBe(1)
    expect(first.attempt.payoutAttemptRef).toBe(
      second.attempt.payoutAttemptRef,
    )
    expect(first.attempt.status).toBe('dispatched')
  })

  it('rejects a replayed intent before any dispatch', async () => {
    const settlement = buildTassadarRunSettlement({
      challenge: verifiedChallenge(),
      lease: activeLease(),
      nowIso,
      request: { ...baseRequest, adapterKind: 'spark_treasury' },
      run: activeRun(),
    })
    const ledger = new MemoryLedgerStore()
    const spark = new CountingSparkAdapter()
    const authority = makeTreasuryPaymentAuthority({
      adapters: [spark.adapter],
      ledgerStore: ledger,
    })

    await Effect.runPromise(
      authority.createPayoutIntent({
        intent: settlement.intent,
        walletReadiness: 'ready',
      }),
    )

    const replay = await Effect.runPromiseExit(
      authority.createPayoutIntent({
        intent: settlement.intent,
        walletReadiness: 'ready',
      }),
    )

    expect(replay._tag).toBe('Failure')
    expect(spark.dispatchCalls).toBe(0)
  })
})

describe('backward-compat: gate without the new optional fields (#5309)', () => {
  it('decodes a gate value that has no maxDailyPayoutSats / runScopedStreaming', () => {
    // This is the exact shape of an ALREADY-ARMED production gate value. It must
    // still decode and behave exactly as before (per-payout-only, allowlist).
    const gate = readTassadarRealSettlementGate({
      OPENAGENTS_REAL_SETTLEMENT_GATE: JSON.stringify({
        enabled: true,
        allowedAdapterKind: 'spark_treasury',
        allowedContributorRefs: ['pylon.contributor.orrery'],
        allowedRunRefs: ['run.tassadar.executor.20260615'],
        maxPayoutSats: 1000,
      }),
    })

    expect(gate.enabled).toBe(true)
    expect(gate.maxPayoutSats).toBe(1000)
    expect(gate.maxDailyPayoutSats).toBeUndefined()
    expect(gate.runScopedStreaming).toBeUndefined()
  })

  it('preserves per-payout-only behavior (always authorized at the daily layer) when no daily cap', () => {
    const gate = parseTassadarRealSettlementGate(
      JSON.stringify({
        enabled: true,
        allowedAdapterKind: 'spark_treasury',
        allowedContributorRefs: ['pylon.contributor.orrery'],
        allowedRunRefs: ['run.tassadar.executor.20260615'],
        maxPayoutSats: 1000,
      }),
    )
    const decision = decideTassadarDailyBudget({
      alreadySettledTodaySats: 999_999,
      amountSats: 5,
      gate,
    })

    expect(decision.authorized).toBe(true)
    expect(decision.effectiveDailyCapSats).toBeNull()
    expect(decision.remainingDailyBudgetSats).toBeNull()
  })

  it('keeps allowlist-only eligibility when runScopedStreaming is absent', () => {
    const gate = parseTassadarRealSettlementGate(
      JSON.stringify({
        enabled: true,
        allowedAdapterKind: 'spark_treasury',
        allowedContributorRefs: ['pylon.contributor.orrery'],
        allowedRunRefs: ['run.tassadar.executor.20260615'],
        maxPayoutSats: 1000,
      }),
    )
    const decision = resolveTassadarSettlementAdapter({
      amountSats: 5,
      contributorRef: 'pylon.contributor.unlisted',
      gate,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: 'run.tassadar.executor.20260615',
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('contributor_not_allowlisted')
  })
})

describe('run-scoped streaming eligibility (#5309/#5310)', () => {
  const runScopedGate: TassadarRealSettlementGate = {
    enabled: true,
    allowedAdapterKind: 'spark_treasury',
    allowedContributorRefs: [],
    allowedRunRefs: ['run.tassadar.executor.20260615'],
    maxPayoutSats: 100,
    maxDailyPayoutSats: 50_000,
    runScopedStreaming: true,
  }

  it('authorizes ANY contributor on an allowlisted run when run-scoped streaming is on', () => {
    const decision = resolveTassadarSettlementAdapter({
      amountSats: 5,
      contributorRef: 'pylon.contributor.brand_new_node',
      gate: runScopedGate,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: 'run.tassadar.executor.20260615',
    })

    expect(decision.realAuthorized).toBe(true)
    expect(decision.eligibilitySource).toBe('run_scoped_streaming')
  })

  it('still requires the run to be allowlisted even under run-scoped streaming', () => {
    const decision = resolveTassadarSettlementAdapter({
      amountSats: 5,
      contributorRef: 'pylon.contributor.brand_new_node',
      gate: runScopedGate,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: 'run.tassadar.executor.NOT_ENROLLED',
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('run_not_allowlisted')
  })

  it('marks an explicitly allowlisted contributor as eligibilitySource:allowlisted', () => {
    const decision = resolveTassadarSettlementAdapter({
      amountSats: 5,
      contributorRef: 'pylon.contributor.stranger',
      gate: { ...enabledGate, runScopedStreaming: true },
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: 'run.tassadar.executor.20260615',
    })

    expect(decision.realAuthorized).toBe(true)
    expect(decision.eligibilitySource).toBe('allowlisted')
  })

  it('still enforces the per-payout cap under run-scoped streaming', () => {
    const decision = resolveTassadarSettlementAdapter({
      amountSats: 101,
      contributorRef: 'pylon.contributor.brand_new_node',
      gate: runScopedGate,
      requestedAdapterKind: 'spark_treasury',
      trainingRunRef: 'run.tassadar.executor.20260615',
    })

    expect(decision.realAuthorized).toBe(false)
    expect(decision.blockedReason).toBe('amount_over_gate_cap')
  })
})

describe('cumulative daily budget cap (fail-closed at the ceiling) (#5309)', () => {
  const dailyGate: TassadarRealSettlementGate = {
    enabled: true,
    allowedAdapterKind: 'spark_treasury',
    allowedContributorRefs: ['pylon.contributor.stranger'],
    allowedRunRefs: ['run.tassadar.executor.20260615'],
    maxPayoutSats: 100,
    maxDailyPayoutSats: 20,
  }

  it('authorizes while under the daily cap and reports remaining budget', () => {
    const decision = decideTassadarDailyBudget({
      alreadySettledTodaySats: 10,
      amountSats: 5,
      gate: dailyGate,
    })

    expect(decision.authorized).toBe(true)
    expect(decision.effectiveDailyCapSats).toBe(20)
    expect(decision.remainingDailyBudgetSats).toBe(5)
  })

  it('authorizes the exact payout that lands on the ceiling', () => {
    const decision = decideTassadarDailyBudget({
      alreadySettledTodaySats: 15,
      amountSats: 5,
      gate: dailyGate,
    })

    expect(decision.authorized).toBe(true)
    expect(decision.remainingDailyBudgetSats).toBe(0)
  })

  it('FAILS CLOSED on the payout that would exceed the ceiling', () => {
    const decision = decideTassadarDailyBudget({
      alreadySettledTodaySats: 18,
      amountSats: 5,
      gate: dailyGate,
    })

    expect(decision.authorized).toBe(false)
    // Remaining reflects the unspent budget; the over-cap payout is rejected.
    expect(decision.remainingDailyBudgetSats).toBe(2)
  })

  it('clamps the declared daily cap to the module hard daily ceiling', () => {
    const overGate = parseTassadarRealSettlementGate(
      JSON.stringify({
        ...dailyGate,
        maxDailyPayoutSats: TassadarRunSettlementHardDailyCapSats + 1,
      }),
    )

    // A cap above the hard ceiling fails the schema decode (fail-closed).
    expect(overGate).toEqual(disabledTassadarRealSettlementGate)
  })

  it('resets across the UTC-day boundary', () => {
    const receipts = [
      {
        createdAt: '2026-06-17T23:59:59.000Z',
        publicProjectionJson: JSON.stringify({
          amountSats: 5,
          moneyMovement: 'real_bitcoin',
          state: 'settled',
        }),
        receiptKind: 'settlement_recorded' as const,
      },
      {
        createdAt: '2026-06-18T00:00:01.000Z',
        publicProjectionJson: JSON.stringify({
          amountSats: 5,
          moneyMovement: 'real_bitcoin',
          state: 'settled',
        }),
        receiptKind: 'settlement_recorded' as const,
      },
    ]

    expect(tassadarRealSettlementUtcDayKey('2026-06-17T23:59:59.000Z')).toBe(
      '2026-06-17',
    )
    expect(tassadarRealSettledSatsForDay(receipts, '2026-06-17')).toBe(5)
    expect(tassadarRealSettledSatsForDay(receipts, '2026-06-18')).toBe(5)
  })

  it('only counts REAL settled receipts toward the daily total', () => {
    const receipts = [
      {
        createdAt: '2026-06-17T10:00:00.000Z',
        publicProjectionJson: JSON.stringify({
          amountSats: 5,
          moneyMovement: 'real_bitcoin',
          state: 'settled',
        }),
        receiptKind: 'settlement_recorded' as const,
      },
      {
        // simulation receipt — must NOT consume real budget
        createdAt: '2026-06-17T10:05:00.000Z',
        publicProjectionJson: JSON.stringify({
          amountSats: 5000,
          moneyMovement: 'none',
          state: 'settled',
        }),
        receiptKind: 'settlement_recorded' as const,
      },
      {
        // wrong receipt kind — ignored
        createdAt: '2026-06-17T10:06:00.000Z',
        publicProjectionJson: JSON.stringify({
          amountSats: 9999,
          moneyMovement: 'real_bitcoin',
          state: 'settled',
        }),
        receiptKind: 'dispatch_recorded' as const,
      },
    ]

    expect(tassadarRealSettledSatsForDay(receipts, '2026-06-17')).toBe(5)
  })
})
