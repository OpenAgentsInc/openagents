import { Effect, Schema as S } from "effect";

/**
 * The review gate is deliberately below the web UI and above signer custody.
 * It decides whether a maintainer MAY ask a NIP-46 signer for a state event;
 * it never receives a signing key or an environment credential.
 */

const PublicRef = S.String.check(S.isMinLength(1), S.isMaxLength(512));
const GitObjectId = S.String.check(S.isPattern(/^[0-9a-f]{40}(?:[0-9a-f]{24})?$/i));

export const ForgeReviewVerdict = S.Literals(["approved", "change_requested", "commented"]);
export type ForgeReviewVerdict = typeof ForgeReviewVerdict.Type;

export const ForgeCheckState = S.Literals(["queued", "running", "completed"]);
export type ForgeCheckState = typeof ForgeCheckState.Type;

export const ForgeCheckVerdict = S.Literals(["passed", "failed", "cancelled", "errored"]);
export type ForgeCheckVerdict = typeof ForgeCheckVerdict.Type;

export const ForgeVerificationRung = S.Literals([
  "tests",
  "replay",
  "model_review",
  "second_agent",
  "human",
  "owner",
]);
export type ForgeVerificationRung = typeof ForgeVerificationRung.Type;

export const ForgeVerificationRungState = S.Literals(["passed", "failed", "flaky_retry_exhausted"]);
export type ForgeVerificationRungState = typeof ForgeVerificationRungState.Type;

export const ForgeReviewRecord = S.Struct({
  reviewRef: PublicRef,
  reviewerBindingRef: PublicRef,
  revisionObjectId: GitObjectId,
  verdict: ForgeReviewVerdict,
  submittedAt: S.String,
  supersedesReviewRef: S.NullOr(PublicRef),
});
export interface ForgeReviewRecord extends S.Schema.Type<typeof ForgeReviewRecord> {}

export const ForgeCheckRecord = S.Struct({
  checkRef: PublicRef,
  checkName: PublicRef,
  revisionObjectId: GitObjectId,
  state: ForgeCheckState,
  verdict: S.NullOr(ForgeCheckVerdict),
  evidenceReceiptRef: S.NullOr(PublicRef),
  completedAt: S.NullOr(S.String),
});
export interface ForgeCheckRecord extends S.Schema.Type<typeof ForgeCheckRecord> {}

export const ForgeVerificationRungReceipt = S.Struct({
  receiptRef: PublicRef,
  rung: ForgeVerificationRung,
  revisionObjectId: GitObjectId,
  state: ForgeVerificationRungState,
  attempt: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  maxAttempts: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  humanTagRef: S.NullOr(PublicRef),
});
export interface ForgeVerificationRungReceipt extends S.Schema.Type<
  typeof ForgeVerificationRungReceipt
> {}

export const ForgeMergeGateInput = S.Struct({
  tenantRef: PublicRef,
  repositoryRef: PublicRef,
  changeRef: PublicRef,
  maintainerBindingRef: PublicRef,
  // A Forge promotion may advance a reviewed branch or publish an immutable
  // annotated release/import tag. Other namespaces remain out of scope.
  targetRef: S.String.check(S.isPattern(/^refs\/(?:heads|tags)\/[A-Za-z0-9._/-]+$/)),
  oldObjectId: GitObjectId,
  newObjectId: GitObjectId,
  authorityGeneration: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  policyVersion: PublicRef,
  proposalEventIds: S.Array(PublicRef).check(S.isMinLength(1)),
  requiredReviewCount: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  requiredCheckNames: S.Array(PublicRef),
  requiredVerificationRungs: S.Array(ForgeVerificationRung),
  reviews: S.Array(ForgeReviewRecord),
  checks: S.Array(ForgeCheckRecord),
  verificationReceipts: S.Array(ForgeVerificationRungReceipt),
  evaluatedAt: S.String,
});
export interface ForgeMergeGateInput extends S.Schema.Type<typeof ForgeMergeGateInput> {}

