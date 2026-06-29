import { readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  hostedMdkDirectPayoutDisabledGate,
  projectMdkPayoutModeGate,
} from '../mdk-payout-mode-gate'
import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutLedgerStore,
  NexusTreasuryPayoutReconciliationEventRecord,
} from '../nexus-treasury-payout-ledger'
import {
  disabledTassadarRealSettlementGate,
  type TassadarRealSettlementGate,
} from '../tassadar-run-settlement-gate'
import { type ServingReceipt } from './openagents-network-adapter'
import {
  decideServingNodePayout,
  type ServingNodePayoutDecision,
} from './serving-node-payout'
import {
  buildKhalaSettlementRecords,
  type KhalaSettlementDeps,
  settleVerifiedServingPayout,
} from './khala-verified-work-settlement'

const nowIso = '2026-06-22T18:00:00.000Z'

// The designated guinea-pig Pylon node ref (the contributor that must be paid
// FIRST). Its REAL Spark receive address lives in the gitignored
// `.secrets/khala-test-payout.env`; we read it at runtime when present and fall
// back to a non-secret placeholder so this test runs in CI without the file and
// NEVER commits the address. The address itself is only handed to the (test)
// destination resolver — it never enters a committed file or assertion.
const GUINEA_PIG_NODE_REF = 'pylon.khala.guinea_pig'

const readGuineaPigSparkAddress = (): string => {
  try {
    const raw = readFileSync(
      join(homedir(), 'work', '.secrets', 'khala-test-payout.env'),
      'utf8',
    )
    for (const line of raw.split('\n')) {
      const trimmed = line.trim()
      if (trimmed.startsWith('KHALA_TEST_PAYOUT_SPARK_ADDRESS=')) {
        const value = trimmed
          .slice('KHALA_TEST_PAYOUT_SPARK_ADDRESS='.length)
          .trim()
        if (value !== '') {
          return value
        }
      }
    }
  } catch {
    // File absent (CI): fall through to the placeholder.
  }
  return 'spark1guineapigplaceholderxxxxxxxxxxxxxxxxxxxxxxx'
}

// A whole-model serving receipt served by the guinea-pig Pylon, parity-verified.
const guineaPigReceipt: ServingReceipt = {
  parityMode: 'exact_greedy_parity',
  parityVerified: true,
  servedModel: 'openagents/khala-mini',
  sharded: false,
  servingRunRef: 'serve.run.khala.guineapig.1',
  stages: [
    { layerEnd: 28, layerStart: 0, nodeRef: GUINEA_PIG_NODE_REF, role: 'stage' },
  ],
}

// The owner-armed MDK payout-mode gate (makes the #5484 decision `armed`). This
// does NOT itself move money; the real-settlement gate below is the second,
// independent owner gate that authorizes the Spark dispatch.
const armedMdkGate = () =>
  projectMdkPayoutModeGate({
    hostedFundedKeyVerified: true,
    hostedProgrammaticPayoutsEnabled: true,
    requestedMode: 'hosted_mdk_direct_payout',
  })

const fullResaleRefs = {
  assignmentReceiptRef: 'ref.assignment',
  dispatchRef: 'ref.dispatch',
  meteringReceiptRef: 'ref.metering',
  pricingPolicyRef: 'ref.pricing',
  providerGrantRef: 'ref.grant',
  routePolicyRef: 'ref.route',
  settlementReceiptRef: 'ref.settlement',
  tosBoundaryRef: 'ref.tos',
}

const SETTLEMENT_RUN_REF = 'run.khala.serving.guineapig'

// Build an ARMED serving-node payout decision for the guinea-pig receipt. The cut
// is expressed in MSAT (the #5484 split is msat-denominated); the settlement path
// floors to whole sats. We use 5_000 msat = 5 sats so it clears the 1-sat dust
// floor and stays tiny + treasury-bounded.
const armedDecision = (
  contributorCutMsat = 5_000,
  receipt: ServingReceipt = guineaPigReceipt,
): ServingNodePayoutDecision =>
  decideServingNodePayout({
    contributorCutMsat,
    payoutGate: armedMdkGate(),
    receipt,
    resaleRefs: fullResaleRefs,
    revenueAsset: 'bitcoin',
  })

