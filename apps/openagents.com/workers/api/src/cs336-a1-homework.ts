import {
  type BuyModeDispatchInput,
  type BuyModeDispatcherResult,
  type BuyModeDispatcherStore,
  type BuyModeRelayPublisher,
  dispatchBuyModeJob,
  isPublicSafeBuyModeProjection,
} from './buy-mode-dispatcher'
import {
  type TrainingVerificationChallengeCreateRequest,
  type TrainingVerificationChallengeRecord,
  type TrainingVerificationStore,
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  runTrainingVerificationClass,
} from './training-verification'

export const Cs336A1HomeworkJobKind = 'cs336_a1_homework'
export const Cs336A1PsionicLaneRef = 'psion_cs336_a1_demo_v1'
export const Cs336A1RequestSchemaRef =
  'psion.cs336_a1_demo_automatic_execution_request.v1'
export const Cs336A1OutputsSchemaRef =
  'psion.cs336_a1_demo_automatic_execution_outputs.v1'

export type Cs336A1HomeworkWorkClass =
  | 'tokenizer_bpe_shard'
  | 'training_step_matrix'

export type Cs336A1VerificationBinding = Readonly<{
  commitmentRefs: ReadonlyArray<string>
  contributionRef: string
  samplingPolicy: 'aggregate' | 'per_contribution'
  verificationClass: 'deterministic_recompute' | 'freivalds_merkle'
  workClass: Cs336A1HomeworkWorkClass
}>

export type Cs336A1HomeworkPayload = Readonly<{
  assignmentRef: string
  jobKind: typeof Cs336A1HomeworkJobKind
  outputSchemaRef: typeof Cs336A1OutputsSchemaRef
  psionicLaneRef: typeof Cs336A1PsionicLaneRef
  requestSchemaRef: typeof Cs336A1RequestSchemaRef
  trainingRunRef: string
  verificationBindings: ReadonlyArray<Cs336A1VerificationBinding>
  windowRef: string
}>

export type Cs336A1CloseoutEvidence = Readonly<{
  assignmentRef: string
  artifactRefs: ReadonlyArray<string>
  checkpointRefs: ReadonlyArray<string>
  contributionRef: string
  metricRefs: ReadonlyArray<string>
  proofRefs: ReadonlyArray<string>
  tokenizerOutputDigestRef: string
  tokenizerRecomputedDigestRef: string
  trainingMatrixPayload: Record<string, unknown>
  workerReceiptRef: string
}>

export type Cs336A1HomeworkDispatchInput = Omit<
  BuyModeDispatchInput,
  'content'
> &
  Readonly<{
    assignmentRef: string
    trainingRunRef: string
    windowRef: string
  }>

export type Cs336A1HomeworkDispatchResult = Readonly<{
  buyModeResult: BuyModeDispatcherResult
  payload: Cs336A1HomeworkPayload
}>

export type Cs336A1NoSpendRehearsalResult = Readonly<{
  accepted: boolean
  assignmentRef: string
  blockerRefs: ReadonlyArray<string>
  challengeRefs: ReadonlyArray<string>
  closeoutEvidenceRefs: ReadonlyArray<string>
  dispatchKind: BuyModeDispatcherResult['kind']
  paidSettlementRequired: true
  receiptRefs: ReadonlyArray<string>
  trainingRunRef: string
  windowRef: string
}>

export class Cs336A1HomeworkUnsafeProjectionError extends Error {
  readonly _tag = 'Cs336A1HomeworkUnsafeProjectionError'
}

const unsafePublicMaterialPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|bolt11|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(invoice|payment|payload|prompt|runner)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet)/i

const publicSafeJson = (value: unknown): string => {
  const json = JSON.stringify(value)

  if (unsafePublicMaterialPattern.test(json)) {
    throw new Cs336A1HomeworkUnsafeProjectionError(
      'CS336 A1 homework payload is not public-safe.',
    )
  }

  return json
}

