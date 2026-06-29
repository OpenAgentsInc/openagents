import {
  isRecord,
  optionalString,
  parseJsonRecord,
  stringArrayFromUnknown,
} from './json-boundary'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
  TrainingWindowRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeCreateRequest } from './training-verification'
import type { TrainingVerificationChallengeRecord } from './training-verification'

export const Cs336A5PsionicLaneRef =
  'psion_cs336_a5_alignment_reference_v1'
export const Cs336A5RolloutJobKind = 'cs336_a5_rollout_batch'
export const Cs336A5GradingJobKind = 'cs336_a5_reward_grading'
export const Cs336A5SftPackingJobKind = 'cs336_a5_sft_packing'
export const Cs336A5UpdateBoundaryRef = 'issue.github.openagents.4669'
export const Cs336A5RequestSchemaRef =
  'openagents.cs336_a5_alignment_homework_request.v1'
export const Cs336A5OutputSchemaRef =
  'openagents.cs336_a5_alignment_homework_output.v1'

export const Cs336A5JobKinds = [
  Cs336A5RolloutJobKind,
  Cs336A5GradingJobKind,
  Cs336A5SftPackingJobKind,
] as const
export type Cs336A5JobKind = (typeof Cs336A5JobKinds)[number]

export type Cs336A5HomeworkPayload = Readonly<{
  assignmentRef: string
  jobKind: Cs336A5JobKind
  outputSchemaRef: typeof Cs336A5OutputSchemaRef
  psionicLaneRef: typeof Cs336A5PsionicLaneRef
  publicEvalSuiteRefs: ReadonlyArray<string>
  requestSchemaRef: typeof Cs336A5RequestSchemaRef
  updateBoundaryRef: typeof Cs336A5UpdateBoundaryRef
  verificationClass: 'deterministic_recompute' | 'seeded_replication'
}>

export type Cs336A5CloseoutEvidence = Readonly<{
  assignmentRef: string
  jobKind: Cs336A5JobKind
  outputDigestRef: string
  recomputedDigestRef?: string
  replicatedDigestRef?: string
  workerReceiptRef: string
}>

export type Cs336A5EvalSuiteProjection = Readonly<{
  evalSuiteRef: string
  metric: 'accuracy' | 'pass_rate' | 'reward_mean'
  psionicLaneRef: typeof Cs336A5PsionicLaneRef
  receiptRefs: ReadonlyArray<string>
  sampleCount: number
  scopeBoundaryRefs: ReadonlyArray<string>
  score: number
  sourceRefs: ReadonlyArray<string>
  splitRef: string
  taskSetRef: 'gsm8k' | 'mmlu' | 'math'
  verificationRefs: ReadonlyArray<string>
  verifiedSampleCount: number
}>

export type Cs336A5EvalDashboardProjection = Readonly<{
  blockerRefs: ReadonlyArray<string>
  evalSuites: ReadonlyArray<Cs336A5EvalSuiteProjection>
  jobKinds: ReadonlyArray<Cs336A5JobKind>
  psionicAskRefs: ReadonlyArray<string>
  schemaVersion: 'openagents.training.a5_eval_dashboard.v1'
  sourceRefs: ReadonlyArray<string>
  updateBoundaryRef: typeof Cs336A5UpdateBoundaryRef
}>

export class Cs336A5UnsafeProjectionError extends Error {
  readonly _tag = 'Cs336A5UnsafeProjectionError'
}

const unsafePublicMaterialPattern =
  /(\"?(prompt|answer|completion|raw[A-Za-z0-9_-]*|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const optionalNumber = (value: unknown): number | undefined => {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value
  }

  if (typeof value !== 'string' || value.trim() === '') {
    return undefined
  }

  const parsed = Number(value)

  return Number.isFinite(parsed) ? parsed : undefined
}

const jobKindVerificationClass = (
  jobKind: Cs336A5JobKind,
): Cs336A5HomeworkPayload['verificationClass'] =>
  jobKind === Cs336A5RolloutJobKind
    ? 'seeded_replication'
    : 'deterministic_recompute'

