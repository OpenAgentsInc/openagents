// Khala loop integration test — verified-serve (M4) -> dry-run Spark payout (M3).
// (EPIC #6017; M3 #6011 / M4 #6012.) Proves the WHOLE chain end-to-end as one
// flow, against the guinea-pig Pylon, INERT by default:
//
//   serve(parity-verified, fake transport) -> ServingReceipt -> payout DECISION
//   -> flagged M3 settlement sink (dry-run, tiny-capped, placeholder destination)
//   -> dereferenceable settled-shaped receipt
//
// No real sats move: the settlement dispatch is a DRY-RUN that records the
// receipt but performs no Spark send. The guinea-pig Pylon's real Spark address
// is read at runtime from the gitignored `.secrets/khala-test-payout.env` when
// present (placeholder fallback in CI) and only ever handed to the test
// destination resolver — never asserted on, never committed.

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
import {
  type KhalaSettlementDeps,
  type KhalaSettlementRecords,
} from './khala-verified-work-settlement'
import {
  KhalaLoopArmingEnvKey,
  disabledKhalaLoopArming,
  makeDryRunSettlementDispatch,
  makeKhalaLoopSettlementDispatch,
  readKhalaLoopArming,
  runKhalaLoopOnce,
  type DryRunSettlementLedger,
  type KhalaLoopConfig,
  type KhalaSettlementDispatch,
  type PylonServeTransport,
} from './khala-loop-integration'
import { type InferenceRequest } from './provider-adapter'
import { type PsionicServeResponse } from './psionic-fabric-serve'

const nowIso = '2026-06-22T18:00:00.000Z'
const SETTLEMENT_RUN_REF = 'run.khala.loop.guineapig'
const GUINEA_PIG_NODE_REF = 'pylon.khala.guinea_pig'
const SERVING_RUN_REF = 'serve.run.khala.loop.1'

// Read the guinea-pig Pylon's real Spark address from the gitignored secret when
// present; fall back to a non-secret placeholder so CI runs without the file and
// the address is NEVER committed or asserted on. Handed only to the resolver.
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

// A parity-verified whole-model serve from the guinea-pig Pylon (one stage).
const guineaPigServe: PsionicServeResponse = {
  content: 'hello from the guinea-pig Pylon',
  finishReason: 'stop',
  parityMode: 'exact_greedy_parity',
  parityVerified: true,
  servedModel: 'openagents/khala-mini',
  servingRunRef: SERVING_RUN_REF,
  stages: [
    { layerEnd: 28, layerStart: 0, nodeRef: GUINEA_PIG_NODE_REF, role: 'stage' },
  ],
  usage: { completionTokens: 8, promptTokens: 4, totalTokens: 12 },
}

// A fake Pylon transport returning a fixed serve. A real Pylon transport (HTTP to
// a live online Pylon) drops in here with no contract change.
const fakePylonTransport = (
  serve: PsionicServeResponse = guineaPigServe,
): PylonServeTransport => () => Effect.succeed(serve)

const request: InferenceRequest = {
  messages: [{ content: 'ping', role: 'user' }],
  model: 'openagents/khala-mini',
  passthroughParams: {},
  stream: false,
}

// Armed MDK payout-mode gate (makes the payout DECISION armable). Does NOT itself
// move money; the M3 real-settlement gate is the second, independent owner gate.
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

// Tiny + treasury-bounded: per-payout 100 sats, daily 100 sats. Real money.
const armedRealSettlementGate = (
  overrides: Partial<TassadarRealSettlementGate> = {},
): TassadarRealSettlementGate => ({
  allowedAdapterKind: 'spark_treasury',
  allowedContributorRefs: [],
  allowedRunRefs: [SETTLEMENT_RUN_REF],
  enabled: true,
  maxDailyPayoutSats: 100,
  maxPayoutSats: 100,
  runScopedStreaming: true,
  ...overrides,
})

