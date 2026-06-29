import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { buildCs336A4ProvenanceReceipt } from './cs336-a4-provenance'
import { nexusPylonPublicReceiptDetailFromLedger } from './nexus-pylon-visibility'
import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutLedgerStore,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import {
  type TrainingAuthorityStore,
  type TrainingRunProjection,
  type TrainingRunPublicSummary,
  type TrainingRunRecord,
  type TrainingWindowEventRecord,
  type TrainingWindowLeaseRecord,
  type TrainingWindowRecord,
  buildTrainingRunRecord,
} from './training-run-window-authority'
import { makeTrainingRunWindowRoutes } from './training-run-window-routes'
import {
  type TrainingVerificationChallengeRecord,
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
} from './training-verification'
import {
  type TreasuryPaymentAuthorityAdapter,
  TreasuryPaymentAuthorityError,
  makeTreasuryPaymentAuthority,
} from './treasury-payment-authority'

/**
 * In-memory treasury payout ledger stub. Reads come from the seeded receipt map
 * (and anything written through createPaymentAuthorityReceipt); the payout
 * intent/attempt/reconciliation creates record into in-memory maps so the
 * settlement-receipt route (openagents #5009) can be exercised end to end.
 */
const makeLedgerStoreStub = (
  seededReceipts: ReadonlyArray<
    readonly [string, { publicProjectionJson: string; receiptKind: string }]
  > = [],
): NexusTreasuryPayoutLedgerStore & {
  readonly receipts: Map<string, NexusPaymentAuthorityReceiptRecord>
} => {
  const receipts = new Map<string, NexusPaymentAuthorityReceiptRecord>()

  for (const [ref, partial] of seededReceipts) {
    receipts.set(ref, {
      archivedAt: null,
      audience: 'public',
      createdAt: '2026-06-10T10:00:00.000Z',
      eventRef: null,
      id: `seed_${ref}`,
      metadataRefs: [],
      payoutAttemptRef: null,
      payoutIntentRef: `intent_${ref}`,
      publicProjectionJson: partial.publicProjectionJson,
      receiptKind:
        partial.receiptKind as NexusPaymentAuthorityReceiptRecord['receiptKind'],
      receiptRef: ref,
    })
  }

  const intents = new Map<string, NexusTreasuryPayoutIntentRecord>()
  const attempts = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  const events = new Map<string, NexusTreasuryPayoutReconciliationEventRecord>()
  const notImplemented = async (): Promise<never> => {
    throw new Error('not implemented in ledger stub')
  }

  return {
    createPaymentAuthorityReceipt: async record => {
      receipts.set(record.receiptRef, record)
    },
    createPayoutAttempt: async record => {
      attempts.set(record.payoutAttemptRef, record)
    },
    createPayoutIntent: async record => {
      intents.set(record.payoutIntentRef, record)
    },
    createPayoutTargetApproval: async () => {},
    createReconciliationEvent: async record => {
      events.set(record.eventRef, record)
    },
    createReleaseGate: async () => {},
    listPaymentAuthorityReceipts: async () => [...receipts.values()],
    readPaymentAuthorityReceiptByRef: async receiptRef =>
      receipts.get(receiptRef),
    readPayoutAttemptByIdempotencyKeyHash: notImplemented,
    readPayoutAttemptByRef: notImplemented,
    readPayoutIntentByBuyerPaymentRef: notImplemented,
    readPayoutIntentByIdempotencyKeyHash: notImplemented,
    readPayoutIntentByRef: async payoutIntentRef =>
      intents.get(payoutIntentRef),
    readReconciliationEventByRef: async eventRef => events.get(eventRef),
    receipts,
  }
}

const jsonRequest = (
  path: string,
  body: Record<string, unknown>,
  init: RequestInit = {},
): Request =>
  new Request(`https://openagents.test${path}`, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json', ...(init.headers ?? {}) },
    method: 'POST',
    ...init,
  })

const runRoute = async (
  route: Effect.Effect<Response> | undefined,
): Promise<Response> => {
  expect(route).toBeDefined()

  return Effect.runPromise(route!)
}

type MemoryTrainingAuthorityStore = TrainingAuthorityStore &
  Readonly<{
    _testSeedChallenge: (challenge: TrainingVerificationChallengeRecord) => void
    _testSeedRun: (run: TrainingRunRecord) => void
  }>

type TrainingRunListJson = Readonly<{
  summaries: ReadonlyArray<TrainingRunPublicSummary>
}>

type TrainingRunDetailJson = Readonly<{
  run: TrainingRunProjection
  summary: TrainingRunPublicSummary
}>

type PublicTrainingRunDetailJson = TrainingRunDetailJson &
  Readonly<{
    generatedAt: string
    sourceRefs: ReadonlyArray<string>
    staleness: TrainingRunProjection['staleness']
  }>

type TrainingRunLeaderboardJson = Readonly<{
  leaderboardRows: ReadonlyArray<
    TrainingRunPublicSummary['realGradient']['leaderboardRows'][number]
  >
}>

type TrainingRunIsoFlopJson = Readonly<{
  blockerRefs: ReadonlyArray<string>
  cells: ReadonlyArray<unknown>
  schemaVersion: string
}>

type TrainingDeviceCapabilityJson = Readonly<{
  blockerRefs: ReadonlyArray<string>
  classDistributions: ReadonlyArray<{
    deviceClassRef: string
    earningEstimate: Readonly<{ basisLabel: string }> | null
    verified: boolean
  }>
  schemaVersion: string
  sameClassReplicationBlockerRefs: ReadonlyArray<string>
  sameClassReplicationSignals: ReadonlyArray<Readonly<{ state: string }>>
  sameClassReplicationStatus: string
  thermalThrottleBlockerRefs: ReadonlyArray<string>
  thermalThrottleDetectionStatus: string
  thermalThrottleFunnelReasonCodes: ReadonlyArray<string>
  thermalThrottleReceiptRefs: ReadonlyArray<string>
  thermalThrottleSignals: ReadonlyArray<Readonly<{ state: string }>>
}>

type TrainingA5EvalJson = Readonly<{
  blockerRefs: ReadonlyArray<string>
  evalSuites: ReadonlyArray<{
    evalSuiteRef: string
    score: number
    taskSetRef: string
  }>
  schemaVersion: string
  updateBoundaryRef: string
}>

type TrainingLeaderboardsJson = Readonly<{
  lanes: ReadonlyArray<{
    lane: string
    rows: ReadonlyArray<{
      contributorRef: string
      rank: number
      score: number
    }>
  }>
  schemaVersion: string
}>

const makeMemoryStore = (): MemoryTrainingAuthorityStore => {
  const runs = new Map<string, TrainingRunRecord>()
  const windows = new Map<string, TrainingWindowRecord>()
  const leases = new Map<string, TrainingWindowLeaseRecord>()
  const challenges = new Map<string, TrainingVerificationChallengeRecord>()
  const events: Array<TrainingWindowEventRecord> = []

  return {
    _testSeedChallenge: (challenge: TrainingVerificationChallengeRecord) => {
      challenges.set(challenge.challengeRef, challenge)
    },
    _testSeedRun: (run: TrainingRunRecord) => {
      runs.set(run.trainingRunRef, run)
    },
    attachRunEvidence: async run => {
      runs.set(run.trainingRunRef, run)

      return run
    },
    beginRunSealBarrier: async (trainingRunRef, nowIso) => {
      const run = runs.get(trainingRunRef)

      if (run !== undefined) {
        runs.set(trainingRunRef, { ...run, sealInFlightAt: nowIso })
      }
    },
    claimLease: async lease => {
      leases.set(lease.leaseRef, lease)

      return lease
    },
    clearRunSealBarrier: async trainingRunRef => {
      const run = runs.get(trainingRunRef)

      if (run !== undefined) {
        runs.set(trainingRunRef, { ...run, sealInFlightAt: null })
      }
    },
    listClaimableWindows: async nowIso =>
      [...windows.values()].filter(
        window =>
          window.state === 'active' &&
          ![...leases.values()].some(
            lease =>
              lease.windowRef === window.windowRef &&
              lease.state === 'active' &&
              Date.parse(lease.leaseExpiresAt) > Date.parse(nowIso),
          ),
      ),
    listRuns: async limit => [...runs.values()].slice(0, limit),
    listVerificationChallengesForRun: async (trainingRunRef, limit) =>
      [...challenges.values()]
        .filter(challenge => challenge.trainingRunRef === trainingRunRef)
        .slice(0, limit),
    listWindowLeasesForRun: async (trainingRunRef, limit) =>
      [...leases.values()]
        .filter(lease => lease.trainingRunRef === trainingRunRef)
        .slice(0, limit),
    listWindowsForRun: async (trainingRunRef, limit) =>
      [...windows.values()]
        .filter(window => window.trainingRunRef === trainingRunRef)
        .slice(0, limit),
    planRun: async run => {
      runs.set(run.trainingRunRef, run)

      return run
    },
    planWindow: async window => {
      windows.set(window.windowRef, window)

      return window
    },
    readRun: async trainingRunRef => runs.get(trainingRunRef),
    readWindow: async windowRef => windows.get(windowRef),
    readWindowLease: async leaseRef => leases.get(leaseRef),
    transitionRun: async run => {
      runs.set(run.trainingRunRef, run)

      return run
    },
    transitionWindow: async (window, event) => {
      windows.set(window.windowRef, window)
      events.push(event)

      return window
    },
  }
}

