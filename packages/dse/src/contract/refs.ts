import { Schema as S } from "effect";

/**
 * DSE branded references and shared scalars.
 *
 * These are portable Effect schemas. A DSE reference allows a `/` so a signature
 * ID such as `AppleFm/HonestChatReply.v1` decodes, unlike the frozen turn
 * reference which forbids it. Every reference is bounded and pattern-checked.
 */

const dseRef = <const Brand extends string>(brand: Brand) =>
  S.String.check(
    S.isMinLength(1),
    S.isMaxLength(256),
    S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:/-]*$/),
  ).pipe(S.brand(brand));

/** ISO-8601 UTC timestamp, matching the frozen turn timestamp shape. */
export const DseTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
);
export type DseTimestamp = typeof DseTimestamp.Type;

/** A 64-character lowercase-hex SHA-256 digest. */
export const Sha256Hex = S.String.check(S.isPattern(/^[a-f0-9]{64}$/));
export type Sha256Hex = typeof Sha256Hex.Type;

/** Honest usage truth, matching the Apple FM contract vocabulary. */
export const DseUsageTruth = S.Literals(["exact", "estimated", "unknown"]);
export type DseUsageTruth = typeof DseUsageTruth.Type;

export const SignatureId = dseRef("DseSignatureId");
export type SignatureId = typeof SignatureId.Type;

export const ExampleId = dseRef("DseExampleId");
export type ExampleId = typeof ExampleId.Type;

export const DatasetId = dseRef("DseDatasetId");
export type DatasetId = typeof DatasetId.Type;

export const DatasetRevisionId = dseRef("DseDatasetRevisionId");
export type DatasetRevisionId = typeof DatasetRevisionId.Type;

export const CandidateId = dseRef("DseCandidateId");
export type CandidateId = typeof CandidateId.Type;

export const ReceiptId = dseRef("DseReceiptId");
export type ReceiptId = typeof ReceiptId.Type;

export const PromotionId = dseRef("DsePromotionId");
export type PromotionId = typeof PromotionId.Type;

export const ProducerId = dseRef("DseProducerId");
export type ProducerId = typeof ProducerId.Type;

export const ReviewerId = dseRef("DseReviewerId");
export type ReviewerId = typeof ReviewerId.Type;

/** Trusted constructors for scripts, tests, and derivation paths. */
export const signatureId = S.decodeUnknownSync(SignatureId);
export const exampleId = S.decodeUnknownSync(ExampleId);
export const datasetId = S.decodeUnknownSync(DatasetId);
export const datasetRevisionId = S.decodeUnknownSync(DatasetRevisionId);
export const candidateId = S.decodeUnknownSync(CandidateId);
export const receiptId = S.decodeUnknownSync(ReceiptId);
export const promotionId = S.decodeUnknownSync(PromotionId);
export const producerId = S.decodeUnknownSync(ProducerId);
export const reviewerId = S.decodeUnknownSync(ReviewerId);
