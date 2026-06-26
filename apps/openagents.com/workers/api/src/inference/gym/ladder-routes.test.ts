import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  buildBenchmarkReport,
  type BenchmarkCell,
  type BenchmarkLaneSample,
  makeRealLaneSeam,
  runBenchmark,
} from '../benchmark'
import {
  OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT,
  compileGymExperiment,
  type GymExperiment,
} from './experiment'
import {
  handleOperatorGymLeaderboardApi,
  handlePublicGymLeaderboardApi,
} from './ladder-routes'
import {
  GYM_LADDER_RECURRING_CONFIG,
  type GymLadderLeaderboard,
} from './ladder'
import type { GymLadderStore } from './ladder-store'

const makeMemoryStore = (): GymLadderStore & {
  snapshot: () => GymLadderLeaderboard | undefined
} => {
  const byRef = new Map<string, GymLadderLeaderboard>()
  return {
    getLadder: ladderRef => Effect.succeed(byRef.get(ladderRef)),
    snapshot: () => byRef.get(GYM_LADDER_RECURRING_CONFIG.ladderRef),
    upsertLadder: ladder =>
      Effect.sync(() => {
        byRef.set(ladder.ladderRef, ladder)
      }),
  }
}

const REALISTIC_SHAPE = {
  id: 'observed-opencode-ladder-route-run',
  inputTokens: 1500,
  outputTokens: 500,
  cacheablePrefixTokens: 700,
  concurrency: 1,
  provenance: 'realistic',
} as const

const LADDER_EXPERIMENT: GymExperiment = {
  ...OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT,
  id: 'gym-opencode-ladder-route-test-v1',
  policy: {
    ...OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT.policy,
    fanout: {
      ...OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT.policy.fanout,
      lanes: ['khala'],
    },
  },
  shapes: [REALISTIC_SHAPE],
  budget: {
    spendCapMsat: 10_000_000,
    maxBillableSamples: 10,
    seam: 'real',
    ownerApprovalRef: 'approval.public.gym.ladder.route.test',
  },
}

const sampleForCost =
  (
    costBasisMsat: number,
  ): ((cell: BenchmarkCell, sampleIndex: number) => BenchmarkLaneSample) =>
  (cell, sampleIndex) => ({
    promptTokens: cell.shape.inputTokens,
    completionTokens: cell.shape.outputTokens,
    totalTokens: cell.shape.inputTokens + cell.shape.outputTokens,
    cachedInputTokens: Math.floor(cell.shape.cacheablePrefixTokens * 0.7),
    ttftMs: 210 + sampleIndex,
    totalWallClockMs: 3_300 + sampleIndex,
    generationWallClockMs: 3_000 + sampleIndex,
    providerTimeMs: 3_210 + sampleIndex,
    gatewayOverheadMs: 90,
    verificationClass: 'test_passed',
    executedVerdict: 'passed',
    scalarReward: 1,
    verifierTimeMs: 850,
    costBasisMsat,
    region: 'openagents',
    clientSurface: {
      client: 'opencode',
      taskRef: 'gym.ladder.route.opencode.smoke.v1',
      configRef: `opencode.ladder.route.${cell.lane}.v1`,
      toolCallsAttempted: 2,
      toolCallsSucceeded: 2,
    },
  })

const decisionGradeReport = (costBasisMsat: number) => {
  const compiled = compileGymExperiment(LADDER_EXPERIMENT)
  const runSet = runBenchmark(
    compiled.matrixConfig,
    makeRealLaneSeam({
      armRealSweep: true,
      executor: sampleForCost(costBasisMsat),
    }),
  )
  return { compiled, report: buildBenchmarkReport(runSet) }
}

const adminAllowed = () => Promise.resolve(true)
const adminDenied = () => Promise.resolve(false)

