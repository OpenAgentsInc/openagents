import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
  TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
  TASSADAR_ALM_LINKED_DENSE_MODULE_KIND,
  tassadarLinkedDenseProgramFixture,
} from '@openagentsinc/tassadar-executor/linked-dense-module'
import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutLedgerStore,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import {
  type TassadarAutoSettlementDeps,
  TassadarCompiledModuleConstructionRewardSats,
  TassadarPerWindowValidatorRewardSats,
  TassadarPerWindowWorkerRewardSats,
  autoSettleVerifiedCompiledModuleConstruction,
  autoSettleVerifiedPair,
  buildTassadarAutoSettlementRequest,
  buildTassadarCompiledModuleConstructionSettlementRequest,
} from './tassadar-auto-settlement'
import { runTassadarReplayValidation } from './tassadar-replay-validator'
import {
  type TassadarRunSettlementRecords,
  buildTassadarRunSettlement,
} from './tassadar-run-settlement'
import type { TassadarRealSettlementGate } from './tassadar-run-settlement-gate'
import { buildTrainingRunRecord } from './training-run-window-authority'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

const nowIso = '2026-06-17T10:05:00.000Z'

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
  challengeRef: 'challenge.tassadar.autostream.1',
  commitmentRefs: ['commitment.tassadar.1'],
  contributionRef: 'contribution.tassadar.1',
  createdAt: '2026-06-17T10:02:00.000Z',
  failureCodes: [],
  homeworkKind: 'admin_dispatched_homework',
  id: 'challenge_1',
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
  verdictRefs: ['verdict.tassadar.1'],
  verificationClass: 'exact_trace_replay',
  verifiedAt: '2026-06-17T10:03:00.000Z',
  windowRef: 'training.window.tassadar.executor.20260615.w1',
})

const verifiedConstructionChallenge = (
  overrides: Partial<TrainingVerificationChallengeRecord> = {},
): TrainingVerificationChallengeRecord => ({
  ...verifiedChallenge(),
  challengeRef: 'challenge.tassadar.compiled_module_construction.1',
  commitmentRefs: [
    `commitment.tassadar.compiled_module.${TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST.slice(0, 16)}`,
  ],
  contributionRef:
    `contribution.tassadar.compiled_module.${TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST.slice(0, 16)}`,
  payloadJson: JSON.stringify({
    moduleDigest: TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
    moduleKind: TASSADAR_ALM_LINKED_DENSE_MODULE_KIND,
    replayedTraceDigest: TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
  }),
  verdictRefs: [
    `verdict.tassadar.compiled_module.${TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST.slice(0, 16)}`,
  ],
  ...overrides,
})

