/**
 * Bounded Psion instruct-SFT lane grading CHALLENGE bridge.
 *
 * The instruct-SFT lane receipt (`training-post-training-instruct-sft.ts`)
 * commits a deterministic `reportDigest` answer key for the fixture-scale
 * `psion_instruct_sft_v1` smoke run, but there was no committed bridge from
 * that answer key to the verification layer a paid `psion_instruct_sft`
 * dispatch would settle against — the same `deterministic_recompute`
 * challenge shape the 2026-06-11 alignment run used for its four Verified
 * challenges. This module supplies the missing PIECE: it builds a
 * `deterministic_recompute` challenge spec from the committed lane report
 * answer key, verifies a worker's CLAIMED report digest against it, and
 * bridges the spec into the exact rail-side
 * `TrainingVerificationChallengeCreateRequest` envelope a paid dispatch
 * would POST.
 *
 * IMPORTANT HONESTY BOUNDARY: the SFT lane itself runs in the Psionic Rust
 * crate, not in this worker, so there is NO in-repo recompute — the spec's
 * answer key is the committed fixture report digest, and the verifier
 * compares a worker's claim against that committed answer key. A real paid
 * dispatch's rail-side `deterministic_recompute` verifier would re-run the
 * Psionic lane to regenerate the digest; this module is the verification
 * math and request envelope that paid dispatch would settle against, NOT
 * the paid work itself. No hosted run, no lease, no spend, no settlement,
 * and no Verified challenge on the OpenAgents rails is created here, so
 * `blocker.product_promises.instruct_sft_paid_dispatch_missing` stays open.
 */

import { Schema as S } from 'effect'

import { TrainingVerificationChallengeCreateRequest } from './training-verification'
import type {
  TrainingVerificationFailureCode,
  TrainingVerificationVerdict,
} from './training-verification'

export class PsionInstructSftGradingChallengeError extends Error {
  readonly _tag = 'PsionInstructSftGradingChallengeError'
}

export const PsionInstructSftGradingChallengeJobKind =
  'psion_instruct_sft_grading'
export const PsionInstructSftGradingChallengeVerificationClass =
  'deterministic_recompute' as const

/**
 * The homework kind a paid `psion_instruct_sft` grading dispatch records
 * under, matching the admin-dispatched lane the 2026-06-11 alignment run
 * used for its rollout/grading challenges.
 */
export const PsionInstructSftGradingHomeworkKind = 'admin_dispatched_homework'

/**
 * Committed answer-key constants for the fixture-scale lane smoke run.
 * These mirror the values published by the instruct-SFT lane receipt; a
 * committed guard test asserts they stay in sync with the projection so a
 * drift between the answer key and the published receipt fails loudly.
 */
export const PsionInstructSftLaneId = 'psion_instruct_sft_v1'
export const PsionInstructSftRunId = 'psion-instruct-sft-smoke-001'
export const PsionInstructSftCompletedSteps = 8
export const PsionInstructSftReportDigest =
  'sha256:76b5524234b4dd6507560c0cda6f28e782fe097c1fb022108aaaae40794d6871'
export const PsionInstructSftTemplateDigest =
  'sha256:7337ec749e64dbf1b23dbfeb3478788846c67e8247813f386d97b1ed1076fca3'
export const PsionInstructSftManifestDigest =
  'sha256:1ce60a17a18975a729fd7d9d81baab556541af6fd280c0fadfb29e09b7e18cc7'

export const PsionInstructSftGradingWorkloadRef =
  'workload.psion_instruct_sft.lane_report_grading.v1'

const digestPattern = /^sha256:[0-9a-f]{64}$/

const normalizeDigest = (digest: string): string => digest.trim().toLowerCase()

/**
 * A `deterministic_recompute` challenge spec for the instruct-SFT lane
 * grading. Carries only public-safe digests, counts, and refs — never
 * prompts, completions, log-probs, or model weights.
 */
export type PsionInstructSftGradingChallengeSpec = Readonly<{
  challengeRef: string
  completedSteps: number
  expectedReportDigest: string
  laneId: typeof PsionInstructSftLaneId
  manifestDigest: string
  runId: typeof PsionInstructSftRunId
  templateDigest: string
  verificationClass: typeof PsionInstructSftGradingChallengeVerificationClass
  workloadRef: typeof PsionInstructSftGradingWorkloadRef
}>

/**
 * Builds the answer-key challenge spec from the committed lane report
 * digest. The resulting `expectedReportDigest` is the digest a paid
 * dispatch must reproduce; the spec is otherwise public-safe metadata.
 */
export const buildPsionInstructSftGradingChallengeSpec =
  (): PsionInstructSftGradingChallengeSpec => {
    if (!digestPattern.test(PsionInstructSftReportDigest)) {
      throw new PsionInstructSftGradingChallengeError(
        'Committed instruct-SFT report digest is malformed.',
      )
    }

    return {
      challengeRef: `challenge.psion_instruct_sft_grading.${PsionInstructSftLaneId}`,
      completedSteps: PsionInstructSftCompletedSteps,
      expectedReportDigest: PsionInstructSftReportDigest,
      laneId: PsionInstructSftLaneId,
      manifestDigest: PsionInstructSftManifestDigest,
      runId: PsionInstructSftRunId,
      templateDigest: PsionInstructSftTemplateDigest,
      verificationClass: PsionInstructSftGradingChallengeVerificationClass,
      workloadRef: PsionInstructSftGradingWorkloadRef,
    }
  }

