import { Schema as S } from 'effect'

import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const AcceptedOutcomesPerKwhEndpoint =
  '/api/public/metrics/accepted-outcomes-per-kwh'
export const AcceptedOutcomesPerKwhSchemaVersion =
  'openagents.metrics.accepted_outcomes_per_kwh.v1'
export const AcceptedOutcomesPerKwhRequiredMeasuredDatapoints = 2
export const AcceptedOutcomesPerKwhStaleness = liveAtReadStaleness([
  'accepted_outcome_receipt_published',
  'labor_escrow_release_receipt_published',
  'energy_telemetry_ingested',
  'product_promise_registry_updated',
])

// Demand provenance (proof.demand_provenance.v1): every revenue-bearing public
// number must label whether the demand behind it was internal (first-party,
// operator-staged, ablation/sweep/conformance plumbing) or external (a third
// party paying real dollars). The rule is: no external dollar, no demand claim.
export const AcceptedOutcomesPerKwhDemandProvenanceKind = S.Literals([
  'internal',
  'external',
])
export type AcceptedOutcomesPerKwhDemandProvenanceKind = S.Schema.Type<
  typeof AcceptedOutcomesPerKwhDemandProvenanceKind
>

export class AcceptedOutcomesPerKwhDemandProvenance extends S.Class<AcceptedOutcomesPerKwhDemandProvenance>(
  'AcceptedOutcomesPerKwhDemandProvenance',
)({
  // 'internal' = first-party / operator-staged demand (plumbing proof, not
  // market proof). 'external' = a third party paid real dollars for the work.
  kind: AcceptedOutcomesPerKwhDemandProvenanceKind,
  // Why this datapoint carries the kind it does, in public-safe terms.
  rationale: S.String,
  // Refs that substantiate the provenance label (e.g. evidence bundle showing
  // the job was operator-staged, or an external buyer settlement receipt).
  evidenceRefs: S.Array(S.String),
}) {}

export class AcceptedOutcomesPerKwhEnergyModel extends S.Class<AcceptedOutcomesPerKwhEnergyModel>(
  'AcceptedOutcomesPerKwhEnergyModel',
)({
  method: S.String,
  modeledPowerKw: S.Number,
  wallClockSeconds: S.Number,
  wallClockHours: S.Number,
  energyKwh: S.Number,
  assumptionRefs: S.Array(S.String),
}) {}

export class AcceptedOutcomesPerKwhMeasuredEnergyTelemetry extends S.Class<AcceptedOutcomesPerKwhMeasuredEnergyTelemetry>(
  'AcceptedOutcomesPerKwhMeasuredEnergyTelemetry',
)({
  measurementMethod: S.String,
  measuredEnergyWh: S.Number,
  measuredEnergyKwh: S.Number,
  meterEvidenceRefs: S.Array(S.String),
  deviceRef: S.String,
}) {}

export class AcceptedOutcomesPerKwhDatapoint extends S.Class<AcceptedOutcomesPerKwhDatapoint>(
  'AcceptedOutcomesPerKwhDatapoint',
)({
  datapointId: S.String,
  label: S.String,
  windowStart: S.String,
  windowEnd: S.String,
  acceptedOutcomeCount: S.Int,
  acceptedOutcomeEvidenceState: S.Literal('receipt_backed'),
  acceptedOutcomeRefs: S.Array(S.String),
  verificationRefs: S.Array(S.String),
  settlementReceiptRefs: S.Array(S.String),
  demandProvenance: AcceptedOutcomesPerKwhDemandProvenance,
  energyEvidenceState: S.Literals(['modeled', 'measured']),
  energyModel: S.NullOr(AcceptedOutcomesPerKwhEnergyModel),
  measuredEnergyTelemetry: S.NullOr(AcceptedOutcomesPerKwhMeasuredEnergyTelemetry),
  acceptedOutcomesPerKwh: S.Number,
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
}) {}

export class AcceptedOutcomesPerKwhMeasuredTelemetryInput extends S.Class<AcceptedOutcomesPerKwhMeasuredTelemetryInput>(
  'AcceptedOutcomesPerKwhMeasuredTelemetryInput',
)({
  acceptedOutcomeCount: S.Int,
  acceptedOutcomeRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  datapointId: S.String,
  demandProvenance: AcceptedOutcomesPerKwhDemandProvenance,
  deviceRef: S.String,
  label: S.String,
  measuredEnergyWh: S.Number,
  measurementMethod: S.String,
  meterEvidenceRefs: S.Array(S.String),
  settlementReceiptRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  verificationRefs: S.Array(S.String),
  windowEnd: S.String,
  windowStart: S.String,
}) {}