const activeLease = (): TrainingWindowLeaseRecord => ({
  claimedAt: '2026-06-17T10:01:00.000Z',
  id: 'lease1',
  leaseExpiresAt: '2026-06-17T12:00:00.000Z',
  leaseRef: 'lease.tassadar.1',
  publicProjectionJson: '{}',
  pylonRef: 'pylon.worker.orrery',
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

// A test dispatch that simulates the receipt-first confirmed-real path: it
// persists the builder's real-bitcoin settlement receipt to the ledger and
// counts each call so we can assert idempotency (one settlement per leg).
const makeDeps = (
  options: Readonly<{
    gate: TassadarRealSettlementGate
    ledger: MemoryLedgerStore
    targets: ReadonlyMap<string, string>
    dispatchCount: { value: number }
  }>,
): TassadarAutoSettlementDeps<Record<string, unknown>> => ({
  dispatchRealSettlement: input =>
    Effect.gen(function* () {
      // Idempotent short-circuit, mirroring dispatchRealRunSettlementCore.
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
  run: run(),
})

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

describe('autoSettleVerifiedPair — both legs (#5309 + #5310)', () => {
  it('settles BOTH worker 5 sats and validator 5 sats on a Verified pair', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedGate(),
      ledger,
      targets: new Map([
        ['pylon.worker.orrery', 'spark1workeraddrxxxxxxxxxxxxxxxxxxxxxxxxxxxx'],
        [
          'pylon.validator.whitefang',
          'spark1validatoraddrxxxxxxxxxxxxxxxxxxxxxxxxx',
        ],
      ]),
    })

    const outcome = await Effect.runPromise(
      autoSettleVerifiedPair(deps, {
        challenge: verifiedChallenge(),
        lease: activeLease(),
        validatorContributorRef: 'pylon.validator.whitefang',
      }),
    )

    expect(outcome.legs).toHaveLength(2)
    const worker = outcome.legs.find(l => l.party === 'worker')!
    const validator = outcome.legs.find(l => l.party === 'validator')!
    expect(worker.settled).toBe(true)
    expect(worker.amountSats).toBe(TassadarPerWindowWorkerRewardSats)
    expect(validator.settled).toBe(true)
    expect(validator.amountSats).toBe(TassadarPerWindowValidatorRewardSats)
    expect(dispatchCount.value).toBe(2)
    expect(ledger.receipts.size).toBe(2)
  })

  it('is IDEMPOTENT: replaying the same Verified pair never double-pays', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedGate(),
      ledger,
      targets: new Map([
        ['pylon.worker.orrery', 'spark1workeraddr'],
        ['pylon.validator.whitefang', 'spark1validatoraddr'],
      ]),
    })

    await Effect.runPromise(
      autoSettleVerifiedPair(deps, {
        challenge: verifiedChallenge(),
        lease: activeLease(),
        validatorContributorRef: 'pylon.validator.whitefang',
      }),
    )
    await Effect.runPromise(
      autoSettleVerifiedPair(deps, {
        challenge: verifiedChallenge(),
        lease: activeLease(),
        validatorContributorRef: 'pylon.validator.whitefang',
      }),
    )

    // Two legs settled once each; the replay dispatched nothing further.
    expect(dispatchCount.value).toBe(2)
    expect(ledger.receipts.size).toBe(2)
  })

  it('SKIPS the validator leg cleanly when it has no registered payout target', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedGate(),
      ledger,
      // worker has a target; validator does NOT
      targets: new Map([['pylon.worker.orrery', 'spark1workeraddr']]),
    })

    const outcome = await Effect.runPromise(
      autoSettleVerifiedPair(deps, {
        challenge: verifiedChallenge(),
        lease: activeLease(),
        validatorContributorRef: 'pylon.validator.no_target',
      }),
    )

    const worker = outcome.legs.find(l => l.party === 'worker')!
    const validator = outcome.legs.find(l => l.party === 'validator')!
    expect(worker.settled).toBe(true)
    expect(validator.settled).toBe(false)
    expect(validator.skipped).toBe('no_payout_destination')
    // No error thrown; the worker still settled.
    expect(dispatchCount.value).toBe(1)
  })

  it('FAILS CLOSED at the daily ceiling: a leg over budget falls back to skip', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    // Daily cap of 5 sats: the worker leg (5) lands exactly on the ceiling and
    // settles; the validator leg (5) would exceed it and is skipped.
    const deps = makeDeps({
      dispatchCount,
      gate: armedGate({ maxDailyPayoutSats: 5 }),
      ledger,
      targets: new Map([
        ['pylon.worker.orrery', 'spark1workeraddr'],
        ['pylon.validator.whitefang', 'spark1validatoraddr'],
      ]),
    })

    const outcome = await Effect.runPromise(
      autoSettleVerifiedPair(deps, {
        challenge: verifiedChallenge(),
        lease: activeLease(),
        validatorContributorRef: 'pylon.validator.whitefang',
      }),
    )

    const worker = outcome.legs.find(l => l.party === 'worker')!
    const validator = outcome.legs.find(l => l.party === 'validator')!
    expect(worker.settled).toBe(true)
    expect(validator.settled).toBe(false)
    expect(validator.skipped).toBe('daily_budget_exhausted')
    expect(dispatchCount.value).toBe(1)
  })

  it('counts already-settled real sats today against the daily budget', async () => {
    const ledger = new MemoryLedgerStore()
    // Pre-seed a real-settled receipt earlier today consuming most of the cap.
    await ledger.createPaymentAuthorityReceipt({
      archivedAt: null,
      audience: 'public',
      createdAt: '2026-06-17T09:00:00.000Z',
      eventRef: 'event.prior',
      id: 'receipt_prior',
      metadataRefs: [],
      payoutAttemptRef: 'attempt.prior',
      payoutIntentRef: 'intent.prior',
      publicProjectionJson: JSON.stringify({
        amountSats: 8,
        moneyMovement: 'real_bitcoin',
        state: 'settled',
      }),
      receiptKind: 'settlement_recorded',
      receiptRef: 'receipt.prior',
    })
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedGate({ maxDailyPayoutSats: 10 }),
      ledger,
      targets: new Map([
        ['pylon.worker.orrery', 'spark1workeraddr'],
        ['pylon.validator.whitefang', 'spark1validatoraddr'],
      ]),
    })

    const outcome = await Effect.runPromise(
      autoSettleVerifiedPair(deps, {
        challenge: verifiedChallenge(),
        lease: activeLease(),
        validatorContributorRef: 'pylon.validator.whitefang',
      }),
    )

    // 8 already + 5 worker = 13 > 10 -> worker itself is skipped (fail-closed).
    const worker = outcome.legs.find(l => l.party === 'worker')!
    expect(worker.settled).toBe(false)
    expect(worker.skipped).toBe('daily_budget_exhausted')
    expect(dispatchCount.value).toBe(0)
  })

  it('no-ops when the gate is OFF (skip everywhere by default)', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: { ...armedGate(), enabled: false },
      ledger,
      targets: new Map([['pylon.worker.orrery', 'spark1workeraddr']]),
    })

    const outcome = await Effect.runPromise(
      autoSettleVerifiedPair(deps, {
        challenge: verifiedChallenge(),
        lease: activeLease(),
        validatorContributorRef: 'pylon.validator.whitefang',
      }),
    )

    expect(outcome.legs.every(l => l.skipped === 'gate_not_authorized')).toBe(
      true,
    )
    expect(dispatchCount.value).toBe(0)
  })

  it('no-ops on a non-Verified challenge', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: armedGate(),
      ledger,
      targets: new Map([['pylon.worker.orrery', 'spark1workeraddr']]),
    })

    const outcome = await Effect.runPromise(
      autoSettleVerifiedPair(deps, {
        challenge: { ...verifiedChallenge(), state: 'Rejected' },
        lease: activeLease(),
        validatorContributorRef: 'pylon.validator.whitefang',
      }),
    )

    expect(outcome.legs).toHaveLength(0)
    expect(dispatchCount.value).toBe(0)
  })

  it('FAIL-SOFT: a dispatch failure never throws into the caller', async () => {
    const ledger = new MemoryLedgerStore()
    const deps: TassadarAutoSettlementDeps<Record<string, unknown>> = {
      dispatchRealSettlement: () =>
        Effect.fail({
          _tag: 'TestDispatchFailure' as const,
          reason: 'spark container unreachable',
        }),
      ledger,
      nowIso,
      readGate: () => armedGate(),
      resolvePayoutDestination: async () => 'spark1workeraddr',
      run: run(),
    }

    const outcome = await Effect.runPromise(
      autoSettleVerifiedPair(deps, {
        challenge: verifiedChallenge(),
        lease: activeLease(),
        validatorContributorRef: 'pylon.validator.whitefang',
      }),
    )

    expect(outcome.legs.every(l => l.skipped === 'settlement_failed')).toBe(
      true,
    )
    expect(outcome.legs.every(l => l.settled === false)).toBe(true)
  })
})

