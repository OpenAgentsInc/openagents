import { Schema as S } from "effect"

import { Digest, NonEmptyString } from "./schema.ts"

/**
 * openagents.assurance.independent-review.v1
 *
 * A receipt for one independent review of one obligation against one candidate.
 *
 * The design and its falsifiers live in
 * `docs/assurance/2026-07-25-sarah-as-independent-reviewer-spec.md`. The rules
 * that matter are encoded here as decode failures rather than left as guidance,
 * because a reviewer contract that can be satisfied by a careless producer is
 * not a contract.
 *
 * The load-bearing property is that `accepted` is hard to reach and `refused`
 * is easy. A reviewer that cannot reproduce something must be able to say so
 * without that reading as approval, which is why `inconclusive` exists as a
 * first-class outcome rather than as a flavour of failure.
 */
export const INDEPENDENT_REVIEW_SCHEMA_ID = "openagents.assurance.independent-review.v1" as const

/** Timestamp in ISO 8601 UTC, e.g. `2026-07-25T00:00:00Z`. */
const IsoInstant = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/),
)

/** A 64-character lowercase hex Nostr public key. */
const Pubkey = S.String.check(S.isPattern(/^[0-9a-f]{64}$/))

export const INDEPENDENT_REVIEW_OUTCOMES = ["accepted", "refused", "inconclusive"] as const
export const IndependentReviewOutcome = S.Literals(INDEPENDENT_REVIEW_OUTCOMES)
export type IndependentReviewOutcome = S.Schema.Type<typeof IndependentReviewOutcome>

/**
 * One check the reviewer ran for itself.
 *
 * `command` is recorded so a third party can re-run it. A review whose own
 * claim cannot be checked has moved the trust problem rather than solved it
 * (spec R5).
 */
export const ReviewReproduction = S.Struct({
  check: NonEmptyString,
  command: NonEmptyString,
  observedDigest: Digest,
  agreesWithProducer: S.Boolean,
})
export interface ReviewReproduction extends S.Schema.Type<typeof ReviewReproduction> {}

/** A place the reviewer's own observation differed from the producer's claim. */
export const ReviewDisagreement = S.Struct({
  criterion: NonEmptyString,
  producerClaim: NonEmptyString,
  reviewerObservation: NonEmptyString,
})
export interface ReviewDisagreement extends S.Schema.Type<typeof ReviewDisagreement> {}

export const IndependentReviewReceipt = S.Struct({
  schema: S.Literal(INDEPENDENT_REVIEW_SCHEMA_ID),
  /** The reviewer identity. Must differ from every producer key (spec R1). */
  reviewerPubkey: Pubkey,
  /** Keys that produced the obligation under review. */
  producerPubkeys: S.Array(Pubkey),
  /** The exact candidate this review is bound to. */
  candidateDigest: Digest,
  obligationRef: NonEmptyString,
  reviewedAt: IsoInstant,
  outcome: IndependentReviewOutcome,
  reproductions: S.Array(ReviewReproduction),
  disagreements: S.Array(ReviewDisagreement),
  /**
   * A prior review of this same candidate that this one replaces.
   *
   * Required when an earlier review exists, so a producer cannot quietly run
   * reviews until one passes (spec R3).
   */
  supersedes: S.optional(Digest),
  supersedesReason: S.optional(NonEmptyString),
  evidenceSha256: Digest,
  /**
   * Schnorr signature over `evidenceSha256` by `reviewerPubkey`.
   *
   * Without this the receipt only *claims* an identity, and any producer could
   * write the reviewer's public key into a document it authored itself. The
   * key is what makes independence checkable rather than asserted.
   */
  reviewerSignature: S.String.check(S.isPattern(/^[0-9a-f]{128}$/)),
})
export interface IndependentReviewReceipt extends S.Schema.Type<typeof IndependentReviewReceipt> {}

export class IndependentReviewError extends Error {}

const fail = (message: string): never => {
  throw new IndependentReviewError(message)
}

const decode = S.decodeUnknownSync(IndependentReviewReceipt)

/**
 * Decode a review receipt, enforcing the spec's rules.
 *
 * Every rule here exists because its absence would let a review look
 * independent without being it.
 */
export const decodeIndependentReviewReceipt = (value: unknown): IndependentReviewReceipt => {
  const receipt = decode(value, { onExcessProperty: "error" })

  // R1. The producer may never sign its own review. This is the whole point.
  if (receipt.producerPubkeys.includes(receipt.reviewerPubkey)) {
    fail("independent review: the reviewer key is also a producer key")
  }
  if (receipt.producerPubkeys.length === 0) {
    fail("independent review: the obligation names no producer to be independent of")
  }
  if (new Set(receipt.producerPubkeys).size !== receipt.producerPubkeys.length) {
    fail("independent review: producer keys repeat")
  }

  // A review that reproduced nothing has reviewed nothing, whatever it says.
  if (receipt.reproductions.length === 0) {
    fail("independent review: no reproduction was recorded")
  }
  const checks = receipt.reproductions.map((reproduction) => reproduction.check)
  if (new Set(checks).size !== checks.length) {
    fail("independent review: a check is reproduced more than once")
  }

  if (receipt.outcome === "accepted") {
    // There is no partial acceptance. One disagreement, or one check the
    // reviewer could not confirm, and this is not an acceptance.
    if (receipt.disagreements.length > 0) {
      fail("independent review: accepted with recorded disagreements")
    }
    if (!receipt.reproductions.every((reproduction) => reproduction.agreesWithProducer)) {
      fail("independent review: accepted while a reproduction disagreed with the producer")
    }
  } else {
    // Refusal and inconclusive must say why, or they are unactionable.
    const explained =
      receipt.disagreements.length > 0 ||
      receipt.reproductions.some((reproduction) => !reproduction.agreesWithProducer)
    if (!explained) {
      fail(`independent review: ${receipt.outcome} without a recorded reason`)
    }
  }

  // R3. Superseding a prior review is allowed, but never silently.
  if (receipt.supersedes !== undefined && receipt.supersedesReason === undefined) {
    fail("independent review: superseded a prior review without a stated reason")
  }
  if (receipt.supersedes === undefined && receipt.supersedesReason !== undefined) {
    fail("independent review: gave a supersede reason without naming the review it replaces")
  }
  if (receipt.supersedes === receipt.evidenceSha256) {
    fail("independent review: a review cannot supersede itself")
  }

  return receipt
}

/**
 * Verify the reviewer actually signed this receipt.
 *
 * Kept separate from decoding because it needs a verifier: decoding proves the
 * shape and the laws, this proves the authorship. A caller that skips it is
 * trusting a public key typed into a file.
 */
export const verifyIndependentReviewSignature = (
  receipt: IndependentReviewReceipt,
  verify: (signature: string, message: string, pubkey: string) => boolean,
): boolean =>
  verify(receipt.reviewerSignature, receipt.evidenceSha256, receipt.reviewerPubkey)

/**
 * True when this receipt admits the obligation.
 *
 * Deliberately narrow, and deliberately not a boolean on the receipt itself:
 * callers must ask this question rather than reading a field that a producer
 * could set.
 */
export const independentReviewAdmits = (receipt: IndependentReviewReceipt): boolean =>
  receipt.outcome === "accepted"