const baseVerificationBindings = (
  assignmentRef: string,
): ReadonlyArray<Cs336A1VerificationBinding> => [
  {
    commitmentRefs: [`commitment.cs336_a1.${assignmentRef}.tokenizer_bpe`],
    contributionRef: `contribution.cs336_a1.${assignmentRef}.tokenizer_bpe`,
    samplingPolicy: 'per_contribution',
    verificationClass: 'deterministic_recompute',
    workClass: 'tokenizer_bpe_shard',
  },
  {
    commitmentRefs: [`commitment.cs336_a1.${assignmentRef}.training_step`],
    contributionRef: `contribution.cs336_a1.${assignmentRef}.training_step`,
    samplingPolicy: 'per_contribution',
    verificationClass: 'freivalds_merkle',
    workClass: 'training_step_matrix',
  },
]

export const buildCs336A1HomeworkPayload = (
  input: Readonly<{
    assignmentRef: string
    trainingRunRef: string
    windowRef: string
  }>,
): Cs336A1HomeworkPayload => {
  const payload: Cs336A1HomeworkPayload = {
    assignmentRef: input.assignmentRef,
    jobKind: Cs336A1HomeworkJobKind,
    outputSchemaRef: Cs336A1OutputsSchemaRef,
    psionicLaneRef: Cs336A1PsionicLaneRef,
    requestSchemaRef: Cs336A1RequestSchemaRef,
    trainingRunRef: input.trainingRunRef,
    verificationBindings: baseVerificationBindings(input.assignmentRef),
    windowRef: input.windowRef,
  }

  publicSafeJson(payload)

  return payload
}

export const dispatchCs336A1HomeworkJob = async (
  store: BuyModeDispatcherStore,
  relay: BuyModeRelayPublisher,
  input: Cs336A1HomeworkDispatchInput,
): Promise<Cs336A1HomeworkDispatchResult> => {
  const payload = buildCs336A1HomeworkPayload({
    assignmentRef: input.assignmentRef,
    trainingRunRef: input.trainingRunRef,
    windowRef: input.windowRef,
  })
  const buyModeResult = await dispatchBuyModeJob(store, relay, {
    ...input,
    content: publicSafeJson(payload),
  })

  return { buyModeResult, payload }
}

export const cs336A1VerificationChallengeRequests = (
  input: Readonly<{
    closeout: Cs336A1CloseoutEvidence
    homeworkKind?: string
    trainingRunRef: string
    windowRef: string
  }>,
): ReadonlyArray<TrainingVerificationChallengeCreateRequest> => [
  {
    commitmentRefs: [`commitment.cs336_a1.${input.closeout.assignmentRef}.tokenizer_bpe`],
    contributionRef: input.closeout.contributionRef,
    homeworkKind: input.homeworkKind ?? 'admin_dispatched_homework',
    payload: {
      contributionRefs: [input.closeout.contributionRef],
      expectedDigestRef: input.closeout.tokenizerOutputDigestRef,
      recomputedDigestRef: input.closeout.tokenizerRecomputedDigestRef,
    },
    samplingPolicy: 'per_contribution',
    trainingRunRef: input.trainingRunRef,
    verificationClass: 'deterministic_recompute',
    windowRef: input.windowRef,
  },
  {
    commitmentRefs: [`commitment.cs336_a1.${input.closeout.assignmentRef}.training_step`],
    contributionRef: input.closeout.contributionRef,
    homeworkKind: input.homeworkKind ?? 'admin_dispatched_homework',
    payload: {
      contributionRefs: [input.closeout.contributionRef],
      ...input.closeout.trainingMatrixPayload,
    },
    samplingPolicy: 'per_contribution',
    trainingRunRef: input.trainingRunRef,
    verificationClass: 'freivalds_merkle',
    windowRef: input.windowRef,
  },
]

