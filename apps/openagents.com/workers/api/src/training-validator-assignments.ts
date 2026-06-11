import type { PylonApiCreateAssignmentRequest } from './pylon-api'
import type {
  TrainingVerificationChallengeRecord,
  TrainingVerificationClass,
  TrainingVerificationFailureCode,
} from './training-verification'

export const TrainingValidatorCapabilityRef =
  'capability.public.training_verification'
export const TrainingValidatorSelectionPolicyRef =
  'selection.public.training_validator.dispatcher_controlled'
export const TrainingValidatorNoSelfValidationPolicyRef =
  'policy.public.training_validator.no_self_validation'
export const TrainingValidatorFreivaldsQuorumPolicyRef =
  'policy.public.training_validator.freivalds_rejection_quorum'

/**
 * The challenge fields the bridge actually reads. A full
 * `TrainingVerificationChallengeRecord` satisfies this, and so does the
 * public challenge projection returned by
 * `GET /api/training/verification/challenges/:challengeRef`, which is what
 * the live dispatch script feeds in.
 */
export type TrainingValidatorChallengeSummary = Pick<
  TrainingVerificationChallengeRecord,
  | 'challengeRef'
  | 'contributionRef'
  | 'homeworkKind'
  | 'samplingPolicy'
  | 'trainingRunRef'
  | 'verificationClass'
  | 'windowRef'
>

export type TrainingValidatorAssignmentBridgeInput = Readonly<{
  assignmentRef?: string
  challenge: TrainingValidatorChallengeSummary
  leaseSeconds?: number
  nowIso: string
  validatorPylonRef: string
  workerPylonRef: string
}>

export type TrainingValidatorAssignmentBridgeResult =
  | Readonly<{
      blockerRefs: ReadonlyArray<string>
      kind: 'blocked'
    }>
  | Readonly<{
      assignmentRequest: PylonApiCreateAssignmentRequest
      blockerRefs: ReadonlyArray<string>
      kind: 'assignment_request'
      paymentBlockedRefs: ReadonlyArray<string>
    }>

export type TrainingValidatorVerdict = Readonly<{
  challengeRef: string
  failureCodes: ReadonlyArray<TrainingVerificationFailureCode>
  state: 'Verified' | 'Rejected'
  validatorPylonRef: string
  verdictRef: string
  verificationClass: TrainingVerificationClass
}>

export type TrainingValidatorConsensusProjection = Readonly<{
  accepted: boolean
  blockerRefs: ReadonlyArray<string>
  challengeRef: string
  requiredValidatorCount: number
  state: 'consensus_verified' | 'consensus_rejected' | 'quorum_pending'
  verdictRefs: ReadonlyArray<string>
  verificationClass: TrainingVerificationClass
}>

const unsafePublicMaterialPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|bolt11|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(invoice|payment|payload|prompt|runner)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet)/i

const uniqueRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> =>
  [
    ...new Set(
      refs.filter(
        (ref): ref is string => typeof ref === 'string' && ref.trim() !== '',
      ),
    ),
  ].sort()

const publicSafe = (value: unknown): boolean =>
  !unsafePublicMaterialPattern.test(JSON.stringify(value))

const classTaskRef = (challenge: TrainingValidatorChallengeSummary): string =>
  `task.public.training_validator.${challenge.verificationClass}`

const expectedResultRef = (
  challenge: TrainingValidatorChallengeSummary,
): string => `result.public.training_validator.${challenge.verificationClass}`

export const trainingValidatorSelfValidationBlockers = (
  input: Pick<
    TrainingValidatorAssignmentBridgeInput,
    'challenge' | 'validatorPylonRef' | 'workerPylonRef'
  >,
): ReadonlyArray<string> => [
  ...(input.validatorPylonRef === input.workerPylonRef
    ? ['blocker.training_validator.self_validation']
    : []),
  ...(input.challenge.contributionRef !== null &&
  input.challenge.contributionRef.includes(input.validatorPylonRef)
    ? ['blocker.training_validator.validator_owns_contribution']
    : []),
]

