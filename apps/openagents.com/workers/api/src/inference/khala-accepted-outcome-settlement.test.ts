// Khala M3 — verified ACCEPTED-OUTCOME -> worker + validator Bitcoin settlement
// (#6011, EPIC #6017). Proves the issue's "Done when": a verified accepted outcome
// settles sats to the serving worker AND the validator with public settlement receipts;
// the path is INERT by default (owner gate OFF), idempotent (a re-verify never
// double-pays), and reuses the SAME proven Spark settlement engine (no parallel money
// path). The guinea-pig Pylon's real Spark address is read from the gitignored
// `.secrets/khala-test-payout.env` when present (placeholder fallback in CI) and only
// ever handed to the test destination resolver — never asserted on, never committed.

import { readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

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
import {
  type KhalaAcceptedOutcome,
  acceptedOutcomeServingRunRef,
  buildAcceptedOutcomeDecision,
  settleVerifiedAcceptedOutcome,
  summarizeAcceptedOutcomeSettlement,
} from './khala-accepted-outcome-settlement'
import { type KhalaSettlementDeps } from './khala-verified-work-settlement'
import {
  KHALA_CODE_ACCEPTED_OUTCOME_PRICE,
  acceptedOutcomeSettlementShares,
  lookupAcceptedOutcomePrice,
} from './pricing'
import { KHALA_CODE_MODEL_ID } from './pricing'
import {
  authorizeInferenceMonetization,
} from '../inference-resale-authorization'
import { validateAssetBoundary } from '../asset-bitcoin-boundary'

const nowIso = '2026-06-22T18:00:00.000Z'

// The serving WORKER (guinea-pig Pylon, paid first) and the VALIDATOR (the verifier).
const WORKER_REF = 'pylon.khala.guinea_pig'
const VALIDATOR_REF = 'khala-code-crossy-road-verifier'

const VERIFICATION_RECEIPT_REF =
  'receipt.inference.khala_code.verification.req_abc.fnv1a32.deadbeef'

// The derived serving-run ref the settlement (and the gate allowlist) key on.
const DERIVED_RUN_REF = acceptedOutcomeServingRunRef(VERIFICATION_RECEIPT_REF)

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
        if (value !== '') return value
      }
    }
  } catch {
    // File absent (CI): fall through to the placeholder.
  }
  return 'spark1guineapigplaceholderxxxxxxxxxxxxxxxxxxxxxxx'
}

// A VERIFIED + EXECUTED accepted outcome for khala-code (the payable trigger).
const verifiedOutcome = (
  overrides: Partial<KhalaAcceptedOutcome> = {},
): KhalaAcceptedOutcome => ({
  executed: true,
  requestId: 'req_abc',
  scalarReward: 1,
  servedModel: KHALA_CODE_MODEL_ID,
  validatorRef: VALIDATOR_REF,
  verificationReceiptRef: VERIFICATION_RECEIPT_REF,
  verified: true,
  workerRef: WORKER_REF,
  ...overrides,
})

// The owner real-settlement gate, ARMED in TEST mode with run-scoped streaming so both
// the worker and the validator contributor refs qualify on the allowlisted run. Tiny +
// treasury-bounded caps — this is real money.
const armedRealSettlementGate = (
  overrides: Partial<TassadarRealSettlementGate> = {},
): TassadarRealSettlementGate => ({
  allowedAdapterKind: 'spark_treasury',
  allowedContributorRefs: [],
  allowedRunRefs: [DERIVED_RUN_REF],
  enabled: true,
  maxDailyPayoutSats: 100,
  maxPayoutSats: 100,
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

// A test dispatch that mirrors the proven receipt-first idempotent Spark dispatch:
// short-circuit if the settlement receipt already exists (idempotency), else count the
// call and persist the realBitcoinMoved-shaped settlement receipt. MOCKED — no real
// Bitcoin ever moves in tests.
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
  settlementRunRef: DERIVED_RUN_REF,
})

const armedTargets = () =>
  new Map([
    [WORKER_REF, readGuineaPigSparkAddress()],
    [VALIDATOR_REF, readGuineaPigSparkAddress()],
  ])