export class AcceptedOutcomesPerKwhGate extends S.Class<AcceptedOutcomesPerKwhGate>(
  'AcceptedOutcomesPerKwhGate',
)({
  state: S.Literals(['yellow', 'green']),
  currentMeasuredDatapointCount: S.Int,
  modeledFigurePublicationAllowed: S.Boolean,
  measuredDatapointShortfall: S.Int,
  measuredFigurePublicationAllowed: S.Boolean,
  measuredTelemetryGateSatisfied: S.Boolean,
  greenGateSatisfied: S.Boolean,
  requiredMeasuredDatapointCount: S.Int,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
}) {}

export class AcceptedOutcomesPerKwhProjection extends S.Class<AcceptedOutcomesPerKwhProjection>(
  'AcceptedOutcomesPerKwhProjection',
)({
  schemaVersion: S.String,
  generatedAt: S.String,
  metricId: S.Literal('metrics.accepted_outcomes_per_kwh.v1'),
  definitionRef: S.String,
  promiseRef: S.String,
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('instrumented_modeled_seed'),
  statusLabel: S.String,
  acceptedOutcomeCounter: S.Struct({
    count: S.Int,
    evidenceState: S.Literal('receipt_backed'),
    sourceRefs: S.Array(S.String),
  }),
  energyAccounting: S.Struct({
    evidenceState: S.Literals(['modeled_seed', 'modeled_and_measured']),
    measuredDatapointCount: S.Int,
    modeledDatapointCount: S.Int,
    sourceRefs: S.Array(S.String),
  }),
  // proof.demand_provenance.v1: typed internal/external split so this
  // revenue-bearing projection never presents internal demand as market demand.
  demandProvenance: S.Struct({
    contractRef: S.Literal('promise:proof.demand_provenance.v1'),
    internalAcceptedOutcomeCount: S.Int,
    externalAcceptedOutcomeCount: S.Int,
    // True only when at least one external (real-dollar) accepted outcome
    // backs the metric. Until then no market-demand claim may be made.
    externalDemandClaimAllowed: S.Boolean,
    rule: S.Literal('no_external_dollar_no_demand_claim'),
    caveatRefs: S.Array(S.String),
  }),
  gate: AcceptedOutcomesPerKwhGate,
  datapoints: S.Array(AcceptedOutcomesPerKwhDatapoint),
  authorityBoundary: S.String,
  unsafeCopy: S.String,
  sourceRefs: S.Array(S.String),
}) {}

const round = (value: number, places = 6): number => {
  const factor = 10 ** places
  return Math.round(value * factor) / factor
}

const ACCEPTED_LABOR_4777 = {
  acceptanceAt: '2026-06-14T02:36:58.442Z',
  resultAt: '2026-06-14T03:06:15.399Z',
  acceptedOutcomeRefs: [
    'work_result.public.788b59de-8ee9-4029-9f5b-c6cf23dc668d',
    'closeout.public.pylon.labor_market.fe1ee748e332a9b9ff7f1e0b',
  ],
  assumptionRefs: [
    'assumption.ao_kwh.seed.provider_power_100w_modeled_not_measured',
    'assumption.ao_kwh.seed.acceptance_to_result_wall_clock',
  ],
  modeledPowerKw: 0.1,
  sourceRefs: [
    'docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md',
    'https://openagents.com/api/forum/work-requests/b74bb55c-849c-43a3-b8d9-9a741316b528',
    'work_request:b74bb55c-849c-43a3-b8d9-9a741316b528',
    'result.public.pylon.labor_market.32751b623cbf3e01071182f7bc52b642d944b345404524871ffe8f5c03e905dd',
  ],
  settlementReceiptRefs: [
    'receipt.labor_escrow.reserve.b74bb55c-849c-43a3-b8d9-9a741316b528.quote.public.pylon.labor_market.b97f312478504e9df212e333',
    'receipt.labor_escrow.release.b74bb55c-849c-43a3-b8d9-9a741316b528.quote.public.pylon.labor_market.b97f312478504e9df212e333',
  ],
  verificationRefs: ['verdict.public.pylon.labor_market.b74bb55c.bun_test.pass'],
} as const

