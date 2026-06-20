/**
 * Bounded post-training vibe-test closeout grading CHALLENGE bridge.
 *
 * The vibe-test rubric module (`post-training-vibe-test-rubric.ts`) runs a
 * deterministic closeout over the repo-owned fixture transcripts and emits a
 * stable `closeoutDigestHex` answer key, and the public projection
 * (`training-post-training-vibe-test-rubric.ts`) publishes it. But there was
 * no committed bridge from that answer key to the verification layer a paid
 * `post_training_vibe_test_grading` dispatch would settle against — the same
 * `deterministic_recompute` challenge shape the 2026-06-11 alignment run used
 * for its four Verified challenges, already mirrored for the DPO
 * (`cs336-a5-dpo-grading-challenge.ts`) and instruct-SFT
 * (`psion-instruct-sft-grading-challenge.ts`) lanes. This module supplies the
 * missing PIECE for the vibe-test lane: it builds a `deterministic_recompute`
 * challenge spec by recomputing the closeout in-repo, verifies a worker's
 * CLAIMED closeout digest against a fresh recompute, and bridges the spec into
 * the exact rail-side `TrainingVerificationChallengeCreateRequest` envelope a
 * paid dispatch would POST.
 *
 * Unlike the instruct-SFT lane (whose Rust crate cannot be recomputed here),
 * the vibe-test rubric scorer runs IN this worker, so the verifier performs a
 * TRUE in-repo `deterministic_recompute`: it never trusts the spec's stored
 * digest blindly — it re-runs the closeout from the committed rubric and
 * fixture transcripts at verify time and rejects if the spec is stale
 * (recompute != stored expected) or the worker's claim disagrees (claim !=
 * recompute). A challenge is `Verified` only when stored expected, fresh
 * recompute, and the worker's claim all agree.
 *
 * IMPORTANT HONESTY BOUNDARY: the transcripts are REPO-OWNED FIXTURE TEXT, not
 * real Psion instruct-model outputs, and no human reviewer has signed a
 * closeout (`reviewerSigned` stays `false`). This module only constructs,
 * verifies, and validates the challenge request — it submits nothing, takes no
 * lease, spends no sats, settles nothing, creates no rail-side challenge, and
 * forges no reviewer signature. It is the verification math and request
 * envelope a paid/reviewed vibe-test dispatch would record against, NOT the
 * reviewed artifact itself, so
 * `blocker.product_promises.vibe_test_artifact_missing` stays open.
 */

import { Schema as S } from 'effect'

import {
  type PostTrainingVibeTestCloseoutResult,
  type VibeTestTranscript,
  PostTrainingVibeTestCloseoutRef,
  PostTrainingVibeTestDefaultThreshold,
  PostTrainingVibeTestRubricRef,
  runPostTrainingVibeTestCloseout,
} from './post-training-vibe-test-rubric'
import { TrainingVerificationChallengeCreateRequest } from './training-verification'
import type {
  TrainingVerificationFailureCode,
  TrainingVerificationVerdict,
} from './training-verification'

export class PostTrainingVibeTestGradingChallengeError extends Error {
  readonly _tag = 'PostTrainingVibeTestGradingChallengeError'
}

export const PostTrainingVibeTestGradingChallengeJobKind =
  'post_training_vibe_test_grading'
export const PostTrainingVibeTestGradingChallengeVerificationClass =
  'deterministic_recompute' as const

/**
 * The homework kind a paid vibe-test grading dispatch records under, matching
 * the admin-dispatched lane the 2026-06-11 alignment run used for its
 * rollout/grading challenges.
 */
export const PostTrainingVibeTestGradingHomeworkKind =
  'admin_dispatched_homework'

export const PostTrainingVibeTestGradingWorkloadRef =
  'workload.training_post_training.vibe_test_closeout_grading.v1'

const digestPattern = /^[0-9a-f]{64}$/

/**
 * A `deterministic_recompute` challenge spec for one vibe-test closeout.
 * Carries only public-safe digests, counts, refs, and aggregate stats —
 * never prompts, completions, transcripts, or model weights.
 */
