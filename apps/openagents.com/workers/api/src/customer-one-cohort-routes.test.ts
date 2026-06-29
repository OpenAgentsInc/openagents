import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CustomerOneCohortEndpoint,
  type CustomerOneCohortPrivateRow,
  CustomerOneCohortProjection,
} from './customer-one-cohort-projection'
import {
  handleOperatorCustomerOneCohortRowsApi,
  handlePublicCustomerOneCohortApi,
} from './customer-one-cohort-routes'
import type { CustomerOneCohortRowStore } from './customer-one-cohort-store'
import { openAgentsOpenApiDocument } from './openagents-openapi'

const generatedAt = '2026-06-17T21:00:00.000Z'

const request = (method = 'GET') =>
  new Request(`https://openagents.com${CustomerOneCohortEndpoint}`, {
    method,
  })

const operatorRequest = (method = 'GET', body?: unknown) =>
  new Request('https://openagents.com/api/operator/customer-one-cohort/rows', {
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    headers: { 'content-type': 'application/json' },
    method,
  })

const completedRow = (index: number): CustomerOneCohortPrivateRow => ({
  completionBundleRef: `completion.customer-one.team-${index}.bundle.v1`,
  privacyReviewRef: `privacy.customer-one.team-${index}.review.v1`,
  reviewRef: `review.customer-one.team-${index}.human.v1`,
  runRef: `run.customer-one.team-${index}.primary.v1`,
  state: 'loop_completed',
  teamCohortRef: `cohort.team.${index}.v1`,
  updatedAt: generatedAt,
  workspaceRef: `workspace.customer-one.team-${index}.v1`,
})

const memoryStore = (): CustomerOneCohortRowStore &
  Readonly<{ rows: CustomerOneCohortPrivateRow[] }> => {
  const rows: CustomerOneCohortPrivateRow[] = []

  return {
    listRows: () =>
      Effect.succeed(
        [...rows].sort((left, right) =>
          left.teamCohortRef.localeCompare(right.teamCohortRef),
        ),
      ),
    rows,
    upsertRow: row =>
      Effect.sync(() => {
        const existingIndex = rows.findIndex(
          candidate => candidate.teamCohortRef === row.teamCohortRef,
        )

        if (existingIndex === -1) {
          rows.push(row)
          return
        }

        rows.splice(existingIndex, 1, row)
      }),
  }
}