describe('accepted-outcome pricing (per accepted outcome alongside per-token)', () => {
  it('khala-code has an accepted-outcome price that splits worker/validator and conserves', () => {
    const price = lookupAcceptedOutcomePrice(KHALA_CODE_MODEL_ID)
    expect(price).toBeDefined()
    expect(price!.priceMsat).toBe(KHALA_CODE_ACCEPTED_OUTCOME_PRICE.priceMsat)

    const { workerMsat, validatorMsat } = acceptedOutcomeSettlementShares(price!)
    // 5_000 msat split 60/40 = 3000 / 2000, summing EXACTLY to the price (no dust).
    expect(workerMsat).toBe(3_000)
    expect(validatorMsat).toBe(2_000)
    expect(workerMsat + validatorMsat).toBe(price!.priceMsat)
  })

  it('non-accepted-outcome models have no accepted-outcome price (per-token only)', () => {
    expect(lookupAcceptedOutcomePrice('openagents/khala-mini')).toBeUndefined()
    expect(lookupAcceptedOutcomePrice('gpt-oss-20b')).toBeUndefined()
  })
})

describe('buildAcceptedOutcomeDecision (two-party split)', () => {
  it('builds a worker-first, validator-second two-share decision from the price', () => {
    const decision = buildAcceptedOutcomeDecision({
      outcome: verifiedOutcome(),
      price: KHALA_CODE_ACCEPTED_OUTCOME_PRICE,
    })
    expect(decision.servingRunRef).toBe(DERIVED_RUN_REF)
    expect(decision.split.shares).toHaveLength(2)
    // Worker is paid FIRST (the guinea-pig Pylon).
    expect(decision.split.shares[0]!.nodeRef).toBe(WORKER_REF)
    expect(decision.split.shares[1]!.nodeRef).toBe(VALIDATOR_REF)
    // 3 sats worker, 2 sats validator (msat in the share, floored to sat by the engine).
    expect(decision.split.shares[0]!.amountMsat).toBe(3_000)
    expect(decision.split.shares[1]!.amountMsat).toBe(2_000)
  })
})

describe('settleVerifiedAcceptedOutcome — worker + validator paid (TEST-armed)', () => {
  it('settles real sats to BOTH the worker and the validator with dereferenceable receipts', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      ledger,
      targets: armedTargets(),
    })

    const result = await Effect.runPromise(
      settleVerifiedAcceptedOutcome(deps, verifiedOutcome()),
    )

    expect(result.eligible).toBe(true)
    expect(result.settlement).not.toBeNull()
    expect(result.settlement!.legs).toHaveLength(2)

    const [workerLeg, validatorLeg] = result.settlement!.legs
    expect(workerLeg!.contributorRef).toBe(WORKER_REF)
    expect(workerLeg!.settled).toBe(true)
    expect(workerLeg!.realBitcoinMoved).toBe(true)
    expect(workerLeg!.amountSats).toBe(3)
    expect(validatorLeg!.contributorRef).toBe(VALIDATOR_REF)
    expect(validatorLeg!.settled).toBe(true)
    expect(validatorLeg!.realBitcoinMoved).toBe(true)
    expect(validatorLeg!.amountSats).toBe(2)

    // One dispatch per party.
    expect(dispatchCount.value).toBe(2)

    // The settled summary surfaces settled:true + BOTH receipt refs + both parties.
    const summary = summarizeAcceptedOutcomeSettlement(verifiedOutcome(), result)
    expect(summary.settled).toBe(true)
    expect(summary.settlementReceiptRefs).toHaveLength(2)
    expect(summary.settledParties).toEqual(['serving_worker', 'validator'])

    // Both receipts are dereferenceable + realBitcoinMoved-shaped; no raw Spark address.
    for (const ref of summary.settlementReceiptRefs) {
      const receipt = await ledger.readPaymentAuthorityReceiptByRef(ref)
      expect(receipt).toBeDefined()
      const projection = JSON.parse(receipt!.publicProjectionJson)
      expect(projection.state).toBe('settled')
      expect(projection.moneyMovement).toBe('real_bitcoin')
      expect(receipt!.publicProjectionJson).not.toMatch(/spark1/i)
    }
  })

  it('is IDEMPOTENT: a re-verify of the same accepted outcome never double-pays', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      ledger,
      targets: armedTargets(),
    })

    await Effect.runPromise(
      settleVerifiedAcceptedOutcome(deps, verifiedOutcome()),
    )
    expect(dispatchCount.value).toBe(2)

    // Second settle for the SAME accepted outcome (a re-verify / redelivered callback):
    // the receipt refs are deterministic from the verification receipt ref, so the
    // receipt-first dispatch short-circuits — NO second payout.
    const second = await Effect.runPromise(
      settleVerifiedAcceptedOutcome(deps, verifiedOutcome()),
    )
    expect(dispatchCount.value).toBe(2)
    // The legs still report settled (the receipts already exist) but moved no new money.
    expect(second.settlement!.legs.every(l => l.settled)).toBe(true)
    expect(ledger.receipts.size).toBe(2)
  })
})

