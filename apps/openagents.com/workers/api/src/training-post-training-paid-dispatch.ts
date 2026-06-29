import {
  InstructSftPaidDispatchMissingBlocker,
  PreferenceRolloutWorkMissingBlocker,
  VibeTestArtifactMissingBlocker,
} from './training-post-training-instruct-sft'
import {
  PostTrainingVibeTestCloseoutRef,
  PostTrainingVibeTestRubricRef,
} from './post-training-vibe-test-rubric'

export const PostTrainingArcPromiseRef = 'promise:training.post_training_arc.v1'
export const PostTrainingPaidDispatchPolicyRef =
  'policy.training.post_training_arc.paid_dispatch.v1'
export const PostTrainingPaidDispatchVerificationCommandRef =
  'command.public.training.post_training_arc.verify_paid_dispatch_v1'
export const PostTrainingVibeTestArtifactGateRef =
  'gate.training.post_training_arc.vibe_test_artifact_required.v1'

export type PostTrainingPaidWorkKind =
  | 'instruct_sft'
  | 'preference_rollout_generation'
  | 'preference_reward_grading'

export type PostTrainingPaidWorkRequest = Readonly<{
  budgetSats: number
  jobKind: PostTrainingPaidWorkKind
  policyRef: typeof PostTrainingPaidDispatchPolicyRef
  promiseRef: typeof PostTrainingArcPromiseRef
  requiredCapabilityRefs: ReadonlyArray<string>
  requiredEvidenceRefs: ReadonlyArray<string>
  requiredVerificationClass: 'deterministic_recompute'
  requestRef: string
  settlementRequired: true
  title: string
  verificationCommandRef: typeof PostTrainingPaidDispatchVerificationCommandRef
}>

export type PostTrainingPaidWorkDispatchReceipt = Readonly<{
  dispatchReceiptRef: string
  jobKind: PostTrainingPaidWorkKind
  requestRef: string
  settlementReceiptRef: string
  workRequestRef: string
}>

export type PostTrainingVibeTestArtifact = Readonly<{
  artifactRef: typeof PostTrainingVibeTestCloseoutRef
  closeoutDigestHex: string
  realModelTranscriptArtifactRef: string
  reviewerSignatureRef: string
  rubricRef: typeof PostTrainingVibeTestRubricRef
}>

export type PostTrainingArcCloseoutInput = Readonly<{
  paidWorkReceipts: ReadonlyArray<PostTrainingPaidWorkDispatchReceipt>
  vibeTestArtifact?: PostTrainingVibeTestArtifact | undefined
}>

export type PostTrainingArcCloseoutDecision = Readonly<{
  accepted: boolean
  blockerRefs: ReadonlyArray<string>
  clearsBlockerRefs: ReadonlyArray<string>
  evidenceRefs: ReadonlyArray<string>
  gateRef: typeof PostTrainingVibeTestArtifactGateRef
}>

export class PostTrainingPaidDispatchError extends Error {
  readonly _tag = 'PostTrainingPaidDispatchError'
}

