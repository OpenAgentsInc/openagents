import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TrainingPublicDistributedRunNetworkScaleQualifiedContributorThreshold,
  TrainingPublicDistributedRunReceiptsBlocker,
  TrainingPublicDistributedRunScaleEndpoint,
  TrainingPublicDistributedRunScaleProjection,
  projectTrainingPublicDistributedRunScale,
  projectTrainingPublicDistributedRunScaleFromEnvelope,
} from './training-public-distributed-run-scale'
import { handleTrainingPublicDistributedRunScaleApi } from './training-public-distributed-run-scale-routes'

const liveRunLikeEnvelope = {
  corpus: {
    acceptedTraceCount: 11,
    traceRefs: ['training.verification.challenge.example.trace'],
    verdictRefs: ['verdict.training.exact_trace_replay.example'],
  },
  generatedAt: '2026-06-20T12:00:00.000Z',
  metrics: {
    providerConfirmedSettledPayoutSats: {
      sourceRefs: [
        'training.run.run.tassadar.executor.20260615.provider_confirmed_settlements',
      ],
      value: 1020,
    },
    qualifiedContributorCount: {
      sourceRefs: [
        'pylon.qualified.example.1',
        'receipt.nexus.tassadar_run_settlement.example.1',
      ],
      value: 5,
    },
    verifiedWorkCount: {
      sourceRefs: ['training.verification.challenge.example.trace'],
      value: 11,
    },
  },
  runRef: 'run.tassadar.executor.20260615',
  runState: 'active',
  schemaVersion: 'openagents.public_tassadar_run_summary.v1',
  settlement: {
    settledPayoutSats: 1020,
    settledReceiptCount: 5,
    sourceRefs: [
      'training.run.run.tassadar.executor.20260615.provider_confirmed_settlements',
    ],
  },
  sourceRefs: [
    'route:/api/public/training/runs/run.tassadar.executor.20260615',
    'route:/api/public/training/runs/run.tassadar.executor.20260615/settlements',
  ],
} as const

describe('training public distributed run scale projection', () => {
  test('publishes the current scale gap without clearing the blocker', () => {
    const projection =
      projectTrainingPublicDistributedRunScaleFromEnvelope(liveRunLikeEnvelope)

    expect(
      S.decodeUnknownSync(TrainingPublicDistributedRunScaleProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.endpoint).toBe(TrainingPublicDistributedRunScaleEndpoint)
    expect(projection.promiseRef).toBe(
      'promise:training.public_distributed_training_run.v1',
    )
    expect(projection.promiseState).toBe('red')
    expect(projection.gate).toMatchObject({
      broadAcceptedWorkReceiptsAvailable: false,
      clearsBlockerRefs: [],
      greenGateSatisfied: false,
      networkScaleThresholdMet: false,
      ownerSignedUpgradeAvailable: false,
      participantCountMethodologyAvailable: true,
      publicProjectionAvailable: true,
      publicRunDefinitionAvailable: true,
      remainingBlockerRefs: [TrainingPublicDistributedRunReceiptsBlocker],
      settlementRefsForMultipleContributorsAvailable: true,
    })
    expect(projection.methodology).toMatchObject({
      networkScaleQualifiedContributorThreshold:
        TrainingPublicDistributedRunNetworkScaleQualifiedContributorThreshold,
      comparableLargestRunContributorBenchmark: 200,
    })
    expect(projection.runScale).toMatchObject({
      acceptedTraceCount: 11,
      currentScaleLabel: 'canary_scale',
      providerConfirmedSettledPayoutSats: 1020,
      qualifiedContributorCount: 5,
      qualifiedContributorDeficit: 45,
      realSettlementReceiptCount: 5,
      runRef: 'run.tassadar.executor.20260615',
      runState: 'active',
    })
    expect(projection.scaleAxes.map(axis => axis.axisId)).toEqual([
      'qualified_contributors',
      'accepted_exact_trace_work',
      'real_settlement_receipts',
    ])
    expect(
      projection.scaleAxes.every(axis => axis.thresholdMet === false),
    ).toBe(true)
  })

  test('keeps network-scale and green gates separate when threshold counters are met', () => {
    const projection = projectTrainingPublicDistributedRunScale({
      acceptedTraceCount: 60,
      generatedAt: '2026-06-20T12:00:00.000Z',
      providerConfirmedSettledPayoutSats: 6000,
      qualifiedContributorCount: 50,
      realSettlementReceiptCount: 50,
      runRef: 'run.tassadar.executor.20260615',
      runState: 'active',
      sourceRefs: ['receipt.example.network_scale'],
      sourceSchemaVersion: 'openagents.public_tassadar_run_summary.v1',
    })

    expect(projection.gate.networkScaleThresholdMet).toBe(true)
    expect(projection.gate.broadAcceptedWorkReceiptsAvailable).toBe(true)
    expect(projection.gate.ownerSignedUpgradeAvailable).toBe(false)
    expect(projection.gate.greenGateSatisfied).toBe(false)
    expect(projection.statusLabel).toContain('owner-signed upgrade')
  })

  test('keeps authority and private material out of the public projection', () => {
    const projection =
      projectTrainingPublicDistributedRunScaleFromEnvelope(liveRunLikeEnvelope)
    const serialized = JSON.stringify(projection)

    expect(projection.authorityBoundary).toContain('grants no')
    expect(projection.unsafeCopy).toContain('Do not claim')
    expect(serialized).not.toMatch(
      /wallet|invoice|preimage|payment_hash|secret|raw_prompt|private_repo|\/home\/|\/Users\//i,
    )
  })

  test('serves the public route as no-store JSON', async () => {
    const response = await Effect.runPromise(
      handleTrainingPublicDistributedRunScaleApi(
        new Request(
          `https://openagents.com${TrainingPublicDistributedRunScaleEndpoint}`,
        ),
        {},
        {
          buildSummaryEnvelope: async () => liveRunLikeEnvelope,
        },
      ),
    )
    const body = (await response.json()) as Readonly<{
      endpoint: string
      gate: Readonly<{
        greenGateSatisfied: boolean
        networkScaleThresholdMet: boolean
        remainingBlockerRefs: ReadonlyArray<string>
      }>
      runScale: Readonly<{
        acceptedTraceCount: number
        qualifiedContributorCount: number
      }>
    }>

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.endpoint).toBe(TrainingPublicDistributedRunScaleEndpoint)
    expect(body.gate.networkScaleThresholdMet).toBe(false)
    expect(body.gate.greenGateSatisfied).toBe(false)
    expect(body.gate.remainingBlockerRefs).toEqual([
      TrainingPublicDistributedRunReceiptsBlocker,
    ])
    expect(body.runScale.qualifiedContributorCount).toBe(5)
    expect(body.runScale.acceptedTraceCount).toBe(11)
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleTrainingPublicDistributedRunScaleApi(
        new Request(
          `https://openagents.com${TrainingPublicDistributedRunScaleEndpoint}`,
          { method: 'POST' },
        ),
        {},
      ),
    )

    expect(response.status).toBe(405)
  })
})