describe('settleVerifiedAcceptedOutcome — fail-closed + inert by default', () => {
  it('is INERT by default: the disabled owner gate pays neither party (no money moves)', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: disabledTassadarRealSettlementGate,
      ledger,
      targets: armedTargets(),
    })

    const result = await Effect.runPromise(
      settleVerifiedAcceptedOutcome(deps, verifiedOutcome()),
    )
    expect(result.eligible).toBe(true)
    expect(result.settlement!.legs.every(l => !l.settled)).toBe(true)
    expect(
      result.settlement!.legs.every(l => l.skipped === 'gate_not_authorized'),
    ).toBe(true)
    expect(dispatchCount.value).toBe(0)
    expect(ledger.receipts.size).toBe(0)

    const summary = summarizeAcceptedOutcomeSettlement(verifiedOutcome(), result)
    expect(summary.settled).toBe(false)
    expect(summary.settlementReceiptRefs).toHaveLength(0)
  })

  it('an UNVERIFIED or un-executed outcome is INELIGIBLE — never pays', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      ledger,
      targets: armedTargets(),
    })

    const unverified = await Effect.runPromise(
      settleVerifiedAcceptedOutcome(deps, verifiedOutcome({ verified: false })),
    )
    expect(unverified.eligible).toBe(false)
    expect(unverified.settlement).toBeNull()

    const unexecuted = await Effect.runPromise(
      settleVerifiedAcceptedOutcome(deps, verifiedOutcome({ executed: false })),
    )
    expect(unexecuted.eligible).toBe(false)

    expect(dispatchCount.value).toBe(0)
    expect(ledger.receipts.size).toBe(0)
  })

  it('a model with no accepted-outcome price is INELIGIBLE', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      ledger,
      targets: armedTargets(),
    })

    const result = await Effect.runPromise(
      settleVerifiedAcceptedOutcome(
        deps,
        verifiedOutcome({ servedModel: 'openagents/khala-mini' }),
      ),
    )
    expect(result.eligible).toBe(false)
    expect(dispatchCount.value).toBe(0)
  })
})

describe('RL-3 — Khala api-inference-resale authorized + asset boundary holds', () => {
  // The full ref chain proving the api_inference_gateway_resale lane (Khala is the
  // ALLOWED case; the no-resale restriction is scoped to SUBSCRIPTION accounts).
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

  it('AUTHORIZES Khala api_inference_gateway_resale on our own api_key account', () => {
    const decision = authorizeInferenceMonetization({
      accountAuthMode: 'api_key',
      kind: 'api_inference_gateway_resale',
      refs: fullResaleRefs,
    })
    expect(decision.authorized).toBe(true)
    expect(decision.blockerRefs).toHaveLength(0)
  })

  it('still FORBIDS subscription-seat resale (the non-waivable lane)', () => {
    const decision = authorizeInferenceMonetization({
      kind: 'subscription_capacity_resale',
    })
    expect(decision.authorized).toBe(false)
  })

  it('asset boundary ALLOWS bitcoin revenue -> bitcoin worker/validator share', () => {
    expect(
      validateAssetBoundary({
        contributorAsset: 'bitcoin',
        movement: 'payout',
        revenueAsset: 'bitcoin',
      }),
    ).toBeNull()
  })

  it('asset boundary DENIES credit/free revenue -> withdrawable bitcoin share', () => {
    expect(
      validateAssetBoundary({
        contributorAsset: 'bitcoin',
        movement: 'payout',
        revenueAsset: 'credit',
      }),
    ).not.toBeNull()
    expect(
      validateAssetBoundary({
        contributorAsset: 'bitcoin',
        movement: 'payout',
        revenueAsset: 'free',
      }),
    ).not.toBeNull()
  })
})
