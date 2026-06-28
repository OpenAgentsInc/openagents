import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  GYM_LADDER_RECURRING_CONFIG,
  type GymLadderLeaderboard,
} from './ladder'
import {
  handleOperatorGymLeaderboardApi,
  handlePublicGymLeaderboardApi,
} from './ladder-routes'
import type { GymLadderSnapshot, GymLadderStore } from './ladder-store'
import type { MirrorCodeRun } from './mirrorcode-contract'
import { handleMirrorCodeRunsApi } from './mirrorcode-routes'
import type { MirrorCodeRunStore } from './mirrorcode-store'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

const adminAllowed = () => Promise.resolve(true)

const makeMemoryMirrorCodeStore = (): MirrorCodeRunStore => {
  const byRunId = new Map<string, MirrorCodeRun>()
  return {
    getRun: runId => Effect.succeed(byRunId.get(runId)),
    listRuns: () =>
      Effect.succeed(
        [...byRunId.values()].sort((left, right) => {
          if (left.startedAt !== right.startedAt) {
            return left.startedAt > right.startedAt ? -1 : 1
          }
          return left.runId.localeCompare(right.runId)
        }),
      ),
    upsertRun: run =>
      Effect.sync(() => {
        byRunId.set(run.runId, run)
      }),
  }
}

const makeMemoryLadderStore = (): GymLadderStore => {
  const byRef = new Map<string, GymLadderSnapshot>()
  return {
    getLadder: ladderRef => Effect.succeed(byRef.get(ladderRef)),
    upsertLadder: (ladder, publishedAt) =>
      Effect.sync(() => {
        byRef.set(ladder.ladderRef, { ladder, publishedAt })
      }),
  }
}

const postJson = (url: string, body: unknown) =>
  new Request(url, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  })

const decisionGradeMirrorCodeRun = {
  runId: 'mc-s-cal-python-integration-0001',
  model: 'openagents/khala',
  taskId: 'cal',
  bucket: 'S',
  language: 'python',
  status: 'passed',
  passRate: 0.81,
  tokens: { total: 1_000_000_777 },
  exactTokenUsageEventRefs: [
    'token_usage_event.gym_mirrorcode.integration.cal.0001',
    'token_usage_event.gym_mirrorcode.integration.cal.0002',
  ],
  startedAt: '2026-06-27T00:00:00.000Z',
  finishedAt: '2026-06-27T02:00:00.000Z',
  summary: 'Decision-grade public S-bucket MirrorCode cal run.',
  grade: 'decision_grade',
} as const

describe('MirrorCode ladder execution integration', () => {
  test('records a decision-grade MirrorCode run, publishes rung4, and serves the public ladder', async () => {
    const mirrorCodeStore = makeMemoryMirrorCodeStore()
    const ladderStore = makeMemoryLadderStore()

    const ingestResponse = await run(
      handleMirrorCodeRunsApi(
        postJson(
          'https://openagents.com/api/gym/mirrorcode/runs',
          decisionGradeMirrorCodeRun,
        ),
        {
          requireAdminApiToken: adminAllowed,
          store: mirrorCodeStore,
        },
      ),
    )
    expect(ingestResponse.status).toBe(201)

    const runsResponse = await run(
      handleMirrorCodeRunsApi(
        new Request('https://openagents.com/api/gym/mirrorcode/runs'),
        {
          requireAdminApiToken: adminAllowed,
          store: mirrorCodeStore,
          nowIso: () => '2026-06-27T02:30:00.000Z',
        },
      ),
    )
    expect(runsResponse.status).toBe(200)
    const runsBody = (await runsResponse.json()) as {
      runs: ReadonlyArray<MirrorCodeRun>
    }
    expect(runsBody.runs).toHaveLength(1)
    expect(runsBody.runs[0]?.decisionGrade).toBe(true)
    expect(runsBody.runs[0]?.demandSource).toBe('gym_mirrorcode')

    const publishResponse = await run(
      handleOperatorGymLeaderboardApi(
        postJson('https://openagents.com/api/operator/gym/leaderboard', {
          reports: [],
          mirrorCodeRuns: runsBody.runs,
        }),
        {
          requireAdminApiToken: adminAllowed,
          store: ladderStore,
          nowIso: () => '2026-06-27T03:00:00.000Z',
        },
      ),
    )
    expect(publishResponse.status).toBe(201)
    const publishBody = (await publishResponse.json()) as {
      ladder: GymLadderLeaderboard
    }
    const publishedRung4 = publishBody.ladder.rungs.find(
      rung => rung.rung === 'rung4',
    )
    expect(publishedRung4?.state).toBe('published')
    expect(publishedRung4?.passRateBps).toBe(8100)
    expect(publishedRung4?.tokensTotal).toBe(1_000_000_777)

    const publicResponse = await run(
      handlePublicGymLeaderboardApi(
        new Request('https://openagents.com/api/public/gym/leaderboard'),
        {
          store: ladderStore,
          nowIso: () => '2026-06-27T03:05:00.000Z',
        },
      ),
    )
    expect(publicResponse.status).toBe(200)
    const publicBody = (await publicResponse.json()) as {
      dataAgeSeconds: number | null
      ladder: GymLadderLeaderboard
      publishedAt: string | null
      scope: string
    }
    const publicRung4 = publicBody.ladder.rungs.find(
      rung => rung.rung === 'rung4',
    )
    expect(publicBody.scope).toBe('public')
    expect(publicBody.publishedAt).toBe('2026-06-27T03:00:00.000Z')
    expect(publicBody.dataAgeSeconds).toBe(300)
    expect(publicRung4?.state).toBe('published')
    expect(publicRung4?.benchmarkFamily).toBe('mirrorcode_public_bucket')
    expect(publicRung4?.entries[0]?.candidateRef).toBe(
      'khala.mirrorcode.S.cal',
    )
    expect(publicRung4?.exactTokenUsageEventRefs).toEqual([
      'token_usage_event.gym_mirrorcode.integration.cal.0001',
      'token_usage_event.gym_mirrorcode.integration.cal.0002',
    ])
    expect(publicBody.ladder.ladderRef).toBe(
      GYM_LADDER_RECURRING_CONFIG.ladderRef,
    )
  })
})