// The owner real-settlement gate, ARMED in TEST mode with run-scoped streaming so
// the guinea-pig contributor qualifies. Per-payout cap 100 sats, daily cap 100
// sats — deliberately tiny + treasury-bounded; this is real money.
const armedRealSettlementGate = (
  overrides: Partial<TassadarRealSettlementGate> = {},
): TassadarRealSettlementGate => ({
  enabled: true,
  allowedAdapterKind: 'spark_treasury',
  allowedContributorRefs: [],
  allowedRunRefs: [SETTLEMENT_RUN_REF],
  maxPayoutSats: 100,
  maxDailyPayoutSats: 100,
  runScopedStreaming: true,
  ...overrides,
})

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

  listPaymentAuthorityReceipts = async (limit: number) =>
    [...this.receipts.values()].slice(0, limit)

  readPayoutAttemptByRef = async (ref: string) => this.attempts.get(ref)

  readPayoutAttemptByIdempotencyKeyHash = async (hash: string) =>
    this.attemptsByIdempotency.get(hash)

  readPayoutIntentByIdempotencyKeyHash = async (hash: string) =>
    this.intentsByIdempotency.get(hash)

  readPayoutIntentByBuyerPaymentRef = async (buyerPaymentRef: string) =>
    [...this.intents.values()].find(i => i.buyerPaymentRef === buyerPaymentRef)

  readPayoutIntentByRef = async (ref: string) => this.intents.get(ref)

  readPaymentAuthorityReceiptByRef = async (ref: string) =>
    this.receipts.get(ref)

  readReconciliationEventByRef = async (ref: string) => this.events.get(ref)
}

// A test dispatch that mirrors the receipt-first confirmed-real path
// (`dispatchRealRunSettlementCore`): it short-circuits if the settlement receipt
// already exists (idempotency), else counts the call and persists the
// `realBitcoinMoved`-shaped settlement receipt to the ledger.
const makeDeps = (
  options: Readonly<{
    gate: TassadarRealSettlementGate
    ledger: MemoryLedgerStore
    targets: ReadonlyMap<string, string>
    dispatchCount: { value: number }
  }>,
): KhalaSettlementDeps => ({
  dispatchRealSettlement: input =>
    Effect.gen(function* () {
      const existing = yield* Effect.promise(() =>
        options.ledger.readPaymentAuthorityReceiptByRef(
          input.settlement.settlementReceiptRef,
        ),
      )
      if (existing !== undefined) {
        return
      }
      options.dispatchCount.value += 1
      yield* Effect.promise(() =>
        options.ledger.createPaymentAuthorityReceipt(
          input.settlement.settlementReceipt,
        ),
      )
    }),
  ledger: options.ledger,
  nowIso,
  readGate: () => options.gate,
  resolvePayoutDestination: async ref => options.targets.get(ref),
  settlementRunRef: SETTLEMENT_RUN_REF,
})

describe('buildKhalaSettlementRecords (record shape)', () => {
  it('builds a realBitcoinMoved-shaped settlement receipt for spark_treasury', () => {
    const records = buildKhalaSettlementRecords({
      adapterKind: 'spark_treasury',
      nowIso,
      servedModel: 'openagents/khala-mini',
      servingRunRef: guineaPigReceipt.servingRunRef,
      share: { amountMsat: 5, nodeRef: GUINEA_PIG_NODE_REF, weight: 28 },
    })
    const projection = JSON.parse(records.settlementReceipt.publicProjectionJson)
    expect(projection.state).toBe('settled')
    expect(projection.moneyMovement).toBe('real_bitcoin')
    expect(projection.amountSats).toBe(5)
    expect(projection.asset).toBe('bitcoin')
    expect(records.settlementReceipt.receiptKind).toBe('settlement_recorded')
    expect(records.settlementReceiptRef).toContain('khala_serving_settlement')
    // No raw Spark address / invoice / preimage anywhere in the projection.
    expect(records.settlementReceipt.publicProjectionJson).not.toMatch(/spark1/i)
  })

  it('builds a money-movement:none receipt for simulation', () => {
    const records = buildKhalaSettlementRecords({
      adapterKind: 'simulation',
      nowIso,
      servedModel: 'openagents/khala-mini',
      servingRunRef: guineaPigReceipt.servingRunRef,
      share: { amountMsat: 5, nodeRef: GUINEA_PIG_NODE_REF, weight: 28 },
    })
    const projection = JSON.parse(records.settlementReceipt.publicProjectionJson)
    expect(projection.moneyMovement).toBe('none')
  })
})

