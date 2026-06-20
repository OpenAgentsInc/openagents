import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CurtailmentDrillSchemaVersion,
  MaxCurtailmentAckLatencyMs,
  MaxCurtailmentHaltLatencyMs,
} from './training-curtailment-drill'
import { DurableCheckpointSealBlocker } from './training-durable-checkpoint-seal'
import {
  TrainingMarathonCurtailmentDrillBlocker,
  TrainingMarathonOperationsEndpoint,
  TrainingMarathonOperationsProjection,
  projectTrainingMarathonOperations,
} from './training-marathon-operations'
import { handleTrainingMarathonOperationsApi } from './training-marathon-operations-routes'
import { StandbyDispatchBlocker } from './training-standby-dispatch'

type MarathonOperationsBody = Readonly<{
  checkpointSurface: Readonly<{
    bootstrapSelectsOnlyDurableSeal: boolean
    durableCheckpointSealReceiptAvailable: boolean
    liveSealBoundaryWired: boolean
    predicateAvailable: boolean
    remoteCheckpointStoreReadbackReceiptAvailable: boolean
  }>
  curtailmentSurface: Readonly<{
    ackSlaMs: number
    checkpointResumeReceiptAvailable: boolean
    curtailmentDrillReceiptAvailable: boolean
    drillScheduled: boolean
    flexibleLoadEvidenceCreated: boolean
    haltSlaMs: number
    predicateAvailable: boolean
    preflightEndpoint: string
    preflightRouteAvailable: boolean
    schemaVersion: string
  }>
  endpoint: string
  gate: Readonly<{
    curtailmentDrillReceiptAvailable: boolean
    durableCheckpointRemoteReadbackReceiptAvailable: boolean
    greenGateSatisfied: boolean
    liveStandbyPromotionReceiptAvailable: boolean
    marathonCloseoutReceiptAvailable: boolean
    publicProjectionAvailable: boolean
    remainingBlockerRefs: ReadonlyArray<string>
  }>
  operationsSummary: Readonly<{
    blockerCount: number
    openReceiptGateCount: number
    predicateSurfaceCount: number
    receiptBackedLiveOperationCount: number
  }>
  promiseRef: string
  promiseState: string
  standbySurface: Readonly<{
    liveHeartbeatTelemetryFeedAvailable: boolean
    livePromotionReceiptAvailable: boolean
    liveVacancyTelemetryFeedAvailable: boolean
    predicateAvailable: boolean
    preflightRouteAvailable: boolean
    receiptBackedPromotionAvailable: boolean
  }>
}>

const remainingBlockers = [
  DurableCheckpointSealBlocker,
  StandbyDispatchBlocker,
  TrainingMarathonCurtailmentDrillBlocker,
]