const taskSetFromUnknown = (
  value: unknown,
): Cs336A5EvalSuiteProjection['taskSetRef'] | undefined => {
  const text = optionalString(value)

  return text === 'gsm8k' || text === 'mmlu' || text === 'math'
    ? text
    : undefined
}

const metricFromUnknown = (
  value: unknown,
): Cs336A5EvalSuiteProjection['metric'] | undefined => {
  const text = optionalString(value)

  return text === 'accuracy' || text === 'pass_rate' || text === 'reward_mean'
    ? text
    : undefined
}

const publicSafeJson = (value: unknown): string => {
  const json = JSON.stringify(value)

  if (unsafePublicMaterialPattern.test(json)) {
    throw new Cs336A5UnsafeProjectionError(
      'CS336 A5 alignment projection contains raw eval, private, or payment material.',
    )
  }

  return json
}

const alignmentEvidenceRecord = (
  run: TrainingRunRecord,
): Record<string, unknown> | undefined => {
  const projection = parseJsonRecord(run.publicProjectionJson)
  const nested = projection?.a5Alignment

  return isRecord(nested) ? nested : undefined
}

const evalSuitesFromEvidence = (
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    evidence: Record<string, unknown> | undefined
    leases: ReadonlyArray<TrainingWindowLeaseRecord>
    run: TrainingRunRecord
    windows: ReadonlyArray<TrainingWindowRecord>
  }>,
): ReadonlyArray<Cs336A5EvalSuiteProjection> => {
  const evalSuites = input.evidence?.evalSuites

  if (!Array.isArray(evalSuites)) {
    return []
  }

  publicSafeJson(evalSuites)

  const verifiedChallengeRefs = input.challenges
    .filter(challenge => challenge.state === 'Verified')
    .map(challenge => challenge.challengeRef)

  return evalSuites.flatMap(
    (suite, index): ReadonlyArray<Cs336A5EvalSuiteProjection> => {
      if (!isRecord(suite)) {
        return []
      }

      const taskSetRef = taskSetFromUnknown(suite.taskSetRef)
      const metric = metricFromUnknown(suite.metric)
      const score = optionalNumber(suite.score)
      const sampleCount = optionalNumber(suite.sampleCount)
      const verifiedSampleCount = optionalNumber(suite.verifiedSampleCount)
      const splitRef = optionalString(suite.splitRef)

      if (
        taskSetRef === undefined ||
        metric === undefined ||
        score === undefined ||
        sampleCount === undefined ||
        verifiedSampleCount === undefined ||
        splitRef === undefined
      ) {
        return []
      }

      const projected: Cs336A5EvalSuiteProjection = {
        evalSuiteRef:
          optionalString(suite.evalSuiteRef) ??
          `eval.cs336_a5.${input.run.trainingRunRef}.${index + 1}`,
        metric,
        psionicLaneRef: Cs336A5PsionicLaneRef,
        receiptRefs: uniqueRefs(stringArrayFromUnknown(suite.receiptRefs)),
        sampleCount,
        scopeBoundaryRefs: [
          'scope.cs336_a5.eval_results_not_capability_claims',
          'scope.cs336_a5.no_raw_prompts_or_answers_public',
          'scope.cs336_a5.update_step_waits_on_training_boundary_4669',
        ],
        score,
        sourceRefs: uniqueRefs([
          ...stringArrayFromUnknown(suite.sourceRefs),
          ...input.run.sourceRefs,
          ...input.windows.flatMap(window => window.sourceRefs),
          ...input.leases.map(lease => lease.leaseRef),
        ]),
        splitRef,
        taskSetRef,
        verificationRefs: uniqueRefs([
          ...stringArrayFromUnknown(suite.verificationRefs),
          ...verifiedChallengeRefs,
        ]),
        verifiedSampleCount,
      }

      publicSafeJson(projected)

      return [projected]
    },
  )
}

