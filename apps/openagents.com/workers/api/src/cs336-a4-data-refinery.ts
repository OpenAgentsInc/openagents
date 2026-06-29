import type { BuyModeDispatchInput } from './buy-mode-dispatcher'
import type { TrainingVerificationChallengeCreateRequest } from './training-verification'

export const Cs336A4DataRefineryJobKind = 'cs336_a4_data_refinery'
export const Cs336A4PsionicLaneRef = 'psion_cs336_a4_data_refinery_reference_v1'
export const Cs336A4RequestSchemaRef = 'psion.cs336_a4_data_refinery_request.v1'
export const Cs336A4OutputSchemaRef = 'psion.cs336_a4_data_refinery_output.v1'

export const Cs336A4HomeworkStages = [
  'pii_masking',
  'gopher_rules',
  'exact_line_dedup',
  'minhash_dedup',
] as const
export type Cs336A4HomeworkStage = (typeof Cs336A4HomeworkStages)[number]

export type Cs336A4HomeworkPayload = Readonly<{
  assignmentRef: string
  inputShardRef: string
  jobKind: typeof Cs336A4DataRefineryJobKind
  outputSchemaRef: typeof Cs336A4OutputSchemaRef
  psionicLaneRef: typeof Cs336A4PsionicLaneRef
  redactionPolicyRefs: ReadonlyArray<string>
  requestSchemaRef: typeof Cs336A4RequestSchemaRef
  stage: Cs336A4HomeworkStage
  verificationClass: 'deterministic_recompute'
}>

export type Cs336A4CloseoutEvidence = Readonly<{
  assignmentRef: string
  inputShardRef: string
  outputDigestRef: string
  recomputedDigestRef: string
  stage: Cs336A4HomeworkStage
  workerReceiptRef: string
}>

export type Cs336A4PaymentPolicy = Readonly<{
  baseRatePolicyRef: string
  bonusPolicyRef: string
  blockerRefs: ReadonlyArray<string>
  qualityMeasurementRef: string
  volumeBoundaryRef: string
}>

export type Cs336A4DispatchInput = Omit<BuyModeDispatchInput, 'content'> &
  Readonly<{
    assignmentRef: string
    inputShardRef: string
    stage: Cs336A4HomeworkStage
  }>

export class Cs336A4DataRefineryUnsafeProjectionError extends Error {
  readonly _tag = 'Cs336A4DataRefineryUnsafeProjectionError'
}

const unsafePublicMaterialPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|bolt11|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|raw[_-]?(dataset|invoice|payment|payload|prompt|runner|shard)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet)/i

const publicSafeJson = (value: unknown): string => {
  const json = JSON.stringify(value)

  if (unsafePublicMaterialPattern.test(json)) {
    throw new Cs336A4DataRefineryUnsafeProjectionError(
      'CS336 A4 data-refinery payload is not public-safe.',
    )
  }

  return json
}

export const buildCs336A4HomeworkPayload = (
  input: Readonly<{
    assignmentRef: string
    inputShardRef: string
    stage: Cs336A4HomeworkStage
  }>,
): Cs336A4HomeworkPayload => {
  const payload: Cs336A4HomeworkPayload = {
    assignmentRef: input.assignmentRef,
    inputShardRef: input.inputShardRef,
    jobKind: Cs336A4DataRefineryJobKind,
    outputSchemaRef: Cs336A4OutputSchemaRef,
    psionicLaneRef: Cs336A4PsionicLaneRef,
    redactionPolicyRefs: [
      'policy.public.data_refinery.no_common_crawl_payload_publication',
      'policy.public.data_refinery.no_contributor_sourced_sensitive_material',
      'policy.public.data_refinery.redaction_rules_from_issue_4644_apply',
    ],
    requestSchemaRef: Cs336A4RequestSchemaRef,
    stage: input.stage,
    verificationClass: 'deterministic_recompute',
  }

  publicSafeJson(payload)

  return payload
}

export const cs336A4VerificationChallengeRequest = (
  input: Readonly<{
    closeout: Cs336A4CloseoutEvidence
    trainingRunRef: string
    windowRef: string
  }>,
): TrainingVerificationChallengeCreateRequest => ({
  commitmentRefs: [
    `commitment.cs336_a4.${input.closeout.assignmentRef}.${input.closeout.stage}`,
  ],
  contributionRef: `contribution.cs336_a4.${input.closeout.assignmentRef}.${input.closeout.stage}`,
  homeworkKind: 'admin_dispatched_homework',
  payload: {
    contributionRefs: [
      `contribution.cs336_a4.${input.closeout.assignmentRef}.${input.closeout.stage}`,
    ],
    expectedDigestRef: input.closeout.outputDigestRef,
    recomputedDigestRef: input.closeout.recomputedDigestRef,
    stage: input.closeout.stage,
  },
  samplingPolicy: 'per_contribution',
  trainingRunRef: input.trainingRunRef,
  verificationClass: 'deterministic_recompute',
  windowRef: input.windowRef,
})

export const cs336A4EvalDeltaPaymentPolicy = (): Cs336A4PaymentPolicy => ({
  baseRatePolicyRef: 'policy.cs336_a4.pay_per_verified_shard_processed',
  blockerRefs: [
    'blocker.cs336_a4.fixed_trainer_eval_loop_required_for_quality_bonus',
    'blocker.cs336_a4.operator_funding_required_for_bonus_settlement',
    'blocker.cs336_a4.psionic_classifier_adapters_partial',
  ],
  bonusPolicyRef: 'policy.cs336_a4.eval_delta_quality_bonus_pending',
  qualityMeasurementRef:
    'measurement.cs336_a4.downstream_eval_delta_fixed_reference_model',
  volumeBoundaryRef:
    'boundary.cs336_a4.pay_quality_delta_not_raw_volume_or_private_data',
})

export const cs336A4NoSpendReadiness = () => ({
  dispatchableStages: Cs336A4HomeworkStages,
  paymentPolicy: cs336A4EvalDeltaPaymentPolicy(),
  psionicConformanceRefs: [
    'psionic#1102:pii_masking_partial',
    'psionic#1102:gopher_rules_landed',
    'psionic#1102:exact_line_dedup_landed',
    'psionic#1102:minhash_dedup_landed',
  ],
})