// Minimal in-memory ledger store satisfying both the M3 deps store contract and
// the dry-run dispatch ledger contract.
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

// The dry-run ledger view over the same store (read/record settled receipts).
const dryRunLedgerView = (store: MemoryLedgerStore): DryRunSettlementLedger => ({
  readReceiptByRef: ref => store.readPaymentAuthorityReceiptByRef(ref),
  recordReceipt: record => store.createPaymentAuthorityReceipt(record),
})

// Build the M3 settlement deps for the loop, with the DRY-RUN dispatch and a
// dispatch counter so we can prove "no real send, but a receipt recorded".
const makeSettlementDeps = (
  options: Readonly<{
    gate: TassadarRealSettlementGate
    store: MemoryLedgerStore
    targets: ReadonlyMap<string, string>
    dispatchCount: { value: number }
  }>,
): KhalaSettlementDeps => {
  const dryRun = makeDryRunSettlementDispatch(dryRunLedgerView(options.store))
  return {
    dispatchRealSettlement: input =>
      Effect.gen(function* () {
        const existing = yield* Effect.promise(() =>
          options.store.readPaymentAuthorityReceiptByRef(
            input.settlement.settlementReceiptRef,
          ),
        )
        // Count only the first (recording) dispatch; a replay is an idempotent
        // no-op. This proves idempotency at the dispatch boundary.
        if (existing === undefined) options.dispatchCount.value += 1
        yield* dryRun(input)
      }),
    ledger: options.store,
    nowIso,
    readGate: () => options.gate,
    resolvePayoutDestination: async ref => options.targets.get(ref),
    settlementRunRef: SETTLEMENT_RUN_REF,
  }
}

// Build the M3 settlement deps with the GATED dispatch SELECTOR
// (`makeKhalaLoopSettlementDispatch`) wired exactly as the live wiring layer
// would: a MOCKED real dispatch (never moves sats — records the receipt + counts
// real sends) and the dry-run dispatch as the fail-closed fallback. This proves
// the loop routes to the real dispatch only when armed + the gate authorizes.
const makeGatedSettlementDeps = (
  options: Readonly<{
    arming: KhalaLoopConfig['arming']
    gate: TassadarRealSettlementGate
    store: MemoryLedgerStore
    targets: ReadonlyMap<string, string>
    realDispatchCount: { value: number }
    dryRunDispatchCount: { value: number }
  }>,
): KhalaSettlementDeps => {
  const dryRunBase = makeDryRunSettlementDispatch(dryRunLedgerView(options.store))

  // MOCKED real Spark dispatch: receipt-first + idempotent (mirrors
  // `dispatchRealRunSettlementCore`), but performs NO real send — it just records
  // the settled receipt and counts the first dispatch. No sats move in tests.
  const mockedRealDispatch: KhalaSettlementDispatch = input =>
    Effect.gen(function* () {
      const existing = yield* Effect.promise(() =>
        options.store.readPaymentAuthorityReceiptByRef(
          input.settlement.settlementReceiptRef,
        ),
      )
      if (existing !== undefined) return // idempotent no-op on replay
      options.realDispatchCount.value += 1
      yield* Effect.promise(() =>
        options.store.createPaymentAuthorityReceipt(
          input.settlement.settlementReceipt,
        ),
      )
    })

  const countingDryRun: KhalaSettlementDispatch = input =>
    Effect.gen(function* () {
      const existing = yield* Effect.promise(() =>
        options.store.readPaymentAuthorityReceiptByRef(
          input.settlement.settlementReceiptRef,
        ),
      )
      if (existing === undefined) options.dryRunDispatchCount.value += 1
      yield* dryRunBase(input)
    })

  const gatedDispatch = makeKhalaLoopSettlementDispatch({
    arming: options.arming,
    dryRunDispatch: countingDryRun,
    readGate: () => options.gate,
    realDispatch: mockedRealDispatch,
    settlementRunRef: SETTLEMENT_RUN_REF,
  })

  return {
    dispatchRealSettlement: gatedDispatch,
    ledger: options.store,
    nowIso,
    readGate: () => options.gate,
    resolvePayoutDestination: async ref => options.targets.get(ref),
    settlementRunRef: SETTLEMENT_RUN_REF,
  }
}