describe('autoSettleVerifiedCompiledModuleConstruction (#5326)', () => {
  it('replay-verifies a non-loop linked dense module and pays construction in simulation mode by default', async () => {
    const replay = await runTassadarReplayValidation({
      assignmentRef:
        `assignment.tassadar.compiled_module.${TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST.slice(0, 16)}`,
      claimedTraceDigest: TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
      pylonDeviceRef: 'device.pylon.constructor.1',
      workload: {
        denseModule: tassadarLinkedDenseProgramFixture.linkedModule
          .composedDenseModule as unknown as Record<string, unknown>,
        model: {},
        steps: tassadarLinkedDenseProgramFixture.steps.map(step => [...step]),
      },
    })

    expect(replay.outcome).toBe('verified')
    expect(replay.replayedTraceDigest).toBe(
      TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
    )
    expect(
      tassadarLinkedDenseProgramFixture.linkedModule.banks.some(
        bank => bank.programId === 'tassadar_corpus.loop_sum_v1',
      ),
    ).toBe(false)

    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: { ...armedGate(), enabled: false },
      ledger,
      targets: new Map(),
    })
    const challenge = verifiedConstructionChallenge()
    const outcome = await Effect.runPromise(
      autoSettleVerifiedCompiledModuleConstruction(deps, {
        challenge,
        lease: activeLease(),
        moduleDigest: TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
        moduleKind: TASSADAR_ALM_LINKED_DENSE_MODULE_KIND,
      }),
    )

    expect(outcome.settled).toBe(true)
    expect(outcome.adapterKind).toBe('simulation')
    expect(outcome.mode).toBe('unpaid_smoke_simulation')
    expect(outcome.realBitcoinMoved).toBe(false)
    expect(outcome.realSettlementAuthorized).toBe(false)
    expect(outcome.realSettlementBlocker).toBe('gate_disabled')
    expect(outcome.amountSats).toBe(TassadarCompiledModuleConstructionRewardSats)
    expect(outcome.skipped).toBe(null)
    expect(dispatchCount.value).toBe(0)
    expect(ledger.receipts.size).toBe(1)

    const receipt = ledger.receipts.get(outcome.settlementReceiptRef!)!
    const projection = JSON.parse(receipt.publicProjectionJson) as {
      constructionSettlement: boolean
      moduleDigest: string
      moduleKind: string
      moneyMovement: string
      settlementSource: string
      state: string
      verificationChallengeRef: string
    }
    expect(projection.state).toBe('settled')
    expect(projection.moneyMovement).toBe('none')
    expect(projection.constructionSettlement).toBe(true)
    expect(projection.settlementSource).toBe('compiled_module_construction')
    expect(projection.moduleDigest).toBe(TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST)
    expect(projection.moduleKind).toBe(TASSADAR_ALM_LINKED_DENSE_MODULE_KIND)
    expect(projection.verificationChallengeRef).toBe(challenge.challengeRef)
    expect(receipt.metadataRefs).toContain(challenge.contributionRef)
    expect(receipt.publicProjectionJson).not.toMatch(/spark1|invoice|preimage/i)
  })

  it('is idempotent for construction simulation receipts', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: { ...armedGate(), enabled: false },
      ledger,
      targets: new Map(),
    })
    const input = {
      challenge: verifiedConstructionChallenge(),
      lease: activeLease(),
      moduleDigest: TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
      moduleKind: TASSADAR_ALM_LINKED_DENSE_MODULE_KIND,
    } as const

    const first = await Effect.runPromise(
      autoSettleVerifiedCompiledModuleConstruction(deps, input),
    )
    const second = await Effect.runPromise(
      autoSettleVerifiedCompiledModuleConstruction(deps, input),
    )

    expect(first.settled).toBe(true)
    expect(first.idempotent).toBe(false)
    expect(second.settled).toBe(true)
    expect(second.idempotent).toBe(true)
    expect(second.settlementReceiptRef).toBe(first.settlementReceiptRef)
    expect(ledger.intents.size).toBe(1)
    expect(ledger.attempts.size).toBe(1)
    expect(ledger.events.size).toBe(1)
    expect(ledger.receipts.size).toBe(1)
    expect(dispatchCount.value).toBe(0)
  })

  it('is fail-soft when construction settlement exceeds the run spend cap', async () => {
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const deps = makeDeps({
      dispatchCount,
      gate: { ...armedGate(), enabled: false },
      ledger,
      targets: new Map(),
    })

    const outcome = await Effect.runPromise(
      autoSettleVerifiedCompiledModuleConstruction(deps, {
        amountSats: 101,
        challenge: verifiedConstructionChallenge(),
        lease: activeLease(),
        moduleDigest: TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
        moduleKind: TASSADAR_ALM_LINKED_DENSE_MODULE_KIND,
      }),
    )

    expect(outcome.settled).toBe(false)
    expect(outcome.skipped).toBe('settlement_policy_blocked')
    expect(ledger.receipts.size).toBe(0)
    expect(dispatchCount.value).toBe(0)
  })
})

