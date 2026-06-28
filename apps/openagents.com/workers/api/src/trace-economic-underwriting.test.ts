import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { openAgentsOpenApiDocument } from './openagents-openapi'
import {
  TraceEconomicUnderwritingEndpoint,
  TraceEconomicUnderwritingMinimumReadyOutcomeCount,
  TraceEconomicUnderwritingProjection,
  TraceEconomicUnderwritingOutcome,
  projectTraceEconomicUnderwriting,
  seedTraceEconomicUnderwritingOutcomes,
} from './trace-economic-underwriting'
import { handleTraceEconomicUnderwritingApi } from './trace-economic-underwriting-routes'

type TraceEconomicUnderwritingBody = Readonly<{
  projectionId: string
  gate: Readonly<{
    state: string
    warrantyOfferAllowed: boolean
  }>
}>

const qualifyingOutcome = (
  index: number,
): TraceEconomicUnderwritingOutcome =>
  new TraceEconomicUnderwritingOutcome({
    accepted: true,
    acceptedOutcomeRef: `accepted.outcome.${index}`,
    claimableLossBps: 10000,
    meterable: true,
    meteringReceiptRef: `receipt.metering.${index}`,
    observedAt: '2026-06-28T12:00:00.000Z',
    outcomeRef: `outcome.${index}`,
    replayVerified: true,
    settlementReceiptRef: `receipt.settlement.${index}`,
    settled: true,
    sourceRefs: [`source.${index}`],
    traceRef: `trace.${index}`,
    verdictRef: `verdict.${index}`,
    workClass: 'codex_agent_task',
  })

describe('Trace-economic underwriting projection', () => {
  test('publishes a schema-valid seed readiness projection with inert warranty shapes', () => {
    const projection = projectTraceEconomicUnderwriting({
      generatedAt: '2026-06-28T12:00:00.000Z',
    })

    expect(
      S.decodeUnknownSync(TraceEconomicUnderwritingProjection)(projection),
    ).toEqual(projection)
    expect(projection.projectionId).toBe('underwriting.trace_economic.v1')
    expect(projection.issueRef).toBe('github:OpenAgentsInc/openagents#6426')
    expect(projection.substrate.traceSurface).toBe('/trace/{uuid}')
    expect(projection.observed.qualifyingOutcomeCount).toBe(1)
    expect(projection.observed.incompleteOutcomeCount).toBe(1)
    expect(projection.gate.state).toBe('yellow')
    expect(projection.gate.warrantyOfferAllowed).toBe(false)
    expect(projection.warrantyShapes).toHaveLength(2)
    expect(projection.warrantyShapes.every(shape => shape.inert)).toBe(true)
    expect(projection.warrantyShapes.every(shape => !shape.policyBound)).toBe(
      true,
    )
    expect(projection.authorityBoundary).toContain('grants no policy binding')
    expect(projection.unsafeCopy).toContain('Do not describe this projection as insurance')
  })

  test('requires accepted, replay-verified, settled, metered outcomes before risk is warranty-ready', () => {
    const projection = projectTraceEconomicUnderwriting({
      outcomes: [
        qualifyingOutcome(1),
        new TraceEconomicUnderwritingOutcome({
          ...qualifyingOutcome(2),
          meteringReceiptRef: '',
          meterable: false,
        }),
        new TraceEconomicUnderwritingOutcome({
          ...qualifyingOutcome(3),
          replayVerified: false,
        }),
      ],
    })

    expect(projection.observed.inputOutcomeCount).toBe(3)
    expect(projection.observed.qualifyingOutcomeCount).toBe(1)
    expect(projection.observed.incompleteOutcomeCount).toBe(2)
    expect(projection.observed.receiptCoverageBps).toBe(3333)
    expect(projection.incompleteOutcomeRefs).toEqual(['outcome.2', 'outcome.3'])
    expect(projection.gate.blockerRefs).toContain(
      'blocker.underwriting.receipt_coverage_below_warranty_threshold',
    )
  })

  test('opens warranty copy gates only after count and receipt coverage thresholds are met', () => {
    const outcomes = Array.from(
      { length: TraceEconomicUnderwritingMinimumReadyOutcomeCount },
      (_, index) => qualifyingOutcome(index),
    )
    const projection = projectTraceEconomicUnderwriting({ outcomes })

    expect(projection.gate.state).toBe('green')
    expect(projection.gate.warrantyOfferAllowed).toBe(true)
    expect(projection.gate.refundPromiseAllowed).toBe(true)
    expect(projection.gate.slaPromiseAllowed).toBe(true)
    expect(projection.gate.blockerRefs).toEqual([])
  })

  test('serves the public route as no-store JSON', async () => {
    const response = await Effect.runPromise(
      handleTraceEconomicUnderwritingApi(
        new Request(`https://openagents.com${TraceEconomicUnderwritingEndpoint}`),
      ),
    )
    const body = (await response.json()) as TraceEconomicUnderwritingBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.projectionId).toBe('underwriting.trace_economic.v1')
    expect(body.gate.state).toBe('yellow')
    expect(body.gate.warrantyOfferAllowed).toBe(false)
  })

  test('documents the public underwriting endpoint in OpenAPI', async () => {
    const document = await Effect.runPromise(openAgentsOpenApiDocument())

    expect(
      (
        document.paths[TraceEconomicUnderwritingEndpoint] as
          | { get?: unknown }
          | undefined
      )?.get,
    ).toEqual(
      expect.objectContaining({
        operationId: 'getTraceEconomicUnderwriting',
      }),
    )
    expect(
      (document.components as { schemas: Record<string, unknown> }).schemas,
    ).toHaveProperty('TraceEconomicUnderwritingProjection')
  })

  test('seed fixture has one qualifying outcome and one blocked counterexample', () => {
    const projection = projectTraceEconomicUnderwriting({
      outcomes: seedTraceEconomicUnderwritingOutcomes(),
    })

    expect(projection.qualifyingOutcomeRefs).toHaveLength(1)
    expect(projection.incompleteOutcomeRefs).toHaveLength(1)
  })
})
