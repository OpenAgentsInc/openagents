import {
  QA_SWARM_RUN_PROJECTION_SCHEMA,
  buildResolverBackedQaSwarmBoardGraph,
  type QaSwarmProjectionEvidence,
  type QaSwarmRunProjection,
} from '@openagentsinc/qa-swarm-contract'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type QaSwarmProjectionStore,
  makeArtifactQaSwarmProjectionStore,
  makeQaSwarmProjectionRoutes,
} from './qa-swarm-projection-routes'

const runRef = 'qa-run.control.round-trip-001'

const projection = (
  status: 'scheduled' | 'running' | 'completed' | 'failed' = 'running',
): QaSwarmRunProjection => {
  const evidence: QaSwarmProjectionEvidence = {
    blockerRefs: ['blocker.qa_swarm.receipt_resolver.not_configured'],
    coverageFrontier: [],
    distilledTests: [],
    execution: {
      status,
      tiers: [{ backend: 'fixture', status: status === 'completed' ? 'passed' : status }],
    },
    generatedAt: status === 'completed'
      ? '2026-07-14T10:01:00.000Z'
      : '2026-07-14T10:00:00.000Z',
    opaqueTargetRefs: [],
    perfBudgets: [],
    projectionRef: 'projection.qa_swarm.run.control.round-trip-001',
    publicSafetyRefs: [],
    runRef,
    schemaVersion: QA_SWARM_RUN_PROJECTION_SCHEMA,
    staleness: {
      contractVersion: 'projection_staleness.v1',
      maxAgeHours: 24,
      mode: 'artifact_snapshot',
    },
    target: { label: 'Published test target', visibility: 'opaque' },
    title: 'Arbitrary published QA Swarm run',
    traceRefs: [],
    verdict: 'inconclusive',
    verdictWall: [],
    videoRefs: [],
  }
  return {
    ...evidence,
    ...buildResolverBackedQaSwarmBoardGraph(evidence, {
      resolve: () => ({
        status: 'unavailable',
        blockerRef: 'blocker.qa_swarm.receipt_resolver.not_configured',
      }),
    }),
  } as QaSwarmRunProjection
}

const makeStore = (): QaSwarmProjectionStore => {
  const rows = new Map<string, QaSwarmRunProjection>()
  return {
    read: ref => Effect.succeed(rows.get(ref) ?? null),
    write: value => Effect.sync(() => void rows.set(value.runRef, value)),
  }
}

const context = {} as ExecutionContext

describe('QA Swarm projection publication routes', () => {
  test('round-trips an arbitrary runRef and serves the exact updated projection', async () => {
    const store = makeStore()
    const routes = makeQaSwarmProjectionRoutes({
      makeStore: () => store,
      requireAdminApiToken: async request =>
        request.headers.get('authorization') === 'Bearer admin-test',
    })
    const operatorUrl =
      `https://openagents.com/api/operator/qa-swarm/runs/${runRef}`
    const publicUrl =
      `https://openagents.com/api/public/qa-swarm/runs/${runRef}`

    for (const status of ['running', 'completed'] as const) {
      const effect = routes.routeQaSwarmProjectionRequest(
        new Request(operatorUrl, {
          method: 'PUT',
          headers: {
            authorization: 'Bearer admin-test',
            'content-type': 'application/json',
          },
          body: JSON.stringify(projection(status)),
        }),
        {},
        context,
      )
      expect(effect).toBeDefined()
      const response = await Effect.runPromise(effect!)
      expect(response.status).toBe(200)
      expect((await response.json()) as unknown).toMatchObject({
        ok: true,
        shareUrl: `/qa/${runRef}`,
        projection: { runRef, execution: { status } },
      })
    }

    const readEffect = routes.routeQaSwarmProjectionRequest(
      new Request(publicUrl),
      {},
      context,
    )
    const read = await Effect.runPromise(readEffect!)
    expect(read.status).toBe(200)
    expect((await read.json()) as unknown).toMatchObject({
      projection: {
        runRef,
        generatedAt: '2026-07-14T10:01:00.000Z',
        execution: { status: 'completed', tiers: [{ status: 'passed' }] },
      },
    })
  })

  test('fails closed for unauthorized, mismatched, private, and unknown runs', async () => {
    const store = makeStore()
    const routes = makeQaSwarmProjectionRoutes({
      makeStore: () => store,
      requireAdminApiToken: async request =>
        request.headers.get('authorization') === 'Bearer admin-test',
    })
    const publish = (body: unknown, authorized = true) =>
      Effect.runPromise(
        routes.routeQaSwarmProjectionRequest(
          new Request(
            `https://openagents.com/api/operator/qa-swarm/runs/${runRef}`,
            {
              method: 'PUT',
              headers: {
                ...(authorized ? { authorization: 'Bearer admin-test' } : {}),
                'content-type': 'application/json',
              },
              body: JSON.stringify(body),
            },
          ),
          {},
          context,
        )!,
      )

    expect((await publish(projection(), false)).status).toBe(401)
    expect((await publish({ ...projection(), runRef: 'qa-run.other' })).status)
      .toBe(422)
    expect((await publish({ ...projection(), title: 'operator@example.com' })).status)
      .toBe(422)

    const unknown = await Effect.runPromise(
      routes.routeQaSwarmProjectionRequest(
        new Request(
          'https://openagents.com/api/public/qa-swarm/runs/qa-run.unknown',
        ),
        {},
        context,
      )!,
    )
    expect(unknown.status).toBe(404)
    expect(await unknown.json()).toEqual({ error: 'not_found' })
  })

  test('fails closed when durable storage contains corrupt or private JSON', async () => {
    let stored = '{not-json'
    const bucket = {
      get: async () => ({ text: async () => stored }),
      put: async () => undefined,
    } as unknown as R2Bucket
    const routes = makeQaSwarmProjectionRoutes({
      makeStore: () => makeArtifactQaSwarmProjectionStore(bucket),
      requireAdminApiToken: async () => true,
    })
    const read = () =>
      Effect.runPromise(
        routes.routeQaSwarmProjectionRequest(
          new Request(
            `https://openagents.com/api/public/qa-swarm/runs/${runRef}`,
          ),
          {},
          context,
        )!,
      )

    const corrupt = await read()
    expect(corrupt.status).toBe(503)
    expect(await corrupt.json()).toEqual({
      error: 'qa_swarm_projection_unavailable',
    })

    stored = JSON.stringify({ ...projection(), title: 'operator@example.com' })
    const privateResponse = await read()
    expect(privateResponse.status).toBe(503)
    expect(JSON.stringify(await privateResponse.json())).not.toContain(
      'operator@example.com',
    )
  })
})