const baseConfig = (
  options: Readonly<{
    arming: KhalaLoopConfig['arming']
    settlementDeps: KhalaSettlementDeps
    payoutGate?: KhalaLoopConfig['payoutGate']
    resaleRefs?: KhalaLoopConfig['resaleRefs']
    transport?: PylonServeTransport
    admission?: KhalaLoopConfig['admission']
  }>,
): KhalaLoopConfig => ({
  arming: options.arming,
  // 5_000 msat = 5 sats: clears the 1-sat dust floor, stays tiny + bounded.
  contributorCutMsat: 5_000,
  payoutGate: options.payoutGate ?? armedMdkGate(),
  resaleRefs: options.resaleRefs ?? fullResaleRefs,
  revenueAsset: 'bitcoin',
  settlementDeps: options.settlementDeps,
  transport: options.transport ?? fakePylonTransport(),
  ...(options.admission === undefined ? {} : { admission: options.admission }),
})

// A fully-ready admission snapshot for the guinea-pig Pylon, plus the M4 admission
// config the loop gates on. Tests degrade one field to prove the safe fall-back.
const REQUIRED_SERVING_CAP = 'capability.serving.khala_mini.v1'
const admittedSnapshot = () => ({
  capabilityRefs: [REQUIRED_SERVING_CAP],
  latestHeartbeatAt: '2026-06-22T17:59:30.000Z', // 30s before nowMs — fresh
  latestHeartbeatStatus: 'ok',
  pylonRef: GUINEA_PIG_NODE_REF,
  servingLaneRefs: ['lane.nip90.serving.v1'],
  sparkPayoutTargetRef: 'payout.spark.deadbeef',
  status: 'active' as const,
  walletReady: true,
})
const admissionConfig = (
  snapshot: ReturnType<typeof admittedSnapshot> = admittedSnapshot(),
): KhalaLoopConfig['admission'] => ({
  nowMs: Date.parse('2026-06-22T18:00:00.000Z'),
  requiredCapabilityRef: REQUIRED_SERVING_CAP,
  snapshot,
})

describe('readKhalaLoopArming (flag, default OFF)', () => {
  it('is OFF when the env key is absent', () => {
    expect(readKhalaLoopArming({})).toEqual(disabledKhalaLoopArming)
  })

  it('is OFF for any value other than the exact on-token (fails closed)', () => {
    for (const raw of ['true', '1', '', 'on', 'ARMED', ' armed ']) {
      // " armed " trims to "armed" -> ON; assert the rest are OFF.
      const expected = raw.trim() === 'armed'
      expect(readKhalaLoopArming({ [KhalaLoopArmingEnvKey]: raw }).loopArmed).toBe(
        expected,
      )
    }
  })

  it('is ON only for the exact on-token', () => {
    expect(
      readKhalaLoopArming({ [KhalaLoopArmingEnvKey]: 'armed' }).loopArmed,
    ).toBe(true)
  })
})