export type PsionInstructSftGradingClaim = Readonly<{
  claimedReportDigest: string
  completedSteps?: number
}>

/**
 * Verifies a worker's claimed instruct-SFT lane report digest against the
 * committed answer key in the challenge spec. Returns a `Verified` verdict
 * only when the spec's expected digest is well-formed and the worker's
 * claim (and, when supplied, its completed-step count) agree with it.
 * Otherwise returns `Rejected` with the precise failure codes. Pure
 * deterministic comparison — no recompute of the Rust lane, no spend,
 * lease, settlement, or rail-side challenge is created.
 */
export const verifyPsionInstructSftGradingResponse = (
  input: Readonly<{
    claim: PsionInstructSftGradingClaim
    spec: PsionInstructSftGradingChallengeSpec
  }>,
): TrainingVerificationVerdict => {
  const { claim, spec } = input
  const failureCodes: Array<TrainingVerificationFailureCode> = []

  if (!digestPattern.test(spec.expectedReportDigest)) {
    return {
      failureCodes: ['VerificationClassUnknown'],
      publicDetails: { reason: 'spec_expected_digest_malformed' },
      state: 'Rejected',
      verdictRefs: [spec.challengeRef],
    }
  }

  const claimedReportDigest = normalizeDigest(claim.claimedReportDigest)

  if (!digestPattern.test(claimedReportDigest)) {
    failureCodes.push('OutputDigestMissing')
  } else if (claimedReportDigest !== spec.expectedReportDigest) {
    failureCodes.push('DigestMismatch')
  }

  if (
    claim.completedSteps !== undefined &&
    claim.completedSteps !== spec.completedSteps
  ) {
    failureCodes.push('DimensionMismatch')
  }

  if (failureCodes.length > 0) {
    return {
      failureCodes,
      publicDetails: {
        expectedCompletedSteps: spec.completedSteps,
        reason: 'claim_disagrees_with_committed_answer_key',
      },
      state: 'Rejected',
      verdictRefs: [spec.challengeRef],
    }
  }

  return {
    failureCodes: [],
    publicDetails: {
      completedSteps: spec.completedSteps,
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
 * Bridges the instruct-SFT grading challenge spec into the exact rail-side
 * `TrainingVerificationChallengeCreateRequest` envelope a paid
 * `psion_instruct_sft` dispatch would POST — the same
 * `deterministic_recompute` challenge shape the alignment run used. The
 * built request is decoded against the real `training-verification` schema,
 * so a structurally invalid request (e.g. a non-public-safe
 * `trainingRunRef`) fails loudly here instead of at the rails.
 *
 * IMPORTANT HONESTY BOUNDARY: this only constructs and validates the request
 * object. It does NOT submit it, create a challenge, take a lease, spend
 * sats, or settle anything — no rail-side mutation occurs. The payload
 * carries only public-safe digests, counts, and refs (never prompts,
 * completions, or weights), so
 * `blocker.product_promises.instruct_sft_paid_dispatch_missing` stays open:
 * the paid dispatch this request describes has not run.
 */
export const buildPsionInstructSftGradingChallengeCreateRequest = (
  input: Readonly<{
    spec: PsionInstructSftGradingChallengeSpec
    trainingRunRef: string
    windowRef?: string
  }>,
): TrainingVerificationChallengeCreateRequest => {
  const { spec, trainingRunRef, windowRef } = input

  if (!digestPattern.test(spec.expectedReportDigest)) {
    throw new PsionInstructSftGradingChallengeError(
      'Cannot build an instruct-SFT grading challenge request from a spec with a malformed expected digest.',
    )
  }

  const challengeRequest = {
    commitmentRefs: [
      `commitment.psion_instruct_sft_grading.${spec.laneId}`,
      spec.workloadRef,
    ],
    contributionRef: `contribution.psion_instruct_sft_grading.${spec.laneId}`,
    homeworkKind: PsionInstructSftGradingHomeworkKind,
    payload: {
      completedSteps: spec.completedSteps,
      expectedReportDigest: spec.expectedReportDigest,
      jobKind: PsionInstructSftGradingChallengeJobKind,
      laneId: spec.laneId,
      manifestDigest: spec.manifestDigest,
      recomputedDigestRef: `recompute.psion_instruct_sft_grading.${spec.laneId}`,
      runId: spec.runId,
      templateDigest: spec.templateDigest,
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
    throw new PsionInstructSftGradingChallengeError(
      'Instruct-SFT grading challenge create-request failed training-verification schema validation; check trainingRunRef/windowRef are public-safe refs.',
    )
  }
}
