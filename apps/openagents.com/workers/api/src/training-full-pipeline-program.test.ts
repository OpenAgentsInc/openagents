import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TrainingFullPipelineProgramBlocker,
  TrainingFullPipelineProgramEndpoint,
  TrainingFullPipelineProgramProjection,
  projectTrainingFullPipelineProgram,
} from './training-full-pipeline-program'
import { handleTrainingFullPipelineProgramApi } from './training-full-pipeline-program-routes'

type FullPipelineBody = Readonly<{
  endpoint: string
  gate: Readonly<{
    everyWorkstreamAtLeastYellow: boolean
    greenGateSatisfied: boolean
    ladderRungEndToEndReceiptAvailable: boolean
    publicProjectionAvailable: boolean
    remainingBlockerRefs: ReadonlyArray<string>
  }>
  promiseRef: string
  promiseState: string
  stageSummary: Readonly<{
    greenReadyOwnerGatedStageCount: number
    liveEndpointCount: number
    partialReceiptSurfaceCount: number
    stageCount: number
  }>
  stages: ReadonlyArray<{
    blockerRefs: ReadonlyArray<string>
    endpointRefs: ReadonlyArray<string>
    promiseId: string
    promiseState: string
    receiptState: string
    stageId: string
  }>
}>

describe('training full pipeline program status projection', () => {
  test('publishes stage reachability without clearing the umbrella blocker', () => {
    const projection = projectTrainingFullPipelineProgram({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })

    expect(
      S.decodeUnknownSync(TrainingFullPipelineProgramProjection)(projection),
    ).toEqual(projection)
    expect(projection.endpoint).toBe(TrainingFullPipelineProgramEndpoint)
    expect(projection.promiseRef).toBe(
      'promise:training.full_pipeline_program.v1',
    )
    expect(projection.promiseState).toBe('planned')
    expect(projection.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
    })
    expect(projection.gate).toEqual({
      endToEndRunReceiptAvailable: false,
      everyWorkstreamAtLeastYellow: false,
      greenGateSatisfied: false,
      ladderRungEndToEndReceiptAvailable: false,
      paidNetworkWorkloadBroadlyLive: false,
      publicProjectionAvailable: true,
      remainingBlockerRefs: [TrainingFullPipelineProgramBlocker],
    })
    expect(projection.stageSummary).toMatchObject({
      greenReadyOwnerGatedStageCount: 1,
      liveEndpointCount: 11,
      partialReceiptSurfaceCount: 9,
      stageCount: 11,
    })
    expect(projection.stages.map(stage => stage.stageId)).toEqual([
      'data_refinery',
      'ablation',
      'public_gradient_windows',
      'public_distributed_run',
      'marathon_operations',
      'post_training',
      'model_ladder',
      'device_capability',
      'verification_classes',
      'tassadar_percepta_executor',
      'artanis_evolution_loop',
    ])
    expect(
      projection.stages.find(stage => stage.stageId === 'ablation'),
    ).toMatchObject({
      endpointRefs: ['/api/public/training/ablation-derisking-ledger'],
      promiseId: 'training.ablation_system.v1',
      receiptState: 'partial_receipt_surface_live',
    })
    expect(
      projection.stages.find(
        stage => stage.stageId === 'public_gradient_windows',
      ),
    ).toMatchObject({
      endpointRefs: ['/api/public/training/public-gradient-windows'],
      evidenceRefs: expect.arrayContaining([
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.test.ts',
      ]),
      promiseId: 'training.public_gradient_windows.v1',
      receiptState: 'partial_receipt_surface_live',
      statusLabel: expect.stringContaining('intake admission predicate'),
    })
    expect(
      projection.stages.find(
        stage => stage.stageId === 'public_distributed_run',
      ),
    ).toMatchObject({
      endpointRefs: [
        '/api/public/training/public-distributed-run-scale',
        '/api/public/training/runs/run.tassadar.executor.20260615',
        '/api/public/training/runs/run.tassadar.executor.20260615/settlements',
      ],
      promiseId: 'training.public_distributed_training_run.v1',
      receiptState: 'partial_receipt_surface_live',
    })
    expect(
      projection.stages.find(stage => stage.stageId === 'marathon_operations'),
    ).toMatchObject({
      endpointRefs: ['/api/public/training/marathon-operations'],
      promiseId: 'training.marathon_operations.v1',
      receiptState: 'partial_receipt_surface_live',
    })
    expect(
      projection.stages.find(stage => stage.stageId === 'model_ladder'),
    ).toMatchObject({
      endpointRefs: ['/api/public/training/model-ladder-rungs'],
      promiseId: 'training.model_ladder.v1',
      receiptState: 'partial_receipt_surface_live',
    })
    expect(
      projection.stages.find(
        stage => stage.stageId === 'artanis_evolution_loop',
      ),
    ).toMatchObject({
      blockerRefs: [],
      promiseId: 'artanis.tassadar_evolution_loop.v1',
      promiseState: 'green',
      receiptState: 'green_ready_owner_gated',
    })
    expect(
      projection.stages.find(
        stage => stage.stageId === 'tassadar_percepta_executor',
      ),
    ).toMatchObject({
      endpointRefs: [
        '/api/public/models/tassadar-percepta-executor/architecture-receipts',
        '/api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts',
      ],
      evidenceRefs: expect.arrayContaining([
        'docs/tassadar/2026-06-21-tassadar-cpu-transform-training-receipt-surface.md',
        'apps/openagents.com/workers/api/src/tassadar-percepta-cpu-transform-training-receipts.ts',
      ]),
      promiseId: 'models.tassadar_percepta_executor.v1',
      promiseState: 'planned',
      receiptState: 'partial_receipt_surface_live',
      statusLabel: expect.stringContaining('CPU-transform receipt status'),
    })
  })

  test('keeps authority and private material out of the public projection', () => {
    const projection = projectTrainingFullPipelineProgram({
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
      handleTrainingFullPipelineProgramApi(
        new Request(
          `https://openagents.com${TrainingFullPipelineProgramEndpoint}`,
        ),
      ),
    )
    const body = (await response.json()) as FullPipelineBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.endpoint).toBe(TrainingFullPipelineProgramEndpoint)
    expect(body.promiseRef).toBe('promise:training.full_pipeline_program.v1')
    expect(body.promiseState).toBe('planned')
    expect(body.gate.publicProjectionAvailable).toBe(true)
    expect(body.gate.greenGateSatisfied).toBe(false)
    expect(body.gate.remainingBlockerRefs).toEqual([
      TrainingFullPipelineProgramBlocker,
    ])
    expect(body.stageSummary.stageCount).toBe(11)
    expect(
      body.stages.some(
        stage =>
          stage.stageId === 'post_training' &&
          stage.endpointRefs.includes(
            '/api/public/training/post-training-arc/instruct-sft-lane',
          ) &&
          stage.endpointRefs.includes(
            '/api/public/training/post-training-arc/dpo-preference-workload',
          ),
      ),
    ).toBe(true)
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleTrainingFullPipelineProgramApi(
        new Request(
          `https://openagents.com${TrainingFullPipelineProgramEndpoint}`,
          {
            method: 'POST',
          },
        ),
      ),
    )

    expect(response.status).toBe(405)
  })
})