export type PostTrainingVibeTestGradingChallengeSpec = Readonly<{
  artifactRef: typeof PostTrainingVibeTestCloseoutRef
  challengeRef: string
  closeoutAcceptable: boolean
  expectedDigestHex: string
  meanScoreMicro: number
  reviewerSigned: false
  rubricRef: typeof PostTrainingVibeTestRubricRef
  thresholdMicro: number
  transcriptCount: number
  verificationClass: typeof PostTrainingVibeTestGradingChallengeVerificationClass
  workloadRef: typeof PostTrainingVibeTestGradingWorkloadRef
}>

type PostTrainingVibeTestGradingChallengeInput = Readonly<{
  threshold?: number
  transcripts?: ReadonlyArray<VibeTestTranscript>
}>

const runCloseout = (
  input: PostTrainingVibeTestGradingChallengeInput,
): Promise<PostTrainingVibeTestCloseoutResult> =>
  runPostTrainingVibeTestCloseout({
    threshold: input.threshold ?? PostTrainingVibeTestDefaultThreshold,
    ...(input.transcripts === undefined
      ? {}
      : { transcripts: input.transcripts }),
  })

const closeoutInput = (
  spec: PostTrainingVibeTestGradingChallengeSpec,
): PostTrainingVibeTestGradingChallengeInput => ({
  threshold: spec.thresholdMicro / 1_000_000,
})

/**
 * Builds the answer-key challenge spec by recomputing the vibe-test closeout
 * over the repo-owned fixture transcripts. The resulting `expectedDigestHex`
 * is the digest a paid dispatch must reproduce; the spec is otherwise
 * public-safe metadata. `reviewerSigned` is always `false` — this module
 * never forges a reviewer signature.
 */
export const buildPostTrainingVibeTestGradingChallengeSpec = async (
  input: PostTrainingVibeTestGradingChallengeInput = {},
): Promise<PostTrainingVibeTestGradingChallengeSpec> => {
  const threshold = input.threshold ?? PostTrainingVibeTestDefaultThreshold

  if (!Number.isFinite(threshold) || threshold <= 0 || threshold > 1) {
    throw new PostTrainingVibeTestGradingChallengeError(
      'Vibe-test grading challenge threshold must be in (0, 1].',
    )
  }

  const closeout = await runCloseout({ ...input, threshold })

  return {
    artifactRef: closeout.artifactRef,
    challengeRef: 'challenge.post_training_vibe_test_grading.fixture_closeout',
    closeoutAcceptable: closeout.closeoutAcceptable,
    expectedDigestHex: closeout.closeoutDigestHex,
    meanScoreMicro: Math.round(closeout.summary.meanScore * 1_000_000),
    reviewerSigned: false,
    rubricRef: closeout.rubricRef,
    thresholdMicro: Math.round(closeout.summary.threshold * 1_000_000),
    transcriptCount: closeout.summary.transcriptCount,
    verificationClass: PostTrainingVibeTestGradingChallengeVerificationClass,
    workloadRef: PostTrainingVibeTestGradingWorkloadRef,
  }
}

export type PostTrainingVibeTestGradingClaim = Readonly<{
  claimedDigestHex: string
  transcriptCount?: number
}>

/**
 * Verifies a worker's claimed vibe-test closeout digest against the challenge
 * spec by recomputing the closeout in-repo. Returns a `Verified` verdict only
 * when the stored expected digest, the fresh recompute, and the worker's claim
 * all agree (and the claimed transcript count, when supplied, matches the
 * recompute). Otherwise returns `Rejected` with the precise failure codes.
 * Pure deterministic verification — no spend, lease, settlement, reviewer
 * signature, or rail-side challenge is created.
 */
