import { describe, expect, test } from 'vitest'

import { runPostTrainingVibeTestCloseout } from './post-training-vibe-test-rubric'
import {
  TrainingPostTrainingVibeTestCloseoutDigestHex,
  TrainingPostTrainingVibeTestCloseoutEndpoint,
  TrainingPostTrainingVibeTestCloseoutReceiptRef,
  projectTrainingPostTrainingVibeTestCloseout,
} from './training-post-training-vibe-test-closeout'

describe('training post-training vibe-test closeout projection', () => {
  test('committed digest reproduces the live machine-checked closeout', async () => {
    const result = await runPostTrainingVibeTestCloseout()

    expect(result.closeoutDigestHex).toBe(
      TrainingPostTrainingVibeTestCloseoutDigestHex,
    )
    expect(result.reviewerSigned).toBe(false)
    expect(result.closeoutAcceptable).toBe(true)
  })

  test('projection publishes the machine-checked half and keeps reviewer-signed gate open', () => {
    const projection = projectTrainingPostTrainingVibeTestCloseout({
      generatedAt: '2026-06-20T00:00:00.000Z',
    })

    expect(projection.endpoint).toBe(
      TrainingPostTrainingVibeTestCloseoutEndpoint,
    )
    expect(projection.promiseState).toBe('planned')
    expect(projection.status).toBe(
      'vibe_test_machine_checked_closeout_available',
    )
    expect(projection.gate.machineCheckedCloseoutAvailable).toBe(true)
    expect(projection.gate.reviewerSignedCloseoutAvailable).toBe(false)
    expect(projection.gate.vibeTestArtifactAvailable).toBe(false)
    expect(projection.gate.greenGateSatisfied).toBe(false)
    expect(projection.gate.remainingProductBlockerRefs).toContain(
      'blocker.product_promises.vibe_test_artifact_missing',
    )
    expect(projection.gate.clearsBlockerRefs).toEqual([])
  })

  test('the single receipt is the deterministic machine-checked closeout', () => {
    const projection = projectTrainingPostTrainingVibeTestCloseout({
      generatedAt: '2026-06-20T00:00:00.000Z',
    })

    expect(projection.receipts).toHaveLength(1)
    const receipt = projection.receipts[0]
    expect(receipt?.receiptRef).toBe(
      TrainingPostTrainingVibeTestCloseoutReceiptRef,
    )
    expect(receipt?.closeoutDigestHex).toBe(
      TrainingPostTrainingVibeTestCloseoutDigestHex,
    )
    expect(receipt?.reviewerSigned).toBe(false)
    expect(receipt?.machineCheckedAvailable).toBe(true)
    expect(receipt?.verificationClass).toBe('deterministic_recompute')
    expect(receipt?.transcriptRefs.length).toBe(4)
  })
})
