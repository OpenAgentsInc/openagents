import { Schema as S } from "effect";
import { ARTIFACT_SCHEMA_LITERAL } from "@openagentsinc/agent-runtime-schema";

import {
  RELEASED_POINTER_SCHEMA_LITERAL,
  ReleasedArtifactPointer,
  releasedArtifactRefFor,
  type CandidateArtifact,
  type DseTimestamp,
  type EvaluationReport,
  type IndependentReviewResult,
  type PromotionRefusalReason,
  type PromotionRequest,
} from "../contract/index.js";

/**
 * The promotion gate with the independent-evaluator role.
 *
 * Promotion admits a candidate only when a reviewer identity DISTINCT from the
 * producer admits it, the holdout delta clears the requested floor, and the
 * reviewed report digests match. The producer can never verify or admit its own
 * obligation. On success the gate emits a released pointer that binds the frozen
 * `ReleasedArtifact` shape; on supersession it records the prior release as the
 * rollback target.
 */

const decodePointer = S.decodeUnknownSync(ReleasedArtifactPointer);

export type PromotionOutcome =
  | { readonly ok: true; readonly pointer: ReleasedArtifactPointer }
  | { readonly ok: false; readonly reason: PromotionRefusalReason };

export interface PromoteArgs {
  readonly request: PromotionRequest;
  readonly review: IndependentReviewResult;
  readonly winner: CandidateArtifact;
  readonly holdoutReport: EvaluationReport;
  readonly priorRelease?: ReleasedArtifactPointer;
  readonly now: () => typeof DseTimestamp.Type;
}

export const promote = (args: PromoteArgs): PromotionOutcome => {
  const { request, review, winner, holdoutReport } = args;

  if (review.promotionId !== request.promotionId)
    return { ok: false, reason: "promotion_mismatch" };
  if (
    request.candidateId !== winner.candidateId ||
    review.candidateId !== winner.candidateId ||
    request.signatureId !== winner.signatureId ||
    review.signatureId !== winner.signatureId
  ) {
    return { ok: false, reason: "candidate_mismatch" };
  }
  // Independence: the reviewer identity must differ from the producer identity.
  // The two identities carry distinct brands, so compare their string values.
  if (String(review.reviewer.id) === String(request.producer.id)) {
    return { ok: false, reason: "producer_cannot_self_admit" };
  }
  if (review.decision !== "admit") return { ok: false, reason: "reviewer_rejected" };
  if (
    request.holdoutReportDigest !== holdoutReport.digest ||
    review.reviewedHoldoutReportDigest !== holdoutReport.digest
  ) {
    return { ok: false, reason: "holdout_report_mismatch" };
  }
  if (review.holdoutDelta < request.minHoldoutDelta) {
    return { ok: false, reason: "holdout_delta_below_floor" };
  }

  const releasedAt = args.now();
  const artifactRef = releasedArtifactRefFor(winner.signatureId, winner.digest);
  const pointer = decodePointer({
    schema: RELEASED_POINTER_SCHEMA_LITERAL,
    signatureId: winner.signatureId,
    candidateId: winner.candidateId,
    promotionId: request.promotionId,
    released: {
      schema: ARTIFACT_SCHEMA_LITERAL,
      artifactRef,
      digest: winner.digest,
      kind: "prompt_program",
      releasedAt,
      ...(args.priorRelease === undefined
        ? {}
        : { rollbackOf: args.priorRelease.released.artifactRef }),
    },
    evaluationReportDigest: holdoutReport.digest,
    releasedAt,
  });
  return { ok: true, pointer };
};