const unsafePublicMaterialPattern =
  /(\/Users\/|\/home\/|access[_-]?token|api[_-]?key|bearer|bolt11|bolt12|cookie|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage|raw)|preimage|private[_-]?(key|repo|source)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(dataset|invoice|payment|payload|prompt|runner|trace)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const assertPublicSafeValue = (value: unknown): void => {
  if (unsafePublicMaterialPattern.test(JSON.stringify(value))) {
    throw new PostTrainingPaidDispatchError(
      'Post-training paid dispatch material must be public-safe.',
    )
  }
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(ref => ref.trim().length > 0)

const paidWorkRequest = (
  input: Readonly<{
    budgetSats: number
    jobKind: PostTrainingPaidWorkKind
    requiredCapabilityRefs: ReadonlyArray<string>
    requiredEvidenceRefs: ReadonlyArray<string>
    requestRef: string
    title: string
  }>,
): PostTrainingPaidWorkRequest => {
  const request: PostTrainingPaidWorkRequest = {
    budgetSats: input.budgetSats,
    jobKind: input.jobKind,
    policyRef: PostTrainingPaidDispatchPolicyRef,
    promiseRef: PostTrainingArcPromiseRef,
    requiredCapabilityRefs: input.requiredCapabilityRefs,
    requiredEvidenceRefs: input.requiredEvidenceRefs,
    requiredVerificationClass: 'deterministic_recompute',
    requestRef: input.requestRef,
    settlementRequired: true,
    title: input.title,
    verificationCommandRef: PostTrainingPaidDispatchVerificationCommandRef,
  }

  if (!Number.isInteger(request.budgetSats) || request.budgetSats <= 0) {
    throw new PostTrainingPaidDispatchError(
      'Post-training paid work requests require a positive integer sat budget.',
    )
  }
  if (request.title.trim().length < 8 || request.title.length > 160) {
    throw new PostTrainingPaidDispatchError(
      'Post-training paid work request titles must be 8-160 characters.',
    )
  }
  if (
    request.requiredCapabilityRefs.length === 0 ||
    request.requiredEvidenceRefs.length === 0
  ) {
    throw new PostTrainingPaidDispatchError(
      'Post-training paid work requests require capability and evidence refs.',
    )
  }

  assertPublicSafeValue(request)
  return request
}

export const buildPostTrainingArcPaidDispatchPlan =
  (): ReadonlyArray<PostTrainingPaidWorkRequest> => [
    paidWorkRequest({
      budgetSats: 40,
      jobKind: 'instruct_sft',
      requestRef: 'request.training.post_training_arc.instruct_sft_paid.v1',
      requiredCapabilityRefs: [
        'capability.training.instruct_sft',
        'capability.verification.deterministic_recompute',
      ],
      requiredEvidenceRefs: [
        'route:/api/public/training/post-training-arc/instruct-sft-lane',
        'receipt.training.post_training_arc.instruct_sft_lane.psion_fixture.v1',
      ],
      title: 'Run the bounded Psion instruct-SFT lane as paid network work',
    }),
    paidWorkRequest({
      budgetSats: 40,
      jobKind: 'preference_rollout_generation',
      requestRef:
        'request.training.post_training_arc.preference_rollout_generation_paid.v1',
      requiredCapabilityRefs: [
        'capability.training.preference_rollout_generation',
        'capability.verification.deterministic_recompute',
      ],
      requiredEvidenceRefs: [
        'route:/api/public/training/post-training-arc/dpo-preference-workload',
        'workload.cs336_a5.dpo_preference_pair_reference_grading.v1',
      ],
      title: 'Generate bounded post-training preference rollouts as paid work',
    }),
    paidWorkRequest({
      budgetSats: 40,
      jobKind: 'preference_reward_grading',
      requestRef:
        'request.training.post_training_arc.preference_reward_grading_paid.v1',
      requiredCapabilityRefs: [
        'capability.training.preference_reward_grading',
        'capability.verification.deterministic_recompute',
      ],
      requiredEvidenceRefs: [
        'route:/api/public/training/post-training-arc/dpo-preference-workload',
        'receipt.training.post_training_arc.dpo_preference_workload.reference_grading.split_a.v1',
      ],
      title: 'Grade bounded post-training preference rewards as paid work',
    }),
  ]

const hasSettledKind = (
  receipts: ReadonlyArray<PostTrainingPaidWorkDispatchReceipt>,
  jobKind: PostTrainingPaidWorkKind,
): boolean =>
  receipts.some(
    receipt =>
      receipt.jobKind === jobKind &&
      receipt.dispatchReceiptRef.trim().length > 0 &&
      receipt.settlementReceiptRef.trim().length > 0 &&
      receipt.workRequestRef.trim().length > 0,
  )

const hasVibeTestArtifact = (
  artifact: PostTrainingVibeTestArtifact | undefined,
): artifact is PostTrainingVibeTestArtifact =>
  artifact !== undefined &&
  artifact.artifactRef === PostTrainingVibeTestCloseoutRef &&
  artifact.rubricRef === PostTrainingVibeTestRubricRef &&
  /^[0-9a-f]{64}$/.test(artifact.closeoutDigestHex) &&
  artifact.realModelTranscriptArtifactRef.trim().length > 0 &&
  artifact.reviewerSignatureRef.trim().length > 0

export const evaluatePostTrainingArcCloseout = (
  input: PostTrainingArcCloseoutInput,
): PostTrainingArcCloseoutDecision => {
  assertPublicSafeValue(input)
  const vibeTestArtifact = input.vibeTestArtifact

  const instructSftSettled = hasSettledKind(
    input.paidWorkReceipts,
    'instruct_sft',
  )
  const preferenceRolloutGenerationSettled = hasSettledKind(
    input.paidWorkReceipts,
    'preference_rollout_generation',
  )
  const preferenceRewardGradingSettled = hasSettledKind(
    input.paidWorkReceipts,
    'preference_reward_grading',
  )
  const artifactAvailable = hasVibeTestArtifact(vibeTestArtifact)

  const blockerRefs = uniqueRefs([
    ...(instructSftSettled ? [] : [InstructSftPaidDispatchMissingBlocker]),
    ...(preferenceRolloutGenerationSettled && preferenceRewardGradingSettled
      ? []
      : [PreferenceRolloutWorkMissingBlocker]),
    ...(artifactAvailable ? [] : [VibeTestArtifactMissingBlocker]),
  ])
  const evidenceRefs = uniqueRefs([
    ...input.paidWorkReceipts.flatMap(receipt => [
      receipt.dispatchReceiptRef,
      receipt.settlementReceiptRef,
      receipt.workRequestRef,
    ]),
    ...(artifactAvailable && vibeTestArtifact !== undefined
      ? [
          vibeTestArtifact.artifactRef,
          vibeTestArtifact.realModelTranscriptArtifactRef,
          vibeTestArtifact.reviewerSignatureRef,
          vibeTestArtifact.rubricRef,
        ]
      : []),
  ])

  const decision: PostTrainingArcCloseoutDecision = {
    accepted: blockerRefs.length === 0,
    blockerRefs,
    clearsBlockerRefs: uniqueRefs([
      ...(instructSftSettled ? [InstructSftPaidDispatchMissingBlocker] : []),
      ...(preferenceRolloutGenerationSettled && preferenceRewardGradingSettled
        ? [PreferenceRolloutWorkMissingBlocker]
        : []),
      ...(artifactAvailable ? [VibeTestArtifactMissingBlocker] : []),
    ]),
    evidenceRefs,
    gateRef: PostTrainingVibeTestArtifactGateRef,
  }

  assertPublicSafeValue(decision)
  return decision
}
