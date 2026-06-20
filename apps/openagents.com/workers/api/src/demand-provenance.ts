import { Schema as S } from 'effect'

import {
  AcceptedOutcomesPerKwhEndpoint,
  type AcceptedOutcomesPerKwhProjection,
  projectAcceptedOutcomesPerKwh,
} from './accepted-outcomes-per-kwh'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const DemandProvenanceEndpoint = '/api/public/demand-provenance'
export const DemandProvenanceSchemaVersion =
  'openagents.demand_provenance.v1'

export const DemandProvenanceRemainingBlockerRefs = [
  'blocker.product_promises.demand_provenance_broad_projection_coverage_missing',
] as const

export class DemandProvenanceSurfaceSummary extends S.Class<DemandProvenanceSurfaceSummary>(
  'DemandProvenanceSurfaceSummary',
)({
  caveatRefs: S.Array(S.String),
  externalAcceptedOutcomeCount: S.Int,
  externalDemandClaimAllowed: S.Boolean,
  internalAcceptedOutcomeCount: S.Int,
  promiseRefs: S.Array(S.String),
  rule: S.Literal('no_external_dollar_no_demand_claim'),
  sourceRefs: S.Array(S.String),
  splitState: S.Literal('serving_internal_external_split'),
  surfaceRef: S.String,
  unlabeledAcceptedOutcomeCount: S.Int,
}) {}

export class DemandProvenanceCoverage extends S.Class<DemandProvenanceCoverage>(
  'DemandProvenanceCoverage',
)({
  coveredRevenueBearingSurfaceCount: S.Int,
  remainingSurfaceRefs: S.Array(S.String),
}) {}

export class DemandProvenanceTotals extends S.Class<DemandProvenanceTotals>(
  'DemandProvenanceTotals',
)({
  externalAcceptedOutcomeCount: S.Int,
  internalAcceptedOutcomeCount: S.Int,
  unlabeledAcceptedOutcomeCount: S.Int,
}) {}

export class DemandProvenanceProjection extends S.Class<DemandProvenanceProjection>(
  'DemandProvenanceProjection',
)({
  schemaVersion: S.String,
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
  kind: S.Literal('demand_provenance_public'),
  promiseId: S.Literal('proof.demand_provenance.v1'),
  promiseState: S.Literal('yellow'),
  publicSafe: S.Boolean,
  rule: S.Literal('no_external_dollar_no_demand_claim'),
  surfaceSummaries: S.Array(DemandProvenanceSurfaceSummary),
  totals: DemandProvenanceTotals,
  coverage: DemandProvenanceCoverage,
  externalDemandClaimAllowed: S.Boolean,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  authorityBoundary: S.String,
  unsafeCopy: S.String,
}) {}

export const DemandProvenanceStaleness = liveAtReadStaleness([
  'accepted_outcome_receipt_published',
  'labor_escrow_release_receipt_published',
  'energy_telemetry_ingested',
  'product_promise_registry_updated',
])

const remainingSurfaceRefs = [
  'route:/api/public/pylon-stats',
  'route:/api/public/training/runs/{trainingRunRef}',
  'route:/api/training/leaderboards/*',
  'projection:training.rung_economics_gates',
] as const

const summarizeAcceptedOutcomesPerKwh = (
  projection: AcceptedOutcomesPerKwhProjection,
): DemandProvenanceSurfaceSummary => {
  const internalAcceptedOutcomeCount =
    projection.demandProvenance.internalAcceptedOutcomeCount
  const externalAcceptedOutcomeCount =
    projection.demandProvenance.externalAcceptedOutcomeCount
  const unlabeledAcceptedOutcomeCount = Math.max(
    0,
    projection.acceptedOutcomeCounter.count -
      internalAcceptedOutcomeCount -
      externalAcceptedOutcomeCount,
  )

  return new DemandProvenanceSurfaceSummary({
    caveatRefs: projection.demandProvenance.caveatRefs,
    externalAcceptedOutcomeCount,
    externalDemandClaimAllowed:
      projection.demandProvenance.externalDemandClaimAllowed,
    internalAcceptedOutcomeCount,
    promiseRefs: [
      projection.promiseRef,
      projection.demandProvenance.contractRef,
    ],
    rule: projection.demandProvenance.rule,
    sourceRefs: [
      `route:${AcceptedOutcomesPerKwhEndpoint}`,
      ...projection.sourceRefs,
    ],
    splitState: 'serving_internal_external_split',
    surfaceRef: `route:${AcceptedOutcomesPerKwhEndpoint}`,
    unlabeledAcceptedOutcomeCount,
  })
}

export const projectDemandProvenance = (
  input: {
    acceptedOutcomesPerKwh?: AcceptedOutcomesPerKwhProjection | undefined
    generatedAt?: string | undefined
  } = {},
): DemandProvenanceProjection => {
  const generatedAt = input.generatedAt ?? currentIsoTimestamp()
  const acceptedOutcomesPerKwh =
    input.acceptedOutcomesPerKwh ??
    projectAcceptedOutcomesPerKwh({ generatedAt })
  const surfaceSummaries = [
    summarizeAcceptedOutcomesPerKwh(acceptedOutcomesPerKwh),
  ]
  const totals = surfaceSummaries.reduce(
    (sum, surface) => ({
      externalAcceptedOutcomeCount:
        sum.externalAcceptedOutcomeCount +
        surface.externalAcceptedOutcomeCount,
      internalAcceptedOutcomeCount:
        sum.internalAcceptedOutcomeCount +
        surface.internalAcceptedOutcomeCount,
      unlabeledAcceptedOutcomeCount:
        sum.unlabeledAcceptedOutcomeCount +
        surface.unlabeledAcceptedOutcomeCount,
    }),
    {
      externalAcceptedOutcomeCount: 0,
      internalAcceptedOutcomeCount: 0,
      unlabeledAcceptedOutcomeCount: 0,
    },
  )

  return new DemandProvenanceProjection({
    authorityBoundary:
      'Demand provenance is a public labeling projection only. It grants no revenue, demand, payout, settlement, dispatch, treasury, reporting, or public-claim upgrade authority.',
    blockerRefs: [...DemandProvenanceRemainingBlockerRefs],
    caveatRefs: [
      'caveat.demand_provenance.internal_demand_is_plumbing_not_market',
      'caveat.demand_provenance.no_external_dollar_no_demand_claim',
      'caveat.demand_provenance.partial_coverage_not_green',
    ],
    coverage: new DemandProvenanceCoverage({
      coveredRevenueBearingSurfaceCount: surfaceSummaries.length,
      remainingSurfaceRefs: [...remainingSurfaceRefs],
    }),
    externalDemandClaimAllowed: totals.externalAcceptedOutcomeCount > 0,
    generatedAt,
    kind: 'demand_provenance_public',
    promiseId: 'proof.demand_provenance.v1',
    promiseState: 'yellow',
    publicSafe: true,
    rule: 'no_external_dollar_no_demand_claim',
    schemaVersion: DemandProvenanceSchemaVersion,
    staleness: DemandProvenanceStaleness,
    surfaceSummaries,
    totals: new DemandProvenanceTotals(totals),
    unsafeCopy:
      'Do not present internal, first-party, operator-staged, or unlabeled demand as external market demand. Do not claim demand provenance is green until the remaining revenue-bearing projections carry the same typed split and a receipt-first transition is recorded.',
  })
}
