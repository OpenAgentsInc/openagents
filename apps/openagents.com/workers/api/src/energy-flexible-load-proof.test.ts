import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  EnergyFlexibleLoadProofProjection,
  projectEnergyFlexibleLoadProof,
} from './energy-flexible-load-proof'

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
      'Settled',
      'Measured response',
      'Verified evidence',
      'Settled event',
    ])
    expect(projection.gate.blockerRefs).toEqual([
      'blocker.product_promises.real_flexible_load_receipt_missing',
      'blocker.product_promises.owner_signed_energy_green_transition_missing',
    ])
    expect(projection.authorityBoundary).toContain('grants no grid dispatch')
  })
})