describe('training run window routes', () => {
  it('lists runs with provenance-labeled public counts and never counts pending as paid', async () => {
    const store = makeMemoryStore()
    let counter = 0
    const settlementReceipts = new Map([
      [
        'receipt.cs336.a2.measurement.1',
        {
          publicProjectionJson: JSON.stringify({
            amountSats: 10,
            state: 'settled',
          }),
          receiptKind: 'settlement_recorded',
        },
      ],
      [
        'receipt.cs336.a5.gsm8k.1',
        {
          publicProjectionJson: JSON.stringify({
            amountSats: 999,
            state: 'pending',
          }),
          receiptKind: 'settlement_recorded',
        },
      ],
    ])
    const ledgerStore = makeLedgerStoreStub([...settlementReceipts])
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => String(++counter).padStart(4, '0'),
      makePayoutLedgerStore: () => ledgerStore,
      makeStore: () => store,
      nowIso: () => '2026-06-10T10:00:00.000Z',
      requireAdminApiToken: async () => true,
    })

    await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs', {
          promiseRef: 'pylon.first_real_model_training_run.v1',
          receiptRefs: ['receipt.training.run.planned'],
          sourceRefs: ['issue.github.openagents.4677'],
          trainingRunRef: 'training.run.cs336.a1.demo',
        }),
        {},
      ),
    )
    await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/windows/plan', {
          datasetRefs: ['dataset.cs336.a1.public'],
          homeworkKind: 'admin_dispatched_homework',
          receiptRefs: ['receipt.training.window.planned'],
          sourceRefs: ['issue.github.openagents.4677'],
          trainingRunRef: 'training.run.cs336.a1.demo',
          windowRef: 'training.window.cs336.a1.1',
        }),
        {},
      ),
    )
    await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          '/api/training/windows/training.window.cs336.a1.1/activate',
          { receiptRef: 'receipt.training.window.active' },
        ),
        {},
      ),
    )
    await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/leases/claim', {
          pylonRef: 'pylon.training.1',
          receiptRefs: ['receipt.training.lease.claimed'],
        }),
        {},
      ),
    )

    const challenge = buildTrainingVerificationChallengeRecord({
      makeId: () => '4677',
      nowIso: '2026-06-10T10:01:00.000Z',
      request: {
        commitmentRefs: ['commitment.training.public.4677'],
        contributionRef: 'contribution.training.pylon.training.1',
        homeworkKind: 'admin_dispatched_homework',
        payload: {
          contributionRefs: ['contribution.training.pylon.training.1'],
          expectedDigestRef: 'digest.training.expected',
          recomputedDigestRef: 'digest.training.expected',
        },
        trainingRunRef: 'training.run.cs336.a1.demo',
        verificationClass: 'deterministic_recompute',
        windowRef: 'training.window.cs336.a1.1',
      },
    }).challenge
    const leasedChallenge = leaseTrainingVerificationChallengeRecord({
      challenge,
      eventId: '4677-lease',
      nowIso: '2026-06-10T10:01:30.000Z',
      request: { validatorRef: 'validator.training.4677' },
    }).challenge
    store._testSeedChallenge(
      finalizeTrainingVerificationChallengeRecord({
        challenge: leasedChallenge,
        eventId: '4677-final',
        nowIso: '2026-06-10T10:02:00.000Z',
        request: { receiptRefs: ['receipt.training.verdict.public'] },
        verdict: {
          failureCodes: [],
          state: 'Verified',
          verdictRefs: ['verdict.training.public.4677'],
        },
      }).challenge,
    )

    const listResponse = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request('https://openagents.test/api/training/runs'),
        {},
      ),
    )
    const listBody = (await listResponse.json()) as TrainingRunListJson

    expect(listResponse.status).toBe(200)
    expect(listBody).toMatchObject({
      summaries: [
        {
          metrics: {
            assignedContributorCount: {
              provenanceLabel:
                'Distinct pylon_ref values from Worker D1 training_window_leases rows.',
              value: 1,
            },
            providerConfirmedSettledPayoutSats: {
              value: 0,
            },
            verifiedWorkCount: {
              provenanceLabel:
                'Worker D1 training_verification_challenges rows with state Verified.',
              value: 1,
            },
          },
          run: {
            promiseRef: 'pylon.first_real_model_training_run.v1',
            trainingRunRef: 'training.run.cs336.a1.demo',
          },
        },
      ],
    })
    expect(listBody.summaries.length).toBe(1)
    const firstSummary = listBody.summaries[0]!
    expect(
      firstSummary.metrics.providerConfirmedSettledPayoutSats.provenanceLabel,
    ).toContain(
      'settled-state SIMULATION receipts (realBitcoinMoved false) are excluded',
    )

    const leaderboardResponse = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request('https://openagents.test/api/training/leaderboards/a1'),
        {},
      ),
    )
    const leaderboard =
      (await leaderboardResponse.json()) as TrainingRunLeaderboardJson

    expect(leaderboardResponse.status).toBe(200)
    expect(leaderboard.leaderboardRows).toEqual([
      expect.objectContaining({
        pylonRef: 'pylon.training.1',
        rank: 1,
        settledPayoutSats: 0,
        trainingRunRef: 'training.run.cs336.a1.demo',
      }),
    ])

    const isoflopResponse = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request('https://openagents.test/api/training/isoflop/a3'),
        {},
      ),
    )
    const isoflop = (await isoflopResponse.json()) as TrainingRunIsoFlopJson

    expect(isoflopResponse.status).toBe(200)
    expect(isoflop).toMatchObject({
      blockerRefs: [
        'blocker.cs336_a3.requires_twenty_verified_cells',
        'blocker.cs336_a3.operator_funding_required_for_paid_cells',
        'blocker.cs336_a3.fit_artifact_not_published',
      ],
      schemaVersion: 'openagents.training.isoflop_dashboard.v1',
    })

    store._testSeedRun({
      ...buildTrainingRunRecord({
        makeId: () => 'a2-route',
        nowIso: '2026-06-10T10:00:00.000Z',
        request: {
          promiseRef: 'pylon.compute_revenue_modes.v1',
          trainingRunRef: 'training.run.cs336.a2.benchmark',
        },
      }),
      publicProjectionJson: JSON.stringify({
        a2DeviceBenchmark: {
          measurements: [
            {
              deviceClassRef: 'device_class.apple_silicon.m3_pro_18gb',
              earningEstimate: {
                p50SatsPerHour: 42,
                sourceRefs: ['receipt.cs336.a2.estimate.1'],
                workClass: 'small_model_local_training',
              },
              max: 2060,
              metric: 'tokens_per_second',
              min: 1710,
              p50: 1900,
              p90: 2025,
              receiptRefs: ['receipt.cs336.a2.measurement.1'],
              sampleCount: 4,
              unit: 'tokens_per_second',
              verificationRefs: ['challenge.cs336.a2.class_check.1'],
              workClass: 'small_model_local_training',
            },
          ],
        },
      }),
    })

    const deviceCapabilityResponse = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/device-capabilities/a2',
        ),
        {},
      ),
    )
    const deviceCapability =
      (await deviceCapabilityResponse.json()) as TrainingDeviceCapabilityJson

    expect(deviceCapabilityResponse.status).toBe(200)
    expect(deviceCapability).toMatchObject({
      blockerRefs: [
        'blocker.cs336_a2.requires_cross_machine_same_class_replication',
      ],
      classDistributions: [
        {
          deviceClassRef: 'device_class.apple_silicon.m3_pro_18gb',
          earningEstimate: {
            basisLabel: 'modeled_from_measured_benchmark_distribution',
          },
          verified: true,
        },
      ],
      schemaVersion: 'openagents.training.device_capability_dashboard.v1',
      sameClassReplicationBlockerRefs: [
        'blocker.cs336_a2.requires_cross_machine_same_class_replication',
      ],
      sameClassReplicationSignals: [{ state: 'same_host_only' }],
      sameClassReplicationStatus: 'same_host_only',
      thermalThrottleBlockerRefs: [
        'blocker.cs336_a2.requires_sustained_vs_burst_thermal_probe',
      ],
      thermalThrottleDetectionStatus: 'missing',
      thermalThrottleSignals: [],
    })

    store._testSeedRun({
      ...buildTrainingRunRecord({
        makeId: () => 'a5-route',
        nowIso: '2026-06-10T10:00:00.000Z',
        request: {
          promiseRef: 'pylon.compute_revenue_modes.v1',
          trainingRunRef: 'training.run.cs336.a5.eval',
        },
      }),
      publicProjectionJson: JSON.stringify({
        a5Alignment: {
          evalSuites: [
            {
              evalSuiteRef: 'eval.cs336.a5.gsm8k.seeded.1',
              metric: 'accuracy',
              receiptRefs: ['receipt.cs336.a5.gsm8k.1'],
              sampleCount: 100,
              score: 0.42,
              splitRef: 'gsm8k.test.public_summary',
              taskSetRef: 'gsm8k',
              verificationRefs: ['challenge.cs336.a5.gsm8k.1'],
              verifiedSampleCount: 100,
            },
          ],
        },
      }),
    })

    const a5EvalResponse = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request('https://openagents.test/api/training/evals/a5'),
        {},
      ),
    )
    const a5Eval = (await a5EvalResponse.json()) as TrainingA5EvalJson

    expect(a5EvalResponse.status).toBe(200)
    expect(a5Eval).toMatchObject({
      blockerRefs: [],
      evalSuites: [
        {
          evalSuiteRef: 'eval.cs336.a5.gsm8k.seeded.1',
          score: 0.42,
          taskSetRef: 'gsm8k',
        },
      ],
      schemaVersion: 'openagents.training.a5_eval_dashboard.v1',
      updateBoundaryRef: 'issue.github.openagents.4669',
    })

    const leaderboardsResponse = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request('https://openagents.test/api/training/leaderboards'),
        {},
      ),
    )
    const leaderboards =
      (await leaderboardsResponse.json()) as TrainingLeaderboardsJson

    expect(leaderboardsResponse.status).toBe(200)
    expect(leaderboards.schemaVersion).toBe(
      'openagents.training.leaderboards.v1',
    )
    // The a2 measurement receipt is `state: settled` but carries NO real-bitcoin
    // movement (no movementMode/moneyMovement/realBitcoinMoved), so it is a
    // settled-STATE simulation receipt. The leaderboard reads the SAME real-only
    // settlement resolution as the run summary metric (which is 0 above), so its
    // settled payout is 0 — settled-state sim does not inflate leaderboard sats.
    expect(
      leaderboards.lanes.find(lane => lane.lane === 'a2_throughput')?.rows,
    ).toEqual([
      expect.objectContaining({
        contributorRef: 'device_class.apple_silicon.m3_pro_18gb',
        rank: 1,
        score: 2025,
        settledPayoutSats: 0,
      }),
    ])
    expect(
      leaderboards.lanes.find(lane => lane.lane === 'a5_accuracy')?.rows,
    ).toEqual([
      expect.objectContaining({
        contributorRef: 'eval.cs336.a5.gsm8k.seeded.1',
        rank: 1,
        score: 0.42,
        settledPayoutSats: 0,
      }),
    ])
  })

  it('serves the public Tassadar run summary alias without admin auth and with live-at-read staleness (#5114)', async () => {
    const store = makeMemoryStore()
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => 'public-tassadar',
      makeStore: () => store,
      nowIso: () => '2026-06-16T13:30:00.000Z',
      requireAdminApiToken: async request =>
        request.headers.get('authorization') === 'Bearer admin-token-test',
    })

    const planned = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          '/api/training/runs',
          {
            promiseRef: 'training.decentralized_training_launch.v1',
            sourceRefs: ['issue.github.openagents.5114'],
            trainingRunRef: 'run.tassadar.executor.20260615',
          },
          { headers: { authorization: 'Bearer admin-token-test' } },
        ),
        {},
      ),
    )
    expect(planned.status).toBe(200)

    const publicRead = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/public/training/runs/run.tassadar.executor.20260615',
        ),
        {},
      ),
    )
    const body = (await publicRead.json()) as PublicTrainingRunDetailJson

    expect(publicRead.status).toBe(200)
    expect(body.generatedAt).toBe('2026-06-16T13:30:00.000Z')
    expect(body.run.trainingRunRef).toBe('run.tassadar.executor.20260615')
    expect(body.summary.run.trainingRunRef).toBe(
      'run.tassadar.executor.20260615',
    )
    expect(body.summary.emptyState.idle).toBe(true)
    expect(body.summary.metrics.verifiedWorkCount.value).toBe(0)
    expect(body.summary.metrics.providerConfirmedSettledPayoutSats.value).toBe(
      0,
    )
    expect(body.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
    })
    expect(body.staleness.rebuildsOn).toContain(
      'training_run_state_transition_recorded',
    )
    expect(body.sourceRefs).toContain(
      'route:/api/public/training/runs/run.tassadar.executor.20260615',
    )
    expect(body.summary.sourceRefs).toContain(
      'route:/api/public/training/runs/run.tassadar.executor.20260615',
    )
  })

  it('transitions a run planned -> active through the run state-transition route (#5006)', async () => {
    const store = makeMemoryStore()
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => 'tassadar',
      makeStore: () => store,
      nowIso: () => '2026-06-14T10:00:00.000Z',
      requireAdminApiToken: async () => true,
    })

    await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs', {
          manifest: {
            verifierPolicy: 'exact_trace_replay',
            workloadFamily: 'executor-trace',
          },
          promiseRef: 'training.decentralized_training_launch.v1',
          trainingRunRef: 'run.tassadar.executor.20260615',
        }),
        {},
      ),
    )

    const activated = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          '/api/training/runs/run.tassadar.executor.20260615/activate',
          { receiptRef: 'approval.operator.20260614.tassadar_run_authority' },
        ),
        {},
      ),
    )
    const activatedBody = (await activated.json()) as {
      run: {
        generatedAt: string
        manifest: { workloadFamily?: string } | null
        maxStalenessSeconds: number
        state: string
      }
    }

    expect(activated.status).toBe(200)
    expect(activatedBody.run.state).toBe('active')
    expect(activatedBody.run.manifest?.workloadFamily).toBe('executor-trace')
    expect(activatedBody.run.maxStalenessSeconds).toBe(0)
    expect(activatedBody.run.generatedAt).toBe('2026-06-14T10:00:00.000Z')

    const illegal = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          '/api/training/runs/run.tassadar.executor.20260615/activate',
          { receiptRef: 'approval.operator.20260614.again' },
        ),
        {},
      ),
    )

    expect(illegal.status).toBe(409)
  })

  it('admits / excludes an executor-trace contributor through the run admit route (#5007)', async () => {
    const store = makeMemoryStore()
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => 'admit',
      makeStore: () => store,
      nowIso: () => '2026-06-14T10:00:00.000Z',
      requireAdminApiToken: async () => true,
    })

    await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs', {
          promiseRef: 'training.decentralized_training_launch.v1',
          trainingRunRef: 'run.tassadar.executor.20260615',
        }),
        {},
      ),
    )

    const admitted = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs/run.tassadar.executor.20260615/admit', {
          capabilityRefs: [
            'capability.tassadar_poc.numeric_model_executor',
            'receipt.tassadar_executor.self_test.v1.0123456789abcdef',
          ],
          hostRamHeadroomGb: 8,
          pylonRef: 'pylon.contributor.5007',
        }),
        {},
      ),
    )
    const admittedBody = (await admitted.json()) as {
      admission: { decision: string; reasonRefs: ReadonlyArray<string> }
    }

    expect(admitted.status).toBe(200)
    expect(admittedBody.admission.decision).toBe('admitted')

    const excluded = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs/run.tassadar.executor.20260615/admit', {
          capabilityRefs: [
            'capability.tassadar_poc.numeric_model_executor',
            'receipt.tassadar_executor.self_test.v1.0123456789abcdef',
          ],
          hostRamHeadroomGb: 1,
          pylonRef: 'pylon.contributor.5007',
        }),
        {},
      ),
    )
    const excludedBody = (await excluded.json()) as {
      admission: { decision: string }
    }

    expect(excluded.status).toBe(200)
    expect(excludedBody.admission.decision).toBe('excluded')

    const notFound = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs/run.unknown/admit', {
          capabilityRefs: [],
          hostRamHeadroomGb: 8,
          pylonRef: 'pylon.x',
        }),
        {},
      ),
    )

    expect(notFound.status).toBe(404)
  })

  it('creates a run+window-tied exact_trace_replay challenge from an executor-trace closeout (#5008)', async () => {
    const store = makeMemoryStore()
    const routes = makeTrainingRunWindowRoutes({
      createVerificationChallenge: async (_env, request) =>
        buildTrainingVerificationChallengeRecord({
          makeId: () => 'chal',
          nowIso: '2026-06-14T10:00:00.000Z',
          request,
        }).challenge,
      makeId: () => 'cl',
      makeStore: () => store,
      nowIso: () => '2026-06-14T10:00:00.000Z',
      requireAdminApiToken: async () => true,
    })

    await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs', {
          promiseRef: 'training.decentralized_training_launch.v1',
          trainingRunRef: 'run.tassadar.executor.20260615',
        }),
        {},
      ),
    )
    await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/windows/plan', {
          trainingRunRef: 'run.tassadar.executor.20260615',
          windowRef: 'training.window.tassadar.executor.20260615.w1',
        }),
        {},
      ),
    )

    const closeout = {
      assignmentRef: 'assignment.tassadar.5008',
      pylonDeviceRef: 'pylon.device.worker',
      replayDigestRef: 'digest.replay.5008',
      sampledWindow: { endStep: 100, startStep: 0 },
      sampledWindowRef: 'sampled.window.5008',
      traceCommitmentDigestRef: 'digest.commitment.5008',
      validatorDeviceRef: 'pylon.device.validator',
      workerReceiptRef: 'receipt.worker.5008',
      workloadFamily: 'article_closeout',
    }

    const created = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          '/api/training/runs/run.tassadar.executor.20260615/executor-trace-closeout',
          {
            closeout,
            windowRef: 'training.window.tassadar.executor.20260615.w1',
          },
        ),
        {},
      ),
    )
    const createdBody = (await created.json()) as {
      challenge: {
        trainingRunRef: string
        verificationClass: string
        windowRef: string | null
      }
    }

    expect(created.status).toBe(200)
    expect(createdBody.challenge.trainingRunRef).toBe(
      'run.tassadar.executor.20260615',
    )
    expect(createdBody.challenge.verificationClass).toBe('exact_trace_replay')
    expect(createdBody.challenge.windowRef).toBe(
      'training.window.tassadar.executor.20260615.w1',
    )

    // distinct-validator-device rule: validator == worker -> 400
    const sameDevice = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          '/api/training/runs/run.tassadar.executor.20260615/executor-trace-closeout',
          {
            closeout: {
              ...closeout,
              validatorDeviceRef: 'pylon.device.worker',
            },
            windowRef: 'training.window.tassadar.executor.20260615.w1',
          },
        ),
        {},
      ),
    )
    expect(sameDevice.status).toBe(400)

    // unknown window -> 404
    const unknownWindow = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          '/api/training/runs/run.tassadar.executor.20260615/executor-trace-closeout',
          { closeout, windowRef: 'training.window.unknown' },
        ),
        {},
      ),
    )
    expect(unknownWindow.status).toBe(404)
  })

  it('settles accepted exact_trace_replay work into a run-linked settlement receipt that surfaces settledPayoutSats (#5009)', async () => {
    const store = makeMemoryStore()
    const ledgerStore = makeLedgerStoreStub()
    const planned = buildTrainingRunRecord({
      makeId: () => 'run5009',
      nowIso: '2026-06-14T10:00:00.000Z',
      request: {
        manifest: {
          artifactDigestRefs: [],
          blockerRefs: [],
          participantCountRule:
            'Qualified contributor count = admitted contributors with accepted, replay-verified useful work and public-safe receipt refs; never raw registrations or stale heartbeats.',
          spendCapSats: 100,
        },
        promiseRef: 'training.decentralized_training_launch.v1',
        trainingRunRef: 'run.tassadar.executor.20260615',
      },
    })
    store._testSeedRun({ ...planned, state: 'active' })

    const lease: TrainingWindowLeaseRecord = {
      claimedAt: '2026-06-14T10:01:00.000Z',
      id: 'lease5009',
      leaseExpiresAt: '2026-06-14T12:00:00.000Z',
      leaseRef: 'lease.tassadar.5009',
      publicProjectionJson: '{}',
      pylonRef: 'pylon.contributor.stranger',
      receiptRefs: [],
      state: 'active',
      trainingRunRef: 'run.tassadar.executor.20260615',
      windowRef: 'training.window.tassadar.executor.20260615.w1',
    }
    await store.claimLease(lease, '2026-06-14T10:01:00.000Z')

    const challenge = buildTrainingVerificationChallengeRecord({
      makeId: () => '5009',
      nowIso: '2026-06-14T10:02:00.000Z',
      request: {
        commitmentRefs: ['commitment.tassadar.5009'],
        contributionRef: 'contribution.tassadar.5009',
        homeworkKind: 'admin_dispatched_homework',
        payload: {
          replayDigestRef: 'digest.replay.5009',
          traceCommitmentDigestRef: 'digest.commitment.5009',
        },
        trainingRunRef: 'run.tassadar.executor.20260615',
        verificationClass: 'exact_trace_replay',
        windowRef: 'training.window.tassadar.executor.20260615.w1',
      },
    }).challenge
    const leased = leaseTrainingVerificationChallengeRecord({
      challenge,
      eventId: '5009-lease',
      nowIso: '2026-06-14T10:02:30.000Z',
      request: { validatorRef: 'validator.tassadar.5009' },
    }).challenge
    const verified = finalizeTrainingVerificationChallengeRecord({
      challenge: leased,
      eventId: '5009-final',
      nowIso: '2026-06-14T10:03:00.000Z',
      request: { receiptRefs: ['receipt.tassadar.verdict.5009'] },
      verdict: {
        failureCodes: [],
        state: 'Verified',
        verdictRefs: ['verdict.tassadar.5009'],
      },
    }).challenge
    store._testSeedChallenge(verified)

    const routes = makeTrainingRunWindowRoutes({
      makeId: () => 'id5009',
      makePayoutLedgerStore: () => ledgerStore,
      makeStore: () => store,
      nowIso: () => '2026-06-14T10:05:00.000Z',
      requireAdminApiToken: async () => true,
    })

    const settled = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          '/api/training/runs/run.tassadar.executor.20260615/settlement-receipt',
          {
            amountSats: 21,
            challengeRef: verified.challengeRef,
            idempotencyRef: 'idem.tassadar.5009',
            leaseRef: 'lease.tassadar.5009',
            operatorApprovalRef: 'operator.approval.5009',
            payoutTargetApprovalRef: 'payout.target.approval.5009',
            payoutTargetRef: 'payout.target.5009',
          },
        ),
        {},
      ),
    )
    const settledBody = (await settled.json()) as {
      settlement: {
        amountSats: number
        contributorRef: string
        settlementReceiptRef: string
      }
      summary: TrainingRunPublicSummary
    }

    expect(settled.status).toBe(200)
    expect(settledBody.settlement.amountSats).toBe(21)
    expect(settledBody.settlement.contributorRef).toBe(
      'pylon.contributor.stranger',
    )
    expect(settledBody.settlement.settlementReceiptRef).toMatch(
      /^receipt\.nexus\.tassadar_run_settlement\./,
    )
    // Gate OFF (default) => this settlement is a settled-STATE SIMULATION
    // receipt (movementMode:simulation / realBitcoinMoved:false; asserted on the
    // settlements-feed row below). The run-endpoint real-money settled total must
    // EXCLUDE it: no bitcoin moved, so providerConfirmedSettledPayoutSats is 0,
    // not 21. The receipt still surfaces (flagged) in the /settlements feed.
    expect(
      settledBody.summary.metrics.providerConfirmedSettledPayoutSats.value,
    ).toBe(0)
    expect(settledBody.summary.metrics.qualifiedContributorCount.value).toBe(1)
    expect(
      settledBody.summary.metrics.qualifiedContributorCount.provenanceLabel,
    ).toContain('raw registrations and stale heartbeats never count')

    // The settlement receipt is linked onto the run, so a fresh GET reflects the
    // SAME real-only resolution from the ledger: a settled-state simulation
    // receipt contributes 0 real settled sats (sim excluded), matching the
    // settlement-receipt response and the public summary endpoint.
    const detail = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/runs/run.tassadar.executor.20260615',
        ),
        {},
      ),
    )
    const detailBody = (await detail.json()) as TrainingRunDetailJson

    expect(detail.status).toBe(200)
    expect(
      detailBody.summary.metrics.providerConfirmedSettledPayoutSats.value,
    ).toBe(0)
    expect(detailBody.run.manifest?.participantCountRule).toContain(
      'never raw registrations or stale heartbeats',
    )
    expect(detailBody.summary.metrics.qualifiedContributorCount.value).toBe(1)

    // Enumerable settled feed keyed by run (#5316): a contributor can list and
    // dereference the run-linked settled receipt without trusting a forum post.
    const settlementsFeed = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/runs/run.tassadar.executor.20260615/settlements',
        ),
        {},
      ),
    )
    const settlementsBody = (await settlementsFeed.json()) as {
      runRef: string
      schemaVersion: string
      settlementRows: ReadonlyArray<{
        amountSats: number
        contributorRef: string | null
        movementMode: string
        realBitcoinMoved: boolean
        receiptRef: string
        state: string
        trainingRunRef: string | null
        verificationChallengeRef: string | null
      }>
      sourceRefs: ReadonlyArray<string>
      staleness: { maxStalenessSeconds: number }
    }

    expect(settlementsFeed.status).toBe(200)
    expect(settlementsBody.runRef).toBe('run.tassadar.executor.20260615')
    expect(settlementsBody.schemaVersion).toBe(
      'openagents.training_run_settlements.v1',
    )
    expect(settlementsBody.staleness.maxStalenessSeconds).toBe(0)
    expect(settlementsBody.settlementRows).toHaveLength(1)
    const row = settlementsBody.settlementRows[0]!
    expect(row.amountSats).toBe(21)
    expect(row.state).toBe('settled')
    expect(row.contributorRef).toBe('pylon.contributor.stranger')
    expect(row.verificationChallengeRef).toBe(verified.challengeRef)
    expect(row.trainingRunRef).toBe('run.tassadar.executor.20260615')
    expect(row.movementMode).toBe('simulation')
    expect(row.realBitcoinMoved).toBe(false)
    expect(row.receiptRef).toMatch(
      /^receipt\.nexus\.tassadar_run_settlement\./,
    )
    // Redaction: no raw spark address, invoice, or preimage in the body.
    expect(JSON.stringify(settlementsBody)).not.toMatch(/spark1|lnbc|preimage/i)

    // Over the run spend cap -> rejected, no money-shaped projection leaks.
    const overCap = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          '/api/training/runs/run.tassadar.executor.20260615/settlement-receipt',
          {
            amountSats: 5000,
            challengeRef: verified.challengeRef,
            idempotencyRef: 'idem.tassadar.5009.overcap',
            leaseRef: 'lease.tassadar.5009',
            operatorApprovalRef: 'operator.approval.5009',
            payoutTargetApprovalRef: 'payout.target.approval.5009',
            payoutTargetRef: 'payout.target.5009',
          },
        ),
        {},
      ),
    )
    expect(overCap.status).toBe(400)
  })

  it('run endpoints read the real-only settled total (1005 not 1010): settled-state simulation is excluded from the metric/settlement total but still enumerated in the feed', async () => {
    const store = makeMemoryStore()
    // Two REAL-bitcoin settled receipts (1000-sat canary + Trigger's 5) and one
    // settled-STATE SIMULATION receipt (5 sats, realBitcoinMoved:false). The
    // real-only total must be 1005, not 1010 — the duplicate real-blind resolver
    // used to count the sim row. All three are run-linked, so all appear in the
    // /settlements feed (the sim flagged), but only the two real ones count.
    const ledgerStore = makeLedgerStoreStub([
      [
        'receipt.tassadar.real.canary.1000',
        {
          publicProjectionJson: JSON.stringify({
            amountSats: 1000,
            contributorRef: 'pylon.canary',
            moneyMovement: 'real_bitcoin',
            movementMode: 'real_bitcoin',
            realBitcoinMoved: true,
            state: 'settled',
            trainingRunRef: 'run.tassadar.executor.20260615',
          }),
          receiptKind: 'settlement_recorded',
        },
      ],
      [
        'receipt.tassadar.real.trigger.5',
        {
          publicProjectionJson: JSON.stringify({
            amountSats: 5,
            contributorRef: 'pylon.trigger',
            moneyMovement: 'real_bitcoin',
            movementMode: 'real_bitcoin',
            realBitcoinMoved: true,
            state: 'settled',
            trainingRunRef: 'run.tassadar.executor.20260615',
          }),
          receiptKind: 'settlement_recorded',
        },
      ],
      [
        'receipt.tassadar.sim.5',
        {
          publicProjectionJson: JSON.stringify({
            amountSats: 5,
            contributorRef: 'pylon.sim',
            movementMode: 'simulation',
            realBitcoinMoved: false,
            state: 'settled',
            trainingRunRef: 'run.tassadar.executor.20260615',
          }),
          receiptKind: 'settlement_recorded',
        },
      ],
    ])
    store._testSeedRun({
      ...buildTrainingRunRecord({
        makeId: () => 'run1005',
        nowIso: '2026-06-15T10:00:00.000Z',
        request: {
          promiseRef: 'training.decentralized_training_launch.v1',
          trainingRunRef: 'run.tassadar.executor.20260615',
        },
      }),
      receiptRefs: [
        'receipt.tassadar.real.canary.1000',
        'receipt.tassadar.real.trigger.5',
        'receipt.tassadar.sim.5',
      ],
      state: 'active',
    })

    const routes = makeTrainingRunWindowRoutes({
      makeId: () => 'id1005',
      makePayoutLedgerStore: () => ledgerStore,
      makeStore: () => store,
      nowIso: () => '2026-06-15T10:05:00.000Z',
      requireAdminApiToken: async () => true,
    })

    const detail = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/runs/run.tassadar.executor.20260615',
        ),
        {},
      ),
    )
    const detailBody = (await detail.json()) as TrainingRunDetailJson & {
      summary: TrainingRunPublicSummary & {
        settlement: { settledPayoutSats: number; settledReceiptCount: number }
      }
    }

    expect(detail.status).toBe(200)
    // Real-only: 1000 + 5 = 1005 (the sim 5 is excluded). Both the metric and
    // the settlement reconciliation total read the same real-only resolution.
    expect(
      detailBody.summary.metrics.providerConfirmedSettledPayoutSats.value,
    ).toBe(1005)
    expect(detailBody.summary.settlement.settledPayoutSats).toBe(1005)
    expect(detailBody.summary.settlement.settledReceiptCount).toBe(2)

    // The list endpoint reads the SAME real-only total.
    const list = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request('https://openagents.test/api/training/runs'),
        {},
      ),
    )
    const listBody = (await list.json()) as TrainingRunListJson
    const summary = listBody.summaries.find(
      candidate =>
        candidate.run.trainingRunRef === 'run.tassadar.executor.20260615',
    )!
    expect(summary.metrics.providerConfirmedSettledPayoutSats.value).toBe(1005)

    // The settled-state SIMULATION receipt is still ENUMERATED in the feed,
    // flagged realBitcoinMoved:false, alongside the two real receipts.
    const feed = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/runs/run.tassadar.executor.20260615/settlements',
        ),
        {},
      ),
    )
    const feedBody = (await feed.json()) as {
      settlementRows: ReadonlyArray<{
        amountSats: number
        movementMode: string
        realBitcoinMoved: boolean
        receiptRef: string
      }>
    }
    expect(feed.status).toBe(200)
    expect(feedBody.settlementRows).toHaveLength(3)
    const simRow = feedBody.settlementRows.find(
      row => row.receiptRef === 'receipt.tassadar.sim.5',
    )!
    expect(simRow.movementMode).toBe('simulation')
    expect(simRow.realBitcoinMoved).toBe(false)
    expect(
      feedBody.settlementRows
        .filter(row => row.realBitcoinMoved === true)
        .reduce((total, row) => total + row.amountSats, 0),
    ).toBe(1005)
  })

  it('projects only Verified exact_trace_replay closed ticks as the Tassadar verified-trace corpus, rebuilding on verdict transitions (#5010)', async () => {
    const store = makeMemoryStore()
    const planned = buildTrainingRunRecord({
      makeId: () => 'run5010',
      nowIso: '2026-06-14T10:00:00.000Z',
      request: {
        promiseRef: 'training.decentralized_training_launch.v1',
        trainingRunRef: 'run.tassadar.executor.20260615',
      },
    })
    store._testSeedRun({ ...planned, state: 'active' })

    const seedChallenge = (
      id: string,
      verificationClass: 'exact_trace_replay' | 'deterministic_recompute',
      verify: boolean,
    ): void => {
      const built = buildTrainingVerificationChallengeRecord({
        makeId: () => id,
        nowIso: '2026-06-14T10:02:00.000Z',
        request: {
          commitmentRefs: [`commitment.tassadar.${id}`],
          contributionRef: `contribution.tassadar.${id}`,
          homeworkKind: 'admin_dispatched_homework',
          payload: { traceCommitmentDigestRef: `digest.commitment.${id}` },
          trainingRunRef: 'run.tassadar.executor.20260615',
          verificationClass,
          windowRef: 'training.window.tassadar.executor.20260615.w1',
        },
      }).challenge
      const leased = leaseTrainingVerificationChallengeRecord({
        challenge: built,
        eventId: `${id}-lease`,
        nowIso: '2026-06-14T10:02:30.000Z',
        request: { validatorRef: `validator.tassadar.${id}` },
      }).challenge

      store._testSeedChallenge(
        verify
          ? finalizeTrainingVerificationChallengeRecord({
              challenge: leased,
              eventId: `${id}-final`,
              nowIso: '2026-06-14T10:03:00.000Z',
              request: { receiptRefs: [`receipt.tassadar.verdict.${id}`] },
              verdict: {
                failureCodes: [],
                state: 'Verified',
                verdictRefs: [`verdict.tassadar.${id}`],
              },
            }).challenge
          : leased,
      )
    }

    // corpus: a Verified exact_trace_replay closed tick.
    seedChallenge('5010a', 'exact_trace_replay', true)
    // excluded: Verified but wrong class.
    seedChallenge('5010b', 'deterministic_recompute', true)
    // excluded: exact_trace_replay but only Leased (not yet Verified).
    seedChallenge('5010c', 'exact_trace_replay', false)

    const routes = makeTrainingRunWindowRoutes({
      makeId: () => 'id5010',
      makeStore: () => store,
      nowIso: () => '2026-06-14T10:05:00.000Z',
      requireAdminApiToken: async () => true,
    })

    const detail = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/runs/run.tassadar.executor.20260615',
        ),
        {},
      ),
    )
    const body = (await detail.json()) as TrainingRunDetailJson

    expect(detail.status).toBe(200)
    expect(body.summary.corpus.acceptedTraceCount).toBe(1)
    expect(body.summary.metrics.qualifiedContributorCount.value).toBe(0)
    expect(body.summary.corpus.laneRef).toBe('tassadar.verified_trace_corpus')
    expect(body.summary.corpus.traceRefs).toContain(
      'training.verification.challenge.5010a',
    )
    expect(body.summary.corpus.traceRefs).not.toContain(
      'training.verification.challenge.5010b',
    )
    expect(body.summary.corpus.traceRefs).not.toContain(
      'training.verification.challenge.5010c',
    )
    expect(body.summary.corpus.verdictRefs).toContain('verdict.tassadar.5010a')
    // Distinct from generic verifiedWorkCount, which counts both Verified rows.
    expect(body.summary.metrics.verifiedWorkCount.value).toBe(2)
    // Live-at-read, rebuilds on verification-challenge transitions, not reg.
    expect(body.summary.corpus.staleness.maxStalenessSeconds).toBe(0)
    expect(body.summary.corpus.staleness.rebuildsOn).toContain(
      'training_verification_challenge_verified_transition_recorded',
    )
  })

  it('returns an honest idle empty state for runs with no windows or verification data', async () => {
    const store = makeMemoryStore()
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => 'idle',
      makeStore: () => store,
      nowIso: () => '2026-06-10T10:00:00.000Z',
      requireAdminApiToken: async () => true,
    })

    await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs', {
          promiseRef: 'pylon.first_real_model_training_run.v1',
          trainingRunRef: 'training.run.idle',
        }),
        {},
      ),
    )

    const readRun = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/runs/training.run.idle',
        ),
        {},
      ),
    )
    const body = (await readRun.json()) as TrainingRunDetailJson

    expect(readRun.status).toBe(200)
    expect(body.summary).toMatchObject({
      emptyState: {
        idle: true,
      },
      metrics: {
        assignedContributorCount: { value: 0 },
        providerConfirmedSettledPayoutSats: { value: 0 },
        verifiedWorkCount: { value: 0 },
      },
    })
  })

  it('returns an empty settlements feed and never enumerates receipts not linked to the run (#5316)', async () => {
    const store = makeMemoryStore()
    // A settled receipt that exists in the ledger but is NOT linked to the run.
    const ledgerStore = makeLedgerStoreStub([
      [
        'receipt.unrelated.run.settled',
        {
          publicProjectionJson: JSON.stringify({
            amountSats: 777,
            contributorRef: 'pylon.unrelated',
            state: 'settled',
            trainingRunRef: 'run.some.other.run',
          }),
          receiptKind: 'settlement_recorded',
        },
      ],
    ])
    const planned = buildTrainingRunRecord({
      makeId: () => 'run5316empty',
      nowIso: '2026-06-16T10:00:00.000Z',
      request: {
        promiseRef: 'training.decentralized_training_launch.v1',
        trainingRunRef: 'run.tassadar.executor.empty',
      },
    })
    store._testSeedRun({ ...planned, state: 'active' })

    const routes = makeTrainingRunWindowRoutes({
      makeId: () => 'id5316empty',
      makePayoutLedgerStore: () => ledgerStore,
      makeStore: () => store,
      nowIso: () => '2026-06-16T10:05:00.000Z',
      requireAdminApiToken: async () => true,
    })

    const feed = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/runs/run.tassadar.executor.empty/settlements',
        ),
        {},
      ),
    )
    const body = (await feed.json()) as {
      runRef: string
      settlementRows: ReadonlyArray<unknown>
    }

    expect(feed.status).toBe(200)
    expect(body.runRef).toBe('run.tassadar.executor.empty')
    // The unrelated settled receipt is not a run-linked ref, so it never appears.
    expect(body.settlementRows).toEqual([])
  })

  it('returns 404 for the settlements feed when the run is not found (#5316)', async () => {
    const store = makeMemoryStore()
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => 'id5316missing',
      makeStore: () => store,
      nowIso: () => '2026-06-16T10:05:00.000Z',
      requireAdminApiToken: async () => true,
    })

    const feed = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/runs/run.does.not.exist/settlements',
        ),
        {},
      ),
    )

    expect(feed.status).toBe(404)
  })

  // #5403 gap 2: the public-LOOKING settlements path (with `/public/`) used to
  // 404; a skeptic curling it got nothing even though the data is public. The
  // public alias must serve the IDENTICAL public-safe handler as the
  // non-`/public/` feed.
  it('serves the per-run settlements feed under the /public/ alias path (#5403)', async () => {
    const store = makeMemoryStore()
    const runRef = 'run.tassadar.executor.alias'
    const linkedReceiptRef = 'receipt.nexus.tassadar_run_settlement.alias.real5'
    const ledgerStore = makeLedgerStoreStub([
      [
        linkedReceiptRef,
        {
          publicProjectionJson: JSON.stringify({
            amountSats: 5,
            contributorRef: 'pylon.public.worker_alias',
            moneyMovement: 'real_bitcoin',
            movementMode: 'real_bitcoin',
            realBitcoinMoved: true,
            state: 'settled',
            trainingRunRef: runRef,
          }),
          receiptKind: 'settlement_recorded',
        },
      ],
    ])
    const planned = buildTrainingRunRecord({
      makeId: () => 'run5403alias',
      nowIso: '2026-06-16T10:00:00.000Z',
      request: {
        promiseRef: 'training.decentralized_training_launch.v1',
        trainingRunRef: runRef,
      },
    })
    store._testSeedRun({
      ...planned,
      receiptRefs: [linkedReceiptRef],
      state: 'active',
    })

    const routes = makeTrainingRunWindowRoutes({
      makeId: () => 'id5403alias',
      makePayoutLedgerStore: () => ledgerStore,
      makeStore: () => store,
      nowIso: () => '2026-06-16T10:05:00.000Z',
      requireAdminApiToken: async () => true,
    })

    const readFeed = async (path: string) => {
      const response = await runRoute(
        routes.routeTrainingRunWindowRequest(
          new Request(`https://openagents.test${path}`),
          {},
        ),
      )
      return {
        body: (await response.json()) as {
          runRef: string
          schemaVersion: string
          settlementRows: ReadonlyArray<{
            amountSats: number
            movementMode: string
            realBitcoinMoved: boolean
            receiptRef: string
          }>
          sourceRefs: ReadonlyArray<string>
          staleness: { maxStalenessSeconds: number }
        },
        status: response.status,
      }
    }

    const canonical = await readFeed(
      `/api/training/runs/${runRef}/settlements`,
    )
    const publicAlias = await readFeed(
      `/api/public/training/runs/${runRef}/settlements`,
    )

    expect(canonical.status).toBe(200)
    expect(publicAlias.status).toBe(200)
    // The alias returns byte-for-byte the same public-safe payload.
    expect(publicAlias.body).toEqual(canonical.body)
    expect(publicAlias.body.schemaVersion).toBe(
      'openagents.training_run_settlements.v1',
    )
    expect(publicAlias.body.staleness.maxStalenessSeconds).toBe(0)
    expect(publicAlias.body.settlementRows).toHaveLength(1)
    expect(publicAlias.body.settlementRows[0]?.movementMode).toBe(
      'real_bitcoin',
    )
    expect(publicAlias.body.settlementRows[0]?.realBitcoinMoved).toBe(true)
    expect(publicAlias.body.sourceRefs).toEqual(
      expect.arrayContaining([
        `route:/api/public/training/runs/${runRef}/settlements`,
        `route:/api/training/runs/${runRef}/settlements`,
      ]),
    )
  })

  it('returns 404 for the /public/ settlements alias when the run is not found (#5403)', async () => {
    const store = makeMemoryStore()
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => 'id5403aliasmissing',
      makeStore: () => store,
      nowIso: () => '2026-06-16T10:05:00.000Z',
      requireAdminApiToken: async () => true,
    })

    const feed = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/public/training/runs/run.does.not.exist/settlements',
        ),
        {},
      ),
    )

    expect(feed.status).toBe(404)
  })

  it('admits receipted scaling-sweep evidence through the admin route and rejects unreceipted cells', async () => {
    const store = makeMemoryStore()
    let counter = 0
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => `a3-evidence-${++counter}`,
      makeStore: () => store,
      nowIso: () => '2026-06-11T08:00:00.000Z',
      requireAdminApiToken: async request =>
        request.headers.get('authorization') === 'Bearer admin-token-test',
    })
    const adminHeaders = { authorization: 'Bearer admin-token-test' }
    const evidencePath =
      '/api/training/runs/run.cs336.a3.scaling_sweep.demo/scaling-sweep-evidence'
    const cell = {
      cellRef: 'cell.cs336_a3.b1.n1',
      computeBudgetFlops: 300_000_000,
      parameterCount: 1_024,
      pylonRef: 'pylon.24819249b4634a4c9d5e',
      receiptRefs: ['receipt.cs336_a3.settlement.cell_1'],
      tokenCount: 48_828,
      validationLoss: 5.0621,
      verificationRefs: ['verdict.training.deterministic_recompute.cell_1'],
    }

    store._testSeedRun(
      buildTrainingRunRecord({
        makeId: () => 'a3-admit',
        nowIso: '2026-06-11T08:00:00.000Z',
        request: {
          promiseRef: 'pylon.compute_revenue_modes.v1',
          trainingRunRef: 'run.cs336.a3.scaling_sweep.demo',
        },
      }),
    )

    const unauthorized = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(evidencePath, { cells: [cell] }),
        {},
      ),
    )

    expect(unauthorized.status).toBe(401)

    const missingRun = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          '/api/training/runs/run.cs336.a3.missing/scaling-sweep-evidence',
          { cells: [cell] },
          { headers: adminHeaders },
        ),
        {},
      ),
    )

    expect(missingRun.status).toBe(404)

    const unreceipted = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          evidencePath,
          { cells: [{ ...cell, receiptRefs: [] }] },
          { headers: adminHeaders },
        ),
        {},
      ),
    )

    expect(unreceipted.status).toBe(400)

    const admitted = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          evidencePath,
          {
            cells: [cell],
            receiptRefs: ['approval.operator.20260611.focus_cs336_issue4679'],
            sourceRefs: ['issue.github.openagents.4679'],
          },
          { headers: adminHeaders },
        ),
        {},
      ),
    )
    const admittedBody = (await admitted.json()) as Readonly<{
      isoflop: Readonly<{
        cells: ReadonlyArray<Record<string, unknown>>
        status: string
      }>
    }>

    expect(admitted.status).toBe(200)
    expect(admittedBody.isoflop.status).toBe('collecting_cells')
    expect(admittedBody.isoflop.cells[0]).toMatchObject({
      cellRef: 'cell.cs336_a3.b1.n1',
      parameterCount: 1_024,
      verified: true,
    })

    const dashboard = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request('https://openagents.test/api/training/isoflop/a3'),
        {},
      ),
    )
    const dashboardBody = (await dashboard.json()) as TrainingRunIsoFlopJson

    expect(dashboard.status).toBe(200)
    expect(dashboardBody.cells).toHaveLength(1)
  })

  it('admits receipted A4 data-refinery evidence through the admin route and serves the refinery dashboard', async () => {
    const store = makeMemoryStore()
    let counter = 0
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => `a4-evidence-${++counter}`,
      makeStore: () => store,
      nowIso: () => '2026-06-11T02:30:00.000Z',
      requireAdminApiToken: async request =>
        request.headers.get('authorization') === 'Bearer admin-token-test',
    })
    const adminHeaders = { authorization: 'Bearer admin-token-test' }
    const evidencePath =
      '/api/training/runs/run.cs336.a4.data_refinery.demo/data-refinery-evidence'
    const outputDigestRef = 'digest.sha256.cs336_a4.pii_masking.aaaa'
    const corpusProvenanceReceipt = await buildCs336A4ProvenanceReceipt({
      assignmentRef: 'assignment.cs336_a4.pii_masking.1',
      finalOutputDigestRef: outputDigestRef,
      inputShardRef: 'shard.cs336_a4.pii_masking.1',
      provenance: {
        acquisitionMode: 'bounded_synthetic_corpus',
        licenseRef: 'license.public.cc0.synthetic_corpus_v1',
        snapshotRef: 'snapshot.cs336_a4.pii_masking.v1',
        sourceRef: 'source.psion.bounded_synthetic_mixture.v1',
      },
      sourceInputDigestRef: 'digest.cs336_a4.pii_masking.source',
      transformChain: [
        {
          codeVersionRef: 'psionic.refinery.v1.pii_masking',
          inputDigestRef: 'digest.cs336_a4.pii_masking.source',
          outputDigestRef,
          recomputedDigestRef: outputDigestRef,
          stage: 'pii_masking',
        },
      ],
    })
    const shard = {
      corpusProvenanceReceipt,
      inputDocumentCount: 64,
      outputDigestRef,
      pylonRef: 'pylon.24819249b4634a4c9d5e',
      receiptRefs: ['receipt.cs336_a4.settlement.pii_masking'],
      shardRef: 'shard.cs336_a4.pii_masking.1',
      sourceRefs: ['commitment.cs336_a4.pii_masking.sha256_abcdef0123456789'],
      stage: 'pii_masking' as const,
      verificationRefs: [
        'verdict.training.deterministic_recompute.pii_masking',
      ],
    }

    store._testSeedRun(
      buildTrainingRunRecord({
        makeId: () => 'a4-admit',
        nowIso: '2026-06-11T02:00:00.000Z',
        request: {
          promiseRef: 'training.data_refinery_corpus.v1',
          trainingRunRef: 'run.cs336.a4.data_refinery.demo',
        },
      }),
    )

    const unauthorized = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(evidencePath, { shards: [shard] }),
        {},
      ),
    )

    expect(unauthorized.status).toBe(401)

    const unreceipted = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          evidencePath,
          { shards: [{ ...shard, receiptRefs: [] }] },
          { headers: adminHeaders },
        ),
        {},
      ),
    )

    expect(unreceipted.status).toBe(400)

    const admitted = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          evidencePath,
          {
            receiptRefs: ['approval.operator.20260611.focus_cs336_issue4680'],
            shards: [shard],
            sourceRefs: ['issue.github.openagents.4680'],
          },
          { headers: adminHeaders },
        ),
        {},
      ),
    )
    const admittedBody = (await admitted.json()) as Readonly<{
      refinery: Readonly<{
        evalDeltaPaymentGate: Readonly<{
          greenGateSatisfied: boolean
          paymentComputationAvailable: boolean
          settlementReceiptAvailable: boolean
          verifiedMeasurementRowCount: number
        }>
        shards: ReadonlyArray<Record<string, unknown>>
        status: string
      }>
    }>

    expect(admitted.status).toBe(200)
    expect(admittedBody.refinery.status).toBe('collecting_shards')
    expect(admittedBody.refinery.shards[0]).toMatchObject({
      corpusProvenanceReceiptRef: corpusProvenanceReceipt.receiptRef,
      corpusProvenanceVerified: true,
      stage: 'pii_masking',
      verified: true,
    })
    expect(admittedBody.refinery).toMatchObject({
      corpusProvenanceReceiptBlockerRefs: [],
      corpusProvenanceReceiptRefs: [corpusProvenanceReceipt.receiptRef],
      corpusProvenanceReceiptStatus: 'available',
      evalDeltaPaymentGate: {
        greenGateSatisfied: false,
        paymentComputationAvailable: true,
        settlementReceiptAvailable: false,
        verifiedMeasurementRowCount: 0,
      },
    })

    const dashboard = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request('https://openagents.test/api/training/refinery/a4'),
        {},
      ),
    )
    const dashboardBody = (await dashboard.json()) as Readonly<{
      corpusProvenanceReceiptStatus: string
      evalDeltaPaymentGate: Readonly<{
        greenGateSatisfied: boolean
        paymentComputationAvailable: boolean
        settlementReceiptAvailable: boolean
        verifiedMeasurementRowCount: number
      }>
      schemaVersion: string
      shards: ReadonlyArray<unknown>
    }>

    expect(dashboard.status).toBe(200)
    expect(dashboardBody.schemaVersion).toBe(
      'openagents.training.data_refinery_dashboard.v1',
    )
    expect(dashboardBody.corpusProvenanceReceiptStatus).toBe('available')
    expect(dashboardBody.evalDeltaPaymentGate).toMatchObject({
      greenGateSatisfied: false,
      paymentComputationAvailable: true,
      settlementReceiptAvailable: false,
      verifiedMeasurementRowCount: 0,
    })
    expect(dashboardBody.shards).toHaveLength(1)
  })

  it('admits receipted A5 alignment evidence through the admin route and serves the eval dashboard', async () => {
    const store = makeMemoryStore()
    let counter = 0
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => `a5-evidence-${++counter}`,
      makeStore: () => store,
      nowIso: () => '2026-06-11T08:00:00.000Z',
      requireAdminApiToken: async request =>
        request.headers.get('authorization') === 'Bearer admin-token-test',
    })
    const adminHeaders = { authorization: 'Bearer admin-token-test' }
    const evidencePath =
      '/api/training/runs/run.cs336.a5.alignment.demo/alignment-eval-evidence'
    const suite = {
      evalSuiteRef: 'eval.cs336_a5.synthetic_math.bounded.1',
      metric: 'accuracy' as const,
      receiptRefs: ['receipt.cs336_a5.settlement.reward_grading.split_a'],
      sampleCount: 256,
      score: 0.66,
      sourceRefs: ['workload.cs336_a5.seeded_rollout_and_reference_grading.v1'],
      splitRef: 'split.cs336_a5.synthetic_math.bounded_combined.v1',
      taskSetRef: 'math' as const,
      verificationRefs: [
        'verdict.training.deterministic_recompute.reward_grading',
      ],
      verifiedSampleCount: 256,
    }

    store._testSeedRun(
      buildTrainingRunRecord({
        makeId: () => 'a5-admit',
        nowIso: '2026-06-11T08:00:00.000Z',
        request: {
          promiseRef: 'training.post_training_arc.v1',
          trainingRunRef: 'run.cs336.a5.alignment.demo',
        },
      }),
    )

    const unauthorized = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(evidencePath, { evalSuites: [suite] }),
        {},
      ),
    )

    expect(unauthorized.status).toBe(401)

    const unreceipted = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          evidencePath,
          { evalSuites: [{ ...suite, receiptRefs: [] }] },
          { headers: adminHeaders },
        ),
        {},
      ),
    )

    expect(unreceipted.status).toBe(400)

    const admitted = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          evidencePath,
          {
            evalSuites: [suite],
            receiptRefs: ['approval.operator.20260611.focus_cs336_issue4682'],
            shards: [
              {
                jobKind: 'cs336_a5_reward_grading',
                outputDigestRef:
                  'digest.cs336_a5.reward_grading.split_a.sha256_abcd',
                pylonRef: 'pylon.24819249b4634a4c9d5e',
                receiptRefs: [
                  'receipt.cs336_a5.settlement.reward_grading.split_a',
                ],
                rolloutCount: 128,
                splitRef: 'split_a',
                verificationRefs: [
                  'verdict.training.deterministic_recompute.reward_grading',
                ],
              },
            ],
            sourceRefs: ['issue.github.openagents.4682'],
          },
          { headers: adminHeaders },
        ),
        {},
      ),
    )
    const admittedBody = (await admitted.json()) as Readonly<{
      evals: Readonly<{
        blockerRefs: ReadonlyArray<string>
        evalSuites: ReadonlyArray<Record<string, unknown>>
      }>
    }>

    expect(admitted.status).toBe(200)
    expect(admittedBody.evals.blockerRefs).toEqual([])
    expect(admittedBody.evals.evalSuites[0]).toMatchObject({
      evalSuiteRef: 'eval.cs336_a5.synthetic_math.bounded.1',
      score: 0.66,
      taskSetRef: 'math',
    })

    const dashboard = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request('https://openagents.test/api/training/evals/a5'),
        {},
      ),
    )
    const dashboardBody = (await dashboard.json()) as Readonly<{
      blockerRefs: ReadonlyArray<string>
      evalSuites: ReadonlyArray<unknown>
      schemaVersion: string
    }>

    expect(dashboard.status).toBe(200)
    expect(dashboardBody.schemaVersion).toBe(
      'openagents.training.a5_eval_dashboard.v1',
    )
    expect(dashboardBody.evalSuites).toHaveLength(1)
    expect(dashboardBody.blockerRefs).toEqual([])
  })

  it('admits receipted device benchmark evidence through the admin route and rejects unsafe or unreceipted rows', async () => {
    const store = makeMemoryStore()
    let counter = 0
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => `a2-evidence-${++counter}`,
      makeStore: () => store,
      nowIso: () => '2026-06-11T08:00:00.000Z',
      requireAdminApiToken: async request =>
        request.headers.get('authorization') === 'Bearer admin-token-test',
    })
    const adminHeaders = { authorization: 'Bearer admin-token-test' }
    const evidencePath =
      '/api/training/runs/run.cs336.a2.device_capability.demo/device-benchmark-evidence'
    const measurement = {
      deviceClassRef: 'device_class.apple_silicon_macos.arm64',
      earningEstimate: {
        p50SatsPerHour: 36,
        sourceRefs: ['receipt.cs336.a2.settlement.1'],
        workClass: 'cs336_a2_device_benchmark',
      },
      max: 2210,
      metric: 'tokens_per_second',
      min: 1810,
      p50: 1995,
      p90: 2120,
      receiptRefs: ['receipt.cs336.a2.settlement.1'],
      sampleCount: 6,
      unit: 'tokens_per_second',
      verificationRefs: ['verdict.training.statistical_cross_check.1'],
      workClass: 'cs336_a2_device_benchmark',
    }

    store._testSeedRun(
      buildTrainingRunRecord({
        makeId: () => 'a2-admit',
        nowIso: '2026-06-11T08:00:00.000Z',
        request: {
          promiseRef: 'training.device_capability_dataset.v1',
          trainingRunRef: 'run.cs336.a2.device_capability.demo',
        },
      }),
    )

    const unauthorized = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(evidencePath, { measurements: [measurement] }),
        {},
      ),
    )

    expect(unauthorized.status).toBe(401)

    const missingRun = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          '/api/training/runs/run.cs336.a2.missing/device-benchmark-evidence',
          { measurements: [measurement] },
          { headers: adminHeaders },
        ),
        {},
      ),
    )

    expect(missingRun.status).toBe(404)

    const unreceipted = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          evidencePath,
          { measurements: [{ ...measurement, receiptRefs: [] }] },
          { headers: adminHeaders },
        ),
        {},
      ),
    )

    expect(unreceipted.status).toBe(400)

    const unsafe = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          evidencePath,
          {
            measurements: [
              {
                ...measurement,
                sourceRefs: ['payment_hash.deadbeef'],
              },
            ],
          },
          { headers: adminHeaders },
        ),
        {},
      ),
    )

    expect(unsafe.status).toBe(400)

    const admitted = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          evidencePath,
          {
            measurements: [measurement],
            receiptRefs: ['receipt.cs336.a2.settlement.1'],
            sourceRefs: ['issue.github.openagents.4681'],
          },
          { headers: adminHeaders },
        ),
        {},
      ),
    )
    const admittedBody = (await admitted.json()) as Readonly<{
      dataset: TrainingDeviceCapabilityJson
    }>

    expect(admitted.status).toBe(200)
    expect(admittedBody.dataset).toMatchObject({
      blockerRefs: [
        'blocker.cs336_a2.requires_cross_machine_same_class_replication',
      ],
      classDistributions: [
        {
          deviceClassRef: 'device_class.apple_silicon_macos.arm64',
          earningEstimate: {
            basisLabel: 'modeled_from_measured_benchmark_distribution',
          },
          verified: true,
        },
      ],
      schemaVersion: 'openagents.training.device_capability_dataset.v1',
      sameClassReplicationBlockerRefs: [
        'blocker.cs336_a2.requires_cross_machine_same_class_replication',
      ],
      sameClassReplicationSignals: [{ state: 'same_host_only' }],
      sameClassReplicationStatus: 'same_host_only',
      thermalThrottleBlockerRefs: [
        'blocker.cs336_a2.requires_sustained_vs_burst_thermal_probe',
      ],
      thermalThrottleDetectionStatus: 'missing',
      thermalThrottleSignals: [],
    })

    const dashboard = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/device-capabilities/a2',
        ),
        {},
      ),
    )
    const dashboardBody =
      (await dashboard.json()) as TrainingDeviceCapabilityJson

    expect(dashboard.status).toBe(200)
    expect(dashboardBody).toMatchObject({
      blockerRefs: [
        'blocker.cs336_a2.requires_cross_machine_same_class_replication',
      ],
      classDistributions: [
        {
          deviceClassRef: 'device_class.apple_silicon_macos.arm64',
          verified: true,
        },
      ],
      sameClassReplicationBlockerRefs: [
        'blocker.cs336_a2.requires_cross_machine_same_class_replication',
      ],
      sameClassReplicationSignals: [{ state: 'same_host_only' }],
      sameClassReplicationStatus: 'same_host_only',
      thermalThrottleBlockerRefs: [
        'blocker.cs336_a2.requires_sustained_vs_burst_thermal_probe',
      ],
      thermalThrottleDetectionStatus: 'missing',
      thermalThrottleSignals: [],
    })
  })

  it('surfaces verified thermal-row receipts and reason codes through the A2 dashboard', async () => {
    const store = makeMemoryStore()
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => 'a2-thermal-route',
      makeStore: () => store,
      nowIso: () => '2026-06-28T00:00:00.000Z',
      requireAdminApiToken: async request =>
        request.headers.get('authorization') === 'Bearer admin-token-test',
    })
    const evidencePath =
      '/api/training/runs/run.cs336.a2.device_capability.thermal/device-benchmark-evidence'
    const measurement = {
      deviceClassRef: 'device_class.example.gpu_24gb',
      digestCommitmentRefs: ['commitment.cs336_a2.thermal.sha256_demo'],
      max: 0.78,
      measurementRef: 'measurement.cs336_a2.thermal.example_gpu_24gb',
      metric: 'sustained_vs_burst_throughput_ratio',
      min: 0.7,
      ownerAcceptedThermalReceiptRefs: [
        'receipt.cs336_a2.thermal.owner_accepted.production.1',
      ],
      p50: 0.74,
      p90: 0.78,
      receiptRefs: ['receipt.cs336_a2.thermal.verified_row.1'],
      sameClassReplicationEvidenceRefs: [
        'evidence.cs336_a2.replication.cross_machine.thermal.1',
      ],
      sameClassReplicationScope: 'cross_machine_same_class',
      sampleCount: 3,
      sourceRefs: ['artifact.cs336_a2.thermal_probe.window_samples.1'],
      unit: 'ratio',
      verificationRefs: ['verdict.training.statistical_cross_check.thermal.1'],
      workClass: 'cs336_a2_device_benchmark',
    }

    store._testSeedRun(
      buildTrainingRunRecord({
        makeId: () => 'a2-thermal',
        nowIso: '2026-06-28T00:00:00.000Z',
        request: {
          promiseRef: 'training.device_capability_dataset.v1',
          trainingRunRef: 'run.cs336.a2.device_capability.thermal',
        },
      }),
    )

    const admitted = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          evidencePath,
          { measurements: [measurement] },
          { headers: { authorization: 'Bearer admin-token-test' } },
        ),
        {},
      ),
    )
    const admittedBody = (await admitted.json()) as Readonly<{
      dataset: TrainingDeviceCapabilityJson
    }>

    expect(admitted.status).toBe(200)
    expect(admittedBody.dataset.thermalThrottleDetectionStatus).toBe(
      'thermal_throttle_observed',
    )
    expect(admittedBody.dataset.thermalThrottleFunnelReasonCodes).toEqual([
      'device_capability.public.thermal_throttle_observed_sustained_ratio_below_floor',
    ])
    expect(admittedBody.dataset.thermalThrottleReceiptRefs).toEqual([
      'receipt.cs336_a2.thermal.owner_accepted.production.1',
    ])

    const dashboard = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/device-capabilities/a2',
        ),
        {},
      ),
    )
    const dashboardBody =
      (await dashboard.json()) as TrainingDeviceCapabilityJson

    expect(dashboardBody.thermalThrottleFunnelReasonCodes).toEqual([
      'device_capability.public.thermal_throttle_observed_sustained_ratio_below_floor',
    ])
    expect(dashboardBody.thermalThrottleReceiptRefs).toEqual([
      'receipt.cs336_a2.thermal.owner_accepted.production.1',
    ])
  })

  it('plans, activates, seals, reconciles, reads, and claims training windows', async () => {
    const store = makeMemoryStore()
    let counter = 0
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => String(++counter).padStart(4, '0'),
      makeStore: () => store,
      nowIso: () => '2026-06-10T10:00:00.000Z',
      requireAdminApiToken: async () => true,
    })

    const plannedRun = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs', {
          promiseRef: 'promise.training.4673',
          sourceRefs: ['issue.github.openagents.4673'],
          trainingRunRef: 'training.run.4673',
        }),
        {},
      ),
    )
    expect(plannedRun.status).toBe(200)

    const plannedWindow = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/windows/plan', {
          datasetRefs: ['dataset.cs336.homework.1'],
          homeworkKind: 'admin_dispatched_homework',
          trainingRunRef: 'training.run.4673',
          windowRef: 'training.window.4673',
        }),
        {},
      ),
    )
    expect(plannedWindow.status).toBe(200)

    const activated = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/windows/training.window.4673/activate', {
          receiptRef: 'receipt.training.activate',
        }),
        {},
      ),
    )
    expect(activated.status).toBe(200)

    const sealMetadata = {
      churn: {
        events: [{ eventRef: 'event.churn.loss.pylon.device2', kind: 'loss' }],
        joinCount: 0,
        lossCount: 1,
        standbyPromotionCount: 0,
      },
      staleness: {
        contributionCount: 2,
        contributions: [
          {
            contributionRef: 'contribution.window.4673.pylon.device1',
            stepsBehind: 0,
          },
          {
            contributionRef: 'contribution.window.4673.pylon.device2',
            stepsBehind: 3,
          },
        ],
        stepsBehindMax: 3,
        stepsBehindMin: 0,
        stepsBehindP50: 1.5,
        stepsBehindP90: 3,
      },
      verificationOverhead: {
        fraction: 0.22,
        ladderRungRef: 'ladder.rung.r1',
      },
    }
    const rejectedSeal = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/windows/training.window.4673/seal', {
          receiptRef: 'receipt.training.seal',
          sealMetadata: {
            ...sealMetadata,
            verificationOverhead: {
              fraction: 1.5,
              ladderRungRef: 'ladder.rung.r1',
            },
          },
        }),
        {},
      ),
    )
    expect(rejectedSeal.status).toBe(400)

    const sealed = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/windows/training.window.4673/seal', {
          receiptRef: 'receipt.training.seal',
          sealMetadata,
        }),
        {},
      ),
    )
    expect(sealed.status).toBe(200)
    await expect(sealed.json()).resolves.toMatchObject({
      window: { sealMetadata, state: 'sealed' },
    })

    const reconciledWindow = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/windows/training.window.4673/reconcile', {
          receiptRef: 'receipt.training.reconcile',
        }),
        {},
      ),
    )
    expect(reconciledWindow.status).toBe(200)

    const readRun = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/runs/training.run.4673',
        ),
        {},
      ),
    )
    expect(readRun.status).toBe(200)
    await expect(readRun.json()).resolves.toMatchObject({
      run: { state: 'planned', trainingRunRef: 'training.run.4673' },
    })

    const readWindow = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/windows/training.window.4673',
        ),
        {},
      ),
    )
    expect(readWindow.status).toBe(200)
    await expect(readWindow.json()).resolves.toMatchObject({
      window: {
        sealMetadata,
        state: 'reconciled',
        windowRef: 'training.window.4673',
      },
    })

    const leaseResponse = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/leases/claim', {
          pylonRef: 'pylon.training.1',
          receiptRefs: ['receipt.training.lease'],
        }),
        {},
      ),
    )
    expect(leaseResponse.status).toBe(404)
  })

  it('claims the admin-dispatched active window before the starter window', async () => {
    const store = makeMemoryStore()
    let counter = 0
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => String(++counter).padStart(4, '0'),
      makeStore: () => store,
      nowIso: () => '2026-06-10T10:00:00.000Z',
      requireAdminApiToken: async () => true,
    })

    await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs', {
          promiseRef: 'promise.training.4673',
          trainingRunRef: 'training.run.4673',
        }),
        {},
      ),
    )

    for (const [windowRef, homeworkKind] of [
      ['training.window.starter', 'auto_starter'],
      ['training.window.admin', 'admin_dispatched_homework'],
    ] as const) {
      await runRoute(
        routes.routeTrainingRunWindowRequest(
          jsonRequest('/api/training/windows/plan', {
            homeworkKind,
            trainingRunRef: 'training.run.4673',
            windowRef,
          }),
          {},
        ),
      )
      await runRoute(
        routes.routeTrainingRunWindowRequest(
          jsonRequest(`/api/training/windows/${windowRef}/activate`, {
            receiptRef: `receipt.${windowRef}.activate`,
          }),
          {},
        ),
      )
    }

    const leaseResponse = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/leases/claim', {
          pylonRef: 'pylon.training.1',
        }),
        {},
      ),
    )
    const body = await leaseResponse.json()

    expect(leaseResponse.status).toBe(200)
    expect(body).toMatchObject({
      lease: { windowRef: 'training.window.admin' },
    })
  })

  it('validates the seal publication cadence as run-authority config', async () => {
    const store = makeMemoryStore()
    let counter = 0
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => String(++counter).padStart(4, '0'),
      makeStore: () => store,
      nowIso: () => '2026-06-12T10:00:00.000Z',
      requireAdminApiToken: async () => true,
    })

    const rejected = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs', {
          promiseRef: 'promise.training.4850',
          sealPublicationCadenceWindows: 0,
          trainingRunRef: 'training.run.4850',
        }),
        {},
      ),
    )
    expect(rejected.status).toBe(400)

    const planned = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs', {
          promiseRef: 'promise.training.4850',
          sealPublicationCadenceWindows: 3,
          trainingRunRef: 'training.run.4850',
        }),
        {},
      ),
    )
    expect(planned.status).toBe(200)
    await expect(planned.json()).resolves.toMatchObject({
      run: {
        sealInFlight: false,
        sealPublicationCadenceWindows: 3,
        trainingRunRef: 'training.run.4850',
      },
    })

    const defaulted = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs', {
          promiseRef: 'promise.training.4850',
          trainingRunRef: 'training.run.4850.defaulted',
        }),
        {},
      ),
    )
    expect(defaulted.status).toBe(200)
    await expect(defaulted.json()).resolves.toMatchObject({
      run: { sealPublicationCadenceWindows: 1 },
    })
  })

  it('grants bootstrap from the last durable seal, queues during an in-flight seal, and refuses without one', async () => {
    const store = makeMemoryStore()
    let counter = 0
    let currentIso = '2026-06-12T10:00:00.000Z'
    const barrierCalls: Array<string> = []
    const instrumentedStore: MemoryTrainingAuthorityStore = {
      ...store,
      beginRunSealBarrier: async (trainingRunRef, nowIso) => {
        barrierCalls.push(`begin:${trainingRunRef}`)
        await store.beginRunSealBarrier(trainingRunRef, nowIso)
      },
      clearRunSealBarrier: async trainingRunRef => {
        barrierCalls.push(`clear:${trainingRunRef}`)
        await store.clearRunSealBarrier(trainingRunRef)
      },
    }
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => String(++counter).padStart(4, '0'),
      makeStore: () => instrumentedStore,
      nowIso: () => currentIso,
      requireAdminApiToken: async () => true,
    })
    const sealMetadataWithDigest = (
      checkpointDigestRef: string,
      windowRef: string,
    ) => ({
      checkpointDigestRef,
      churn: { joinCount: 0, lossCount: 0, standbyPromotionCount: 0 },
      durableCheckpointSeal: {
        checkpointDigestRef,
        replicationFactor: 2,
        retrievalProofRef: `receipt.${windowRef}.checkpoint.readback`,
        retrievalVerified: true,
        sizeBytes: 1_048_576,
        storageClass: 'content_addressed_object_store',
        windowRef,
      },
      staleness: {
        contributionCount: 0,
        stepsBehindMax: 0,
        stepsBehindMin: 0,
        stepsBehindP50: 0,
        stepsBehindP90: 0,
      },
      verificationOverhead: {
        fraction: 0.2,
        ladderRungRef: 'ladder.rung.r1',
      },
    })
    const sealWindow = async (windowRef: string, digestRef: string) => {
      await runRoute(
        routes.routeTrainingRunWindowRequest(
          jsonRequest('/api/training/windows/plan', {
            trainingRunRef: 'training.run.4850',
            windowRef,
          }),
          {},
        ),
      )
      await runRoute(
        routes.routeTrainingRunWindowRequest(
          jsonRequest(`/api/training/windows/${windowRef}/activate`, {
            receiptRef: `receipt.${windowRef}.activate`,
          }),
          {},
        ),
      )
      const sealed = await runRoute(
        routes.routeTrainingRunWindowRequest(
          jsonRequest(`/api/training/windows/${windowRef}/seal`, {
            receiptRef: `receipt.${windowRef}.seal`,
            sealMetadata: sealMetadataWithDigest(digestRef, windowRef),
          }),
          {},
        ),
      )
      expect(sealed.status).toBe(200)
    }
    const requestGrant = async (): Promise<Record<string, unknown>> => {
      const response = await runRoute(
        routes.routeTrainingRunWindowRequest(
          jsonRequest('/api/training/runs/training.run.4850/bootstrap-grant', {
            joinerRef: 'pylon.joiner.1',
            receiptRefs: ['receipt.joiner.qualification'],
          }),
          {},
        ),
      )
      expect(response.status).toBe(200)

      return (await response.json()) as Record<string, unknown>
    }

    await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs', {
          promiseRef: 'promise.training.4850',
          trainingRunRef: 'training.run.4850',
        }),
        {},
      ),
    )

    // No durable seal yet: typed refusal, not an error.
    expect(await requestGrant()).toMatchObject({
      outcome: {
        kind: 'refused',
        reasonCode: 'training.bootstrap.public.no_durable_seal',
      },
    })

    // First durable seal: grant pins its checkpoint digest. The seal
    // route raised and cleared the run-level barrier around the write.
    const firstDigestRef = `sha256:${'a'.repeat(64)}`
    await sealWindow('training.window.4850.a', firstDigestRef)
    expect(barrierCalls).toEqual([
      'begin:training.run.4850',
      'clear:training.run.4850',
    ])
    expect(await requestGrant()).toMatchObject({
      outcome: {
        grant: {
          checkpointDigestRef: firstDigestRef,
          joinerReceiptRefs: ['receipt.joiner.qualification'],
          joinerRef: 'pylon.joiner.1',
          sealReceiptRefs: [
            'receipt.training.window.4850.a.activate',
            'receipt.training.window.4850.a.seal',
          ],
          sealedWindowRef: 'training.window.4850.a',
          trainingRunRef: 'training.run.4850',
        },
        kind: 'granted',
      },
    })

    // Seal in flight: the same request queues with the join-lifecycle
    // deferral reason code instead of being rejected.
    await store.beginRunSealBarrier('training.run.4850', currentIso)
    expect(await requestGrant()).toMatchObject({
      outcome: {
        joinerRef: 'pylon.joiner.1',
        kind: 'queued',
        reasonCode: 'join_lifecycle.public.join_deferred_seal_in_flight',
      },
    })

    // Barrier clears and a newer window seals: the replayed request
    // proceeds against the NEW last durable seal.
    await store.clearRunSealBarrier('training.run.4850')
    currentIso = '2026-06-12T11:00:00.000Z'
    const secondDigestRef = `sha256:${'b'.repeat(64)}`
    await sealWindow('training.window.4850.b', secondDigestRef)
    expect(await requestGrant()).toMatchObject({
      outcome: {
        grant: {
          checkpointDigestRef: secondDigestRef,
          sealedWindowRef: 'training.window.4850.b',
        },
        kind: 'granted',
      },
    })
  })

  it('admin preflights standby dispatch without mutating or promoting a standby', async () => {
    const store = makeMemoryStore()
    const routes = makeTrainingRunWindowRoutes({
      makeStore: () => store,
      nowIso: () => '2026-06-20T10:00:00.000Z',
      requireAdminApiToken: async request =>
        request.headers.get('authorization') === 'Bearer admin-token-test',
    })
    const adminHeaders = { authorization: 'Bearer admin-token-test' }
    const run = buildTrainingRunRecord({
      makeId: () => 'standby-run',
      nowIso: '2026-06-20T09:55:00.000Z',
      request: {
        promiseRef: 'training.marathon_operations.v1',
        trainingRunRef: 'training.run.standby.preflight',
      },
    })
    const standbyDispatch = {
      bannedForRound: false,
      bootstrapSealVerified: true,
      bootstrapSealWindowRef: 'training.window.standby.0007',
      lastHeartbeatAgeMs: 5_000,
      liveSealedWindowRef: 'training.window.standby.0007',
      liveVacancyCount: 1,
      qualified: true,
      runRef: 'training.run.standby.preflight',
      standbyContributorRef: 'pylon.standby.0003',
    }
    const path =
      '/api/training/runs/training.run.standby.preflight/standby-dispatch-preflight'

    store._testSeedRun(run)

    const unauthorized = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(path, standbyDispatch),
        {},
      ),
    )

    expect(unauthorized.status).toBe(401)

    const malformed = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          path,
          { standbyContributorRef: 'pylon.standby.0003' },
          { headers: adminHeaders },
        ),
        {},
      ),
    )
    const malformedBody = (await malformed.json()) as Readonly<{
      standbyDispatch: Readonly<{
        decision: string
        promotable: boolean
        reasons: ReadonlyArray<string>
      }>
    }>

    expect(malformed.status).toBe(200)
    expect(malformedBody.standbyDispatch).toMatchObject({
      decision: 'hold_standby',
      promotable: false,
      reasons: ['dispatch_descriptor_malformed'],
    })

    const mismatchedRun = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          path,
          { ...standbyDispatch, runRef: 'training.run.other' },
          { headers: adminHeaders },
        ),
        {},
      ),
    )
    const mismatchedBody = (await mismatchedRun.json()) as Readonly<{
      standbyDispatch: Readonly<{
        decision: string
        promotable: boolean
        reasons: ReadonlyArray<string>
      }>
    }>

    expect(mismatchedRun.status).toBe(200)
    expect(mismatchedBody.standbyDispatch).toMatchObject({
      decision: 'hold_standby',
      promotable: false,
      reasons: ['dispatch_descriptor_malformed'],
    })

    const preflight = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(path, standbyDispatch, { headers: adminHeaders }),
        {},
      ),
    )
    const preflightBody = (await preflight.json()) as Readonly<{
      run: Readonly<{ trainingRunRef: string }>
      standbyDispatch: Readonly<{
        decision: string
        promotable: boolean
        reasons: ReadonlyArray<string>
      }>
    }>

    expect(preflight.status).toBe(200)
    expect(preflightBody.run.trainingRunRef).toBe(
      'training.run.standby.preflight',
    )
    expect(preflightBody.standbyDispatch).toMatchObject({
      decision: 'promote_standby',
      promotable: true,
      reasons: [],
    })
    expect(await store.readRun('training.run.standby.preflight')).toEqual(run)
  })

  it('admin preflights a curtailment drill without mutating or curtailing the run', async () => {
    const store = makeMemoryStore()
    const routes = makeTrainingRunWindowRoutes({
      makeStore: () => store,
      nowIso: () => '2026-06-20T10:00:00.000Z',
      requireAdminApiToken: async request =>
        request.headers.get('authorization') === 'Bearer admin-token-test',
    })
    const adminHeaders = { authorization: 'Bearer admin-token-test' }
    const run = buildTrainingRunRecord({
      makeId: () => 'curtailment-run',
      nowIso: '2026-06-20T09:55:00.000Z',
      request: {
        promiseRef: 'training.marathon_operations.v1',
        trainingRunRef: 'training.run.curtailment.preflight',
      },
    })
    const drill = {
      ackLatencyMs: 4_000,
      drillRef: 'training.drill.curtailment.0001',
      durableCheckpointSealed: true,
      haltCompleted: true,
      haltLatencyMs: 120_000,
      resumeVerified: true,
      runRef: 'training.run.curtailment.preflight',
      scheduled: true,
      signalAcknowledged: true,
    }
    const path =
      '/api/training/runs/training.run.curtailment.preflight/curtailment-drill-preflight'

    store._testSeedRun(run)

    const unauthorized = await runRoute(
      routes.routeTrainingRunWindowRequest(jsonRequest(path, drill), {}),
    )

    expect(unauthorized.status).toBe(401)

    const malformed = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          path,
          { drillRef: 'training.drill.curtailment.0001' },
          { headers: adminHeaders },
        ),
        {},
      ),
    )
    const malformedBody = (await malformed.json()) as Readonly<{
      curtailmentDrill: Readonly<{
        decision: string
        passed: boolean
        reasons: ReadonlyArray<string>
      }>
    }>

    expect(malformed.status).toBe(200)
    expect(malformedBody.curtailmentDrill).toMatchObject({
      decision: 'drill_incomplete',
      passed: false,
      reasons: ['drill_descriptor_malformed'],
    })

    const mismatchedRun = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          path,
          { ...drill, runRef: 'training.run.other' },
          { headers: adminHeaders },
        ),
        {},
      ),
    )
    const mismatchedBody = (await mismatchedRun.json()) as Readonly<{
      curtailmentDrill: Readonly<{
        decision: string
        passed: boolean
        reasons: ReadonlyArray<string>
      }>
    }>

    expect(mismatchedRun.status).toBe(200)
    expect(mismatchedBody.curtailmentDrill).toMatchObject({
      decision: 'drill_incomplete',
      passed: false,
      reasons: ['drill_descriptor_malformed'],
    })

    const incomplete = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          path,
          { ...drill, resumeVerified: false },
          { headers: adminHeaders },
        ),
        {},
      ),
    )
    const incompleteBody = (await incomplete.json()) as Readonly<{
      curtailmentDrill: Readonly<{
        decision: string
        passed: boolean
        reasons: ReadonlyArray<string>
      }>
    }>

    expect(incomplete.status).toBe(200)
    expect(incompleteBody.curtailmentDrill).toMatchObject({
      decision: 'drill_incomplete',
      passed: false,
      reasons: ['resume_not_verified'],
    })

    const preflight = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(path, drill, { headers: adminHeaders }),
        {},
      ),
    )
    const preflightBody = (await preflight.json()) as Readonly<{
      curtailmentDrill: Readonly<{
        decision: string
        passed: boolean
        reasons: ReadonlyArray<string>
      }>
      run: Readonly<{ trainingRunRef: string }>
    }>

    expect(preflight.status).toBe(200)
    expect(preflightBody.run.trainingRunRef).toBe(
      'training.run.curtailment.preflight',
    )
    expect(preflightBody.curtailmentDrill).toMatchObject({
      decision: 'drill_passed',
      passed: true,
      reasons: [],
    })
    expect(await store.readRun('training.run.curtailment.preflight')).toEqual(
      run,
    )
  })
})