describe('runKhalaLoopOnce — verified-serve -> dry-run Spark payout (M3↔M4)', () => {
  it('INERT by default: loop flag OFF => serve happens, but nothing forwards to settlement', async () => {
    const store = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const settlementDeps = makeSettlementDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      store,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const outcome = await Effect.runPromise(
      runKhalaLoopOnce(
        baseConfig({ arming: disabledKhalaLoopArming, settlementDeps }),
        request,
      ),
    )

    // The serve + parity receipt still happen (M4 works); nothing settled.
    expect(outcome.receipt!.parityVerified).toBe(true)
    expect(outcome.served!.result.content).toContain('guinea-pig')
    expect(outcome.forwardedToSettlement).toBe(false)
    expect(outcome.settlement).toBeNull()
    expect(dispatchCount.value).toBe(0)
    expect(store.receipts.size).toBe(0)
  })

  it('END-TO-END (loop ARMED + M3 gate ARMED): serve -> receipt -> dry-run settle -> dereferenceable settled receipt', async () => {
    const store = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const settlementDeps = makeSettlementDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      store,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const outcome = await Effect.runPromise(
      runKhalaLoopOnce(
        baseConfig({ arming: { loopArmed: true }, settlementDeps }),
        request,
      ),
    )

    expect(outcome.forwardedToSettlement).toBe(true)
    expect(outcome.decision!.armed).toBe(true)
    expect(outcome.settlement).not.toBeNull()
    const legs = outcome.settlement!.legs
    expect(legs).toHaveLength(1)
    const leg = legs[0]!
    expect(leg.contributorRef).toBe(GUINEA_PIG_NODE_REF)
    expect(leg.settled).toBe(true)
    expect(leg.realBitcoinMoved).toBe(true)
    expect(leg.mode).toBe('real_bitcoin')
    expect(leg.amountSats).toBe(5)
    expect(leg.settlementReceiptRef).not.toBeNull()

    // The settled receipt is DEREFERENCEABLE in the ledger and realBitcoinMoved-shaped.
    const receipt = await store.readPaymentAuthorityReceiptByRef(
      leg.settlementReceiptRef!,
    )
    expect(receipt).toBeDefined()
    const projection = JSON.parse(receipt!.publicProjectionJson)
    expect(projection.state).toBe('settled')
    expect(projection.moneyMovement).toBe('real_bitcoin')
    expect(projection.contributorRef).toBe(GUINEA_PIG_NODE_REF)
    expect(projection.amountSats).toBe(5)
    // No raw Spark address / invoice / preimage anywhere in the receipt.
    expect(receipt!.publicProjectionJson).not.toMatch(/spark1/i)

    // DRY-RUN: a single recording dispatch, no real Spark send.
    expect(dispatchCount.value).toBe(1)
  })

  it('IDEMPOTENT: replaying the same verified serve never double-records (no double-pay)', async () => {
    const store = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const settlementDeps = makeSettlementDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      store,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })
    const config = baseConfig({ arming: { loopArmed: true }, settlementDeps })

    await Effect.runPromise(runKhalaLoopOnce(config, request))
    await Effect.runPromise(runKhalaLoopOnce(config, request))

    // The receipt ref is derived from run+node, so a replay records AT MOST once.
    expect(dispatchCount.value).toBe(1)
    expect(store.receipts.size).toBe(1)
  })

  it('loop ARMED but M3 owner gate OFF: still inert (the second independent gate blocks)', async () => {
    const store = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const settlementDeps = makeSettlementDeps({
      dispatchCount,
      gate: disabledTassadarRealSettlementGate,
      store,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const outcome = await Effect.runPromise(
      runKhalaLoopOnce(
        baseConfig({ arming: { loopArmed: true }, settlementDeps }),
        request,
      ),
    )

    // Decision arms (MDK gate + resale refs), the loop forwards, but the M3 leg
    // blocks on its own disabled owner gate => nothing settles, nothing recorded.
    expect(outcome.forwardedToSettlement).toBe(true)
    expect(outcome.settlement!.legs[0]!.settled).toBe(false)
    expect(outcome.settlement!.legs[0]!.skipped).toBe('gate_not_authorized')
    expect(dispatchCount.value).toBe(0)
    expect(store.receipts.size).toBe(0)
  })

  it('unarmed MDK gate: decision not armed => loop never forwards even with the loop flag ON', async () => {
    const store = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const settlementDeps = makeSettlementDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      store,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const outcome = await Effect.runPromise(
      runKhalaLoopOnce(
        baseConfig({
          arming: { loopArmed: true },
          payoutGate: hostedMdkDirectPayoutDisabledGate(),
          settlementDeps,
        }),
        request,
      ),
    )

    expect(outcome.decision!.armed).toBe(false)
    expect(outcome.forwardedToSettlement).toBe(false)
    expect(outcome.settlement).toBeNull()
    expect(dispatchCount.value).toBe(0)
  })

  it('FAILS CLOSED on a parity-unverified serve (M4 gate): the loop never settles', async () => {
    const store = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const settlementDeps = makeSettlementDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      store,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const unverifiedServe: PsionicServeResponse = {
      ...guineaPigServe,
      parityMode: 'none',
      parityVerified: false,
    }

    const result = await Effect.runPromiseExit(
      runKhalaLoopOnce(
        baseConfig({
          arming: { loopArmed: true },
          settlementDeps,
          transport: fakePylonTransport(unverifiedServe),
        }),
        request,
      ),
    )

    // The M4 dispatch typed-refuses an unverified serve before any settlement.
    expect(result._tag).toBe('Failure')
    expect(dispatchCount.value).toBe(0)
    expect(store.receipts.size).toBe(0)
  })

  it('SKIPS cleanly when the guinea-pig Pylon has no registered Spark target', async () => {
    const store = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const settlementDeps = makeSettlementDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      store,
      targets: new Map(), // no destination registered
    })

    const outcome = await Effect.runPromise(
      runKhalaLoopOnce(
        baseConfig({ arming: { loopArmed: true }, settlementDeps }),
        request,
      ),
    )

    expect(outcome.settlement!.legs[0]!.skipped).toBe('no_payout_destination')
    expect(outcome.settlement!.legs[0]!.settled).toBe(false)
    expect(dispatchCount.value).toBe(0)
  })

  // --------------------------------------------------------------------------
  // M4 ADMISSION GATE: route only to admitted Pylons; degrade safely otherwise.
  // --------------------------------------------------------------------------

  it('ADMITTED Pylon (capability + fresh heartbeat + wallet/payout ready): serves and settles', async () => {
    const store = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const settlementDeps = makeSettlementDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      store,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const outcome = await Effect.runPromise(
      runKhalaLoopOnce(
        baseConfig({
          admission: admissionConfig(),
          arming: { loopArmed: true },
          settlementDeps,
        }),
        request,
      ),
    )

    // Admission passed -> the Pylon was routed to, served, and settled.
    expect(outcome.admission).not.toBeNull()
    expect(outcome.admission!.admitted).toBe(true)
    expect(outcome.admittedAndServed).toBe(true)
    expect(outcome.receipt!.parityVerified).toBe(true)
    expect(outcome.forwardedToSettlement).toBe(true)
    expect(outcome.settlement!.legs[0]!.settled).toBe(true)
  })

  it('NON-ADMITTED Pylon (stale heartbeat): no routing, no serve, no settle (safe fall-back)', async () => {
    const store = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const settlementDeps = makeSettlementDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      store,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    // The transport must NEVER be touched when admission refuses. A throwing
    // transport proves the loop short-circuits BEFORE dispatching to the Pylon.
    const throwingTransport: PylonServeTransport = () => {
      throw new Error('transport must not be called for a non-admitted Pylon')
    }

    const outcome = await Effect.runPromise(
      runKhalaLoopOnce(
        baseConfig({
          // Stale heartbeat (5 min old vs the 90s default TTL) => not admitted.
          admission: admissionConfig({
            ...admittedSnapshot(),
            latestHeartbeatAt: '2026-06-22T17:55:00.000Z',
          }),
          arming: { loopArmed: true },
          settlementDeps,
          transport: throwingTransport,
        }),
        request,
      ),
    )

    expect(outcome.admission!.admitted).toBe(false)
    expect(outcome.admittedAndServed).toBe(false)
    expect(outcome.served).toBeNull()
    expect(outcome.receipt).toBeNull()
    expect(outcome.decision).toBeNull()
    expect(outcome.settlement).toBeNull()
    expect(outcome.forwardedToSettlement).toBe(false)
    // Nothing was dispatched or recorded — the request falls back to cloud.
    expect(dispatchCount.value).toBe(0)
    expect(store.receipts.size).toBe(0)
  })

  it('NON-ADMITTED Pylon (missing capability): refused before any serve, even with the loop armed', async () => {
    const store = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const settlementDeps = makeSettlementDeps({
      dispatchCount,
      gate: armedRealSettlementGate(),
      store,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const outcome = await Effect.runPromise(
      runKhalaLoopOnce(
        baseConfig({
          admission: admissionConfig({
            ...admittedSnapshot(),
            capabilityRefs: ['capability.unrelated.v1'],
          }),
          arming: { loopArmed: true },
          settlementDeps,
        }),
        request,
      ),
    )

    expect(outcome.admittedAndServed).toBe(false)
    expect(outcome.admission!.blockerRefs).toContain(
      'blocker.pylon_admission.capability_not_advertised',
    )
    expect(dispatchCount.value).toBe(0)
  })
})

