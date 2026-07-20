import { Schema as S } from "effect";

import {
  CandidateId,
  DseTimestamp,
  ProducerId,
  PromotionId,
  ReviewerId,
  Sha256Hex,
  SignatureId,
} from "./refs.js";

/**
 * Promotion and the independent-evaluator role.
 *
 * A `PromotionRequest` is raised by the producer of a candidate. An
 * `IndependentReviewResult` is issued by a reviewer identity. Promotion admits a
 * candidate only when the reviewer is distinct from the producer, the reviewer
 * admitted the candidate, the holdout delta meets the requested floor, and the
 * reviewed report digests match. The producer can never admit its own
 * obligation. A human can fill the reviewer role; the mechanism enforces the
 * separation regardless.
 */

export const PROMOTION_REQUEST_SCHEMA_LITERAL = "openagents.dse.promotion_request.v1" as const;
export const INDEPENDENT_REVIEW_SCHEMA_LITERAL =
  "openagents.dse.independent_review_result.v1" as const;

export const ProducerIdentity = S.Struct({
  kind: S.Literal("producer"),
  id: ProducerId,
});
export type ProducerIdentity = typeof ProducerIdentity.Type;

export const ReviewerIdentity = S.Struct({
  kind: S.Literal("reviewer"),
  id: ReviewerId,
});
export type ReviewerIdentity = typeof ReviewerIdentity.Type;

/** The producer's request to release a candidate, with its report digests and delta floor. */
export const PromotionRequest = S.Struct({
  schema: S.Literal(PROMOTION_REQUEST_SCHEMA_LITERAL),
  promotionId: PromotionId,
  signatureId: SignatureId,
  candidateId: CandidateId,
  producer: ProducerIdentity,
  validationReportDigest: Sha256Hex,
  holdoutReportDigest: Sha256Hex,
  baselineCandidateId: S.optionalKey(CandidateId),
  minHoldoutDelta: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  requestedAt: DseTimestamp,
});
export type PromotionRequest = typeof PromotionRequest.Type;

/** The independent reviewer's decision on a promotion request. */
export const IndependentReviewResult = S.Struct({
  schema: S.Literal(INDEPENDENT_REVIEW_SCHEMA_LITERAL),
  promotionId: PromotionId,
  signatureId: SignatureId,
  candidateId: CandidateId,
  reviewer: ReviewerIdentity,
  decision: S.Literals(["admit", "reject"]),
  holdoutDelta: S.Number.check(S.isGreaterThanOrEqualTo(-1), S.isLessThanOrEqualTo(1)),
  reviewedHoldoutReportDigest: Sha256Hex,
  reason: S.String.check(S.isMinLength(1), S.isMaxLength(2000)),
  reviewedAt: DseTimestamp,
});
export type IndependentReviewResult = typeof IndependentReviewResult.Type;

/** Why a promotion was refused. The producer cannot self-admit. */
export type PromotionRefusalReason =
  | "producer_cannot_self_admit"
  | "reviewer_rejected"
  | "promotion_mismatch"
  | "candidate_mismatch"
  | "holdout_report_mismatch"
  | "holdout_delta_below_floor";
