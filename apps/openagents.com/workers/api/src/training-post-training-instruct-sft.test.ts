import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  InstructSftLaneMissingBlocker,
  InstructSftPaidDispatchMissingBlocker,
  PreferenceRolloutWorkMissingBlocker,
  TrainingPostTrainingInstructSftEndpoint,
  TrainingPostTrainingInstructSftProjection,
  TrainingPostTrainingInstructSftReceiptRef,
  VibeTestArtifactMissingBlocker,
  projectTrainingPostTrainingInstructSft,
} from './training-post-training-instruct-sft'
import { handleTrainingPostTrainingInstructSftApi } from './training-post-training-instruct-sft-routes'

type InstructSftBody = Readonly<{
  endpoint: string
  gate: Readonly<{
    committedReportFixtureSyncAvailable: boolean
    greenGateSatisfied: boolean
    instructSftLaneAvailable: boolean
    instructSftPaidDispatchAvailable: boolean
    preferenceRolloutWorkAvailable: boolean
    vibeTestArtifactAvailable: boolean
  }>
  promiseRef: string
  promiseState: string
}>

describe('training post-training instruct SFT receipt projection', () => {
  test('publishes the bounded instruct SFT lane receipt without claiming paid dispatch', () => {
    const projection = projectTrainingPostTrainingInstructSft({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })

    expect(
      S.decodeUnknownSync(TrainingPostTrainingInstructSftProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.endpoint).toBe(TrainingPostTrainingInstructSftEndpoint)
    expect(projection.promiseRef).toBe('promise:training.post_training_arc.v1')
    expect(projection.promiseState).toBe('planned')
    expect(projection.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
    })
    expect(projection.gate).toEqual({
      clearsBlockerRefs: [InstructSftLaneMissingBlocker],
      greenGateSatisfied: false,
      committedReportFixtureSyncAvailable: true,
      instructSftLaneAvailable: true,
      instructSftPaidDispatchAvailable: false,
      preferenceRolloutWorkAvailable: false,
      publicProjectionAvailable: true,
      remainingBlockerRefs: [
        InstructSftPaidDispatchMissingBlocker,
        PreferenceRolloutWorkMissingBlocker,
        VibeTestArtifactMissingBlocker,
      ],
      vibeTestArtifactAvailable: false,
    })
    expect(projection.receiptSummary).toEqual({
      instructSftReceiptCount: 1,
      paidDispatchCount: 0,
      preferenceRolloutReceiptCount: 0,
      vibeTestArtifactReceiptCount: 0,
    })
    expect(projection.receipts[0]).toMatchObject({
      completedSteps: 8,
      laneId: 'psion_instruct_sft_v1',
      learningRateRatioBps: 1000,
      lossImproved: true,
      paidDispatchState: 'not_dispatched',
      publicSafe: true,
      receiptRef: TrainingPostTrainingInstructSftReceiptRef,
      reportDigest:
        'sha256:76b5524234b4dd6507560c0cda6f28e782fe097c1fb022108aaaae40794d6871',
      resumeDrill: {
        checkpointAtStep: 3,
        postResumeReceiptDigestsMatch: true,
        resumeBitExact: true,
        resumedSteps: 5,
      },
      template: {
        templateDigest:
          'sha256:7337ec749e64dbf1b23dbfeb3478788846c67e8247813f386d97b1ed1076fca3',
      },
    })
    expect(projection.receipts[0]?.clearsBlockerRefs).toEqual([
      InstructSftLaneMissingBlocker,
    ])
    expect(projection.receipts[0]?.blockerRefs).toEqual([
      InstructSftPaidDispatchMissingBlocker,
    ])
    expect(JSON.stringify(projection)).toContain(
      'scripts/check-psion-instruct-sft-lane.sh',
    )
    expect(JSON.stringify(projection)).toContain(
      'https://github.com/OpenAgentsInc/psionic/pull/1132',
    )
  })

  test('keeps authority and private material out of the public projection', () => {
    const projection = projectTrainingPostTrainingInstructSft({
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
      handleTrainingPostTrainingInstructSftApi(
        new Request(
          `https://openagents.com${TrainingPostTrainingInstructSftEndpoint}`,
        ),
      ),
    )
    const body = (await response.json()) as InstructSftBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.endpoint).toBe(TrainingPostTrainingInstructSftEndpoint)
    expect(body.promiseRef).toBe('promise:training.post_training_arc.v1')
    expect(body.promiseState).toBe('planned')
    expect(body.gate.instructSftLaneAvailable).toBe(true)
    expect(body.gate.committedReportFixtureSyncAvailable).toBe(true)
    expect(body.gate.instructSftPaidDispatchAvailable).toBe(false)
    expect(body.gate.preferenceRolloutWorkAvailable).toBe(false)
    expect(body.gate.vibeTestArtifactAvailable).toBe(false)
    expect(body.gate.greenGateSatisfied).toBe(false)
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleTrainingPostTrainingInstructSftApi(
        new Request(
          `https://openagents.com${TrainingPostTrainingInstructSftEndpoint}`,
          { method: 'POST' },
        ),
      ),
    )

    expect(response.status).toBe(405)
  })
})