export const modeledLabor4777AoKwhDatapoint =
  (): AcceptedOutcomesPerKwhDatapoint => {
    const wallClockSeconds =
      (Date.parse(ACCEPTED_LABOR_4777.resultAt) -
        Date.parse(ACCEPTED_LABOR_4777.acceptanceAt)) /
      1000
    const wallClockHours = wallClockSeconds / 3600
    const energyKwh = round(
      ACCEPTED_LABOR_4777.modeledPowerKw * wallClockHours,
    )
    const acceptedOutcomesPerKwh = round(1 / energyKwh, 3)

    return new AcceptedOutcomesPerKwhDatapoint({
      acceptedOutcomeCount: 1,
      acceptedOutcomeEvidenceState: 'receipt_backed',
      acceptedOutcomeRefs: [...ACCEPTED_LABOR_4777.acceptedOutcomeRefs],
      acceptedOutcomesPerKwh,
      caveatRefs: [
        'caveat.ao_kwh.seed.energy_modeled_not_measured',
        'caveat.ao_kwh.seed.single_labor_job_not_comparable',
        'caveat.ao_kwh.seed.provider_power_assumption_replace_with_telemetry',
      ],
      datapointId: 'ao_kwh.seed.labor_4777.modeled',
      demandProvenance: new AcceptedOutcomesPerKwhDemandProvenance({
        evidenceRefs: [
          'docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md',
          'promise:provider.compliant_usage_labor.v1',
        ],
        kind: 'internal',
        rationale:
          'The first settled labor job (#4777) was operator-staged and settled on the internal credit ledger (1 sat), not driven by an external paying customer over the reliable-tips ladder. It is internal demand (plumbing proof), not market demand.',
      }),
      energyEvidenceState: 'modeled',
      energyModel: new AcceptedOutcomesPerKwhEnergyModel({
        assumptionRefs: [...ACCEPTED_LABOR_4777.assumptionRefs],
        energyKwh,
        method: 'modeled_power_kw_times_acceptance_to_result_wall_clock',
        modeledPowerKw: ACCEPTED_LABOR_4777.modeledPowerKw,
        wallClockHours: round(wallClockHours),
        wallClockSeconds: round(wallClockSeconds, 3),
      }),
      measuredEnergyTelemetry: null,
      label: 'First settled labor job (#4777), modeled energy seed',
      settlementReceiptRefs: [...ACCEPTED_LABOR_4777.settlementReceiptRefs],
      sourceRefs: [...ACCEPTED_LABOR_4777.sourceRefs],
      verificationRefs: [...ACCEPTED_LABOR_4777.verificationRefs],
      windowEnd: ACCEPTED_LABOR_4777.resultAt,
      windowStart: ACCEPTED_LABOR_4777.acceptanceAt,
    })
  }

export const measuredTelemetryAoKwhDatapoint = (
  input: AcceptedOutcomesPerKwhMeasuredTelemetryInput,
): AcceptedOutcomesPerKwhDatapoint => {
  if (input.acceptedOutcomeCount <= 0) {
    throw new RangeError(
      'AO/kWh measured telemetry requires at least one accepted outcome',
    )
  }
  if (input.measuredEnergyWh <= 0) {
    throw new RangeError(
      'AO/kWh measured telemetry requires measuredEnergyWh > 0',
    )
  }
  const measuredEnergyKwh = round(input.measuredEnergyWh / 1000)
  const acceptedOutcomesPerKwh = round(
    input.acceptedOutcomeCount / measuredEnergyKwh,
    3,
  )

  return new AcceptedOutcomesPerKwhDatapoint({
    acceptedOutcomeCount: input.acceptedOutcomeCount,
    acceptedOutcomeEvidenceState: 'receipt_backed',
    acceptedOutcomeRefs: [...input.acceptedOutcomeRefs],
    acceptedOutcomesPerKwh,
    caveatRefs: [...input.caveatRefs],
    datapointId: input.datapointId,
    demandProvenance: input.demandProvenance,
    energyEvidenceState: 'measured',
    energyModel: null,
    label: input.label,
    measuredEnergyTelemetry: new AcceptedOutcomesPerKwhMeasuredEnergyTelemetry({
      deviceRef: input.deviceRef,
      measuredEnergyKwh,
      measuredEnergyWh: input.measuredEnergyWh,
      measurementMethod: input.measurementMethod,
      meterEvidenceRefs: [...input.meterEvidenceRefs],
    }),
    settlementReceiptRefs: [...input.settlementReceiptRefs],
    sourceRefs: [...input.sourceRefs],
    verificationRefs: [...input.verificationRefs],
    windowEnd: input.windowEnd,
    windowStart: input.windowStart,
  })
}