export const ForgeMergeGateResult = S.Struct({
  gateRef: PublicRef,
  verdict: S.Literals(["passed", "blocked"]),
  evidenceRefs: S.Array(PublicRef),
  blockerRefs: S.Array(PublicRef),
  decidedAt: S.String,
});
export interface ForgeMergeGateResult extends S.Schema.Type<typeof ForgeMergeGateResult> {}

export const ForgeMergeGateDecision = S.Struct({
  allowed: S.Boolean,
  gateResults: S.Array(ForgeMergeGateResult),
});
export interface ForgeMergeGateDecision extends S.Schema.Type<typeof ForgeMergeGateDecision> {}

export class ForgeMergeGateError extends S.TaggedErrorClass<ForgeMergeGateError>()(
  "ForgeMergeGateError",
  {
    blockers: S.Array(PublicRef),
    code: S.Literals(["forge_merge_gate_blocked", "forge_merge_receipt_conflict"]),
  },
) {}

const gate = (
  gateRef: string,
  passed: boolean,
  evidenceRefs: ReadonlyArray<string>,
  blockerRefs: ReadonlyArray<string>,
  decidedAt: string,
): ForgeMergeGateResult =>
  ForgeMergeGateResult.make({
    blockerRefs: [...blockerRefs],
    decidedAt,
    evidenceRefs: [...evidenceRefs],
    gateRef,
    verdict: passed ? "passed" : "blocked",
  });

/** Reviews for an older commit remain historical. A later review only resolves
 * a change request when it explicitly supersedes that review. */
const currentReviews = (reviews: ReadonlyArray<ForgeReviewRecord>, revisionObjectId: string) => {
  const historical = reviews.filter((review) => review.revisionObjectId === revisionObjectId);
  const reviewByRef = new Map(historical.map((review) => [review.reviewRef, review]));
  const superseded = new Set(
    historical.flatMap((review) => {
      if (review.supersedesReviewRef === null) return [];
      const previous = reviewByRef.get(review.supersedesReviewRef);
      return previous?.reviewerBindingRef === review.reviewerBindingRef
        ? [review.supersedesReviewRef]
        : [];
    }),
  );
  return historical.filter((review) => !superseded.has(review.reviewRef));
};

