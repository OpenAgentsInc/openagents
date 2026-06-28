import { describe, expect, test } from 'vitest'

import {
  PostTrainingVibeTestCloseoutRef,
  PostTrainingVibeTestRubricRef,
} from './post-training-vibe-test-rubric'
import {
  InstructSftPaidDispatchMissingBlocker,
  PreferenceRolloutWorkMissingBlocker,
  VibeTestArtifactMissingBlocker,
} from './training-post-training-instruct-sft'
import {
  PostTrainingPaidDispatchPolicyRef,
  PostTrainingPaidDispatchVerificationCommandRef,
  buildPostTrainingArcPaidDispatchPlan,
  evaluatePostTrainingArcCloseout,
  type PostTrainingPaidWorkDispatchReceipt,
  type PostTrainingVibeTestArtifact,
} from './training-post-training-paid-dispatch'

const settledReceipt = (
  jobKind: PostTrainingPaidWorkDispatchReceipt['jobKind'],
): PostTrainingPaidWorkDispatchReceipt => ({
  dispatchReceiptRef: `dispatch.training.post_training_arc.${jobKind}.paid.v1`,
  jobKind,
  requestRef: `request.training.post_training_arc.${jobKind}.paid.v1`,
  settlementReceiptRef:
    `settlement.training.post_training_arc.${jobKind}.paid.v1`,
  workRequestRef: `work_request.training.post_training_arc.${jobKind}.paid.v1`,
})

const reviewedVibeArtifact = (): PostTrainingVibeTestArtifact => ({
  artifactRef: PostTrainingVibeTestCloseoutRef,
  closeoutDigestHex:
    '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
  realModelTranscriptArtifactRef:
    'artifact.training.post_training_arc.real_model_transcripts.reviewed.v1',
  reviewerSignatureRef:
    'signature.training.post_training_arc.vibe_test.reviewer.v1',
  rubricRef: PostTrainingVibeTestRubricRef,
})

describe('training post-training paid dispatch policy', () => {
  test('builds paid work requests for SFT plus preference rollout generation and reward grading', () => {
    const plan = buildPostTrainingArcPaidDispatchPlan()

    expect(plan.map(request => request.jobKind)).toEqual([
      'instruct_sft',
      'preference_rollout_generation',
      'preference_reward_grading',
    ])
    expect(
      plan.every(
        request =>
          request.policyRef === PostTrainingPaidDispatchPolicyRef &&
          request.promiseRef === 'promise:training.post_training_arc.v1' &&
          request.settlementRequired === true &&
          request.requiredVerificationClass === 'deterministic_recompute' &&
          request.verificationCommandRef ===
            PostTrainingPaidDispatchVerificationCommandRef,
      ),
    ).toBe(true)
    expect(plan[0]?.requiredEvidenceRefs).toContain(
      'route:/api/public/training/post-training-arc/instruct-sft-lane',
    )
    expect(plan[1]?.requiredEvidenceRefs).toContain(
      'route:/api/public/training/post-training-arc/dpo-preference-workload',
    )
    expect(plan[2]?.requiredEvidenceRefs).toContain(
      'receipt.training.post_training_arc.dpo_preference_workload.reference_grading.split_a.v1',
    )
    expect(JSON.stringify(plan)).not.toMatch(
      /wallet|invoice|preimage|payment_hash|secret|raw_prompt|private_repo|\/home\/|\/Users\//i,
    )
  })

  test('blocks closeout without settled paid work receipts', () => {
    const decision = evaluatePostTrainingArcCloseout({
      paidWorkReceipts: [],
      vibeTestArtifact: reviewedVibeArtifact(),
    })

    expect(decision.accepted).toBe(false)
    expect(decision.blockerRefs).toEqual([
      InstructSftPaidDispatchMissingBlocker,
      PreferenceRolloutWorkMissingBlocker,
    ])
    expect(decision.clearsBlockerRefs).toEqual([VibeTestArtifactMissingBlocker])
  })

  test('keeps preference rollout blocked until generation and reward grading are both paid', () => {
    const decision = evaluatePostTrainingArcCloseout({
      paidWorkReceipts: [
        settledReceipt('instruct_sft'),
        settledReceipt('preference_rollout_generation'),
      ],
      vibeTestArtifact: reviewedVibeArtifact(),
    })

    expect(decision.accepted).toBe(false)
    expect(decision.blockerRefs).toEqual([PreferenceRolloutWorkMissingBlocker])
    expect(decision.clearsBlockerRefs).toEqual([
      InstructSftPaidDispatchMissingBlocker,
      VibeTestArtifactMissingBlocker,
    ])
  })

  test('blocks closeout without a reviewed real-model vibe-test artifact', () => {
    const decision = evaluatePostTrainingArcCloseout({
      paidWorkReceipts: [
        settledReceipt('instruct_sft'),
        settledReceipt('preference_rollout_generation'),
        settledReceipt('preference_reward_grading'),
      ],
    })

    expect(decision.accepted).toBe(false)
    expect(decision.blockerRefs).toEqual([VibeTestArtifactMissingBlocker])
    expect(decision.clearsBlockerRefs).toEqual([
      InstructSftPaidDispatchMissingBlocker,
      PreferenceRolloutWorkMissingBlocker,
    ])
  })

  test('accepts only when paid dispatch receipts and vibe-test artifact gate are satisfied', () => {
    const decision = evaluatePostTrainingArcCloseout({
      paidWorkReceipts: [
        settledReceipt('instruct_sft'),
        settledReceipt('preference_rollout_generation'),
        settledReceipt('preference_reward_grading'),
      ],
      vibeTestArtifact: reviewedVibeArtifact(),
    })

    expect(decision).toMatchObject({
      accepted: true,
      blockerRefs: [],
      clearsBlockerRefs: [
        InstructSftPaidDispatchMissingBlocker,
        PreferenceRolloutWorkMissingBlocker,
        VibeTestArtifactMissingBlocker,
      ],
      gateRef: 'gate.training.post_training_arc.vibe_test_artifact_required.v1',
    })
    expect(decision.evidenceRefs).toEqual(
      expect.arrayContaining([
        'settlement.training.post_training_arc.instruct_sft.paid.v1',
        'settlement.training.post_training_arc.preference_rollout_generation.paid.v1',
        'settlement.training.post_training_arc.preference_reward_grading.paid.v1',
        'artifact.training.post_training_arc.real_model_transcripts.reviewed.v1',
        'signature.training.post_training_arc.vibe_test.reviewer.v1',
      ]),
    )
  })
})
