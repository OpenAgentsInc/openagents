import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CustomerOneCohortEndpoint,
  type CustomerOneCohortPrivateRow,
  CustomerOneCohortProjection,
} from './customer-one-cohort-projection'
import { handlePublicCustomerOneCohortApi } from './customer-one-cohort-routes'
import { openAgentsOpenApiDocument } from './openagents-openapi'

const generatedAt = '2026-06-17T21:00:00.000Z'

const request = (method = 'GET') =>
  new Request(`https://openagents.com${CustomerOneCohortEndpoint}`, {
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
  })
})
