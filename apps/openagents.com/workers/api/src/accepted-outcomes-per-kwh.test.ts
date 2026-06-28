import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AcceptedOutcomesPerKwhEndpoint,
  AcceptedOutcomesPerKwhMeasuredTelemetryInput,
  AcceptedOutcomesPerKwhProjection,
  AcceptedOutcomesPerKwhRequiredMeasuredDatapoints,
  measuredTelemetryAoKwhDatapoint,
  modeledLabor4777AoKwhDatapoint,
  projectAcceptedOutcomesPerKwh,
} from './accepted-outcomes-per-kwh'
import { handleAcceptedOutcomesPerKwhApi } from './accepted-outcomes-per-kwh-routes'
import { openAgentsOpenApiDocument } from './openagents-openapi'

type AcceptedOutcomesPerKwhBody = Readonly<{
  datapoints: ReadonlyArray<unknown>
  metricId: string
}>

const measuredTelemetry = (
  datapointId: string,
  measuredEnergyWh: number,
) =>
  new AcceptedOutcomesPerKwhMeasuredTelemetryInput({
    acceptedOutcomeCount: 2,
    acceptedOutcomeRefs: [
      `accepted_outcome.public.${datapointId}.one`,
      `accepted_outcome.public.${datapointId}.two`,
    ],
    caveatRefs: ['caveat.ao_kwh.measured_single_device_window'],
    datapointId,
    demandProvenance: {
      evidenceRefs: [`receipt.public.${datapointId}.customer_payment`],
      kind: 'external',
      rationale:
        'This fixture datapoint represents a third-party paid accepted-outcome window.',
    },
    deviceRef: `device.public.${datapointId}`,
    label: `Measured fixture ${datapointId}`,
    measuredEnergyWh,
    measurementMethod: 'wall_meter_wh_for_bounded_accepted_outcome_window',
    meterEvidenceRefs: [`meter.public.${datapointId}.wh`],
    settlementReceiptRefs: [`settlement.public.${datapointId}`],
    sourceRefs: [`source.public.${datapointId}`],
    verificationRefs: [`verdict.public.${datapointId}`],
    windowEnd: '2026-06-28T01:30:00.000Z',
    windowStart: '2026-06-28T01:00:00.000Z',
  })

