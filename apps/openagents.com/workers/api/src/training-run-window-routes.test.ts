import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  buildTrainingRunRecord,
  type TrainingAuthorityStore,
  type TrainingRunPublicSummary,
  type TrainingRunRecord,
  type TrainingWindowEventRecord,
  type TrainingWindowLeaseRecord,
  type TrainingWindowRecord,
} from './training-run-window-authority'
import { makeTrainingRunWindowRoutes } from './training-run-window-routes'
import {
  type TrainingVerificationChallengeRecord,
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
} from './training-verification'

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
  summary: TrainingRunPublicSummary
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
    claimLease: async lease => {
      leases.set(lease.leaseRef, lease)

      return lease
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
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => String(++counter).padStart(4, '0'),
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
      'pending, offered, claimed, or wallet-side records are excluded',
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
      blockerRefs: [],
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
    expect(
      leaderboards.lanes.find(lane => lane.lane === 'a2_throughput')?.rows,
    ).toEqual([
      expect.objectContaining({
        contributorRef: 'device_class.apple_silicon.m3_pro_18gb',
        rank: 1,
        score: 2025,
      }),
    ])
    expect(
      leaderboards.lanes.find(lane => lane.lane === 'a5_accuracy')?.rows,
    ).toEqual([
      expect.objectContaining({
        contributorRef: 'eval.cs336.a5.gsm8k.seeded.1',
        rank: 1,
        score: 0.42,
      }),
    ])
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
    const shard = {
      inputDocumentCount: 64,
      outputDigestRef: 'digest.sha256.cs336_a4.pii_masking.aaaa',
      pylonRef: 'pylon.24819249b4634a4c9d5e',
      receiptRefs: ['receipt.cs336_a4.settlement.pii_masking'],
      shardRef: 'shard.cs336_a4.pii_masking.1',
      sourceRefs: ['commitment.cs336_a4.pii_masking.sha256_abcdef0123456789'],
      stage: 'pii_masking' as const,
      verificationRefs: ['verdict.training.deterministic_recompute.pii_masking'],
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
        shards: ReadonlyArray<Record<string, unknown>>
        status: string
      }>
    }>

    expect(admitted.status).toBe(200)
    expect(admittedBody.refinery.status).toBe('collecting_shards')
    expect(admittedBody.refinery.shards[0]).toMatchObject({
      stage: 'pii_masking',
      verified: true,
    })

    const dashboard = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request('https://openagents.test/api/training/refinery/a4'),
        {},
      ),
    )
    const dashboardBody = (await dashboard.json()) as Readonly<{
      schemaVersion: string
      shards: ReadonlyArray<unknown>
    }>

    expect(dashboard.status).toBe(200)
    expect(dashboardBody.schemaVersion).toBe(
      'openagents.training.data_refinery_dashboard.v1',
    )
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
      sourceRefs: [
        'workload.cs336_a5.seeded_rollout_and_reference_grading.v1',
      ],
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
      blockerRefs: [],
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
      blockerRefs: [],
      classDistributions: [
        {
          deviceClassRef: 'device_class.apple_silicon_macos.arm64',
          verified: true,
        },
      ],
    })
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

    for (const action of ['activate', 'seal', 'reconcile']) {
      const response = await runRoute(
        routes.routeTrainingRunWindowRequest(
          jsonRequest(`/api/training/windows/training.window.4673/${action}`, {
            receiptRef: `receipt.training.${action}`,
          }),
          {},
        ),
      )
      expect(response.status).toBe(200)
    }

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
      window: { state: 'reconciled', windowRef: 'training.window.4673' },
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
})