describe('public customer one cohort route', () => {
  test('serves the default empty blocked projection as no-store JSON', async () => {
    const response = await Effect.runPromise(
      handlePublicCustomerOneCohortApi(request(), {
        nowIso: () => generatedAt,
      }),
    )
    const body = await response.json()

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(S.decodeUnknownSync(CustomerOneCohortProjection)(body)).toEqual(body)
    expect(body).toMatchObject({
      authority: 'evidence_only',
      counts: {
        loop_completed: 0,
      },
      gate: {
        reasonRefs: ['reason.customer_one.cohort_completion_bundles_missing'],
        state: 'blocked',
      },
      generatedAt,
      rows: [],
      staleness: {
        composition: 'live_at_read',
        contractVersion: 'projection_staleness.v1',
        maxStalenessSeconds: 0,
        rebuildsOn: ['cohort_row_written', 'privacy_review_recorded'],
      },
    })
  })

  test('projects injected source rows without changing route authority', async () => {
    const response = await Effect.runPromise(
      handlePublicCustomerOneCohortApi(request(), {
        nowIso: () => generatedAt,
        store: {
          listRows: () =>
            Effect.succeed([completedRow(1), completedRow(2), completedRow(3)]),
        },
      }),
    )
    const body = S.decodeUnknownSync(CustomerOneCohortProjection)(
      await response.json(),
    )

    expect(response.status).toBe(200)
    expect(body.authority).toBe('evidence_only')
    expect(body.counts.loop_completed).toBe(3)
    expect(body.gate).toEqual({
      reasonRefs: [],
      state: 'ready',
    })
    expect(body.rows.map(row => row.displayLabel)).toEqual([
      'Team 1',
      'Team 2',
      'Team 3',
    ])
  })

  test('rejects unsupported methods', async () => {
    const response = await Effect.runPromise(
      handlePublicCustomerOneCohortApi(request('POST')),
    )

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
  })

  test('records operator rows and serves the public projection from the same source', async () => {
    const store = memoryStore()
    const denied = await Effect.runPromise(
      handleOperatorCustomerOneCohortRowsApi(
        operatorRequest('POST', completedRow(1)),
        {
          requireAdminApiToken: () => Promise.resolve(false),
          store,
        },
      ),
    )
    const incomplete = await Effect.runPromise(
      handleOperatorCustomerOneCohortRowsApi(
        operatorRequest('POST', {
          completionBundleRef: 'completion.customer-one.team-alpha.bundle.v1',
          state: 'loop_completed',
          teamCohortRef: 'cohort.team.alpha.v1',
          updatedAt: generatedAt,
        }),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          store,
        },
      ),
    )
    const operatorList = await Effect.runPromise(
      handleOperatorCustomerOneCohortRowsApi(operatorRequest(), {
        nowIso: () => generatedAt,
        requireAdminApiToken: () => Promise.resolve(true),
        store,
      }),
    )
    const publicProjection = await Effect.runPromise(
      handlePublicCustomerOneCohortApi(request(), {
        nowIso: () => generatedAt,
        store,
      }),
    )
    const privateRows = (await operatorList.json()) as Readonly<{
      rows: ReadonlyArray<CustomerOneCohortPrivateRow>
    }>
    const publicBody = S.decodeUnknownSync(CustomerOneCohortProjection)(
      await publicProjection.json(),
    )

    expect(denied.status).toBe(401)
    expect(incomplete.status).toBe(201)
    expect(privateRows.rows).toHaveLength(1)
    expect(publicBody.counts.loop_completed).toBe(0)
    expect(publicBody.gate.state).toBe('blocked')
    expect(publicBody.rows[0]?.blockerRefs).toEqual([
      'customer-one-cohort-blocker:cohort.team.alpha.v1:missing-privacy-review',
    ])
  })

  test('opens the public gate after three stored privacy-reviewed completions', async () => {
    const store = memoryStore()

    await Effect.runPromise(
      handleOperatorCustomerOneCohortRowsApi(
        operatorRequest('POST', completedRow(1)),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          store,
        },
      ),
    )
    await Effect.runPromise(
      handleOperatorCustomerOneCohortRowsApi(
        operatorRequest('POST', completedRow(2)),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          store,
        },
      ),
    )
    await Effect.runPromise(
      handleOperatorCustomerOneCohortRowsApi(
        operatorRequest('POST', completedRow(3)),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          store,
        },
      ),
    )

    const response = await Effect.runPromise(
      handlePublicCustomerOneCohortApi(request(), {
        nowIso: () => generatedAt,
        store,
      }),
    )
    const body = S.decodeUnknownSync(CustomerOneCohortProjection)(
      await response.json(),
    )

    expect(body.counts.loop_completed).toBe(3)
    expect(body.gate).toEqual({
      reasonRefs: [],
      state: 'ready',
    })
  })

  test('rejects unsafe operator intake before storage', async () => {
    const store = memoryStore()
    const privateMaterial = await Effect.runPromise(
      handleOperatorCustomerOneCohortRowsApi(
        operatorRequest('POST', {
          sourceUrl: 'https://customer.example/private-workspace',
          state: 'candidate',
          teamCohortRef: 'cohort.team.source.v1',
          updatedAt: generatedAt,
        }),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          store,
        },
      ),
    )
    const invalidTeamRef = await Effect.runPromise(
      handleOperatorCustomerOneCohortRowsApi(
        operatorRequest('POST', {
          state: 'candidate',
          teamCohortRef: 'acme-team',
          updatedAt: generatedAt,
        }),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          store,
        },
      ),
    )

    expect(privateMaterial.status).toBe(400)
    expect(invalidTeamRef.status).toBe(400)
    expect(store.rows).toHaveLength(0)
  })

  test('documents the route in OpenAPI', async () => {
    const document = await Effect.runPromise(openAgentsOpenApiDocument())

    expect(
      (
        document.paths[CustomerOneCohortEndpoint] as
          | { get?: unknown }
          | undefined
      )?.get,
    ).toEqual(
      expect.objectContaining({
        operationId: 'getPublicCustomerOneCohort',
      }),
    )
    expect(
      (document.components as { schemas: Record<string, unknown> }).schemas,
    ).toHaveProperty('CustomerOneCohortProjection')
    expect(
      (
        document.paths['/api/operator/customer-one-cohort/rows'] as
          | { get?: unknown; post?: unknown }
          | undefined
      )?.post,
    ).toEqual(
      expect.objectContaining({
        operationId: 'operatorUpsertCustomerOneCohortRow',
      }),
    )
  })
})