describe('Accepted Outcomes per kWh metric', () => {
  test('publishes a receipt-backed modeled seed datapoint with caveats', () => {
    const projection = projectAcceptedOutcomesPerKwh({
      generatedAt: '2026-06-15T22:15:00.000Z',
    })

    expect(
      S.decodeUnknownSync(AcceptedOutcomesPerKwhProjection)(projection),
    ).toEqual(projection)
    expect(projection.status).toBe('instrumented_modeled_seed')
    expect(projection.gate.state).toBe('yellow')
    expect(projection.gate.greenGateSatisfied).toBe(false)
    expect(projection.gate.modeledFigurePublicationAllowed).toBe(true)
    expect(projection.gate.measuredFigurePublicationAllowed).toBe(false)
    expect(projection.gate.requiredMeasuredDatapointCount).toBe(
      AcceptedOutcomesPerKwhRequiredMeasuredDatapoints,
    )
    expect(projection.gate.currentMeasuredDatapointCount).toBe(0)
    expect(projection.gate.measuredDatapointShortfall).toBe(2)
    expect(projection.gate.measuredTelemetryGateSatisfied).toBe(false)
    expect(projection.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
    })
    expect(projection.acceptedOutcomeCounter.count).toBe(1)
    expect(projection.energyAccounting.measuredDatapointCount).toBe(0)
    expect(projection.energyAccounting.modeledDatapointCount).toBe(1)
    expect(projection.datapoints[0]?.energyEvidenceState).toBe('modeled')
    expect(projection.datapoints[0]?.settlementReceiptRefs).toContain(
      'receipt.labor_escrow.release.b74bb55c-849c-43a3-b8d9-9a741316b528.quote.public.pylon.labor_market.b97f312478504e9df212e333',
    )
    expect(projection.unsafeCopy).toContain(
      'Do not describe the seed datapoint as measured',
    )
    expect(projection.unsafeCopy).toContain(
      'until at least two real telemetry datapoints are published',
    )
  })

  test('exposes the measured-telemetry green threshold without upgrading the modeled seed', () => {
    const projection = projectAcceptedOutcomesPerKwh({
      generatedAt: '2026-06-15T22:15:00.000Z',
    })

    expect(projection.gate.requiredMeasuredDatapointCount).toBe(2)
    expect(projection.gate.currentMeasuredDatapointCount).toBe(
      projection.energyAccounting.measuredDatapointCount,
    )
    expect(projection.gate.currentMeasuredDatapointCount).toBeLessThan(
      projection.gate.requiredMeasuredDatapointCount,
    )
    expect(projection.gate.measuredDatapointShortfall).toBe(
      projection.gate.requiredMeasuredDatapointCount -
        projection.gate.currentMeasuredDatapointCount,
    )
    expect(projection.gate.blockerRefs).toContain(
      'blocker.product_promises.ao_kwh_requires_two_measured_datapoints',
    )
    expect(projection.statusLabel).toContain('0 of 2 required measured datapoints')
  })

  test('ingests measured per-device telemetry into a measured AO/kWh datapoint', () => {
    const input = measuredTelemetry('ao_kwh.measured.fixture_one', 250)
    const datapoint = measuredTelemetryAoKwhDatapoint(input)

    expect(datapoint.energyEvidenceState).toBe('measured')
    expect(datapoint.energyModel).toBeNull()
    expect(datapoint.measuredEnergyTelemetry).toMatchObject({
      deviceRef: 'device.public.ao_kwh.measured.fixture_one',
      measuredEnergyKwh: 0.25,
      measuredEnergyWh: 250,
      measurementMethod: 'wall_meter_wh_for_bounded_accepted_outcome_window',
    })
    expect(datapoint.measuredEnergyTelemetry?.meterEvidenceRefs).toContain(
      'meter.public.ao_kwh.measured.fixture_one.wh',
    )
    expect(datapoint.acceptedOutcomesPerKwh).toBe(8)
  })

  test('projects measured datapoints and satisfies the measured telemetry gate only at two measured datapoints', () => {
    const oneMeasured = projectAcceptedOutcomesPerKwh({
      generatedAt: '2026-06-28T01:45:00.000Z',
      measuredTelemetry: [measuredTelemetry('ao_kwh.measured.fixture_one', 250)],
    })
    const twoMeasured = projectAcceptedOutcomesPerKwh({
      generatedAt: '2026-06-28T01:45:00.000Z',
      measuredTelemetry: [
        measuredTelemetry('ao_kwh.measured.fixture_one', 250),
        measuredTelemetry('ao_kwh.measured.fixture_two', 500),
      ],
    })

    expect(oneMeasured.datapoints).toHaveLength(2)
    expect(oneMeasured.energyAccounting.evidenceState).toBe(
      'modeled_and_measured',
    )
    expect(oneMeasured.energyAccounting.measuredDatapointCount).toBe(1)
    expect(oneMeasured.gate.measuredTelemetryGateSatisfied).toBe(false)
    expect(oneMeasured.gate.measuredFigurePublicationAllowed).toBe(false)
    expect(oneMeasured.gate.measuredDatapointShortfall).toBe(1)

    expect(twoMeasured.datapoints).toHaveLength(3)
    expect(twoMeasured.energyAccounting.measuredDatapointCount).toBe(2)
    expect(twoMeasured.energyAccounting.modeledDatapointCount).toBe(1)
    expect(twoMeasured.gate.measuredTelemetryGateSatisfied).toBe(true)
    expect(twoMeasured.gate.measuredFigurePublicationAllowed).toBe(true)
    expect(twoMeasured.gate.measuredDatapointShortfall).toBe(0)
    expect(twoMeasured.gate.blockerRefs).not.toContain(
      'blocker.product_promises.ao_kwh_requires_two_measured_datapoints',
    )
    expect(twoMeasured.gate.blockerRefs).toContain(
      'blocker.product_promises.ao_kwh_green_transition_receipt_missing',
    )
    expect(twoMeasured.gate.greenGateSatisfied).toBe(false)
    expect(twoMeasured.gate.state).toBe('yellow')
  })

  test('labels demand provenance internal/external and forbids unlabeled market-demand claims', () => {
    const projection = projectAcceptedOutcomesPerKwh({
      generatedAt: '2026-06-15T22:15:00.000Z',
    })

    // proof.demand_provenance.v1: the revenue-bearing projection carries a
    // typed internal/external split serving real data.
    expect(projection.demandProvenance.contractRef).toBe(
      'promise:proof.demand_provenance.v1',
    )
    expect(projection.demandProvenance.rule).toBe(
      'no_external_dollar_no_demand_claim',
    )
    // The only accepted outcome (#4777) was operator-staged → internal demand.
    expect(projection.demandProvenance.internalAcceptedOutcomeCount).toBe(1)
    expect(projection.demandProvenance.externalAcceptedOutcomeCount).toBe(0)
    // No external dollar yet → no market-demand claim allowed.
    expect(projection.demandProvenance.externalDemandClaimAllowed).toBe(false)

    // Every datapoint must carry a provenance kind.
    for (const datapoint of projection.datapoints) {
      expect(['internal', 'external']).toContain(
        datapoint.demandProvenance.kind,
      )
    }
    expect(projection.datapoints[0]?.demandProvenance.kind).toBe('internal')

    // The internal/external counts must reconcile to the accepted-outcome total
    // so an aggregate can never silently include unlabeled demand.
    expect(
      projection.demandProvenance.internalAcceptedOutcomeCount +
        projection.demandProvenance.externalAcceptedOutcomeCount,
    ).toBe(projection.acceptedOutcomeCounter.count)

    // Copy gate: the unsafeCopy must forbid presenting internal demand as market
    // demand under the no-external-dollar rule.
    expect(projection.unsafeCopy).toContain('no external dollar, no demand claim')
  })

  test('computes the modeled datapoint from public acceptance→result timing', () => {
    const datapoint = modeledLabor4777AoKwhDatapoint()

    expect(datapoint.windowStart).toBe('2026-06-14T02:36:58.442Z')
    expect(datapoint.windowEnd).toBe('2026-06-14T03:06:15.399Z')
    expect(datapoint.energyModel).not.toBeNull()
    expect(datapoint.energyModel?.method).toBe(
      'modeled_power_kw_times_acceptance_to_result_wall_clock',
    )
    expect(datapoint.energyModel?.modeledPowerKw).toBe(0.1)
    expect(datapoint.energyModel?.wallClockSeconds).toBe(1756.957)
    expect(datapoint.energyModel?.energyKwh).toBe(0.048804)
    expect(datapoint.acceptedOutcomesPerKwh).toBe(20.49)
  })

  test('serves the public metric route as no-store JSON', async () => {
    const response = await Effect.runPromise(
      handleAcceptedOutcomesPerKwhApi(
        new Request(`https://openagents.com${AcceptedOutcomesPerKwhEndpoint}`),
      ),
    )
    const body = (await response.json()) as AcceptedOutcomesPerKwhBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.metricId).toBe('metrics.accepted_outcomes_per_kwh.v1')
    expect(body.datapoints).toHaveLength(1)
  })

  test('documents the public metric endpoint in OpenAPI', async () => {
    const document = await Effect.runPromise(openAgentsOpenApiDocument())

    expect(
      (
        document.paths[AcceptedOutcomesPerKwhEndpoint] as
          | { get?: unknown }
          | undefined
      )?.get,
    ).toEqual(
      expect.objectContaining({
        operationId: 'getAcceptedOutcomesPerKwhMetric',
      }),
    )
    expect(
      (document.components as { schemas: Record<string, unknown> }).schemas,
    ).toHaveProperty(
      'AcceptedOutcomesPerKwhProjection',
    )
  })
})