export const buildCs336A5HomeworkPayload = (
  input: Readonly<{
    assignmentRef: string
    jobKind: Cs336A5JobKind
  }>,
): Cs336A5HomeworkPayload => {
  const payload: Cs336A5HomeworkPayload = {
    assignmentRef: input.assignmentRef,
    jobKind: input.jobKind,
    outputSchemaRef: Cs336A5OutputSchemaRef,
    psionicLaneRef: Cs336A5PsionicLaneRef,
    publicEvalSuiteRefs: ['eval_suite.gsm8k', 'eval_suite.mmlu'],
    requestSchemaRef: Cs336A5RequestSchemaRef,
    updateBoundaryRef: Cs336A5UpdateBoundaryRef,
    verificationClass: jobKindVerificationClass(input.jobKind),
  }

  publicSafeJson(payload)

  return payload
}

export const cs336A5VerificationChallengeRequest = (
  input: Readonly<{
    closeout: Cs336A5CloseoutEvidence
    trainingRunRef: string
    windowRef: string
  }>,
): TrainingVerificationChallengeCreateRequest => {
  const verificationClass = jobKindVerificationClass(input.closeout.jobKind)
  const replicatedDigestRef =
    input.closeout.replicatedDigestRef ?? input.closeout.outputDigestRef
  const recomputedDigestRef =
    input.closeout.recomputedDigestRef ?? input.closeout.outputDigestRef

  return {
    commitmentRefs: [
      `commitment.cs336_a5.${input.closeout.assignmentRef}.${input.closeout.jobKind}`,
    ],
    contributionRef: `contribution.cs336_a5.${input.closeout.assignmentRef}.${input.closeout.jobKind}`,
    homeworkKind: 'admin_dispatched_homework',
    payload: {
      expectedDigestRef: input.closeout.outputDigestRef,
      replicatedDigestRef:
        verificationClass === 'seeded_replication'
          ? replicatedDigestRef
          : undefined,
      recomputedDigestRef:
        verificationClass === 'deterministic_recompute'
          ? recomputedDigestRef
          : undefined,
    },
    samplingPolicy:
      verificationClass === 'seeded_replication'
        ? 'aggregate'
        : 'per_contribution',
    trainingRunRef: input.trainingRunRef,
    verificationClass,
    windowRef: input.windowRef,
  }
}

export const publicCs336A5EvalProjection = (
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    leases: ReadonlyArray<TrainingWindowLeaseRecord>
    run: TrainingRunRecord
    windows: ReadonlyArray<TrainingWindowRecord>
  }>,
): Cs336A5EvalDashboardProjection => {
  const evidence = alignmentEvidenceRecord(input.run)
  const evalSuites = evalSuitesFromEvidence({ ...input, evidence })

  return {
    blockerRefs:
      evalSuites.length > 0 &&
      evalSuites.some(suite => suite.verificationRefs.length > 0)
        ? []
        : [
            'blocker.cs336_a5.requires_rollout_receipts',
            'blocker.cs336_a5.requires_grading_verification',
            'blocker.cs336_a5.requires_public_eval_suite_receipt',
            'blocker.cs336_a5.policy_gradient_update_waits_on_4669',
          ],
    evalSuites,
    jobKinds: Cs336A5JobKinds,
    psionicAskRefs: [
      'psionic#1101:alignment_reference_tranche_landed',
      'psionic.todo.response_log_probs_model_coupled',
      'psionic.todo.grpo_train_step_training_boundary_4669',
      'psionic.todo.parser_fixture_conformance_before_paid_grading',
    ],
    schemaVersion: 'openagents.training.a5_eval_dashboard.v1',
    sourceRefs: uniqueRefs([
      'route:/api/training/evals/a5',
      `route:/api/training/runs/${input.run.trainingRunRef}`,
      ...input.run.sourceRefs,
      ...input.windows.flatMap(window => window.sourceRefs),
      ...input.leases.map(lease => lease.leaseRef),
    ]),
    updateBoundaryRef: Cs336A5UpdateBoundaryRef,
  }
}

export const cs336A5NoSpendReadiness = () => ({
  jobKinds: Cs336A5JobKinds.map(jobKind =>
    buildCs336A5HomeworkPayload({
      assignmentRef: `assignment.cs336_a5.${jobKind}.example`,
      jobKind,
    }),
  ),
  psionicLaneRef: Cs336A5PsionicLaneRef,
  updateBoundaryRef: Cs336A5UpdateBoundaryRef,
})
