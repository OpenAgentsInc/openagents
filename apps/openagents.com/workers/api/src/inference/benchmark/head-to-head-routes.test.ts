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
} from '../gym/experiment'
import {
  KHALA_HEAD_TO_HEAD_RECURRING_CONFIG,
  type KhalaHeadToHead,
} from './head-to-head'
import {
  handleOperatorKhalaHeadToHeadApi,
  handlePublicKhalaHeadToHeadApi,
} from './head-to-head-routes'
import type { KhalaHeadToHeadStore } from './head-to-head-store'

const makeMemoryStore = (): KhalaHeadToHeadStore & {
  snapshot: () => KhalaHeadToHead | undefined
} => {
  const byRef = new Map<string, KhalaHeadToHead>()
  return {
    getHeadToHead: ref => Effect.succeed(byRef.get(ref)),
    snapshot: () =>
      byRef.get(KHALA_HEAD_TO_HEAD_RECURRING_CONFIG.headToHeadRef),
    upsertHeadToHead: headToHead =>
      Effect.sync(() => {
        byRef.set(headToHead.headToHeadRef, headToHead)
      }),
  }
}

const REALISTIC_SHAPE = {
  id: 'observed-head-to-head-route-run',
  inputTokens: 1500,
  outputTokens: 500,
  cacheablePrefixTokens: 700,
  concurrency: 1,
  provenance: 'realistic',
} as const

const H2H_EXPERIMENT: GymExperiment = {
  ...OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT,
  id: 'khala-head-to-head-route-test-v1',
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
    ownerApprovalRef: 'approval.public.khala.head_to_head.route.test',
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
      taskRef: 'khala.head_to_head.route.opencode.smoke.v1',
      configRef: `opencode.head_to_head.route.${cell.lane}.v1`,
      toolCallsAttempted: 2,
      toolCallsSucceeded: 2,
    },
  })

const decisionGradeReport = (costBasisMsat: number) => {
  const compiled = compileGymExperiment(H2H_EXPERIMENT)
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

describe('GET /api/public/khala/head-to-head', () => {
  test('serves the honest empty head-to-head when nothing is published', async () => {
    const store = makeMemoryStore()
    const response = await Effect.runPromise(
      handlePublicKhalaHeadToHeadApi(new Request('https://x/'), { store }),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      scope: string
      staleness: { composition: string; contractVersion: string }
      headToHead: KhalaHeadToHead
    }
    expect(body.scope).toBe('public')
    expect(body.staleness.composition).toBe('stored_snapshot')
    expect(body.staleness.contractVersion).toBe('projection_staleness.v1')
    expect(body.headToHead.khala).toBeNull()
    expect(body.headToHead.matchups.every(m => m.state === 'awaiting_owner')).toBe(
      true,
    )
  })

  test('rejects non-GET', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaHeadToHeadApi(
        new Request('https://x/', { method: 'POST' }),
        {},
      ),
    )
    expect(response.status).toBe(405)
  })
})

describe('POST /api/operator/khala/head-to-head', () => {
  test('publishes a decision-grade matchup and serves it publicly', async () => {
    const store = makeMemoryStore()
    const khala = decisionGradeReport(400)
    const bigPickle = decisionGradeReport(900)
    const publishResponse = await Effect.runPromise(
      handleOperatorKhalaHeadToHeadApi(
        new Request('https://x/', {
          method: 'POST',
          body: JSON.stringify({
            reports: [
              {
                ...khala,
                reportRef: 'report.khala.head_to_head.route.khala',
                receiptRef: 'receipt.khala.head_to_head.route.khala',
                candidateRef: 'khala.head_to_head.route',
              },
              {
                ...bigPickle,
                reportRef: 'report.khala.head_to_head.route.bigpickle',
                receiptRef: 'receipt.khala.head_to_head.route.bigpickle',
                candidateRef: 'bigpickle.head_to_head.route',
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
      headToHead: KhalaHeadToHead
    }
    expect(published.kind).toBe('khala_head_to_head_published')
    const bigPickleMatchup = published.headToHead.matchups.find(
      m => m.lane === 'bigpickle',
    )
    expect(bigPickleMatchup?.state).toBe('published')
    expect(bigPickleMatchup?.verdict).toBe('khala_wins_cost')

    // Now the public surface serves it.
    const publicResponse = await Effect.runPromise(
      handlePublicKhalaHeadToHeadApi(new Request('https://x/'), { store }),
    )
    const publicBody = (await publicResponse.json()) as {
      headToHead: KhalaHeadToHead
    }
    expect(
      publicBody.headToHead.matchups.find(m => m.lane === 'bigpickle')?.state,
    ).toBe('published')
    expect(publicBody.headToHead.khala?.lane).toBe('khala')
  })

  test('rejects unauthorized publish', async () => {
    const store = makeMemoryStore()
    const response = await Effect.runPromise(
      handleOperatorKhalaHeadToHeadApi(
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
      handleOperatorKhalaHeadToHeadApi(
        new Request('https://x/', {
          method: 'POST',
          body: JSON.stringify({ notReports: true }),
        }),
        { store, requireAdminApiToken: adminAllowed },
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('khala_head_to_head_publish_rejected')
    expect(store.snapshot()).toBeUndefined()
  })

  test('rejects unsafe refs at publish', async () => {
    const store = makeMemoryStore()
    const khala = decisionGradeReport(400)
    const response = await Effect.runPromise(
      handleOperatorKhalaHeadToHeadApi(
        new Request('https://x/', {
          method: 'POST',
          body: JSON.stringify({
            reports: [
              {
                ...khala,
                reportRef: 'raw_prompt.private',
                receiptRef: 'receipt.khala.head_to_head.route.safe',
                candidateRef: 'khala.head_to_head.route',
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