export const recordCs336A1VerificationChallenges = async (
  store: TrainingVerificationStore,
  input: Readonly<{
    closeout: Cs336A1CloseoutEvidence
    makeId: () => string
    nowIso: string
    trainingRunRef: string
    windowRef: string
  }>,
): Promise<ReadonlyArray<TrainingVerificationChallengeRecord>> => {
  const records: TrainingVerificationChallengeRecord[] = []

  for (const request of cs336A1VerificationChallengeRequests(input)) {
    const built = buildTrainingVerificationChallengeRecord({
      makeId: input.makeId,
      nowIso: input.nowIso,
      request,
    })
    records.push(await store.createChallenge(built.challenge, built.event))
  }

  return records
}

export const verifyCs336A1NoSpendCloseout = async (
  store: TrainingVerificationStore,
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    makeId: () => string
    nowIso: string
  }>,
): Promise<ReadonlyArray<TrainingVerificationChallengeRecord>> => {
  const finalized: TrainingVerificationChallengeRecord[] = []

  for (const challenge of input.challenges) {
    const leased: TrainingVerificationChallengeRecord = {
      ...challenge,
      leaseExpiresAt: input.nowIso,
      leaseRef: `training.verification.lease.${input.makeId()}`,
      leasedToRef: 'validator.cs336_a1.no_spend_rehearsal',
      state: 'Leased',
      updatedAt: input.nowIso,
    }
    const verdict = await runTrainingVerificationClass({ challenge: leased })
    const final = finalizeTrainingVerificationChallengeRecord({
      challenge: leased,
      eventId: input.makeId(),
      nowIso: input.nowIso,
      request: { receiptRefs: ['receipt.cs336_a1.no_spend.verification'] },
      verdict,
    })
    finalized.push(
      await store.transitionChallenge(final.challenge, final.event),
    )
  }

  return finalized
}

export const projectCs336A1NoSpendRehearsal = (
  input: Readonly<{
    assignmentRef: string
    buyModeResult: BuyModeDispatcherResult
    closeout: Cs336A1CloseoutEvidence
    finalizedChallenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    trainingRunRef: string
    windowRef: string
  }>,
): Cs336A1NoSpendRehearsalResult => {
  const blockerRefs = [
    ...(input.buyModeResult.kind === 'dispatched'
      ? []
      : [`blocker.cs336_a1.dispatch_${input.buyModeResult.kind}`]),
    ...input.finalizedChallenges
      .filter(challenge => challenge.state !== 'Verified')
      .map(challenge => `blocker.cs336_a1.verification_${challenge.state}`),
    'blocker.cs336_a1.paid_settlement_requires_operator_spend_approval',
    'blocker.cs336_a1.psionic_execution_boundary_external',
  ]
  const projection: Cs336A1NoSpendRehearsalResult = {
    accepted:
      input.buyModeResult.kind === 'dispatched' &&
      input.finalizedChallenges.length > 0 &&
      input.finalizedChallenges.every(challenge => challenge.state === 'Verified'),
    assignmentRef: input.assignmentRef,
    blockerRefs,
    challengeRefs: input.finalizedChallenges.map(
      challenge => challenge.challengeRef,
    ),
    closeoutEvidenceRefs: [
      input.closeout.workerReceiptRef,
      ...input.closeout.artifactRefs,
      ...input.closeout.checkpointRefs,
      ...input.closeout.metricRefs,
      ...input.closeout.proofRefs,
    ].sort(),
    dispatchKind: input.buyModeResult.kind,
    paidSettlementRequired: true,
    receiptRefs: ['receipt.cs336_a1.no_spend.rehearsal'],
    trainingRunRef: input.trainingRunRef,
    windowRef: input.windowRef,
  }

  if (!isPublicSafeBuyModeProjection(projection)) {
    throw new Cs336A1HomeworkUnsafeProjectionError(
      'CS336 A1 rehearsal projection is not public-safe.',
    )
  }

  return projection
}