// REAL Bitcoin settlement route wiring (openagents #5232, Gate 2).
//
// These tests exercise routeRunSettlementReceipt end to end through the owner
// gate. The gate env value is OPENAGENTS_REAL_SETTLEMENT_GATE; absent/unset =>
// simulation (byte-for-byte today). The real branch is reachable only when the
// gate is enabled AND the recipient/run are allowlisted AND the amount is under
// the cap; it dispatches through a mocked payment authority whose Spark-like
// adapter counts dispatches.
describe('real settlement route wiring (#5232, gate default OFF)', () => {
  const REAL_RUN_REF = 'run.tassadar.executor.20260615'
  const REAL_CONTRIBUTOR = 'pylon.contributor.stranger'

  // A full in-memory ledger with idempotency reads, so the authority dedupe
  // (no-double-pay) path is real, not stubbed.
  const makeRealLedgerStore = (): NexusTreasuryPayoutLedgerStore & {
    readonly receipts: Map<string, NexusPaymentAuthorityReceiptRecord>
  } => {
    const intents = new Map<string, NexusTreasuryPayoutIntentRecord>()
    const intentsByIdem = new Map<string, NexusTreasuryPayoutIntentRecord>()
    const attempts = new Map<string, NexusTreasuryPayoutAttemptRecord>()
    const attemptsByIdem = new Map<string, NexusTreasuryPayoutAttemptRecord>()
    const events = new Map<
      string,
      NexusTreasuryPayoutReconciliationEventRecord
    >()
    const receipts = new Map<string, NexusPaymentAuthorityReceiptRecord>()
    const notImplemented = async (): Promise<never> => {
      throw new Error('not implemented in real ledger store')
    }

    return {
      createPaymentAuthorityReceipt: async record => {
        receipts.set(record.receiptRef, record)
      },
      createPayoutAttempt: async record => {
        attempts.set(record.payoutAttemptRef, record)
        attemptsByIdem.set(record.idempotencyKeyHash, record)
      },
      createPayoutIntent: async record => {
        intents.set(record.payoutIntentRef, record)
        intentsByIdem.set(record.idempotencyKeyHash, record)
      },
      createPayoutTargetApproval: async () => {},
      createReconciliationEvent: async record => {
        events.set(record.eventRef, record)
      },
      createReleaseGate: async () => {},
      listPaymentAuthorityReceipts: async () => [...receipts.values()],
      readPaymentAuthorityReceiptByRef: async receiptRef =>
        receipts.get(receiptRef),
      readPayoutAttemptByIdempotencyKeyHash: async idempotencyKeyHash =>
        attemptsByIdem.get(idempotencyKeyHash),
      readPayoutAttemptByRef: async payoutAttemptRef =>
        attempts.get(payoutAttemptRef),
      readPayoutIntentByBuyerPaymentRef: notImplemented,
      readPayoutIntentByIdempotencyKeyHash: async idempotencyKeyHash =>
        intentsByIdem.get(idempotencyKeyHash),
      readPayoutIntentByRef: async payoutIntentRef =>
        intents.get(payoutIntentRef),
      readReconciliationEventByRef: async eventRef => events.get(eventRef),
      receipts,
    }
  }

  // A Spark-like adapter that counts dispatches and stamps the real_bitcoin /
  // matched projection, exactly as the production Spark adapter does.
  class CountingSparkAdapter {
    dispatchCalls = 0
    failDispatch = false

    adapter: TreasuryPaymentAuthorityAdapter = {
      adapterKind: 'spark_treasury',
      dispatch: input =>
        Effect.suspend(() => {
          this.dispatchCalls += 1

          if (this.failDispatch) {
            return Effect.fail(
              new TreasuryPaymentAuthorityError({
                message: 'spark_treasury_pay_failed',
                reason: 'adapter_unavailable',
              }),
            )
          }

          return Effect.succeed({
            ...input.attempt,
            adapterKind: 'spark_treasury' as const,
            publicProjectionJson: JSON.stringify({
              adapter: 'spark_treasury',
              moneyMovement: 'real_bitcoin',
              rawMaterialStored: false,
              state: 'dispatch_reported',
            }),
            redactedPaymentRef: 'payment.redacted.spark_treasury.test',
            status: 'dispatched' as const,
          })
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
      reconcile: input =>
        Effect.succeed({
          ...input.event,
          adapterKind: 'spark_treasury',
          publicProjectionJson: JSON.stringify({
            adapter: 'spark_treasury',
            moneyMovement: 'real_bitcoin',
            state: 'reconciliation_matched',
          }),
          status: 'matched' as const,
        }),
    }
  }

  const seedRealSettlementStore = () => {
    const store = makeMemoryStore()
    const planned = buildTrainingRunRecord({
      makeId: () => 'run5232',
      nowIso: '2026-06-17T10:00:00.000Z',
      request: {
        manifest: {
          artifactDigestRefs: [],
          blockerRefs: [],
          spendCapSats: 100,
        },
        promiseRef: 'training.decentralized_training_launch.v1',
        trainingRunRef: REAL_RUN_REF,
      },
    })
    store._testSeedRun({ ...planned, state: 'active' })

    const lease: TrainingWindowLeaseRecord = {
      claimedAt: '2026-06-17T10:01:00.000Z',
      id: 'lease5232',
      leaseExpiresAt: '2026-06-17T12:00:00.000Z',
      leaseRef: 'lease.tassadar.5232',
      publicProjectionJson: '{}',
      pylonRef: REAL_CONTRIBUTOR,
      receiptRefs: [],
      state: 'active',
      trainingRunRef: REAL_RUN_REF,
      windowRef: 'training.window.tassadar.executor.20260615.w1',
    }

    const verified = finalizeTrainingVerificationChallengeRecord({
      challenge: leaseTrainingVerificationChallengeRecord({
        challenge: buildTrainingVerificationChallengeRecord({
          makeId: () => '5232',
          nowIso: '2026-06-17T10:02:00.000Z',
          request: {
            commitmentRefs: ['commitment.tassadar.5232'],
            contributionRef: 'contribution.tassadar.5232',
            homeworkKind: 'admin_dispatched_homework',
            payload: {
              replayDigestRef: 'digest.replay.5232',
              traceCommitmentDigestRef: 'digest.commitment.5232',
            },
            trainingRunRef: REAL_RUN_REF,
            verificationClass: 'exact_trace_replay',
            windowRef: 'training.window.tassadar.executor.20260615.w1',
          },
        }).challenge,
        eventId: '5232-lease',
        nowIso: '2026-06-17T10:02:30.000Z',
        request: { validatorRef: 'validator.tassadar.5232' },
      }).challenge,
      eventId: '5232-final',
      nowIso: '2026-06-17T10:03:00.000Z',
      request: { receiptRefs: ['receipt.tassadar.verdict.5232'] },
      verdict: {
        failureCodes: [],
        state: 'Verified',
        verdictRefs: ['verdict.tassadar.5232'],
      },
    }).challenge

    return { lease, store, verified }
  }

  const realSettlementBody = (
    challengeRef: string,
    overrides: Record<string, unknown> = {},
  ) => ({
    adapterKind: 'spark_treasury',
    amountSats: 21,
    challengeRef,
    idempotencyRef: 'idem.tassadar.5232',
    leaseRef: 'lease.tassadar.5232',
    operatorApprovalRef: 'operator.approval.5232',
    payoutTargetApprovalRef: 'payout.target.approval.5232',
    payoutTargetRef: 'payout.target.5232',
    ...overrides,
  })

  const enabledGateEnv = (overrides: Record<string, unknown> = {}) => ({
    OPENAGENTS_REAL_SETTLEMENT_GATE: JSON.stringify({
      enabled: true,
      allowedAdapterKind: 'spark_treasury',
      allowedContributorRefs: [REAL_CONTRIBUTOR],
      allowedRunRefs: [REAL_RUN_REF],
      maxPayoutSats: 50,
      ...overrides,
    }),
  })

  const settlementReceiptFromStore = (
    ledger: ReturnType<typeof makeRealLedgerStore>,
  ): NexusPaymentAuthorityReceiptRecord | undefined =>
    [...ledger.receipts.values()].find(
      receipt => receipt.receiptKind === 'settlement_recorded',
    )

  it('gate OFF (default) => simulation, no payout dispatch, realBitcoinMoved:false', async () => {
    const { lease, store, verified } = seedRealSettlementStore()
    await store.claimLease(lease, '2026-06-17T10:01:00.000Z')
    store._testSeedChallenge(verified)
    const ledger = makeRealLedgerStore()
    const spark = new CountingSparkAdapter()

    const routes = makeTrainingRunWindowRoutes({
      makePayoutLedgerStore: () => ledger,
      makeSettlementPaymentAuthority: (_env, context) =>
        makeTreasuryPaymentAuthority({
          adapters: [spark.adapter],
          ledgerStore: context.ledgerStore,
        }),
      makeStore: () => store,
      nowIso: () => '2026-06-17T10:05:00.000Z',
      readSettlementWalletReadiness: async () => 'ready',
      requireAdminApiToken: async () => true,
      resolveSettlementPayoutDestination: async () => 'destination.test',
    })

    // No gate env set anywhere => byte-for-byte simulation, even though the
    // admin explicitly requested spark_treasury.
    const settled = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          `/api/training/runs/${REAL_RUN_REF}/settlement-receipt`,
          realSettlementBody(verified.challengeRef),
        ),
        {},
      ),
    )

    expect(settled.status).toBe(200)
    expect(spark.dispatchCalls).toBe(0)
    const receipt = settlementReceiptFromStore(ledger)
    expect(receipt).toBeDefined()
    const projection = JSON.parse(receipt!.publicProjectionJson) as {
      moneyMovement: string
    }
    expect(projection.moneyMovement).toBe('none')
  })

  it('gate ON + allowlisted + under cap => exactly one dispatch, realBitcoinMoved:true receipt', async () => {
    const { lease, store, verified } = seedRealSettlementStore()
    await store.claimLease(lease, '2026-06-17T10:01:00.000Z')
    store._testSeedChallenge(verified)
    const ledger = makeRealLedgerStore()
    const spark = new CountingSparkAdapter()

    const routes = makeTrainingRunWindowRoutes({
      makePayoutLedgerStore: () => ledger,
      makeSettlementPaymentAuthority: (_env, context) =>
        makeTreasuryPaymentAuthority({
          adapters: [spark.adapter],
          ledgerStore: context.ledgerStore,
        }),
      makeStore: () => store,
      nowIso: () => '2026-06-17T10:05:00.000Z',
      readSettlementWalletReadiness: async () => 'ready',
      requireAdminApiToken: async () => true,
      resolveSettlementPayoutDestination: async () => 'destination.test',
    })

    const settled = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          `/api/training/runs/${REAL_RUN_REF}/settlement-receipt`,
          realSettlementBody(verified.challengeRef),
        ),
        enabledGateEnv(),
      ),
    )

    expect(settled.status).toBe(200)
    expect(spark.dispatchCalls).toBe(1)
    const receipt = settlementReceiptFromStore(ledger)
    expect(receipt).toBeDefined()
    const projection = JSON.parse(receipt!.publicProjectionJson) as {
      moneyMovement: string
      state: string
    }
    expect(projection.moneyMovement).toBe('real_bitcoin')
    expect(projection.state).toBe('settled')

    const detail = nexusPylonPublicReceiptDetailFromLedger({
      appUrl: 'https://openagents.com',
      attempt: await ledger.readPayoutAttemptByRef(receipt!.payoutAttemptRef!),
      event:
        receipt!.eventRef === null
          ? undefined
          : await ledger.readReconciliationEventByRef(receipt!.eventRef),
      intent: await ledger.readPayoutIntentByRef(receipt!.payoutIntentRef),
      nowIso: '2026-06-17T10:05:00.000Z',
      receipt: receipt!,
    })
    expect(detail.realBitcoinMoved).toBe(true)
  })

  it('retry of the same run-window+recipient settlement dispatches at most once (idempotent)', async () => {
    const { lease, store, verified } = seedRealSettlementStore()
    await store.claimLease(lease, '2026-06-17T10:01:00.000Z')
    store._testSeedChallenge(verified)
    const ledger = makeRealLedgerStore()
    const spark = new CountingSparkAdapter()

    const routes = makeTrainingRunWindowRoutes({
      makePayoutLedgerStore: () => ledger,
      makeSettlementPaymentAuthority: (_env, context) =>
        makeTreasuryPaymentAuthority({
          adapters: [spark.adapter],
          ledgerStore: context.ledgerStore,
        }),
      makeStore: () => store,
      nowIso: () => '2026-06-17T10:05:00.000Z',
      readSettlementWalletReadiness: async () => 'ready',
      requireAdminApiToken: async () => true,
      resolveSettlementPayoutDestination: async () => 'destination.test',
    })

    const first = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          `/api/training/runs/${REAL_RUN_REF}/settlement-receipt`,
          realSettlementBody(verified.challengeRef),
        ),
        enabledGateEnv(),
      ),
    )
    const second = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          `/api/training/runs/${REAL_RUN_REF}/settlement-receipt`,
          realSettlementBody(verified.challengeRef),
        ),
        enabledGateEnv(),
      ),
    )

    expect(first.status).toBe(200)
    expect(second.status).toBe(200)
    // Receipt-first idempotent short-circuit + authority dedupe => one dispatch.
    expect(spark.dispatchCalls).toBe(1)
    expect(ledger.receipts.size).toBe(1)
  })

  it('payout FAILS => no real-settled receipt, typed error', async () => {
    const { lease, store, verified } = seedRealSettlementStore()
    await store.claimLease(lease, '2026-06-17T10:01:00.000Z')
    store._testSeedChallenge(verified)
    const ledger = makeRealLedgerStore()
    const spark = new CountingSparkAdapter()
    spark.failDispatch = true

    const routes = makeTrainingRunWindowRoutes({
      makePayoutLedgerStore: () => ledger,
      makeSettlementPaymentAuthority: (_env, context) =>
        makeTreasuryPaymentAuthority({
          adapters: [spark.adapter],
          ledgerStore: context.ledgerStore,
        }),
      makeStore: () => store,
      nowIso: () => '2026-06-17T10:05:00.000Z',
      readSettlementWalletReadiness: async () => 'ready',
      requireAdminApiToken: async () => true,
      resolveSettlementPayoutDestination: async () => 'destination.test',
    })

    const failed = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          `/api/training/runs/${REAL_RUN_REF}/settlement-receipt`,
          realSettlementBody(verified.challengeRef),
        ),
        enabledGateEnv(),
      ),
    )

    expect(failed.status).toBeGreaterThanOrEqual(400)
    expect(spark.dispatchCalls).toBe(1)
    // No real-settled receipt was written (a policy_rejected operator receipt
    // may exist, but never a settlement_recorded one).
    expect(settlementReceiptFromStore(ledger)).toBeUndefined()
    const body = (await failed.json()) as { reason?: string }
    expect(body.reason).toContain('real_settlement_payout_blocked')
  })

  it('intent not durably persisted => fails CLOSED, no dispatch, no real-settled receipt (#5232)', async () => {
    const { lease, store, verified } = seedRealSettlementStore()
    await store.claimLease(lease, '2026-06-17T10:01:00.000Z')
    store._testSeedChallenge(verified)
    const spark = new CountingSparkAdapter()

    // Reproduce the production incident: a store whose createPayoutIntent
    // silently drops the row (as `INSERT OR IGNORE` did on a constraint
    // conflict) so the intent is never findable by ref. The fix makes the D1
    // store throw on this, but here we assert the END-TO-END contract: when the
    // intent is not durably persisted, the real dispatch must fail CLOSED —
    // never call the Spark adapter and never write a settlement_recorded
    // receipt. No money, no "paid" claim.
    const baseLedger = makeRealLedgerStore()
    const ledger: NexusTreasuryPayoutLedgerStore & {
      readonly receipts: Map<string, NexusPaymentAuthorityReceiptRecord>
    } = {
      ...baseLedger,
      createPayoutIntent: async () => {},
    }

    const routes = makeTrainingRunWindowRoutes({
      makePayoutLedgerStore: () => ledger,
      makeSettlementPaymentAuthority: (_env, context) =>
        makeTreasuryPaymentAuthority({
          adapters: [spark.adapter],
          ledgerStore: context.ledgerStore,
        }),
      makeStore: () => store,
      nowIso: () => '2026-06-17T10:05:00.000Z',
      readSettlementWalletReadiness: async () => 'ready',
      requireAdminApiToken: async () => true,
      resolveSettlementPayoutDestination: async () => 'destination.test',
    })

    const failed = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          `/api/training/runs/${REAL_RUN_REF}/settlement-receipt`,
          realSettlementBody(verified.challengeRef),
        ),
        enabledGateEnv(),
      ),
    )

    expect(failed.status).toBeGreaterThanOrEqual(400)
    // No dispatch happened: the missing intent fails closed before the rail.
    expect(spark.dispatchCalls).toBe(0)
    expect(settlementReceiptFromStore(ledger)).toBeUndefined()
    const body = (await failed.json()) as { reason?: string }
    expect(body.reason).toContain('payout_intent_not_found')
  })

  it('gate ON but amount over the gate cap => simulation, no dispatch', async () => {
    const { lease, store, verified } = seedRealSettlementStore()
    await store.claimLease(lease, '2026-06-17T10:01:00.000Z')
    store._testSeedChallenge(verified)
    const ledger = makeRealLedgerStore()
    const spark = new CountingSparkAdapter()

    const routes = makeTrainingRunWindowRoutes({
      makePayoutLedgerStore: () => ledger,
      makeSettlementPaymentAuthority: (_env, context) =>
        makeTreasuryPaymentAuthority({
          adapters: [spark.adapter],
          ledgerStore: context.ledgerStore,
        }),
      makeStore: () => store,
      nowIso: () => '2026-06-17T10:05:00.000Z',
      readSettlementWalletReadiness: async () => 'ready',
      requireAdminApiToken: async () => true,
      resolveSettlementPayoutDestination: async () => 'destination.test',
    })

    // amountSats 60 > gate cap 50 (still under the run spendCap 100), so the
    // gate fails closed to simulation.
    const settled = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest(
          `/api/training/runs/${REAL_RUN_REF}/settlement-receipt`,
          realSettlementBody(verified.challengeRef, { amountSats: 60 }),
        ),
        enabledGateEnv(),
      ),
    )

    expect(settled.status).toBe(200)
    expect(spark.dispatchCalls).toBe(0)
    const projection = JSON.parse(
      settlementReceiptFromStore(ledger)!.publicProjectionJson,
    ) as { moneyMovement: string }
    expect(projection.moneyMovement).toBe('none')
  })
})
