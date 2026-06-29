import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PylonLargestConcreteComparableContributorBenchmark,
  PylonLargestDecentralizedTrainingClaimBlocker,
  PylonLargestDecentralizedTrainingClaimEndpoint,
  PylonLargestDecentralizedTrainingClaimProjection,
  PylonLargestTranscriptTargetContributorBenchmark,
  projectPylonLargestDecentralizedTrainingClaimStatusFromEnvelope,
} from './pylon-largest-decentralized-training-claim-status'
import { handlePylonLargestDecentralizedTrainingClaimStatusApi } from './pylon-largest-decentralized-training-claim-status-routes'
import { TrainingPublicDistributedRunScaleEndpoint } from './training-public-distributed-run-scale'

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

describe('pylon largest decentralized training claim status projection', () => {
  test('compares the current run to largest-run benchmarks without clearing the blocker', () => {
    const projection =
      projectPylonLargestDecentralizedTrainingClaimStatusFromEnvelope(
        liveRunLikeEnvelope,
      )

    expect(
      S.decodeUnknownSync(PylonLargestDecentralizedTrainingClaimProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.endpoint).toBe(
      PylonLargestDecentralizedTrainingClaimEndpoint,
    )
    expect(projection.promiseRef).toBe(
      'promise:pylon.largest_decentralized_training_claim.v1',
    )
    expect(projection.promiseState).toBe('red')
    expect(projection.gate).toMatchObject({
      clearsBlockerRefs: [],
      comparableRunResearchAvailable: true,
      concreteComparableThresholdMet: false,
      greenGateSatisfied: false,
      ownerSignedUpgradeAvailable: false,
      participantCountMethodologyAvailable: true,
      publicContributorReceiptsAtClaimBenchmarkAvailable: false,
      publicRunScaleProjectionAvailable: true,
      remainingBlockerRefs: [PylonLargestDecentralizedTrainingClaimBlocker],
      transcriptTargetThresholdMet: false,
    })
    expect(projection.benchmark).toMatchObject({
      concreteComparableContributorBenchmark:
        PylonLargestConcreteComparableContributorBenchmark,
      transcriptTargetContributorBenchmark:
        PylonLargestTranscriptTargetContributorBenchmark,
    })
    expect(projection.runScale).toMatchObject({
      acceptedTraceCount: 11,
      currentScaleLabel: 'canary_scale',
      providerConfirmedSettledPayoutSats: 1020,
      qualifiedContributorCount: 5,
      realSettlementReceiptCount: 5,
      runRef: 'run.tassadar.executor.20260615',
      runState: 'active',
      sourceScaleEndpoint: TrainingPublicDistributedRunScaleEndpoint,
    })
    expect(projection.comparisonRows.map(row => row.benchmarkId)).toEqual([
      'templar_covenant_72b_published_comparable',
      'episode_236_transcript_target',
    ])
    expect(projection.comparisonRows[0]).toMatchObject({
      currentQualifiedContributorCount: 5,
      deficit: 65,
      requiredQualifiedContributorCount: 70,
      thresholdMet: false,
    })
    expect(projection.comparisonRows[1]).toMatchObject({
      currentQualifiedContributorCount: 5,
      deficit: 195,
      requiredQualifiedContributorCount: 200,
      thresholdMet: false,
    })
  })

  test('keeps owner signoff and green gates closed even when counters reach the target', () => {
    const projection =
      projectPylonLargestDecentralizedTrainingClaimStatusFromEnvelope({
        ...liveRunLikeEnvelope,
        corpus: { ...liveRunLikeEnvelope.corpus, acceptedTraceCount: 210 },
        metrics: {
          ...liveRunLikeEnvelope.metrics,
          qualifiedContributorCount: {
            sourceRefs: ['receipt.example.benchmark'],
            value: 200,
          },
          verifiedWorkCount: {
            sourceRefs: ['training.verification.challenge.example.benchmark'],
            value: 210,
          },
        },
        settlement: {
          ...liveRunLikeEnvelope.settlement,
          settledReceiptCount: 200,
        },
      })

    expect(projection.gate.concreteComparableThresholdMet).toBe(true)
    expect(projection.gate.transcriptTargetThresholdMet).toBe(true)
    expect(
      projection.gate.publicContributorReceiptsAtClaimBenchmarkAvailable,
    ).toBe(true)
    expect(projection.gate.ownerSignedUpgradeAvailable).toBe(false)
    expect(projection.gate.greenGateSatisfied).toBe(false)
    expect(projection.statusLabel).toContain('owner-signed')
  })

  test('keeps authority and private material out of the public projection', () => {
    const projection =
      projectPylonLargestDecentralizedTrainingClaimStatusFromEnvelope(
        liveRunLikeEnvelope,
      )
    const serialized = JSON.stringify(projection)

    expect(projection.authorityBoundary).toContain('grants no')
    expect(projection.unsafeCopy).toContain('Do not say')
    expect(serialized).not.toMatch(
      /wallet|invoice|preimage|payment_hash|secret|raw_prompt|private_repo|\/home\/|\/Users\//i,
    )
  })

  test('serves the public route as no-store JSON', async () => {
    const response = await Effect.runPromise(
      handlePylonLargestDecentralizedTrainingClaimStatusApi(
        new Request(
          `https://openagents.com${PylonLargestDecentralizedTrainingClaimEndpoint}`,
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
        remainingBlockerRefs: ReadonlyArray<string>
        transcriptTargetThresholdMet: boolean
      }>
      runScale: Readonly<{
        qualifiedContributorCount: number
      }>
    }>

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.endpoint).toBe(PylonLargestDecentralizedTrainingClaimEndpoint)
    expect(body.gate.transcriptTargetThresholdMet).toBe(false)
    expect(body.gate.greenGateSatisfied).toBe(false)
    expect(body.gate.remainingBlockerRefs).toEqual([
      PylonLargestDecentralizedTrainingClaimBlocker,
    ])
    expect(body.runScale.qualifiedContributorCount).toBe(5)
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handlePylonLargestDecentralizedTrainingClaimStatusApi(
        new Request(
          `https://openagents.com${PylonLargestDecentralizedTrainingClaimEndpoint}`,
          { method: 'POST' },
        ),
        {},
      ),
    )

    expect(response.status).toBe(405)
  })
})
