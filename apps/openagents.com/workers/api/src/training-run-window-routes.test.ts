import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
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