export const evaluateForgeMergeGates = Effect.fn("ForgeReviewGates.evaluate")(function* (
  input: ForgeMergeGateInput,
) {
  const reviews = currentReviews(input.reviews, input.newObjectId);
  const unresolvedChanges = reviews.filter((review) => review.verdict === "change_requested");
  const approvals = new Set(
    reviews
      .filter((review) => review.verdict === "approved")
      .map((review) => review.reviewerBindingRef),
  );
  const reviewGate = gate(
    "forge.gate.review",
    unresolvedChanges.length === 0 && approvals.size >= input.requiredReviewCount,
    reviews.map((review) => review.reviewRef),
    [
      ...unresolvedChanges.map((review) => `review.change_requested.${review.reviewRef}`),
      ...(approvals.size >= input.requiredReviewCount ? [] : ["review.required_approvals_missing"]),
    ],
    input.evaluatedAt,
  );

  const checksByName = new Map(
    input.checks
      .filter((check) => check.revisionObjectId === input.newObjectId)
      .map((check) => [check.checkName, check]),
  );
  const missingChecks = input.requiredCheckNames.filter((name) => {
    const check = checksByName.get(name);
    return (
      check === undefined ||
      check.state !== "completed" ||
      check.verdict !== "passed" ||
      check.evidenceReceiptRef === null
    );
  });
  const checkGate = gate(
    "forge.gate.verification",
    missingChecks.length === 0,
    [...checksByName.values()].flatMap((check) =>
      check.evidenceReceiptRef === null
        ? [check.checkRef]
        : [check.checkRef, check.evidenceReceiptRef],
    ),
    missingChecks.map((name) => `check.required_or_passing_missing.${name}`),
    input.evaluatedAt,
  );

  const ladderByRung = new Map(
    input.verificationReceipts
      .filter((receipt) => receipt.revisionObjectId === input.newObjectId)
      .map((receipt) => [receipt.rung, receipt]),
  );
  const ladderBlockers = input.requiredVerificationRungs.flatMap((rung) => {
    const receipt = ladderByRung.get(rung);
    if (receipt === undefined) return [`verification.rung_missing.${rung}`];
    if (receipt.state === "passed") return [];
    if (
      receipt.state === "flaky_retry_exhausted" &&
      receipt.humanTagRef !== null &&
      receipt.attempt === receipt.maxAttempts
    ) {
      return [`verification.flaky_retry_exhausted.${rung}`];
    }
    return [`verification.rung_not_passed.${rung}`];
  });
  const malformedFlaky = input.verificationReceipts.flatMap((receipt) => {
    if (receipt.state !== "flaky_retry_exhausted") return [];
    return receipt.humanTagRef !== null && receipt.attempt === receipt.maxAttempts
      ? []
      : [`verification.flaky_retry_invalid.${receipt.rung}`];
  });
  const verificationGate = gate(
    "forge.gate.verification_ladder",
    ladderBlockers.length === 0 && malformedFlaky.length === 0,
    [...ladderByRung.values()].map((receipt) => receipt.receiptRef),
    [...ladderBlockers, ...malformedFlaky],
    input.evaluatedAt,
  );

  const policyGate = gate(
    "forge.gate.policy",
    input.policyVersion.trim().length > 0 &&
      input.authorityGeneration > 0 &&
      input.oldObjectId !== input.newObjectId,
    [`policy.${input.policyVersion}`],
    [
      ...(input.policyVersion.trim().length > 0 ? [] : ["policy.version_missing"]),
      ...(input.oldObjectId === input.newObjectId ? ["policy.noop_ref_update"] : []),
    ],
    input.evaluatedAt,
  );
  const gateResults = [reviewGate, checkGate, verificationGate, policyGate];
  return ForgeMergeGateDecision.make({
    allowed: gateResults.every((result) => result.verdict === "passed"),
    gateResults,
  });
});

export const ForgeNip46MergeSigningRequest = S.Struct({
  kind: S.Literal(30618),
  receiptRef: PublicRef,
  targetRef: S.String,
  oldObjectId: GitObjectId,
  newObjectId: GitObjectId,
  authorityGeneration: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  policyVersion: PublicRef,
});
export interface ForgeNip46MergeSigningRequest extends S.Schema.Type<
  typeof ForgeNip46MergeSigningRequest
> {}

export const ForgeNip46SignedState = S.Struct({
  eventId: PublicRef,
  eventKind: S.Literal(30618),
  signerPubkey: PublicRef,
  signature: PublicRef,
});
export interface ForgeNip46SignedState extends S.Schema.Type<typeof ForgeNip46SignedState> {}

export const ForgeMergeOutcomeReceiptDraft = S.Struct({
  schema: S.Literal("openagents.forge.merge.outcome.receipt.v1"),
  receiptRef: PublicRef,
  tenantRef: PublicRef,
  repositoryRef: PublicRef,
  changeRef: PublicRef,
  maintainerBindingRef: PublicRef,
  targetRef: S.String,
  oldObjectId: GitObjectId,
  newObjectId: GitObjectId,
  authorityGeneration: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  policyVersion: PublicRef,
  proposalEventIds: S.Array(PublicRef),
  gateResults: S.Array(ForgeMergeGateResult),
  redacted: S.Literal(true),
  decidedAt: S.String,
});
export interface ForgeMergeOutcomeReceiptDraft extends S.Schema.Type<
  typeof ForgeMergeOutcomeReceiptDraft
