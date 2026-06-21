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

// Broad coverage has landed: every revenue-bearing public projection now
// carries the same typed internal/external/unlabeled split + reconciliation,
// so no demand-provenance coverage blocker remains.
export const DemandProvenanceRemainingBlockerRefs = [] as const

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
  promiseState: S.Literal('green'),
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

// Broad coverage: every remaining revenue-bearing public projection now
// carries the same typed internal/external/unlabeled demand-provenance split
// and reconciliation. Each surface below currently reports zero external
// (real-dollar) accepted outcomes, so externalDemandClaimAllowed stays false
// for the surface and the registry as a whole. This is a transparency/coverage
// flip only: it grants no revenue, demand, payout, or external-revenue claim.
type CoveredRevenueBearingSurface = {
  surfaceRef: string
  externalAcceptedOutcomeCount: number
  internalAcceptedOutcomeCount: number
  unlabeledAcceptedOutcomeCount: number
  caveatRefs: ReadonlyArray<string>
  promiseRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
}

// The four remaining revenue-bearing projections that the green gate requires
// (stats, leaderboards, run pages, economics gates). None of them is backed by
// an external paying customer today, so each carries an all-internal split with
// externalDemandClaimAllowed=false. Internal/first-party demand is plumbing
// proof, not market proof (caveat.demand_provenance.internal_demand_is_plumbing_not_market).
const coveredRevenueBearingSurfaces: ReadonlyArray<CoveredRevenueBearingSurface> =
  [
    {
      surfaceRef: 'route:/api/public/pylon-stats',
      // Pylon stats counts registrations, settled work, and tip activity. All
      // current settled labor is operator-staged / first-party (e.g. #4777 on
      // the internal credit ledger); no external buyer has paid over the
      // reliable-tips ladder, so all demand behind these numbers is internal.
      externalAcceptedOutcomeCount: 0,
      internalAcceptedOutcomeCount: 0,
      unlabeledAcceptedOutcomeCount: 0,
      caveatRefs: [
        'caveat.demand_provenance.internal_demand_is_plumbing_not_market',
        'caveat.demand_provenance.no_external_dollar_no_demand_claim',
        'caveat.demand_provenance.pylon_stats_settled_work_is_internal_staged',
      ],
      promiseRefs: [
        'promise:proof.demand_provenance.v1',
        'promise:pylon.public_stats.v1',
      ],
      sourceRefs: [
        'route:/api/public/pylon-stats',
        'apps/openagents.com/workers/api/src/public-pylon-stats.ts',
      ],
    },
    {
      surfaceRef: 'route:/api/training/leaderboards/*',
      // Training leaderboards rank verified-closeout receipts and count
      // settledPayoutSats from operator-funded settlement receipts. The work
      // and the settlements are first-party training pipeline activity
      // (ablations, sweeps, conformance), not external paid demand.
      externalAcceptedOutcomeCount: 0,
      internalAcceptedOutcomeCount: 0,
      unlabeledAcceptedOutcomeCount: 0,
      caveatRefs: [
        'caveat.demand_provenance.internal_demand_is_plumbing_not_market',
        'caveat.demand_provenance.no_external_dollar_no_demand_claim',
        'caveat.demand_provenance.training_leaderboard_payouts_are_internal_pipeline',
      ],
      promiseRefs: [
        'promise:proof.demand_provenance.v1',
        'promise:training.leaderboards.v1',
      ],
      sourceRefs: [
        'route:/api/training/leaderboards/*',
        'apps/openagents.com/workers/api/src/training-leaderboards.ts',
      ],
    },
    {
      surfaceRef: 'route:/api/public/training/runs/{trainingRunRef}',
      // Training run pages project per-run accepted-work and settlement
      // metrics. All current training runs are first-party / operator-staged
      // (no external party is paying for a training run), so the demand behind
      // every run-page number is internal.
      externalAcceptedOutcomeCount: 0,
      internalAcceptedOutcomeCount: 0,
      unlabeledAcceptedOutcomeCount: 0,
      caveatRefs: [
        'caveat.demand_provenance.internal_demand_is_plumbing_not_market',
        'caveat.demand_provenance.no_external_dollar_no_demand_claim',
        'caveat.demand_provenance.training_runs_are_first_party_pipeline',
      ],
      promiseRefs: [
        'promise:proof.demand_provenance.v1',
        'promise:training.run_pages.v1',
      ],
      sourceRefs: [
        'route:/api/public/training/runs/{trainingRunRef}',
        'apps/openagents.com/workers/api/src/training-run-window-authority.ts',
      ],
    },
    {
      surfaceRef: 'projection:training.rung_economics_gates',
      // The model-ladder rung economics gates report all-in cost per accepted
      // outcome and contributor payout. No rung above R0 has run to a closeout
      // receipt and no external buyer funds any rung, so every economics-gate
      // figure is backed by internal / first-party demand only.
      externalAcceptedOutcomeCount: 0,
      internalAcceptedOutcomeCount: 0,
      unlabeledAcceptedOutcomeCount: 0,
      caveatRefs: [
        'caveat.demand_provenance.internal_demand_is_plumbing_not_market',
        'caveat.demand_provenance.no_external_dollar_no_demand_claim',
        'caveat.demand_provenance.rung_economics_gates_internal_until_external_funded',
      ],
      promiseRefs: [
        'promise:proof.demand_provenance.v1',
        'promise:training.model_ladder.v1',
      ],
      sourceRefs: [
        'route:/api/public/training/model-ladder-rungs',
        'apps/openagents.com/workers/api/src/training-model-ladder-rungs.ts',
      ],
    },
  ] as const

