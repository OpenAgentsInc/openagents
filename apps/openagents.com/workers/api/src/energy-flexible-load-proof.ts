import { Schema as S } from 'effect'

import { exampleErcotLmpWindowRecord } from './ercot-lmp-ingestion'
import {
  examplePylonFlexibleLoadEvent,
  projectPylonFlexibleLoadEvent,
} from './pylon-flexible-load-events'
import {
  examplePylonFlexibleLoadProfile,
  projectPylonFlexibleLoadProfile,
} from './pylon-flexible-load-profiles'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const EnergyFlexibleLoadProofEndpoint =
  '/api/public/energy/flexible-load-proof'
export const EnergyFlexibleLoadProofSchemaVersion =
  'openagents.energy.flexible_load_proof.v1'

export const EnergyFlexibleLoadProofGate = S.Struct({
  blockerRefs: S.Array(S.String),
  greenGateSatisfied: S.Boolean,
  marketPriceIngestionAvailable: S.Boolean,
  modeledOperatorReportAvailable: S.Boolean,
  ownerSignedTransitionReceiptAvailable: S.Boolean,
  realFlexibleLoadReceiptAvailable: S.Boolean,
  workClassFlexProfilesAvailable: S.Boolean,
})
export type EnergyFlexibleLoadProofGate =
  typeof EnergyFlexibleLoadProofGate.Type

export class EnergyFlexibleLoadProofProjection extends S.Class<EnergyFlexibleLoadProofProjection>(
  'EnergyFlexibleLoadProofProjection',
)({
  authorityBoundary: S.String,
  eventHistory: S.Struct({
    evidenceStateLabels: S.Array(S.String),
    events: S.Array(S.Unknown),
    projectedEventCount: S.Int,
  }),
  gate: EnergyFlexibleLoadProofGate,
  generatedAt: S.String,
  marketPrices: S.Struct({
    decodedRowCount: S.Int,
    source: S.Literal('ercot_public_api_v2_fixture'),
    windows: S.Array(S.Unknown),
  }),
  promiseId: S.Literal('energy.flexible_load_proof.v1'),
  schemaVersion: S.Literal(EnergyFlexibleLoadProofSchemaVersion),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('evidence_scaffolded_receipt_gated'),
  workClassFlexProfiles: S.Struct({
    profiles: S.Array(S.Unknown),
    projectedProfileCount: S.Int,
  }),
}) {}

const authorityBoundary =
  'Read-only public flexible-load proof projection. It decodes public market-price rows, exposes work-class flexibility profiles, and projects labeled flexible-load event history, but grants no grid dispatch, capacity assignment, runner launch, wallet spend, payout, settlement, or public promise-state authority.'

const sourceRefs = [
  'docs/metrics/2026-06-19-flexible-load-operator-proof-report-modeled.md',
  'apps/openagents.com/workers/api/src/ercot-lmp-ingestion.ts',
  'apps/openagents.com/workers/api/src/ercot-lmp-ingestion.test.ts',
  'apps/openagents.com/workers/api/src/pylon-flexible-load-profiles.ts',
  'apps/openagents.com/workers/api/src/pylon-flexible-load-profiles.test.ts',
  'apps/openagents.com/workers/api/src/pylon-flexible-load-events.ts',
  'apps/openagents.com/workers/api/src/pylon-flexible-load-events.test.ts',
]

export const projectEnergyFlexibleLoadProof = (
  nowIso: string = currentIsoTimestamp(),
): EnergyFlexibleLoadProofProjection => {
  const priceWindow = exampleErcotLmpWindowRecord()
  const profile = projectPylonFlexibleLoadProfile(
    examplePylonFlexibleLoadProfile(),
    'public',
    nowIso,
  )
  const event = projectPylonFlexibleLoadEvent(
    {
      ...examplePylonFlexibleLoadEvent(),
      compensationRefs: [],
      evidenceRefs: [],
      settlementRefs: [],
      state: 'measured',
    },
    'public',
    nowIso,
  )

  return new EnergyFlexibleLoadProofProjection({
    authorityBoundary,
    eventHistory: {
      evidenceStateLabels: [
        event.stateLabel,
        event.measuredClaimAllowed ? 'Measured response' : 'Not measured',
        event.verifiedClaimAllowed ? 'Verified evidence' : 'Not verified',
        event.settlementClaimAllowed ? 'Settled event' : 'Not settled',
      ],
      events: [event],
      projectedEventCount: 1,
    },
    gate: {
      blockerRefs: [
        'blocker.product_promises.real_flexible_load_receipt_missing',
        'blocker.product_promises.owner_signed_energy_green_transition_missing',
      ],
      greenGateSatisfied: false,
      marketPriceIngestionAvailable: true,
      modeledOperatorReportAvailable: true,
      ownerSignedTransitionReceiptAvailable: false,
      realFlexibleLoadReceiptAvailable: false,
      workClassFlexProfilesAvailable: true,
    },
    generatedAt: nowIso,
    marketPrices: {
      decodedRowCount: priceWindow.rowCount,
      source: 'ercot_public_api_v2_fixture',
      windows: [priceWindow],
    },
    promiseId: 'energy.flexible_load_proof.v1',
    schemaVersion: EnergyFlexibleLoadProofSchemaVersion,
    sourceRefs,
    staleness: liveAtReadStaleness([
      'energy_market_price_rows_ingested',
      'flexible_load_work_class_profile_changed',
      'flexible_load_event_recorded',
      'product_promise_registry_changed',
    ]),
    status: 'evidence_scaffolded_receipt_gated',
    workClassFlexProfiles: {
      profiles: [profile],
      projectedProfileCount: 1,
    },
  })
}
