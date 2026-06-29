/**
 * Bounded CS336 A5 DPO preference-grading CHALLENGE VERIFIER.
 *
 * The DPO reference workload (`cs336-a5-dpo-preference-workload.ts`)
 * produces a deterministic `outputDigestHex` answer key for a split, but
 * there was no committed bridge from that answer key to the verification
 * layer a paid `cs336_a5_dpo_grading` dispatch would settle against. This
 * module supplies the missing PIECE: it builds a `deterministic_recompute`
 * challenge spec from the reference answer key and verifies a worker's
 * CLAIMED grading digest by recomputing the reference grade and comparing.
 *
 * The verifier is the same shape the alignment run used for its four
 * Verified challenges: it never trusts the spec's stored digest blindly —
 * it RECOMPUTES the reference grade from the committed seed at verify time
 * and rejects if either the spec is stale (recompute != stored expected)
 * or the worker's claim disagrees (claim != recompute). A challenge is
 * `Verified` only when stored expected, fresh recompute, and the worker's
 * claim all agree.
 *
 * IMPORTANT HONESTY BOUNDARY: no hosted LLM, no real policy/reference
 * model, no paid dispatch, no lease, no settlement, and no Verified
 * challenge on the OpenAgents rails is created here. The verdict is a pure
 * function over deterministic digests. This is the verification math a
 * paid preference-grading dispatch would record against, NOT the paid work
 * itself, so `blocker.product_promises.preference_rollout_work_missing`
 * stays open. The DPO/policy-gradient update step also stays behind the
 * #4669 training boundary.
 */

import { Schema as S } from 'effect'

import {
  type Cs336A5DpoGradingResult,
  Cs336A5DpoDefaultBeta,
  Cs336A5DpoJobKind,
  Cs336A5DpoPreferenceWorkloadRef,
  Cs336A5DpoUpdateBoundaryRef,
  runCs336A5DpoPreferenceGrading,
} from './cs336-a5-dpo-preference-workload'
import type { Cs336A5Split } from './cs336-a5-rollout-workload'
import { TrainingVerificationChallengeCreateRequest } from './training-verification'
import type {
  TrainingVerificationFailureCode,
  TrainingVerificationVerdict,
} from './training-verification'

export class Cs336A5DpoGradingChallengeError extends Error {
  readonly _tag = 'Cs336A5DpoGradingChallengeError'
}

export const Cs336A5DpoGradingChallengeJobKind = 'cs336_a5_dpo_grading'
export const Cs336A5DpoGradingChallengeVerificationClass =
  'deterministic_recompute' as const

const digestPattern = /^[0-9a-f]{64}$/

/**
 * A `deterministic_recompute` challenge spec for one DPO grading split.
 * Carries only public-safe digests, counts, and refs — never prompts,
 * completions, log-probs, or model weights.
 */
export type Cs336A5DpoGradingChallengeSpec = Readonly<{
  betaMicro: number
  challengeRef: string
  expectedDigestHex: string
  pairCount: number
  rolloutsPerTask: number | undefined
  splitRef: Cs336A5Split
  taskCount: number | undefined
  updateBoundaryRef: typeof Cs336A5DpoUpdateBoundaryRef
  verificationClass: typeof Cs336A5DpoGradingChallengeVerificationClass
  workloadRef: typeof Cs336A5DpoPreferenceWorkloadRef
}>

type Cs336A5DpoGradingChallengeInput = Readonly<{
  beta?: number
  rolloutsPerTask?: number
  splitRef: Cs336A5Split
  taskCount?: number
}>

const gradingInput = (
  spec: Cs336A5DpoGradingChallengeSpec,
): Cs336A5DpoGradingChallengeInput => ({
  beta: spec.betaMicro / 1_000_000,
  splitRef: spec.splitRef,
  ...(spec.rolloutsPerTask === undefined
    ? {}
    : { rolloutsPerTask: spec.rolloutsPerTask }),
  ...(spec.taskCount === undefined ? {} : { taskCount: spec.taskCount }),
})

