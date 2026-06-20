import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { DemandProvenanceEndpoint, projectDemandProvenance } from './demand-provenance'
import { handleDemandProvenanceApi } from './demand-provenance-routes'
import { openAgentsOpenApiDocument } from './openagents-openapi'

type DemandProvenanceBody = Readonly<{
  externalDemandClaimAllowed: boolean
  kind: string
  promiseId: string
  promiseState: string
  surfaceSummaries: ReadonlyArray<{
    externalAcceptedOutcomeCount: number
    internalAcceptedOutcomeCount: number
    rule: string
    surfaceRef: string
    unlabeledAcceptedOutcomeCount: number
  }>
}>

describe('demand provenance public projection', () => {
  test('summarizes serving internal/external splits without claiming external demand', () => {
    const projection = projectDemandProvenance({
      generatedAt: '2026-06-20T06:30:00.000Z',
    })

    expect(projection.kind).toBe('demand_provenance_public')
    expect(projection.promiseId).toBe('proof.demand_provenance.v1')
    expect(projection.promiseState).toBe('yellow')
    expect(projection.rule).toBe('no_external_dollar_no_demand_claim')
    expect(projection.publicSafe).toBe(true)
    expect(projection.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
    })
    expect(projection.surfaceSummaries).toHaveLength(1)
    expect(projection.surfaceSummaries[0]).toMatchObject({
      externalAcceptedOutcomeCount: 0,
      internalAcceptedOutcomeCount: 1,
      rule: 'no_external_dollar_no_demand_claim',
      splitState: 'serving_internal_external_split',
      surfaceRef: 'route:/api/public/metrics/accepted-outcomes-per-kwh',
      unlabeledAcceptedOutcomeCount: 0,
    })
    expect(projection.totals).toEqual({
      externalAcceptedOutcomeCount: 0,
      internalAcceptedOutcomeCount: 1,
      unlabeledAcceptedOutcomeCount: 0,
    })
    expect(projection.externalDemandClaimAllowed).toBe(false)
    expect(projection.blockerRefs).toEqual([
      'blocker.product_promises.demand_provenance_broad_projection_coverage_missing',
    ])
    expect(projection.coverage.remainingSurfaceRefs).toEqual(
      expect.arrayContaining([
        'route:/api/public/pylon-stats',
        'route:/api/public/training/runs/{trainingRunRef}',
        'route:/api/training/leaderboards/*',
        'projection:training.rung_economics_gates',
      ]),
    )
    expect(projection.unsafeCopy).toContain(
      'Do not present internal, first-party, operator-staged, or unlabeled demand as external market demand',
    )
  })

  test('serves the public demand-provenance route as no-store JSON', async () => {
    const response = await Effect.runPromise(
      handleDemandProvenanceApi(
        new Request(`https://openagents.com${DemandProvenanceEndpoint}`),
      ),
    )
    const body = (await response.json()) as DemandProvenanceBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.kind).toBe('demand_provenance_public')
    expect(body.promiseId).toBe('proof.demand_provenance.v1')
    expect(body.promiseState).toBe('yellow')
    expect(body.externalDemandClaimAllowed).toBe(false)
    expect(body.surfaceSummaries[0]).toMatchObject({
      externalAcceptedOutcomeCount: 0,
      internalAcceptedOutcomeCount: 1,
      rule: 'no_external_dollar_no_demand_claim',
      surfaceRef: 'route:/api/public/metrics/accepted-outcomes-per-kwh',
      unlabeledAcceptedOutcomeCount: 0,
    })
  })

  test('documents the public demand-provenance endpoint in OpenAPI', async () => {
    const document = await Effect.runPromise(openAgentsOpenApiDocument())

    expect(
      (
        document.paths[DemandProvenanceEndpoint] as
          | { get?: unknown }
          | undefined
      )?.get,
    ).toEqual(
      expect.objectContaining({
        operationId: 'getPublicDemandProvenance',
      }),
    )
    expect(
      (document.components as { schemas: Record<string, unknown> }).schemas,
    ).toHaveProperty('DemandProvenanceProjection')
  })
})