describe('settleVerifiedServingPayout — guinea-pig Pylon paid first', () => {
  it('settles real sats to the guinea-pig Pylon with a dereferenceable realBitcoinMoved receipt (TEST-armed)', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      ledger,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const outcome = await Effect.runPromise(
      settleVerifiedServingPayout(deps, {
        decision: armedDecision(),
        parityVerified: guineaPigReceipt.parityVerified,
        servedModel: guineaPigReceipt.servedModel,
      }),
    )

    expect(outcome.legs).toHaveLength(1)
    const leg = outcome.legs[0]!
    expect(leg.contributorRef).toBe(GUINEA_PIG_NODE_REF)
    expect(leg.settled).toBe(true)
    expect(leg.realBitcoinMoved).toBe(true)
    expect(leg.mode).toBe('real_bitcoin')
    expect(leg.amountSats).toBe(5)
    expect(leg.settlementReceiptRef).not.toBeNull()
    expect(dispatchCount.value).toBe(1)

    // The receipt is dereferenceable in the ledger and shaped realBitcoinMoved.
    const receipt = await ledger.readPaymentAuthorityReceiptByRef(
      leg.settlementReceiptRef!,
    )
    expect(receipt).toBeDefined()
    const projection = JSON.parse(receipt!.publicProjectionJson)
    expect(projection.state).toBe('settled')
    expect(projection.moneyMovement).toBe('real_bitcoin')
    expect(projection.contributorRef).toBe(GUINEA_PIG_NODE_REF)
  })

  it('is INERT by default: the disabled real-settlement gate authorizes nothing', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: disabledTassadarRealSettlementGate,
      ledger,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const outcome = await Effect.runPromise(
      settleVerifiedServingPayout(deps, {
        decision: armedDecision(),
        parityVerified: guineaPigReceipt.parityVerified,
        servedModel: guineaPigReceipt.servedModel,
      }),
    )

    expect(outcome.legs[0]!.settled).toBe(false)
    expect(outcome.legs[0]!.realBitcoinMoved).toBe(false)
    expect(outcome.legs[0]!.skipped).toBe('gate_not_authorized')
    expect(dispatchCount.value).toBe(0)
    expect(ledger.receipts.size).toBe(0)
  })

  it('is IDEMPOTENT: replaying the same verified serving payout never double-pays', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      ledger,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const args = {
      decision: armedDecision(),
      parityVerified: guineaPigReceipt.parityVerified,
      servedModel: guineaPigReceipt.servedModel,
    }
    await Effect.runPromise(settleVerifiedServingPayout(deps, args))
    await Effect.runPromise(settleVerifiedServingPayout(deps, args))

    expect(dispatchCount.value).toBe(1)
    expect(ledger.receipts.size).toBe(1)
  })

  it('SKIPS parity-unverified work (born-verified gate fails closed)', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      ledger,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const outcome = await Effect.runPromise(
      settleVerifiedServingPayout(deps, {
        decision: armedDecision(),
        parityVerified: false,
        servedModel: guineaPigReceipt.servedModel,
      }),
    )

    expect(outcome.legs[0]!.skipped).toBe('parity_unverified')
    expect(dispatchCount.value).toBe(0)
  })

  it('SKIPS cleanly when the contributor has no registered Spark target', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      ledger,
      targets: new Map(), // no target registered
    })

    const outcome = await Effect.runPromise(
      settleVerifiedServingPayout(deps, {
        decision: armedDecision(),
        parityVerified: guineaPigReceipt.parityVerified,
        servedModel: guineaPigReceipt.servedModel,
      }),
    )

    expect(outcome.legs[0]!.skipped).toBe('no_payout_destination')
    expect(outcome.legs[0]!.settled).toBe(false)
    expect(dispatchCount.value).toBe(0)
  })

  it('FAILS CLOSED at the daily ceiling: an over-budget leg falls back to skip', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      // daily cap 4 sats, payout would be 5 sats -> over budget
      gate: armedRealSettlementGate({ maxDailyPayoutSats: 4 }),
      ledger,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const outcome = await Effect.runPromise(
      settleVerifiedServingPayout(deps, {
        decision: armedDecision(),
        parityVerified: guineaPigReceipt.parityVerified,
        servedModel: guineaPigReceipt.servedModel,
      }),
    )

    expect(outcome.legs[0]!.skipped).toBe('daily_budget_exhausted')
    expect(dispatchCount.value).toBe(0)
  })

  it('SKIPS a sub-1-sat dust share as amount_not_positive (no dust moves)', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      ledger,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    // 500 msat = 0.5 sat -> floors to 0 sats.
    const outcome = await Effect.runPromise(
      settleVerifiedServingPayout(deps, {
        decision: armedDecision(500),
        parityVerified: guineaPigReceipt.parityVerified,
        servedModel: guineaPigReceipt.servedModel,
      }),
    )

    expect(outcome.legs[0]!.skipped).toBe('amount_not_positive')
    expect(dispatchCount.value).toBe(0)
  })

  it('the production default MDK gate keeps the #5484 decision unarmed (no settlement attempted)', async () => {
    // When the upstream decision is NOT armed (default disabled MDK gate), it
    // produces an empty/blocked split; the settlement path has nothing to pay.
    const unarmed = decideServingNodePayout({
      contributorCutMsat: 5_000,
      payoutGate: hostedMdkDirectPayoutDisabledGate(),
      receipt: guineaPigReceipt,
      resaleRefs: fullResaleRefs,
      revenueAsset: 'bitcoin',
    })
    expect(unarmed.armed).toBe(false)
  })
})
