import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { parseJsonRecord } from './json-boundary'
import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutLedgerStore,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import {
  type PylonSparkPayoutTargetRecord,
  type PylonSparkPayoutTargetStore,
  resolveSparkPayoutDestination,
} from './pylon-api'
import {
  type TassadarAutoSettlementDeps,
  TassadarPerWindowValidatorRewardSats,
  TassadarPerWindowWorkerRewardSats,
  autoSettleVerifiedPair,
} from './tassadar-auto-settlement'
import type { TassadarRealSettlementGate } from './tassadar-run-settlement-gate'
import { buildTrainingRunRecord } from './training-run-window-authority'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

// L-2 PROOF (openagents #5394, EPIC #5392). The "anybody plugs in -> gets paid
// Bitcoin, no operator" claim hinges on the VALIDATOR leg of the verified-pair
// autostream resolving its OWN registered Spark payout target with NO operator
// step. The validator submits its verdict with its DEVICE-ref (its nodeId),
// which is NOT a `pylonRef`; before the #5310 device-ref backstop the
// production owner-resolver (readRegistration keyed on pylon_ref) returned
// undefined for that device-ref, so the validator leg skipped
// `no_payout_destination` and a human had to retro-settle. These tests exercise
// the REAL `resolveSparkPayoutDestination` chain wired exactly as index.ts
// wires it, proving the validator now resolves autonomously and both legs
// settle hands-off.

// Real shape from the live nodes: nodeId = `pylon_<hash>`,
// pylonRef = `pylon.<hash>` — DISTINCT strings (apps/pylon/src/state.ts).
const WORKER_PYLON_REF = 'pylon.worker81f0facfe7971870'
const WORKER_DEVICE_REF = 'pylon_workerab12cd34ef56'
const VALIDATOR_PYLON_REF = 'pylon.validator45b58c56783c'
const VALIDATOR_DEVICE_REF = 'pylon_validator9f8e7d6c5b4a'
const WORKER_OWNER = 'agent_orrery'
const VALIDATOR_OWNER = 'agent_whitefang'
const WORKER_SPARK = 'spark1pqqqqq0000000000000000000000000000workerorrery'
const VALIDATOR_SPARK = 'spark1pqqqqq0000000000000000000000000000validatorwf'

const nowIso = '2026-06-17T10:05:00.000Z'

// --- Minimal in-memory Spark target store (mirrors the D1 store contract) ---
class MemorySparkPayoutTargetStore implements PylonSparkPayoutTargetStore {
  records = new Map<string, PylonSparkPayoutTargetRecord>()

  upsert = async (record: PylonSparkPayoutTargetRecord) => {
    this.records.set(record.pylonRef, record)
    return record
  }

  read = async (pylonRef: string) => this.records.get(pylonRef)

  readByOwner = async (ownerAgentUserId: string) =>
    [...this.records.values()]
      .filter(record => record.ownerAgentUserId === ownerAgentUserId)
      .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))[0]
}

// --- Minimal registry + contribution device->pylon index, mirroring the
// production wiring in index.ts (readRegistration keyed on pylonRef; the
// device->worker-pylon map comes from training_trace_contributions). ---
const registrationOwners = new Map<string, string>([
  [WORKER_PYLON_REF, WORKER_OWNER],
  [VALIDATOR_PYLON_REF, VALIDATOR_OWNER],
])
// Each device acted as a worker under its own pylonRef on some window. This is
// exactly what `readMostRecentPylonRefByDeviceRef` returns from the
// contributions table.
const devicePylonRef = new Map<string, string>([
  [WORKER_DEVICE_REF, WORKER_PYLON_REF],
  [VALIDATOR_DEVICE_REF, VALIDATOR_PYLON_REF],
])

// The PRODUCTION owner-resolver after the fix (index.ts
// resolveContributorOwnerAgentUserId): pylonRef-first, device-ref fallback via
// the contributions device->worker-pylon index. Bound to the device's own
// historical worker pylon; never crosses ownership.
const resolveContributorOwnerAgentUserId = async (
  contributorRef: string,
): Promise<string | undefined> => {
  const direct = registrationOwners.get(contributorRef)

  if (direct !== undefined) {
    return direct
  }

  const pylonRefForDevice = devicePylonRef.get(contributorRef)

  if (pylonRefForDevice === undefined) {
    return undefined
  }

  return registrationOwners.get(pylonRefForDevice)
}

// The BUGGY pre-fix owner-resolver: readRegistration keyed on pylon_ref only.
const resolveOwnerPylonRefOnly = async (
  contributorRef: string,
): Promise<string | undefined> => registrationOwners.get(contributorRef)