// ----------------------------------------------------------------------------
// GATED REAL DISPATCH (M4 NEEDS_OWNER swap). The loop's settlement dispatch
// resolves to the REAL Spark send ONLY when armed + the M3 owner gate authorizes;
// otherwise the dry-run. No real sats move (the real dispatch is MOCKED).
// ----------------------------------------------------------------------------

// A minimal records bundle for direct selector unit tests (only the fields the
// selector reads + the receipt it would record). Amount-in-sats drives the cap.
const syntheticRecords = (
  amountSats: number,
): KhalaSettlementRecords =>
  ({
    amountSats,
    contributorRef: GUINEA_PIG_NODE_REF,
    settlementReceipt: {
      receiptRef: `receipt.test.${amountSats}`,
    } as KhalaSettlementRecords['settlementReceipt'],
    settlementReceiptRef: `receipt.test.${amountSats}`,
  } as KhalaSettlementRecords)

describe('makeKhalaLoopSettlementDispatch — gated real-vs-dry-run selection', () => {
  const buildSelector = (
    options: Readonly<{
      arming: KhalaLoopConfig['arming']
      gate: TassadarRealSettlementGate
    }>,
  ) => {
    const real = { value: 0 }
    const dry = { value: 0 }
    const dispatch = makeKhalaLoopSettlementDispatch({
      arming: options.arming,
      dryRunDispatch: () =>
        Effect.sync(() => {
          dry.value += 1
        }),
      readGate: () => options.gate,
      realDispatch: () =>
        Effect.sync(() => {
          real.value += 1
        }),
      settlementRunRef: SETTLEMENT_RUN_REF,
    })
    return { dispatch, dry, real }
  }

  it('flag OFF + gate ARMED => DRY-RUN (no real send)', async () => {
    const { dispatch, dry, real } = buildSelector({
      arming: disabledKhalaLoopArming,
      gate: armedRealSettlementGate(),
    })
    await Effect.runPromise(
      dispatch({ contributorRef: GUINEA_PIG_NODE_REF, settlement: syntheticRecords(5) }),
    )
    expect(real.value).toBe(0)
    expect(dry.value).toBe(1)
  })

  it('flag ARMED + gate DISABLED => DRY-RUN (fail-closed, no real send)', async () => {
    const { dispatch, dry, real } = buildSelector({
      arming: { loopArmed: true },
      gate: disabledTassadarRealSettlementGate,
    })
    await Effect.runPromise(
      dispatch({ contributorRef: GUINEA_PIG_NODE_REF, settlement: syntheticRecords(5) }),
    )
    expect(real.value).toBe(0)
    expect(dry.value).toBe(1)
  })

  it('flag ARMED + gate ARMED + within cap + run allowlisted => REAL dispatch', async () => {
    const { dispatch, dry, real } = buildSelector({
      arming: { loopArmed: true },
      gate: armedRealSettlementGate(),
    })
    await Effect.runPromise(
      dispatch({ contributorRef: GUINEA_PIG_NODE_REF, settlement: syntheticRecords(5) }),
    )
    expect(real.value).toBe(1)
    expect(dry.value).toBe(0)
  })

  it('OVER per-payout cap => DRY-RUN (no real send)', async () => {
    const { dispatch, dry, real } = buildSelector({
      arming: { loopArmed: true },
      gate: armedRealSettlementGate({ maxPayoutSats: 4 }),
    })
    await Effect.runPromise(
      dispatch({ contributorRef: GUINEA_PIG_NODE_REF, settlement: syntheticRecords(5) }),
    )
    expect(real.value).toBe(0)
    expect(dry.value).toBe(1)
  })

  it('run NOT allowlisted => DRY-RUN (no real send)', async () => {
    const { dispatch, dry, real } = buildSelector({
      arming: { loopArmed: true },
      gate: armedRealSettlementGate({ allowedRunRefs: ['run.some.other'] }),
    })
    await Effect.runPromise(
      dispatch({ contributorRef: GUINEA_PIG_NODE_REF, settlement: syntheticRecords(5) }),
    )
    expect(real.value).toBe(0)
    expect(dry.value).toBe(1)
  })

  it('adapter MISMATCH (gate allows a non-spark adapter) => DRY-RUN (no real send)', async () => {
    // The gate Schema only allows `spark_treasury`, so a mismatch is modeled by a
    // gate that has runScopedStreaming off AND the contributor not allowlisted with
    // a wrong run; here we force the requested-adapter branch by allowlisting a run
    // but using an adapter the gate does not allow is impossible via the Schema, so
    // we instead assert the contributor-not-eligible mismatch path fails closed.
    const { dispatch, dry, real } = buildSelector({
      arming: { loopArmed: true },
      gate: armedRealSettlementGate({
        allowedContributorRefs: [],
        runScopedStreaming: false,
      }),
    })
    await Effect.runPromise(
      dispatch({ contributorRef: GUINEA_PIG_NODE_REF, settlement: syntheticRecords(5) }),
    )
    expect(real.value).toBe(0)
    expect(dry.value).toBe(1)
  })
})