export const projectAcceptedOutcomesPerKwh = (
  input: {
    generatedAt?: string | undefined
    measuredTelemetry?: ReadonlyArray<AcceptedOutcomesPerKwhMeasuredTelemetryInput>
  } = {},
): AcceptedOutcomesPerKwhProjection => {
  const measuredDatapoints = (input.measuredTelemetry ?? []).map(
    measuredTelemetryAoKwhDatapoint,
  )
  const datapoints = [modeledLabor4777AoKwhDatapoint(), ...measuredDatapoints]
  const measuredDatapointCount = datapoints.filter(
    datapoint => datapoint.energyEvidenceState === 'measured',
  ).length
  const measuredDatapointShortfall = Math.max(
    0,
    AcceptedOutcomesPerKwhRequiredMeasuredDatapoints - measuredDatapointCount,
  )
  const measuredTelemetryGateSatisfied =
    measuredDatapointCount >= AcceptedOutcomesPerKwhRequiredMeasuredDatapoints

  const internalAcceptedOutcomeCount = datapoints
    .filter(datapoint => datapoint.demandProvenance.kind === 'internal')
    .reduce((sum, datapoint) => sum + datapoint.acceptedOutcomeCount, 0)
  const externalAcceptedOutcomeCount = datapoints
    .filter(datapoint => datapoint.demandProvenance.kind === 'external')
    .reduce((sum, datapoint) => sum + datapoint.acceptedOutcomeCount, 0)

  return new AcceptedOutcomesPerKwhProjection({
    acceptedOutcomeCounter: {
      count: datapoints.reduce(
        (sum, datapoint) => sum + datapoint.acceptedOutcomeCount,
        0,
      ),
      evidenceState: 'receipt_backed',
      sourceRefs: datapoints.flatMap(datapoint => datapoint.acceptedOutcomeRefs),
    },
    authorityBoundary:
      'AO/kWh is a public efficiency metric projection only. It grants no assignment, payout, settlement, dispatch, treasury, energy-market, investment, or grid-operations authority.',
    datapoints,
    definitionRef: 'docs/metrics/2026-06-15-accepted-outcomes-per-kwh.md',
    demandProvenance: {
      caveatRefs: [
        'caveat.demand_provenance.internal_demand_is_plumbing_not_market',
        'caveat.demand_provenance.no_external_dollar_no_demand_claim',
      ],
      contractRef: 'promise:proof.demand_provenance.v1',
      externalAcceptedOutcomeCount,
      externalDemandClaimAllowed: externalAcceptedOutcomeCount > 0,
      internalAcceptedOutcomeCount,
      rule: 'no_external_dollar_no_demand_claim',
    },
    energyAccounting: {
      evidenceState:
        measuredDatapointCount > 0 ? 'modeled_and_measured' : 'modeled_seed',
      measuredDatapointCount,
      modeledDatapointCount: datapoints.filter(
        datapoint => datapoint.energyEvidenceState === 'modeled',
      ).length,
      sourceRefs: datapoints.flatMap(datapoint => [
        ...(datapoint.energyModel?.assumptionRefs ?? []),
        ...(datapoint.measuredEnergyTelemetry?.meterEvidenceRefs ?? []),
        ...datapoint.sourceRefs,
      ]),
    },
    gate: new AcceptedOutcomesPerKwhGate({
      blockerRefs: measuredTelemetryGateSatisfied
        ? ['blocker.product_promises.ao_kwh_green_transition_receipt_missing']
        : [
            'blocker.product_promises.ao_kwh_measured_datapoints_missing',
            'blocker.product_promises.ao_kwh_requires_two_measured_datapoints',
          ],
      caveatRefs: [
        'caveat.ao_kwh.figures_must_label_modeled_vs_measured',
        'caveat.ao_kwh.seed_not_a_ranking_or_efficiency_claim',
      ],
      currentMeasuredDatapointCount: measuredDatapointCount,
      greenGateSatisfied: false,
      measuredDatapointShortfall,
      measuredFigurePublicationAllowed: measuredTelemetryGateSatisfied,
      measuredTelemetryGateSatisfied,
      modeledFigurePublicationAllowed: true,
      requiredMeasuredDatapointCount:
        AcceptedOutcomesPerKwhRequiredMeasuredDatapoints,
      state: 'yellow',
    }),
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    metricId: 'metrics.accepted_outcomes_per_kwh.v1',
    promiseRef: 'promise:metrics.accepted_outcomes_per_kwh.v1',
    schemaVersion: AcceptedOutcomesPerKwhSchemaVersion,
    sourceRefs: [
      'docs/metrics/2026-06-15-accepted-outcomes-per-kwh.md',
      'docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md',
      'apps/openagents.com/workers/api/src/accepted-outcomes-per-kwh.ts',
    ],
    staleness: AcceptedOutcomesPerKwhStaleness,
    status: 'instrumented_modeled_seed',
    statusLabel:
      `AO/kWh has one receipt-backed modeled seed datapoint and ${measuredDatapointCount} of ${AcceptedOutcomesPerKwhRequiredMeasuredDatapoints} required measured datapoints.`,
    unsafeCopy:
      'Do not describe the seed datapoint as measured, broadly representative, a ranking, a provider efficiency claim, investment advice, grid advice, or proof that production energy routing is live. Do not present AO/kWh as green or measured until at least two real telemetry datapoints are published with evidence-state labels and transition receipts. Do not present the internal, operator-staged accepted outcome as external market demand or revenue: no external dollar, no demand claim.',
  })
}
