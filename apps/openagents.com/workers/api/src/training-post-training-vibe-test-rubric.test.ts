import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PostTrainingVibeTestCloseoutRef,
  PostTrainingVibeTestRubricRef,
  runPostTrainingVibeTestCloseout,
} from './post-training-vibe-test-rubric'
import {
  VibeTestArtifactMissingBlocker,
} from './training-post-training-instruct-sft'
import { handleTrainingPostTrainingVibeTestRubricApi } from './training-post-training-vibe-test-rubric-routes'
import {
  TrainingPostTrainingVibeTestRubricEndpoint,
  TrainingPostTrainingVibeTestRubricProjection,
  TrainingPostTrainingVibeTestRubricReceiptRef,
  projectTrainingPostTrainingVibeTestRubric,
} from './training-post-training-vibe-test-rubric'

type VibeTestRubricBody = Readonly<{
  endpoint: string
  gate: Readonly<{
    closeoutAcceptable: boolean
    deterministicCloseoutDigestAvailable: boolean
    greenGateSatisfied: boolean
    realModelTranscriptArtifactAvailable: boolean
    reviewerSignedCloseoutAvailable: boolean
    vibeTestArtifactAvailable: boolean
  }>
  promiseRef: string
  promiseState: string
  receiptSummary: Readonly<{
    realModelTranscriptArtifactCount: number
    reviewerSignedCloseoutCount: number
    rubricReceiptCount: number
  }>
  receipts: ReadonlyArray<
    Readonly<{
      closeoutDigestHex: string
      reviewerSigned: boolean
      stats: Readonly<{ transcriptCount: number }>
    }>
  >
}>

describe('training post-training vibe-test rubric projection', () => {
  test('publishes the deterministic rubric closeout without claiming a reviewed artifact', async () => {
    const projection = await projectTrainingPostTrainingVibeTestRubric({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })

    expect(
      S.decodeUnknownSync(TrainingPostTrainingVibeTestRubricProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.endpoint).toBe(
      TrainingPostTrainingVibeTestRubricEndpoint,
    )
    expect(projection.promiseRef).toBe('promise:training.post_training_arc.v1')
    expect(projection.promiseState).toBe('planned')
    expect(projection.gate).toMatchObject({
      clearsBlockerRefs: [],
      closeoutAcceptable: true,
      deterministicCloseoutDigestAvailable: true,
      greenGateSatisfied: false,
      publicProjectionAvailable: true,
      realModelTranscriptArtifactAvailable: false,
      remainingBlockerRefs: [VibeTestArtifactMissingBlocker],
      repoOwnedFixtureTranscriptsAvailable: true,
      reviewerSignedCloseoutAvailable: false,
      rubricAvailable: true,
      vibeTestArtifactAvailable: false,
    })
    expect(projection.receiptSummary).toEqual({
      realModelTranscriptArtifactCount: 0,
      reviewerSignedCloseoutCount: 0,
      rubricReceiptCount: 1,
    })
    expect(projection.receipts[0]).toMatchObject({
      artifactRef: PostTrainingVibeTestCloseoutRef,
      closeoutAcceptable: true,
      fixtureTranscriptBoundary: 'repo_owned_fixture_not_model_output',
      publicSafe: true,
      receiptRef: TrainingPostTrainingVibeTestRubricReceiptRef,
      reviewerSigned: false,
      rubricRef: PostTrainingVibeTestRubricRef,
      verificationClass: 'deterministic_recompute',
    })
    expect(projection.receipts[0]?.blockerRefs).toEqual([
      VibeTestArtifactMissingBlocker,
    ])
    expect(projection.receipts[0]?.clearsBlockerRefs).toEqual([])
    expect(projection.receipts[0]?.closeoutDigestHex).toMatch(
      /^[0-9a-f]{64}$/,
    )
    expect(projection.receipts[0]?.stats).toMatchObject({
      meanScoreMicro: 1_000_000,
      passRateBp: 10_000,
      passedTranscriptCount: 4,
      thresholdMicro: 900_000,
      transcriptCount: 4,
    })
  })

  test('pins the public receipt digest to the deterministic closeout runner', async () => {
    const recomputed = await runPostTrainingVibeTestCloseout()
    const projection = await projectTrainingPostTrainingVibeTestRubric()

    expect(projection.receipts[0]?.closeoutDigestHex).toBe(
      recomputed.closeoutDigestHex,
    )
    expect(recomputed.reviewerSigned).toBe(false)
  })

  test('keeps authority and private material out of the public projection', async () => {
    const projection = await projectTrainingPostTrainingVibeTestRubric({
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
      handleTrainingPostTrainingVibeTestRubricApi(
        new Request(
          `https://openagents.com${TrainingPostTrainingVibeTestRubricEndpoint}`,
        ),
      ),
    )
    const body = (await response.json()) as VibeTestRubricBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.endpoint).toBe(TrainingPostTrainingVibeTestRubricEndpoint)
    expect(body.promiseRef).toBe('promise:training.post_training_arc.v1')
    expect(body.promiseState).toBe('planned')
    expect(body.gate.closeoutAcceptable).toBe(true)
    expect(body.gate.deterministicCloseoutDigestAvailable).toBe(true)
    expect(body.gate.realModelTranscriptArtifactAvailable).toBe(false)
    expect(body.gate.reviewerSignedCloseoutAvailable).toBe(false)
    expect(body.gate.vibeTestArtifactAvailable).toBe(false)
    expect(body.gate.greenGateSatisfied).toBe(false)
    expect(body.receiptSummary).toEqual({
      realModelTranscriptArtifactCount: 0,
      reviewerSignedCloseoutCount: 0,
      rubricReceiptCount: 1,
    })
    expect(body.receipts[0]?.closeoutDigestHex).toMatch(/^[0-9a-f]{64}$/)
    expect(body.receipts[0]?.reviewerSigned).toBe(false)
    expect(body.receipts[0]?.stats.transcriptCount).toBe(4)
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleTrainingPostTrainingVibeTestRubricApi(
        new Request(
          `https://openagents.com${TrainingPostTrainingVibeTestRubricEndpoint}`,
          { method: 'POST' },
        ),
      ),
    )

    expect(response.status).toBe(405)
  })
})