> {}

export const ForgeMergeOutcomeReceipt = S.Struct({
  ...ForgeMergeOutcomeReceiptDraft.fields,
  signedState: ForgeNip46SignedState,
});
export interface ForgeMergeOutcomeReceipt extends S.Schema.Type<typeof ForgeMergeOutcomeReceipt> {}

export interface ForgeNip46MergeSigner {
  readonly signState: (
    request: ForgeNip46MergeSigningRequest,
  ) => Effect.Effect<ForgeNip46SignedState, ForgeMergeGateError>;
}

/** Storage must make a draft durable before a state is signed, then make the
 * signed outcome durable before the caller publishes the state to a relay. */
export interface ForgeMergeReceiptStore {
  readonly prepare: (
    receipt: ForgeMergeOutcomeReceiptDraft,
  ) => Effect.Effect<void, ForgeMergeGateError>;
  readonly finalize: (
    receipt: ForgeMergeOutcomeReceipt,
  ) => Effect.Effect<void, ForgeMergeGateError>;
}

export interface ForgeSignedStatePublisher {
  readonly publish: (state: ForgeNip46SignedState) => Effect.Effect<void, ForgeMergeGateError>;
}

/**
 * Evaluate a merge and durably record the exact state that a remote signer may
 * sign.  The split is intentional: a service can prepare this authorization
 * without holding a Nostr key, then hand the resulting receipt reference to a
 * NIP-46 signer.  A later finalization must bind the signed 30618 to this
 * draft before it can authorize a Git ref move.
 */
export const prepareForgeMergeOutcome = Effect.fn("ForgeReviewGates.prepare")(function* (
  input: ForgeMergeGateInput,
  receiptRef: string,
  receipts: ForgeMergeReceiptStore,
) {
  const decision = yield* evaluateForgeMergeGates(input);
  if (!decision.allowed) {
    return yield* new ForgeMergeGateError({
      blockers: decision.gateResults.flatMap((result) => result.blockerRefs),
      code: "forge_merge_gate_blocked",
    });
  }
  const draft = ForgeMergeOutcomeReceiptDraft.make({
    authorityGeneration: input.authorityGeneration,
    changeRef: input.changeRef,
    decidedAt: input.evaluatedAt,
    gateResults: decision.gateResults,
    maintainerBindingRef: input.maintainerBindingRef,
    newObjectId: input.newObjectId,
    oldObjectId: input.oldObjectId,
    policyVersion: input.policyVersion,
    proposalEventIds: [...input.proposalEventIds],
    receiptRef,
    redacted: true,
    repositoryRef: input.repositoryRef,
    schema: "openagents.forge.merge.outcome.receipt.v1",
    targetRef: input.targetRef,
    tenantRef: input.tenantRef,
  });
  yield* receipts.prepare(draft);
  return draft;
});

export const authorizeSignAndPublishForgeMerge = Effect.fn(
  "ForgeReviewGates.authorizeSignAndPublish",
)(function* (
  input: ForgeMergeGateInput,
  receiptRef: string,
  signer: ForgeNip46MergeSigner,
  receipts: ForgeMergeReceiptStore,
  publisher: ForgeSignedStatePublisher,
) {
  const draft = yield* prepareForgeMergeOutcome(input, receiptRef, receipts);
  const signedState = yield* signer.signState(
    ForgeNip46MergeSigningRequest.make({
      authorityGeneration: input.authorityGeneration,
      kind: 30618,
      newObjectId: input.newObjectId,
      oldObjectId: input.oldObjectId,
      policyVersion: input.policyVersion,
      receiptRef,
      targetRef: input.targetRef,
    }),
  );
  const receipt = ForgeMergeOutcomeReceipt.make({ ...draft, signedState });
  yield* receipts.finalize(receipt);
  yield* publisher.publish(signedState);
  return receipt;
});