describe('training marathon operations projection', () => {
  test('publishes marathon status without claiming live receipts', () => {
    const projection = projectTrainingMarathonOperations({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })

    expect(
      S.decodeUnknownSync(TrainingMarathonOperationsProjection)(projection),
    ).toEqual(projection)
    expect(projection.endpoint).toBe(TrainingMarathonOperationsEndpoint)
    expect(projection.promiseRef).toBe(
      'promise:training.marathon_operations.v1',
    )
    expect(projection.promiseState).toBe('planned')
    expect(projection.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
    })
    expect(projection.gate).toEqual({
      clearsBlockerRefs: [],
      curtailmentDrillReceiptAvailable: false,
      durableCheckpointRemoteReadbackReceiptAvailable: false,
      greenGateSatisfied: false,
      liveStandbyPromotionReceiptAvailable: false,
      marathonCloseoutReceiptAvailable: false,
      publicProjectionAvailable: true,
      remainingBlockerRefs: remainingBlockers,
    })
    expect(projection.checkpointSurface).toMatchObject({
      bootstrapSelectsOnlyDurableSeal: true,
      durableCheckpointSealReceiptAvailable: false,
      liveSealBoundaryWired: true,
      minimumDurableReplicationFactor: 2,
      predicateAvailable: true,
      remoteCheckpointStoreReadbackReceiptAvailable: false,
    })
    expect(projection.standbySurface).toMatchObject({
      liveHeartbeatTelemetryFeedAvailable: false,
      livePromotionReceiptAvailable: false,
      liveVacancyTelemetryFeedAvailable: false,
      predicateAvailable: true,
      preflightRouteAvailable: true,
      receiptBackedPromotionAvailable: false,
    })
    expect(projection.curtailmentSurface).toMatchObject({
      ackSlaMs: MaxCurtailmentAckLatencyMs,
      checkpointResumeReceiptAvailable: false,
      curtailmentDrillReceiptAvailable: false,
      drillScheduled: false,
      flexibleLoadEvidenceCreated: false,
      haltSlaMs: MaxCurtailmentHaltLatencyMs,
      predicateAvailable: true,
      preflightEndpoint:
        '/api/training/runs/{trainingRunRef}/curtailment-drill-preflight',
      preflightRouteAvailable: true,
      schemaVersion: CurtailmentDrillSchemaVersion,
    })
    expect(projection.operationsSummary).toEqual({
      blockerCount: 3,
      openReceiptGateCount: 3,
      predicateSurfaceCount: 3,
      publicEndpointCount: 1,
      receiptBackedLiveOperationCount: 0,
    })
  })

  test('keeps authority and private material out of the public projection', () => {
    const projection = projectTrainingMarathonOperations({
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
      handleTrainingMarathonOperationsApi(
        new Request(
          `https://openagents.com${TrainingMarathonOperationsEndpoint}`,
        ),
      ),
    )
    const body = (await response.json()) as MarathonOperationsBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.endpoint).toBe(TrainingMarathonOperationsEndpoint)
    expect(body.promiseRef).toBe('promise:training.marathon_operations.v1')
    expect(body.promiseState).toBe('planned')
    expect(body.gate.publicProjectionAvailable).toBe(true)
    expect(body.gate.greenGateSatisfied).toBe(false)
    expect(body.gate.remainingBlockerRefs).toEqual(remainingBlockers)
    expect(body.gate.durableCheckpointRemoteReadbackReceiptAvailable).toBe(
      false,
    )
    expect(body.gate.liveStandbyPromotionReceiptAvailable).toBe(false)
    expect(body.gate.curtailmentDrillReceiptAvailable).toBe(false)
    expect(body.gate.marathonCloseoutReceiptAvailable).toBe(false)
    expect(body.checkpointSurface.predicateAvailable).toBe(true)
    expect(body.checkpointSurface.liveSealBoundaryWired).toBe(true)
    expect(
      body.checkpointSurface.remoteCheckpointStoreReadbackReceiptAvailable,
    ).toBe(false)
    expect(body.standbySurface.predicateAvailable).toBe(true)
    expect(body.standbySurface.preflightRouteAvailable).toBe(true)
    expect(body.standbySurface.livePromotionReceiptAvailable).toBe(false)
    expect(body.curtailmentSurface.predicateAvailable).toBe(true)
    expect(body.curtailmentSurface.schemaVersion).toBe(
      CurtailmentDrillSchemaVersion,
    )
    expect(body.curtailmentSurface.ackSlaMs).toBe(
      MaxCurtailmentAckLatencyMs,
    )
    expect(body.curtailmentSurface.haltSlaMs).toBe(
      MaxCurtailmentHaltLatencyMs,
    )
    expect(body.curtailmentSurface.drillScheduled).toBe(false)
    expect(body.curtailmentSurface.flexibleLoadEvidenceCreated).toBe(false)
    expect(body.curtailmentSurface.preflightRouteAvailable).toBe(true)
    expect(body.curtailmentSurface.preflightEndpoint).toBe(
      '/api/training/runs/{trainingRunRef}/curtailment-drill-preflight',
    )
    expect(body.operationsSummary).toMatchObject({
      blockerCount: 3,
      openReceiptGateCount: 3,
      predicateSurfaceCount: 3,
      receiptBackedLiveOperationCount: 0,
    })
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleTrainingMarathonOperationsApi(
        new Request(
          `https://openagents.com${TrainingMarathonOperationsEndpoint}`,
          { method: 'POST' },
        ),
      ),
    )

    expect(response.status).toBe(405)
  })
})