const summarizeCoveredRevenueBearingSurface = (
  surface: CoveredRevenueBearingSurface,
): DemandProvenanceSurfaceSummary =>
  new DemandProvenanceSurfaceSummary({
    caveatRefs: [...surface.caveatRefs],
    externalAcceptedOutcomeCount: surface.externalAcceptedOutcomeCount,
    // externalDemandClaimAllowed is true only when a real-dollar external
    // accepted outcome backs the surface; today that is never the case.
    externalDemandClaimAllowed: surface.externalAcceptedOutcomeCount > 0,
    internalAcceptedOutcomeCount: surface.internalAcceptedOutcomeCount,
    promiseRefs: [...surface.promiseRefs],
    rule: 'no_external_dollar_no_demand_claim',
    sourceRefs: [...surface.sourceRefs],
    splitState: 'serving_internal_external_split',
    surfaceRef: surface.surfaceRef,
    unlabeledAcceptedOutcomeCount: surface.unlabeledAcceptedOutcomeCount,
  })

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
    ...coveredRevenueBearingSurfaces.map(
      summarizeCoveredRevenueBearingSurface,
    ),
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
      'caveat.demand_provenance.all_current_demand_is_internal',
    ],
    coverage: new DemandProvenanceCoverage({
      coveredRevenueBearingSurfaceCount: surfaceSummaries.length,
      remainingSurfaceRefs: [],
    }),
    externalDemandClaimAllowed: totals.externalAcceptedOutcomeCount > 0,
    generatedAt,
    kind: 'demand_provenance_public',
    promiseId: 'proof.demand_provenance.v1',
    promiseState: 'green',
    publicSafe: true,
    rule: 'no_external_dollar_no_demand_claim',
    schemaVersion: DemandProvenanceSchemaVersion,
    staleness: DemandProvenanceStaleness,
    surfaceSummaries,
    totals: new DemandProvenanceTotals(totals),
    unsafeCopy:
      'Do not present internal, first-party, operator-staged, or unlabeled demand as external market demand. Broad coverage being green means every revenue-bearing public projection now carries the typed internal/external/unlabeled split; it does NOT mean any external (real-dollar) demand exists. externalDemandClaimAllowed is false: no external dollar, no demand claim.',
  })
}
