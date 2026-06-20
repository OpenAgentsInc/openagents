/**
 * Bounded CS336 A5 DPO preference-grading CHALLENGE LIFECYCLE harness.
 *
 * The previous pass added `buildCs336A5DpoGradingChallengeCreateRequest`,
 * which proves the DPO grading challenge create-request DECODES against the
 * real `training-verification` schema. But schema validity alone does not
 * prove the request actually drives the rail-side challenge state machine to
 * a terminal verdict. This module supplies the missing PIECE: a deterministic
 * in-repo dry-run that pushes the create-request through the SAME state-machine
 * functions a paid `cs336_a5_dpo_grading` dispatch would use —
 * `buildTrainingVerificationChallengeRecord` (Queued) ->
 * `leaseTrainingVerificationChallengeRecord` (Leased) -> the in-repo recompute
 * verifier -> `finalizeTrainingVerificationChallengeRecord` (Verified/Rejected)
 * — so an honest worker's claim reaches a `Verified` terminal state and a
 * forged claim reaches `Rejected`, with the full Queued -> Leased -> terminal
 * transition history.
 *
 * IMPORTANT HONESTY BOUNDARY: this is a deterministic, in-memory simulation.
 * It takes an injectable clock and id generator and never touches D1, takes no
 * real lease, creates no rail-side challenge, dispatches no work, spends no
 * sats, and settles nothing. It demonstrates the verification lifecycle the
 * paid dispatch would follow, NOT the paid work itself, so
 * `blocker.product_promises.preference_rollout_work_missing` stays open. The
 * DPO/policy-gradient update step also stays behind the #4669 training
 * boundary.
 */

import {
  type Cs336A5DpoGradingChallengeSpec,
  type Cs336A5DpoGradingClaim,
  buildCs336A5DpoGradingChallengeCreateRequest,
  verifyCs336A5DpoGradingResponse,
} from './cs336-a5-dpo-grading-challenge'
import {
  type TrainingVerificationChallengeCreateRequest,
  type TrainingVerificationChallengeEventRecord,
  type TrainingVerificationChallengeRecord,
  type TrainingVerificationChallengeState,
  type TrainingVerificationVerdict,
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
} from './training-verification'

const DefaultLifecycleNowIso = '2026-06-20T00:00:00.000Z'

export type Cs336A5DpoGradingLifecycleResult = Readonly<{
  challengeRef: string
  createRequest: TrainingVerificationChallengeCreateRequest
  events: ReadonlyArray<TrainingVerificationChallengeEventRecord>
  finalState: TrainingVerificationChallengeState
  records: Readonly<{
    finalized: TrainingVerificationChallengeRecord
    leased: TrainingVerificationChallengeRecord
    queued: TrainingVerificationChallengeRecord
  }>
  validatorRef: string
  verdict: TrainingVerificationVerdict
}>

/**
 * Drives a single DPO grading challenge through the real training-verification
 * state machine to a terminal verdict, in-repo and deterministically. The
 * worker's `claim` is verified by the same `verifyCs336A5DpoGradingResponse`
 * recompute verifier the create-request describes; an honest claim finalizes
 * `Verified`, a forged claim finalizes `Rejected`. No rail mutation, lease,
 * spend, or settlement occurs.
 */
export const runCs336A5DpoGradingChallengeLifecycle = async (
  input: Readonly<{
    claim: Cs336A5DpoGradingClaim
    idSeed?: string
    nowIso?: string
    spec: Cs336A5DpoGradingChallengeSpec
    trainingRunRef: string
    validatorRef?: string
    windowRef?: string
  }>,
): Promise<Cs336A5DpoGradingLifecycleResult> => {
  const nowIso = input.nowIso ?? DefaultLifecycleNowIso
  const idSeed = input.idSeed ?? `dpo_grading_${input.spec.splitRef}`
  let counter = 0
  const makeId = (): string => `${idSeed}_${String(++counter).padStart(4, '0')}`
  const validatorRef =
    input.validatorRef ?? `validator.cs336_a5_dpo_grading.${input.spec.splitRef}`

  const createRequest = buildCs336A5DpoGradingChallengeCreateRequest({
    spec: input.spec,
    trainingRunRef: input.trainingRunRef,
    ...(input.windowRef === undefined ? {} : { windowRef: input.windowRef }),
  })

  const created = buildTrainingVerificationChallengeRecord({
    makeId,
    nowIso,
    request: createRequest,
  })

  const leased = leaseTrainingVerificationChallengeRecord({
    challenge: created.challenge,
    eventId: makeId(),
    nowIso,
    request: { validatorRef },
  })

  const verdict = await verifyCs336A5DpoGradingResponse({
    claim: input.claim,
    spec: input.spec,
  })

  const finalized = finalizeTrainingVerificationChallengeRecord({
    challenge: leased.challenge,
    eventId: makeId(),
    nowIso,
    request: { receiptRefs: verdict.verdictRefs },
    validatorRef,
    verdict,
  })

  return {
    challengeRef: created.challenge.challengeRef,
    createRequest,
    events: [created.event, leased.event, finalized.event],
    finalState: finalized.challenge.state,
    records: {
      finalized: finalized.challenge,
      leased: leased.challenge,
      queued: created.challenge,
    },
    validatorRef,
    verdict,
  }
}
