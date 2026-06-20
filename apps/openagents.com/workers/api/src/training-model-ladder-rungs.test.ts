import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TrainingModelLadderNetworkRungBlocker,
  TrainingModelLadderR1FullRehearsalBlocker,
  TrainingModelLadderRungsEndpoint,
  TrainingModelLadderRungsProjection,
  projectTrainingModelLadderRungs,
} from './training-model-ladder-rungs'
import { handleTrainingModelLadderRungsApi } from './training-model-ladder-rungs-routes'

type ModelLadderRungsBody = Readonly<{
  economicsGate: Readonly<{
    fieldCount: number
    formatAvailable: boolean
    gateOutcomeAvailable: boolean
    r1PopulatedReportAvailable: boolean
    settledNetworkEconomicsAvailable: boolean
  }>
  endpoint: string
  gate: Readonly<{
    greenGateSatisfied: boolean
    networkRungRemainingBlockerRefs: ReadonlyArray<string>
    publicProjectionAvailable: boolean
    r1CloseoutReceiptAvailable: boolean
    r1FullRehearsalAvailable: boolean
    r2NetworkRungReceiptAvailable: boolean
    remainingBlockerRefs: ReadonlyArray<string>
    rungEconomicsGateFormatAvailable: boolean
  }>
  promiseRef: string
  promiseState: string
  r1CloseoutCriteria: ReadonlyArray<{
    receiptAvailable: boolean
    status: string
  }>
  rungSummary: Readonly<{
    closedRungCount: number
    highestClosedRung: string
    nextRequiredRung: string
    r1CloseoutCriteriaCount: number
    rungCount: number
  }>
  rungs: ReadonlyArray<{
    closeoutReceiptAvailable: boolean
    networkRung: boolean
    rung: string
    status: string
  }>
}>

describe('training model ladder rungs projection', () => {
  test('publishes rung status without claiming an R1 closeout', () => {
    const projection = projectTrainingModelLadderRungs({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })

    expect(
      S.decodeUnknownSync(TrainingModelLadderRungsProjection)(projection),
    ).toEqual(projection)
    expect(projection.endpoint).toBe(TrainingModelLadderRungsEndpoint)
    expect(projection.promiseRef).toBe('promise:training.model_ladder.v1')
    expect(projection.promiseState).toBe('planned')
    expect(projection.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
    })
    expect(projection.gate).toEqual({
      clearsBlockerRefs: [],
      greenGateSatisfied: false,
      networkRungRemainingBlockerRefs: [TrainingModelLadderNetworkRungBlocker],
      publicProjectionAvailable: true,
      r1CloseoutReceiptAvailable: false,
      r1FullRehearsalAvailable: false,
      r2NetworkRungReceiptAvailable: false,
      remainingBlockerRefs: [TrainingModelLadderR1FullRehearsalBlocker],
      rungEconomicsGateFormatAvailable: true,
    })
    expect(projection.rungSummary).toEqual({
      closedRungCount: 1,
      economicsGateFieldCount: 5,
      highestClosedRung: 'R0',
      nextRequiredRung: 'R1',
      r1CloseoutCriteriaCount: 6,
      rungCount: 5,
    })
    expect(projection.rungs.map(rung => rung.rung)).toEqual([
      'R0',
      'R1',
      'R2',
      'R3',
      'R4',
    ])
    expect(projection.rungs[0]).toMatchObject({
      closeoutReceiptAvailable: true,
      networkRung: false,
      rung: 'R0',
      status: 'retained_rehearsal',
    })
    expect(projection.rungs[1]).toMatchObject({
      blockerRefs: [TrainingModelLadderR1FullRehearsalBlocker],
      closeoutReceiptAvailable: false,
      rung: 'R1',
      status: 'not_run',
    })
    expect(projection.rungs[2]).toMatchObject({
      blockerRefs: [TrainingModelLadderNetworkRungBlocker],
      closeoutReceiptAvailable: false,
      networkRung: true,
      rung: 'R2',
      status: 'not_run',
    })
    expect(
      projection.r1CloseoutCriteria.every(
        criterion =>
          !criterion.receiptAvailable && criterion.status === 'missing_receipt',
      ),
    ).toBe(true)
    expect(projection.economicsGate).toMatchObject({
      fieldCount: 5,
      formatAvailable: true,
      gateOutcomeAvailable: false,
      r1PopulatedReportAvailable: false,
      settledNetworkEconomicsAvailable: false,
    })
  })

  test('keeps authority and private material out of the public projection', () => {
    const projection = projectTrainingModelLadderRungs({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })
    const serialized = JSON.stringify(projection)

    expect(projection.authorityBoundary).toContain('grants no')
    expect(projection.unsafeCopy).toContain('Do not claim')
    expect(serialized).not.toMatch(
      /wallet|invoice|preimage|payment_hash|secret|raw_prompt|private_repo|\/home\/|\/Users\//i,
    )
  })

  test('serves the public route as no-store JSON', async () => {
    const response = await Effect.runPromise(
      handleTrainingModelLadderRungsApi(
        new Request(
          `https://openagents.com${TrainingModelLadderRungsEndpoint}`,
        ),
      ),
    )
    const body = (await response.json()) as ModelLadderRungsBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.endpoint).toBe(TrainingModelLadderRungsEndpoint)
    expect(body.promiseRef).toBe('promise:training.model_ladder.v1')
    expect(body.promiseState).toBe('planned')
    expect(body.gate.publicProjectionAvailable).toBe(true)
    expect(body.gate.rungEconomicsGateFormatAvailable).toBe(true)
    expect(body.gate.r1FullRehearsalAvailable).toBe(false)
    expect(body.gate.r1CloseoutReceiptAvailable).toBe(false)
    expect(body.gate.r2NetworkRungReceiptAvailable).toBe(false)
    expect(body.gate.greenGateSatisfied).toBe(false)
    expect(body.gate.remainingBlockerRefs).toEqual([
      TrainingModelLadderR1FullRehearsalBlocker,
    ])
    expect(body.gate.networkRungRemainingBlockerRefs).toEqual([
      TrainingModelLadderNetworkRungBlocker,
    ])
    expect(body.rungSummary).toMatchObject({
      closedRungCount: 1,
      highestClosedRung: 'R0',
      nextRequiredRung: 'R1',
      r1CloseoutCriteriaCount: 6,
      rungCount: 5,
    })
    expect(body.rungs.find(rung => rung.rung === 'R0')).toMatchObject({
      closeoutReceiptAvailable: true,
      status: 'retained_rehearsal',
    })
    expect(body.rungs.find(rung => rung.rung === 'R1')).toMatchObject({
      closeoutReceiptAvailable: false,
      status: 'not_run',
    })
    expect(
      body.r1CloseoutCriteria.every(criterion => !criterion.receiptAvailable),
    ).toBe(true)
    expect(body.economicsGate.formatAvailable).toBe(true)
    expect(body.economicsGate.r1PopulatedReportAvailable).toBe(false)
    expect(body.economicsGate.settledNetworkEconomicsAvailable).toBe(false)
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleTrainingModelLadderRungsApi(
        new Request(
          `https://openagents.com${TrainingModelLadderRungsEndpoint}`,
          { method: 'POST' },
        ),
      ),
    )

    expect(response.status).toBe(405)
  })
})