const runGrading = (
  input: Cs336A5DpoGradingChallengeInput,
): Promise<Cs336A5DpoGradingResult> =>
  runCs336A5DpoPreferenceGrading({
    beta: input.beta ?? Cs336A5DpoDefaultBeta,
    splitRef: input.splitRef,
    ...(input.rolloutsPerTask === undefined
      ? {}
      : { rolloutsPerTask: input.rolloutsPerTask }),
    ...(input.taskCount === undefined ? {} : { taskCount: input.taskCount }),
  })

/**
 * Builds the answer-key challenge spec by recomputing the reference DPO
 * grading for the split. The resulting `expectedDigestHex` is the digest a
 * paid dispatch must reproduce; the spec is otherwise public-safe metadata.
 */
export const buildCs336A5DpoGradingChallengeSpec = async (
  input: Cs336A5DpoGradingChallengeInput,
): Promise<Cs336A5DpoGradingChallengeSpec> => {
  const beta = input.beta ?? Cs336A5DpoDefaultBeta

  if (!Number.isFinite(beta) || beta <= 0) {
    throw new Cs336A5DpoGradingChallengeError(
      'CS336 A5 DPO challenge beta must be a positive finite number.',
    )
  }

  const reference = await runGrading({ ...input, beta })

  return {
    betaMicro: Math.round(reference.beta * 1_000_000),
    challengeRef: `challenge.cs336_a5_dpo_grading.${input.splitRef}`,
    expectedDigestHex: reference.outputDigestHex,
    pairCount: reference.pairCount,
    rolloutsPerTask: input.rolloutsPerTask,
    splitRef: input.splitRef,
    taskCount: input.taskCount,
    updateBoundaryRef: Cs336A5DpoUpdateBoundaryRef,
    verificationClass: Cs336A5DpoGradingChallengeVerificationClass,
    workloadRef: Cs336A5DpoPreferenceWorkloadRef,
  }
}

export type Cs336A5DpoGradingClaim = Readonly<{
  claimedDigestHex: string
  pairCount?: number
}>

/**
 * Verifies a worker's claimed DPO grading digest against the challenge
 * spec by recomputing the reference grade from the committed seed. Returns
 * a `Verified` verdict only when the stored expected digest, the fresh
 * recompute, and the worker's claim all agree (and the claimed pair count,
 * when supplied, matches the recompute). Otherwise returns `Rejected` with
 * the precise failure codes. Pure deterministic verification — no spend,
 * lease, settlement, or rail-side challenge is created.
 */
export const verifyCs336A5DpoGradingResponse = async (
  input: Readonly<{
    claim: Cs336A5DpoGradingClaim
    spec: Cs336A5DpoGradingChallengeSpec
  }>,
): Promise<TrainingVerificationVerdict> => {
  const { claim, spec } = input
  const failureCodes: Array<TrainingVerificationFailureCode> = []
  const claimedDigestHex = claim.claimedDigestHex.toLowerCase()

  if (!digestPattern.test(spec.expectedDigestHex)) {
    return {
      failureCodes: ['VerificationClassUnknown'],
      publicDetails: { reason: 'spec_expected_digest_malformed' },
      state: 'Rejected',
      verdictRefs: [spec.challengeRef],
    }
  }

  const recompute = await runGrading(gradingInput(spec))

  if (recompute.outputDigestHex !== spec.expectedDigestHex) {
    // The stored answer key no longer matches a fresh recompute: the spec
    // is stale or forged, so no claim against it can be Verified.
    return {
      failureCodes: ['DigestMismatch'],
      publicDetails: {
        reason: 'spec_stale_recompute_disagrees',
        recomputedPairCount: recompute.pairCount,
      },
      state: 'Rejected',
      verdictRefs: [spec.challengeRef],
    }
  }

  if (!digestPattern.test(claimedDigestHex)) {
    failureCodes.push('OutputDigestMissing')
  } else if (claimedDigestHex !== recompute.outputDigestHex) {
    failureCodes.push('DigestMismatch')
  }

  if (
    claim.pairCount !== undefined &&
    claim.pairCount !== recompute.pairCount
  ) {
    failureCodes.push('DimensionMismatch')
  }

  if (failureCodes.length > 0) {
    return {
      failureCodes,
      publicDetails: {
        expectedPairCount: recompute.pairCount,
        reason: 'claim_disagrees_with_recompute',
      },
      state: 'Rejected',
      verdictRefs: [spec.challengeRef],
    }
  }

  return {
    failureCodes: [],
    publicDetails: {
      pairCount: recompute.pairCount,
      verificationClass: spec.verificationClass,
    },
    state: 'Verified',
    verdictRefs: [spec.challengeRef],
  }
}

