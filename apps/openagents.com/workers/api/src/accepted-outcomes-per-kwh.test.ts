import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AcceptedOutcomesPerKwhEndpoint,
  AcceptedOutcomesPerKwhProjection,
  modeledLabor4777AoKwhDatapoint,
  projectAcceptedOutcomesPerKwh,
} from './accepted-outcomes-per-kwh'
import { handleAcceptedOutcomesPerKwhApi } from './accepted-outcomes-per-kwh-routes'
import { openAgentsOpenApiDocument } from './openagents-openapi'

type AcceptedOutcomesPerKwhBody = Readonly<{
  datapoints: ReadonlyArray<unknown>
  metricId: string
}>

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
  })

  test('computes the modeled datapoint from public acceptance→result timing', () => {
    const datapoint = modeledLabor4777AoKwhDatapoint()

    expect(datapoint.windowStart).toBe('2026-06-14T02:36:58.442Z')
    expect(datapoint.windowEnd).toBe('2026-06-14T03:06:15.399Z')
    expect(datapoint.energyModel.method).toBe(
      'modeled_power_kw_times_acceptance_to_result_wall_clock',
    )
    expect(datapoint.energyModel.modeledPowerKw).toBe(0.1)
    expect(datapoint.energyModel.wallClockSeconds).toBe(1756.957)
    expect(datapoint.energyModel.energyKwh).toBe(0.048804)
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