const seededTargetStore = (): MemorySparkPayoutTargetStore => {
  const store = new MemorySparkPayoutTargetStore()
  // Both parties registered their Spark target under their pylonRef (the only
  // key the register route accepts) — NOT under their device-ref.
  store.records.set(WORKER_PYLON_REF, {
    pylonRef: WORKER_PYLON_REF,
    ownerAgentUserId: WORKER_OWNER,
    payoutTargetRef: 'payout.spark.workerdigest',
    rawSparkAddress: WORKER_SPARK,
    createdAt: '2026-06-16T00:00:00.000Z',
    updatedAt: '2026-06-16T00:00:00.000Z',
  })
  store.records.set(VALIDATOR_PYLON_REF, {
    pylonRef: VALIDATOR_PYLON_REF,
    ownerAgentUserId: VALIDATOR_OWNER,
    payoutTargetRef: 'payout.spark.validatordigest',
    rawSparkAddress: VALIDATOR_SPARK,
    createdAt: '2026-06-16T00:00:00.000Z',
    updatedAt: '2026-06-16T00:00:00.000Z',
  })
  return store
}

const run = (): TrainingRunRecord => ({
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
})

const verifiedChallenge = (): TrainingVerificationChallengeRecord => ({
  challengeRef: 'challenge.tassadar.autostream.l2.1',
  commitmentRefs: ['commitment.tassadar.l2.1'],
  contributionRef: 'contribution.tassadar.l2.1',
  createdAt: '2026-06-17T10:02:00.000Z',
  failureCodes: [],
  homeworkKind: 'admin_dispatched_homework',
  id: 'challenge_l2_1',
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
  verdictRefs: ['verdict.tassadar.l2.1'],
  verificationClass: 'exact_trace_replay',
  verifiedAt: '2026-06-17T10:03:00.000Z',
  windowRef: 'training.window.tassadar.executor.20260615.w1',
})