/**
 * The homework kind a paid `cs336_a5_dpo_grading` dispatch records under,
 * matching the admin-dispatched lane the 2026-06-11 alignment run used for
 * its rollout/grading challenges.
 */
export const Cs336A5DpoGradingHomeworkKind = 'admin_dispatched_homework'

const decodeChallengeCreateRequest = S.decodeUnknownSync(
  TrainingVerificationChallengeCreateRequest,
)

/**
 * Bridges the DPO grading challenge spec into the exact rail-side
 * `TrainingVerificationChallengeCreateRequest` envelope a paid
 * `cs336_a5_dpo_grading` dispatch would POST — the same
 * `deterministic_recompute` challenge shape the alignment run used. The
 * built request is decoded against the real `training-verification` schema,
 * so a structurally invalid request (e.g. a non-public-safe `trainingRunRef`)
 * fails loudly here instead of at the rails.
 *
 * IMPORTANT HONESTY BOUNDARY: this only constructs and validates the request
 * object. It does NOT submit it, create a challenge, take a lease, spend
 * sats, or settle anything — no rail-side mutation occurs. The payload
 * carries only public-safe digests, counts, and refs (never prompts,
 * completions, log-probs, or weights), so
 * `blocker.product_promises.preference_rollout_work_missing` stays open: the
 * paid dispatch this request describes has not run.
 */
export const buildCs336A5DpoGradingChallengeCreateRequest = (
  input: Readonly<{
    spec: Cs336A5DpoGradingChallengeSpec
    trainingRunRef: string
    windowRef?: string
  }>,
): TrainingVerificationChallengeCreateRequest => {
  const { spec, trainingRunRef, windowRef } = input

  if (!digestPattern.test(spec.expectedDigestHex)) {
    throw new Cs336A5DpoGradingChallengeError(
      'Cannot build a DPO grading challenge request from a spec with a malformed expected digest.',
    )
  }

  const challengeRequest = {
    commitmentRefs: [
      `commitment.cs336_a5_dpo_grading.${spec.splitRef}`,
      spec.workloadRef,
    ],
    contributionRef: `contribution.cs336_a5_dpo_grading.${spec.splitRef}`,
    homeworkKind: Cs336A5DpoGradingHomeworkKind,
    payload: {
      betaMicro: spec.betaMicro,
      expectedDigestHex: spec.expectedDigestHex,
      jobKind: Cs336A5DpoJobKind,
      pairCount: spec.pairCount,
      recomputedDigestRef: `recompute.cs336_a5_dpo_grading.${spec.splitRef}`,
      splitRef: spec.splitRef,
      updateBoundaryRef: spec.updateBoundaryRef,
      workloadRef: spec.workloadRef,
    },
    samplingPolicy: 'per_contribution' as const,
    trainingRunRef,
    verificationClass: spec.verificationClass,
    ...(windowRef === undefined ? {} : { windowRef }),
  }

  try {
    return decodeChallengeCreateRequest(challengeRequest)
  } catch {
    throw new Cs336A5DpoGradingChallengeError(
      'DPO grading challenge create-request failed training-verification schema validation; check trainingRunRef/windowRef are public-safe refs.',
    )
  }
}