describe('GET /api/public/gym/leaderboard', () => {
  test('serves the honest empty ladder when nothing is published', async () => {
    const store = makeMemoryStore()
    const response = await Effect.runPromise(
      handlePublicGymLeaderboardApi(new Request('https://x/'), { store }),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      scope: string
      staleness: { composition: string; contractVersion: string }
      ladder: GymLadderLeaderboard
    }
    expect(body.scope).toBe('public')
    expect(body.staleness.composition).toBe('stored_snapshot')
    expect(body.staleness.contractVersion).toBe('projection_staleness.v1')
    expect(body.ladder.rungs.map(r => r.state)).toEqual([
      'awaiting_owner',
      'awaiting_owner',
      'awaiting_owner',
    ])
  })

  test('rejects non-GET', async () => {
    const response = await Effect.runPromise(
      handlePublicGymLeaderboardApi(
        new Request('https://x/', { method: 'POST' }),
        {},
      ),
    )
    expect(response.status).toBe(405)
  })
})

describe('POST /api/operator/gym/leaderboard', () => {
  test('publishes a decision-grade rung1 ladder and serves it publicly', async () => {
    const store = makeMemoryStore()
    const khala = decisionGradeReport(400)
    const bigPickle = decisionGradeReport(900)
    const publishResponse = await Effect.runPromise(
      handleOperatorGymLeaderboardApi(
        new Request('https://x/', {
          method: 'POST',
          body: JSON.stringify({
            reports: [
              {
                ...khala,
                reportRef: 'report.gym.ladder.route.khala',
                receiptRef: 'receipt.gym.ladder.route.khala',
                candidateRef: 'khala.ladder.route',
              },
              {
                ...bigPickle,
                reportRef: 'report.gym.ladder.route.bigpickle',
                receiptRef: 'receipt.gym.ladder.route.bigpickle',
                candidateRef: 'bigpickle.ladder.route',
              },
            ],
          }),
        }),
        { store, requireAdminApiToken: adminAllowed },
      ),
    )
    expect(publishResponse.status).toBe(201)
    const published = (await publishResponse.json()) as {
      kind: string
      ladder: GymLadderLeaderboard
    }
    expect(published.kind).toBe('gym_ladder_published')
    expect(
      published.ladder.rungs.find(r => r.rung === 'rung1')?.state,
    ).toBe('published')

    // Now the public surface serves it.
    const publicResponse = await Effect.runPromise(
      handlePublicGymLeaderboardApi(new Request('https://x/'), { store }),
    )
    const publicBody = (await publicResponse.json()) as {
      ladder: GymLadderLeaderboard
    }
    expect(publicBody.ladder.rungs.find(r => r.rung === 'rung1')?.state).toBe(
      'published',
    )
    expect(
      publicBody.ladder.rungs
        .find(r => r.rung === 'rung1')
        ?.entries.map(e => e.lane),
    ).toEqual(['khala', 'bigpickle'])
  })

  test('rejects unauthorized publish', async () => {
    const store = makeMemoryStore()
    const response = await Effect.runPromise(
      handleOperatorGymLeaderboardApi(
        new Request('https://x/', {
          method: 'POST',
          body: JSON.stringify({ reports: [] }),
        }),
        { store, requireAdminApiToken: adminDenied },
      ),
    )
    expect(response.status).toBe(401)
    expect(store.snapshot()).toBeUndefined()
  })

  test('rejects a malformed publish body with a typed 400', async () => {
    const store = makeMemoryStore()
    const response = await Effect.runPromise(
      handleOperatorGymLeaderboardApi(
        new Request('https://x/', {
          method: 'POST',
          body: JSON.stringify({ notReports: true }),
        }),
        { store, requireAdminApiToken: adminAllowed },
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('gym_ladder_publish_rejected')
    expect(store.snapshot()).toBeUndefined()
  })

  test('rejects unsafe leaderboard refs at publish', async () => {
    const store = makeMemoryStore()
    const khala = decisionGradeReport(400)
    const response = await Effect.runPromise(
      handleOperatorGymLeaderboardApi(
        new Request('https://x/', {
          method: 'POST',
          body: JSON.stringify({
            reports: [
              {
                ...khala,
                reportRef: 'raw_prompt.private',
                receiptRef: 'receipt.gym.ladder.route.safe',
                candidateRef: 'khala.ladder.route',
              },
            ],
          }),
        }),
        { store, requireAdminApiToken: adminAllowed },
      ),
    )
    expect(response.status).toBe(400)
    expect(store.snapshot()).toBeUndefined()
  })
})
