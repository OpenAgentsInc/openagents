import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PreferenceRolloutWorkMissingBlocker,
} from './training-post-training-instruct-sft'
import {
  TrainingPostTrainingDpoPreferenceWorkloadEndpoint,
  TrainingPostTrainingDpoPreferenceWorkloadOutputDigestHex,
  TrainingPostTrainingDpoPreferenceWorkloadProjection,
  TrainingPostTrainingDpoPreferenceWorkloadReceiptRef,
  projectTrainingPostTrainingDpoPreferenceWorkload,
} from './training-post-training-dpo-preference-workload'
import { handleTrainingPostTrainingDpoPreferenceWorkloadApi } from './training-post-training-dpo-preference-workload-routes'
import { runCs336A5DpoPreferenceGrading } from './cs336-a5-dpo-preference-workload'

type DpoPreferenceWorkloadBody = Readonly<{
  endpoint: string
  gate: Readonly<{
    deterministicReferenceWorkloadAvailable: boolean
    greenGateSatisfied: boolean
    paidPreferenceDispatchAvailable: boolean
    preferenceRolloutWorkAvailable: boolean
    realModelLogprobMeasurementAvailable: boolean
    settlementReceiptAvailable: boolean
    verifiedChallengeAvailable: boolean
  }>
  promiseRef: string
  promiseState: string
  receiptSummary: Readonly<{ referenceWorkloadReceiptCount: number }>
  receipts: ReadonlyArray<Readonly<{ outputDigestHex: string }>>
}>

describe('training post-training DPO preference workload projection', () => {
  test('publishes the bounded deterministic DPO reference workload without claiming paid preference work', () => {
    const projection = projectTrainingPostTrainingDpoPreferenceWorkload({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })

    expect(
      S.decodeUnknownSync(TrainingPostTrainingDpoPreferenceWorkloadProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.endpoint).toBe(
      TrainingPostTrainingDpoPreferenceWorkloadEndpoint,
    )
    expect(projection.promiseRef).toBe('promise:training.post_training_arc.v1')
    expect(projection.promiseState).toBe('planned')
    expect(projection.gate).toMatchObject({
      clearsBlockerRefs: [],
      deterministicReferenceWorkloadAvailable: true,
      dpoUpdateAvailable: false,
      greenGateSatisfied: false,
      paidPreferenceDispatchAvailable: false,
      preferenceRolloutWorkAvailable: false,
      publicProjectionAvailable: true,
      realModelLogprobMeasurementAvailable: false,
      remainingBlockerRefs: [PreferenceRolloutWorkMissingBlocker],
      settlementReceiptAvailable: false,
      verifiedChallengeAvailable: false,
    })
    expect(projection.receiptSummary).toEqual({
      paidPreferenceDispatchCount: 0,
      referenceWorkloadReceiptCount: 1,
      settlementReceiptCount: 0,
      verifiedChallengeCount: 0,
    })
    expect(projection.receipts[0]).toMatchObject({
      beta: 0.1,
      deterministicRecomputeAvailable: true,
      jobKind: 'cs336_a5_dpo_grading',
      outputDigestHex:
        TrainingPostTrainingDpoPreferenceWorkloadOutputDigestHex,
      paidDispatchState: 'not_dispatched',
      pairCount: 25,
      publicSafe: true,
      receiptRef: TrainingPostTrainingDpoPreferenceWorkloadReceiptRef,
      splitRef: 'split_a',
      syntheticLogprobBoundary: 'deterministic_synthetic_public_safe',
      verificationClass: 'deterministic_recompute',
      workloadRef: 'workload.cs336_a5.dpo_preference_pair_reference_grading.v1',
    })
    expect(projection.receipts[0]?.blockerRefs).toEqual([
      PreferenceRolloutWorkMissingBlocker,
    ])
    expect(projection.receipts[0]?.clearsBlockerRefs).toEqual([])
  })

  test('pins the public receipt digest to the recomputable workload result', async () => {
    const recomputed = await runCs336A5DpoPreferenceGrading({
      splitRef: 'split_a',
    })

    expect(recomputed.outputDigestHex).toBe(
      TrainingPostTrainingDpoPreferenceWorkloadOutputDigestHex,
    )
    expect(recomputed.pairCount).toBe(25)
  })

  test('keeps authority and private material out of the public projection', () => {
    const projection = projectTrainingPostTrainingDpoPreferenceWorkload({
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
      handleTrainingPostTrainingDpoPreferenceWorkloadApi(
        new Request(
          `https://openagents.com${TrainingPostTrainingDpoPreferenceWorkloadEndpoint}`,
        ),
      ),
    )
    const body = (await response.json()) as DpoPreferenceWorkloadBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.endpoint).toBe(
      TrainingPostTrainingDpoPreferenceWorkloadEndpoint,
    )
    expect(body.promiseRef).toBe('promise:training.post_training_arc.v1')
    expect(body.promiseState).toBe('planned')
    expect(body.gate.deterministicReferenceWorkloadAvailable).toBe(true)
    expect(body.gate.paidPreferenceDispatchAvailable).toBe(false)
    expect(body.gate.preferenceRolloutWorkAvailable).toBe(false)
    expect(body.gate.realModelLogprobMeasurementAvailable).toBe(false)
    expect(body.gate.verifiedChallengeAvailable).toBe(false)
    expect(body.gate.settlementReceiptAvailable).toBe(false)
    expect(body.gate.greenGateSatisfied).toBe(false)
    expect(body.receiptSummary.referenceWorkloadReceiptCount).toBe(1)
    expect(body.receipts[0]?.outputDigestHex).toBe(
      TrainingPostTrainingDpoPreferenceWorkloadOutputDigestHex,
    )
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleTrainingPostTrainingDpoPreferenceWorkloadApi(
        new Request(
          `https://openagents.com${TrainingPostTrainingDpoPreferenceWorkloadEndpoint}`,
          { method: 'POST' },
        ),
      ),
    )

    expect(response.status).toBe(405)
  })
})