describe('redaction-safe deterministic requests (#5309/#5310)', () => {
  it('derives distinct deterministic public-safe refs per party', () => {
    const worker = buildTassadarAutoSettlementRequest({
      amountSats: 5,
      challengeRef: 'challenge.tassadar.autostream.1',
      leaseRef: 'lease.tassadar.1',
      party: 'worker',
    })
    const validator = buildTassadarAutoSettlementRequest({
      amountSats: 5,
      challengeRef: 'challenge.tassadar.autostream.1',
      leaseRef: 'lease.tassadar.1',
      party: 'validator',
    })

    expect(worker.idempotencyRef).not.toBe(validator.idempotencyRef)
    expect(worker.adapterKind).toBe('spark_treasury')
    // No raw spark addresses anywhere in the request refs.
    expect(JSON.stringify(worker)).not.toMatch(/spark1/)
    expect(JSON.stringify(validator)).not.toMatch(/spark1/)
  })

  it('derives public-safe refs for compiled-module construction settlement', () => {
    const request = buildTassadarCompiledModuleConstructionSettlementRequest({
      adapterKind: 'simulation',
      amountSats: 25,
      challengeRef: 'challenge.tassadar.compiled_module_construction.1',
      constructionContributionRef:
        `contribution.tassadar.compiled_module.${TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST.slice(0, 16)}`,
      leaseRef: 'lease.tassadar.1',
      moduleDigest: TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
    })

    expect(request.adapterKind).toBe('simulation')
    expect(request.idempotencyRef).toContain(
      'idempotency.tassadar.compiled_module_construction',
    )
    expect(request.operatorApprovalRef).toBe(
      'operator_approval.tassadar.compiled_module_construction.v1',
    )
    expect(JSON.stringify(request)).not.toMatch(/spark1|invoice|preimage/i)
  })

  it('produces a settlement receipt projection with NO raw address or payment material', () => {
    const settlement: TassadarRunSettlementRecords = buildTassadarRunSettlement({
      challenge: verifiedChallenge(),
      lease: activeLease(),
      nowIso,
      request: {
        ...buildTassadarAutoSettlementRequest({
          amountSats: 5,
          challengeRef: 'challenge.tassadar.autostream.1',
          leaseRef: 'lease.tassadar.1',
          party: 'worker',
        }),
        adapterKind: 'spark_treasury',
      },
      run: run(),
    })

    const projection = settlement.settlementReceipt.publicProjectionJson
    expect(projection).not.toMatch(/spark1/)
    expect(projection).not.toMatch(/preimage/i)
    expect(projection).toContain('real_bitcoin')
    expect(JSON.parse(projection).state).toBe('settled')
  })
})