export const buildTrainingValidatorAssignmentRequest = (
  input: TrainingValidatorAssignmentBridgeInput,
): TrainingValidatorAssignmentBridgeResult => {
  const blockerRefs = trainingValidatorSelfValidationBlockers(input)

  if (blockerRefs.length > 0) {
    return { blockerRefs, kind: 'blocked' }
  }

  const assignmentRequest: PylonApiCreateAssignmentRequest = {
    acceptanceCriteriaRefs: [
      `acceptance.public.training_validator.${input.challenge.verificationClass}`,
      TrainingValidatorNoSelfValidationPolicyRef,
    ],
    assignmentRef:
      input.assignmentRef ??
      `assignment.public.training_validator.${input.challenge.challengeRef.replaceAll(/[^A-Za-z0-9_.:-]/g, '_')}`,
    campaignPaused: false,
    campaignPolicyRefs: [
      'policy.public.training_validator.small_sats_operator_approved',
      TrainingValidatorFreivaldsQuorumPolicyRef,
    ],
    campaignRef: 'campaign.public.training_validator.cs336',
    closeoutPathRefs: [
      'closeout.public.training_validator.verdict_evidence_required',
    ],
    codingAssignment: {
      challengeRef: input.challenge.challengeRef,
      homeworkKind: input.challenge.homeworkKind,
      samplingPolicy: input.challenge.samplingPolicy,
      trainingRunRef: input.challenge.trainingRunRef,
      verificationClass: input.challenge.verificationClass,
      windowRef: input.challenge.windowRef,
      workerPylonRef: input.workerPylonRef,
    },
    forumAutoPublishAllowed: false,
    idempotencyRefs: [
      `idempotency.public.training_validator.${input.challenge.challengeRef}`,
    ],
    jobKind: 'validation',
    leaseSeconds: input.leaseSeconds ?? 15 * 60,
    noDuplicateAssignmentRefs: [
      `dedupe.public.training_validator.${input.challenge.challengeRef}`,
    ],
    noForumAutoPublishRefs: ['policy.public.no_forum_auto_publish'],
    operatorPauseRefs: ['pause.public.training_validator.dispatch'],
    paymentMode: 'payable_pending_settlement',
    pylonRef: input.validatorPylonRef,
    requiredCapabilityRefs: [TrainingValidatorCapabilityRef],
    resultExpectationRefs: [expectedResultRef(input.challenge)],
    rollbackRefs: ['rollback.public.training_validator.cancel_assignment'],
    selectionPolicyRefs: [TrainingValidatorSelectionPolicyRef],
    spendCapRefs: [
      'spendcap.public.training_validator.small_sats_operator_required',
    ],
    taskRefs: [
      input.challenge.challengeRef,
      classTaskRef(input.challenge),
      TrainingValidatorNoSelfValidationPolicyRef,
    ],
  }
  const paymentBlockedRefs = [
    'blocker.training_validator.operator_spend_approval_required',
  ]

  if (!publicSafe(assignmentRequest)) {
    return {
      blockerRefs: ['blocker.training_validator.assignment_payload_unsafe'],
      kind: 'blocked',
    }
  }

  return {
    assignmentRequest,
    blockerRefs: [],
    kind: 'assignment_request',
    paymentBlockedRefs,
  }
}

const requiredValidatorCountFor = (
  verificationClass: TrainingVerificationClass,
  state: TrainingValidatorVerdict['state'],
): number =>
  verificationClass === 'freivalds_merkle' && state === 'Rejected' ? 2 : 1

const distinctValidatorVerdicts = (
  verdicts: ReadonlyArray<TrainingValidatorVerdict>,
): ReadonlyArray<TrainingValidatorVerdict> =>
  [
    ...new Map(
      verdicts.map(verdict => [verdict.validatorPylonRef, verdict]),
    ).values(),
  ].sort((left, right) => left.verdictRef.localeCompare(right.verdictRef))

export const projectTrainingValidatorConsensus = (
  input: Readonly<{
    challengeRef: string
    verdicts: ReadonlyArray<TrainingValidatorVerdict>
    verificationClass: TrainingVerificationClass
  }>,
): TrainingValidatorConsensusProjection => {
  const verified = distinctValidatorVerdicts(
    input.verdicts.filter(verdict => verdict.state === 'Verified'),
  )
  const rejected = distinctValidatorVerdicts(
    input.verdicts.filter(verdict => verdict.state === 'Rejected'),
  )
  const verifiedRequired = requiredValidatorCountFor(
    input.verificationClass,
    'Verified',
  )
  const rejectedRequired = requiredValidatorCountFor(
    input.verificationClass,
    'Rejected',
  )
  const state: TrainingValidatorConsensusProjection['state'] =
    verified.length >= verifiedRequired
      ? 'consensus_verified'
      : rejected.length >= rejectedRequired
        ? 'consensus_rejected'
        : 'quorum_pending'
  const projection: TrainingValidatorConsensusProjection = {
    accepted: state !== 'quorum_pending',
    blockerRefs:
      state === 'quorum_pending'
        ? ['blocker.training_validator.quorum_pending']
        : [],
    challengeRef: input.challengeRef,
    requiredValidatorCount:
      rejected.length > 0 ? rejectedRequired : verifiedRequired,
    state,
    verdictRefs: uniqueRefs(input.verdicts.map(verdict => verdict.verdictRef)),
    verificationClass: input.verificationClass,
  }

  if (!publicSafe(projection)) {
    return {
      ...projection,
      accepted: false,
      blockerRefs: [
        ...projection.blockerRefs,
        'blocker.training_validator.consensus_projection_unsafe',
      ],
      state: 'quorum_pending',
    }
  }

  return projection
}