export const verifyPostTrainingVibeTestGradingResponse = async (
  input: Readonly<{
    claim: PostTrainingVibeTestGradingClaim
    spec: PostTrainingVibeTestGradingChallengeSpec
  }>,
): Promise<TrainingVerificationVerdict> => {
  const { claim, spec } = input
  const failureCodes: Array<TrainingVerificationFailureCode> = []
  const claimedDigestHex = claim.claimedDigestHex.trim().toLowerCase()

  if (!digestPattern.test(spec.expectedDigestHex)) {
    return {
      failureCodes: ['VerificationClassUnknown'],
      publicDetails: { reason: 'spec_expected_digest_malformed' },
      state: 'Rejected',
      verdictRefs: [spec.challengeRef],
    }
  }

  const recompute = await runCloseout(closeoutInput(spec))

  if (recompute.closeoutDigestHex !== spec.expectedDigestHex) {
    // The stored answer key no longer matches a fresh recompute: the spec is
    // stale or forged, so no claim against it can be Verified.
    return {
      failureCodes: ['DigestMismatch'],
      publicDetails: {
        reason: 'spec_stale_recompute_disagrees',
        recomputedTranscriptCount: recompute.summary.transcriptCount,
      },
      state: 'Rejected',
      verdictRefs: [spec.challengeRef],
    }
  }

  if (!digestPattern.test(claimedDigestHex)) {
    failureCodes.push('OutputDigestMissing')
  } else if (claimedDigestHex !== recompute.closeoutDigestHex) {
    failureCodes.push('DigestMismatch')
  }

  if (
    claim.transcriptCount !== undefined &&
    claim.transcriptCount !== recompute.summary.transcriptCount
  ) {
    failureCodes.push('DimensionMismatch')
  }

  if (failureCodes.length > 0) {
    return {
      failureCodes,
      publicDetails: {
        expectedTranscriptCount: recompute.summary.transcriptCount,
        reason: 'claim_disagrees_with_recompute',
      },
      state: 'Rejected',
      verdictRefs: [spec.challengeRef],
    }
  }

  return {
    failureCodes: [],
    publicDetails: {
      closeoutAcceptable: recompute.closeoutAcceptable,
      transcriptCount: recompute.summary.transcriptCount,
      verificationClass: spec.verificationClass,
    },
    state: 'Verified',
    verdictRefs: [spec.challengeRef],
  }
}

const decodeChallengeCreateRequest = S.decodeUnknownSync(
  TrainingVerificationChallengeCreateRequest,
)

/**
 * Bridges the vibe-test grading challenge spec into the exact rail-side
 * `TrainingVerificationChallengeCreateRequest` envelope a paid
 * `post_training_vibe_test_grading` dispatch would POST — the same
 * `deterministic_recompute` challenge shape the alignment run used. The built
 * request is decoded against the real `training-verification` schema, so a
 * structurally invalid request (e.g. a non-public-safe `trainingRunRef`) fails
 * loudly here instead of at the rails.
 *
 * IMPORTANT HONESTY BOUNDARY: this only constructs and validates the request
 * object. It does NOT submit it, create a challenge, take a lease, spend sats,
 * settle anything, or forge a reviewer signature — no rail-side mutation
 * occurs. The payload carries only public-safe digests, counts, refs, and
 * aggregate stats (never prompts, completions, transcripts, or weights), so
 * `blocker.product_promises.vibe_test_artifact_missing` stays open: the
 * paid/reviewed dispatch this request describes has not run.
 */
export const buildPostTrainingVibeTestGradingChallengeCreateRequest = (
  input: Readonly<{
    spec: PostTrainingVibeTestGradingChallengeSpec
    trainingRunRef: string
    windowRef?: string
  }>,
): TrainingVerificationChallengeCreateRequest => {
  const { spec, trainingRunRef, windowRef } = input

  if (!digestPattern.test(spec.expectedDigestHex)) {
    throw new PostTrainingVibeTestGradingChallengeError(
      'Cannot build a vibe-test grading challenge request from a spec with a malformed expected digest.',
    )
  }

  const challengeRequest = {
    commitmentRefs: [
      'commitment.post_training_vibe_test_grading.fixture_closeout',
      spec.workloadRef,
    ],
    contributionRef:
      'contribution.post_training_vibe_test_grading.fixture_closeout',
    homeworkKind: PostTrainingVibeTestGradingHomeworkKind,
    payload: {
      artifactRef: spec.artifactRef,
      closeoutAcceptable: spec.closeoutAcceptable,
      expectedDigestHex: spec.expectedDigestHex,
      jobKind: PostTrainingVibeTestGradingChallengeJobKind,
      meanScoreMicro: spec.meanScoreMicro,
      recomputedDigestRef:
        'recompute.post_training_vibe_test_grading.fixture_closeout',
      reviewerSigned: spec.reviewerSigned,
      rubricRef: spec.rubricRef,
      thresholdMicro: spec.thresholdMicro,
      transcriptCount: spec.transcriptCount,
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
    throw new PostTrainingVibeTestGradingChallengeError(
      'Vibe-test grading challenge create-request failed training-verification schema validation; check trainingRunRef/windowRef are public-safe refs.',
    )
  }
}