// The lease's pylonRef is the WORKER's verified registered pylonRef (the verdict
// route's requireOwnedLease guarantees this for the worker leg).
const activeLease = (): TrainingWindowLeaseRecord => ({
  claimedAt: '2026-06-17T10:01:00.000Z',
  id: 'lease_l2_1',
  leaseExpiresAt: '2026-06-17T12:00:00.000Z',
  leaseRef: 'lease.tassadar.l2.1',
  publicProjectionJson: '{}',
  pylonRef: WORKER_PYLON_REF,
  receiptRefs: [],
  state: 'active',
  trainingRunRef: 'run.tassadar.executor.20260615',
  windowRef: 'training.window.tassadar.executor.20260615.w1',
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

const armedGate = (
  overrides: Partial<TassadarRealSettlementGate> = {},
): TassadarRealSettlementGate => ({
  enabled: true,
  allowedAdapterKind: 'spark_treasury',
  allowedContributorRefs: [],
  allowedRunRefs: ['run.tassadar.executor.20260615'],
  maxPayoutSats: 100,
  maxDailyPayoutSats: 50_000,
  runScopedStreaming: true,
  ...overrides,
})

// Records-tracking dispatch that mirrors dispatchRealRunSettlementCore's
// receipt-first idempotent confirm, and asserts the per-leg PRIVATE destination
// it received resolved correctly (so the validator's own target was found, with
// NO operator step).
const makeDeps = (
  options: Readonly<{
    dispatchCount: { value: number }
    dispatchedDestinations: Array<string | undefined>
    ledger: MemoryLedgerStore
    resolveDestination: (ref: string) => Promise<string | undefined>
  }>,
): TassadarAutoSettlementDeps<Record<string, unknown>> => ({
  dispatchRealSettlement: input =>
    Effect.gen(function* () {
      const destination = yield* Effect.promise(() =>
        options.resolveDestination(input.contributorRef),
      )
      // The real dispatch fails closed without a vetted destination; replicate
      // that so a missing validator target would surface as a failed leg here.
      if (destination === undefined || destination.trim() === '') {
        return yield* Effect.fail({ _tag: 'NoDestination' as const })
      }
      options.dispatchedDestinations.push(destination)
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
  readGate: () => armedGate(),
  resolvePayoutDestination: options.resolveDestination,
  run: run(),
})

describe('L-2 autonomous validator payout resolution (#5394 / #5310 device-ref backstop)', () => {
  it('CONTROL (the #5306 bug): a pylonRef-only owner-resolver leaves the validator UNRESOLVED', async () => {
    const sparkStore = seededTargetStore()
    // Worker leg: lease.pylonRef IS the registered pylonRef -> resolves.
    expect(
      await resolveSparkPayoutDestination(
        sparkStore,
        WORKER_PYLON_REF,
        resolveOwnerPylonRefOnly,
      ),
    ).toBe(WORKER_SPARK)
    // Validator leg: contributorRef is the validator DEVICE-ref. The pylonRef-only
    // resolver cannot map it -> undefined -> the leg would skip no_payout_destination.
    expect(
      await resolveSparkPayoutDestination(
        sparkStore,
        VALIDATOR_DEVICE_REF,
        resolveOwnerPylonRefOnly,
      ),
    ).toBeUndefined()
  })

  it('FIX: the device-ref backstop resolves the validator OWN registered target (its own owning agent only)', async () => {
    const sparkStore = seededTargetStore()
    const resolved = await resolveSparkPayoutDestination(
      sparkStore,
      VALIDATOR_DEVICE_REF,
      resolveContributorOwnerAgentUserId,
    )
    expect(resolved).toBe(VALIDATOR_SPARK)
  })

  it('FIX is owner-bound: a device-ref whose worker-pylon owner has NO target fails closed (no cross-owner leak)', async () => {
    const sparkStore = seededTargetStore()
    // Remove the validator's registered target; the device-ref must NOT fall back
    // to the worker's (different-owner) target.
    sparkStore.records.delete(VALIDATOR_PYLON_REF)
    const resolved = await resolveSparkPayoutDestination(
      sparkStore,
      VALIDATOR_DEVICE_REF,
      resolveContributorOwnerAgentUserId,
    )
    expect(resolved).toBeUndefined()
  })

  it('END-TO-END: autoSettleVerifiedPair pays BOTH worker AND validator autonomously (NO operator step)', async () => {
    const sparkStore = seededTargetStore()
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const dispatchedDestinations: Array<string | undefined> = []
    const resolveDestination = (ref: string) =>
      resolveSparkPayoutDestination(
        sparkStore,
        ref,
        resolveContributorOwnerAgentUserId,
      )
    const deps = makeDeps({
      dispatchCount,
      dispatchedDestinations,
      ledger,
      resolveDestination,
    })

    const outcome = await Effect.runPromise(
      autoSettleVerifiedPair(deps, {
        challenge: verifiedChallenge(),
        lease: activeLease(),
        // The verdict route passes the validator DEVICE-ref here.
        validatorContributorRef: VALIDATOR_DEVICE_REF,
      }),
    )

    expect(outcome.legs).toHaveLength(2)
    const worker = outcome.legs.find(l => l.party === 'worker')!
    const validator = outcome.legs.find(l => l.party === 'validator')!
    expect(worker.settled).toBe(true)
    expect(worker.amountSats).toBe(TassadarPerWindowWorkerRewardSats)
    expect(validator.settled).toBe(true)
    expect(validator.amountSats).toBe(TassadarPerWindowValidatorRewardSats)
    expect(validator.skipped).toBe(null)
    // Both legs dispatched to the correct PRIVATE Spark destinations.
    expect(dispatchedDestinations).toContain(WORKER_SPARK)
    expect(dispatchedDestinations).toContain(VALIDATOR_SPARK)
    expect(dispatchCount.value).toBe(2)
    expect(ledger.receipts.size).toBe(2)
    // Receipts are real_bitcoin, settled, and carry NO raw payment material.
    const projections = [...ledger.receipts.values()].map(receipt => ({
      json: receipt.publicProjectionJson,
      parsed: parseJsonRecord(receipt.publicProjectionJson) ?? {},
    }))
    expect(projections).toHaveLength(2)
    expect(projections.every(p => p.parsed.state === 'settled')).toBe(true)
    expect(
      projections.every(p => p.parsed.moneyMovement === 'real_bitcoin'),
    ).toBe(true)
    expect(
      projections.every(p => !/spark1|invoice|preimage/i.test(p.json)),
    ).toBe(true)
  })

  it('END-TO-END is IDEMPOTENT: replaying the same Verified pair never double-pays', async () => {
    const sparkStore = seededTargetStore()
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const dispatchedDestinations: Array<string | undefined> = []
    const resolveDestination = (ref: string) =>
      resolveSparkPayoutDestination(
        sparkStore,
        ref,
        resolveContributorOwnerAgentUserId,
      )
    const deps = makeDeps({
      dispatchCount,
      dispatchedDestinations,
      ledger,
      resolveDestination,
    })

    await Effect.runPromise(
      autoSettleVerifiedPair(deps, {
        challenge: verifiedChallenge(),
        lease: activeLease(),
        validatorContributorRef: VALIDATOR_DEVICE_REF,
      }),
    )
    await Effect.runPromise(
      autoSettleVerifiedPair(deps, {
        challenge: verifiedChallenge(),
        lease: activeLease(),
        validatorContributorRef: VALIDATOR_DEVICE_REF,
      }),
    )

    expect(dispatchCount.value).toBe(2)
    expect(ledger.receipts.size).toBe(2)
  })
})
