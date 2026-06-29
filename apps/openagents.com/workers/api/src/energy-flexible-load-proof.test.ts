import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  EnergyFlexibleLoadProofEndpoint,
  EnergyFlexibleLoadProofProjection,
  projectEnergyFlexibleLoadProof,
} from './energy-flexible-load-proof'
import { handleEnergyFlexibleLoadProofApi } from './energy-flexible-load-proof-routes'
import { openAgentsOpenApiDocument } from './openagents-openapi'

describe('energy flexible-load proof projection', () => {
  test('returns decoded market rows, queryable flex profiles, and labeled event history without flipping green', () => {
    const projection = projectEnergyFlexibleLoadProof(
      '2026-06-20T00:10:00.000Z',
    )

    expect(S.decodeUnknownSync(EnergyFlexibleLoadProofProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      gate: {
        greenGateSatisfied: false,
        marketPriceIngestionAvailable: true,
        modeledOperatorReportAvailable: true,
        ownerSignedTransitionReceiptAvailable: false,
        realFlexibleLoadReceiptAvailable: false,
        workClassFlexProfilesAvailable: true,
      },
      marketPrices: {
        decodedRowCount: 96,
        source: 'ercot_public_api_v2_fixture',
      },
      promiseId: 'energy.flexible_load_proof.v1',
      status: 'evidence_scaffolded_receipt_gated',
      workClassFlexProfiles: {
        projectedProfileCount: 1,
      },
    })
    expect(projection.eventHistory.projectedEventCount).toBe(1)
    expect(projection.eventHistory.evidenceStateLabels).toEqual([
      'Measured',
      'Measured response',
      'Not verified',
      'Not settled',
    ])
    expect(projection.gate.blockerRefs).toEqual([
      'blocker.product_promises.real_flexible_load_receipt_missing',
      'blocker.product_promises.owner_signed_energy_green_transition_missing',
    ])
    expect(projection.authorityBoundary).toContain('grants no grid dispatch')
  })

  test('serves the public route as no-store JSON and documents it in OpenAPI', async () => {
    const response = await Effect.runPromise(
      handleEnergyFlexibleLoadProofApi(
        new Request(`https://openagents.com${EnergyFlexibleLoadProofEndpoint}`),
      ),
    )
    const body = await response.json()
    const document = await Effect.runPromise(openAgentsOpenApiDocument())

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body).toEqual(expect.objectContaining({
      promiseId: 'energy.flexible_load_proof.v1',
      status: 'evidence_scaffolded_receipt_gated',
    }))
    expect(
      (
        document.paths[EnergyFlexibleLoadProofEndpoint] as
          | { get?: { operationId?: string } }
          | undefined
      )?.get?.operationId,
    ).toBe('getEnergyFlexibleLoadProof')
    expect(
      (document.components as { schemas: Record<string, unknown> }).schemas,
    ).toHaveProperty('EnergyFlexibleLoadProofProjection')
  })
})