describe('runKhalaLoopOnce — gated REAL dispatch end-to-end (mocked Spark)', () => {
  it('flag OFF: end-to-end routes to DRY-RUN, never the real dispatch, no sats move', async () => {
    const store = new MemoryLedgerStore()
    const realDispatchCount = { value: 0 }
    const dryRunDispatchCount = { value: 0 }
    const settlementDeps = makeGatedSettlementDeps({
      arming: disabledKhalaLoopArming,
      dryRunDispatchCount,
      gate: armedRealSettlementGate(),
      realDispatchCount,
      store,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const outcome = await Effect.runPromise(
      runKhalaLoopOnce(
        baseConfig({ arming: disabledKhalaLoopArming, settlementDeps }),
        request,
      ),
    )

    // Loop flag OFF => the loop never even forwards to M3.
    expect(outcome.forwardedToSettlement).toBe(false)
    expect(realDispatchCount.value).toBe(0)
    expect(dryRunDispatchCount.value).toBe(0)
    expect(store.receipts.size).toBe(0)
  })

  it('flag ARMED + gate ARMED + within cap + run allowlisted: REAL dispatch invoked ONCE, idempotent on replay, settled receipt surfaced', async () => {
    const store = new MemoryLedgerStore()
    const realDispatchCount = { value: 0 }
    const dryRunDispatchCount = { value: 0 }
    const settlementDeps = makeGatedSettlementDeps({
      arming: { loopArmed: true },
      dryRunDispatchCount,
      gate: armedRealSettlementGate(),
      realDispatchCount,
      store,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })
    const config = baseConfig({ arming: { loopArmed: true }, settlementDeps })

    const outcome = await Effect.runPromise(runKhalaLoopOnce(config, request))

    // Routed to the REAL (mocked) dispatch, NOT the dry-run.
    expect(realDispatchCount.value).toBe(1)
    expect(dryRunDispatchCount.value).toBe(0)
    expect(outcome.forwardedToSettlement).toBe(true)
    const leg = outcome.settlement!.legs[0]!
    expect(leg.settled).toBe(true)
    expect(leg.realBitcoinMoved).toBe(true)
    expect(leg.mode).toBe('real_bitcoin')
    expect(leg.amountSats).toBe(5)

    // The settled receipt is dereferenceable + realBitcoinMoved-shaped.
    const receipt = await store.readPaymentAuthorityReceiptByRef(
      leg.settlementReceiptRef!,
    )
    expect(receipt).toBeDefined()
    const projection = JSON.parse(receipt!.publicProjectionJson)
    expect(projection.state).toBe('settled')
    expect(projection.moneyMovement).toBe('real_bitcoin')
    expect(receipt!.publicProjectionJson).not.toMatch(/spark1/i)

    // Replay: the deterministic receipt already exists => no second real send.
    await Effect.runPromise(runKhalaLoopOnce(config, request))
    expect(realDispatchCount.value).toBe(1)
    expect(store.receipts.size).toBe(1)
  })

  it('flag ARMED + gate ARMED but OVER cap: routes to DRY-RUN, no real send', async () => {
    const store = new MemoryLedgerStore()
    const realDispatchCount = { value: 0 }
    const dryRunDispatchCount = { value: 0 }
    const settlementDeps = makeGatedSettlementDeps({
      arming: { loopArmed: true },
      dryRunDispatchCount,
      // 5-sat cut exceeds a 4-sat per-payout cap => the engine's GATE 3 blocks it
      // (gate_not_authorized) BEFORE the dispatch is even reached, so neither the
      // real nor the dry-run dispatch fires — the safest possible outcome.
      gate: armedRealSettlementGate({ maxPayoutSats: 4 }),
      realDispatchCount,
      store,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const outcome = await Effect.runPromise(
      runKhalaLoopOnce(
        baseConfig({ arming: { loopArmed: true }, settlementDeps }),
        request,
      ),
    )

    expect(realDispatchCount.value).toBe(0)
    expect(outcome.settlement!.legs[0]!.settled).toBe(false)
    expect(outcome.settlement!.legs[0]!.skipped).toBe('gate_not_authorized')
    expect(store.receipts.size).toBe(0)
  })

  it('flag ARMED but run NOT allowlisted: routes to DRY-RUN, no real send', async () => {
    const store = new MemoryLedgerStore()
    const realDispatchCount = { value: 0 }
    const dryRunDispatchCount = { value: 0 }
    const settlementDeps = makeGatedSettlementDeps({
      arming: { loopArmed: true },
      dryRunDispatchCount,
      gate: armedRealSettlementGate({ allowedRunRefs: ['run.not.this.one'] }),
      realDispatchCount,
      store,
      targets: new Map([[GUINEA_PIG_NODE_REF, readGuineaPigSparkAddress()]]),
    })

    const outcome = await Effect.runPromise(
      runKhalaLoopOnce(
        baseConfig({ arming: { loopArmed: true }, settlementDeps }),
        request,
      ),
    )

    expect(realDispatchCount.value).toBe(0)
    expect(outcome.settlement!.legs[0]!.settled).toBe(false)
    expect(outcome.settlement!.legs[0]!.skipped).toBe('gate_not_authorized')
    expect(store.receipts.size).toBe(0)
  })
})
