import { Schema as S } from "effect";

import { canonicalStringify } from "../internal/canonical.js";
import { sha256Hex } from "../internal/sha256.js";
import { CandidateId, DseTimestamp, Sha256Hex, SignatureId } from "./refs.js";
import { ReleasedArtifactPointer } from "./artifact.js";

/**
 * The gated-activation release channel and its evidence records (AFS-09).
 *
 * A released DSE artifact is never activated wholesale. A channel moves through
 * SHADOW (the compiled artifact is present but the hand-written baseline is
 * served — no dispatch, no user-visible substitution), CANARY (a bounded,
 * deterministic population is served the released artifact), ACTIVE (the
 * released artifact is served), and ROLLED_BACK (the previous released artifact
 * or the hand-written baseline is restored, with no application rebuild). The
 * baseline is content-addressed so it is a first-class shadow and rollback
 * target, not an implicit fallback.
 *
 * The channel is portable state only. It DECIDES which artifact a request should
 * serve; a runtime host ENACTS the decision. This module holds no dispatch,
 * provider, Apple FM, or Node authority.
 */

export const BASELINE_POINTER_SCHEMA_LITERAL = "openagents.dse.baseline_pointer.v1" as const;
export const RELEASE_CHANNEL_SCHEMA_LITERAL = "openagents.dse.release_channel.v1" as const;
export const CANARY_PLAN_SCHEMA_LITERAL = "openagents.dse.canary_plan.v1" as const;
export const ACTIVATION_RECEIPT_SCHEMA_LITERAL = "openagents.dse.activation_receipt.v1" as const;
export const UNCERTAINTY_RECORD_SCHEMA_LITERAL = "openagents.dse.uncertainty_record.v1" as const;

/** The four release modes a channel can occupy. */
export const ReleaseMode = S.Literals(["shadow", "canary", "active", "rolled_back"]);
export type ReleaseMode = typeof ReleaseMode.Type;

/**
 * The content-addressed hand-written baseline. Its digest covers the exact bytes
 * of the current hand-written prompt behavior, so a rollback to the baseline is
 * verifiable and a drift in the hand-written prompt changes the pointer.
 */
export const BaselinePointer = S.Struct({
  schema: S.Literal(BASELINE_POINTER_SCHEMA_LITERAL),
  signatureId: SignatureId,
  baselineRef: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  digest: Sha256Hex,
  description: S.String.check(S.isMinLength(1), S.isMaxLength(1000)),
});
export type BaselinePointer = typeof BaselinePointer.Type;

/**
 * The explicit canary rollout bounds. A canary is never open-ended: it names the
 * served population fraction, a maximum duration, an error-rate abort threshold,
 * and whether a measured regression aborts the rollout.
 */
export const CanaryPlan = S.Struct({
  schema: S.Literal(CANARY_PLAN_SCHEMA_LITERAL),
  populationFraction: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  maxDurationMs: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  abortErrorRate: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  abortOnRegression: S.Boolean,
});
export type CanaryPlan = typeof CanaryPlan.Type;

/**
 * A release channel for one signature. `candidate` is the released artifact
 * under rollout; `prior` is the previously active released artifact and the
 * rollback target. `baseline` is always present so SHADOW and a baseline
 * rollback have a verifiable target.
 */
export const ReleaseChannel = S.Struct({
  schema: S.Literal(RELEASE_CHANNEL_SCHEMA_LITERAL),
  signatureId: SignatureId,
  mode: ReleaseMode,
  baseline: BaselinePointer,
  candidate: S.optionalKey(ReleasedArtifactPointer),
  prior: S.optionalKey(ReleasedArtifactPointer),
  canary: S.optionalKey(CanaryPlan),
  updatedAt: DseTimestamp,
});
export type ReleaseChannel = typeof ReleaseChannel.Type;

/** The transitions a channel can record. */
export const ActivationTransition = S.Literals([
  "begin_shadow",
  "begin_canary",
  "promote",
  "abort_canary",
  "rollback",
]);
export type ActivationTransition = typeof ActivationTransition.Type;

/** An append-only activation receipt. It is evidence, never release authority. */
export const ActivationReceipt = S.Struct({
  schema: S.Literal(ACTIVATION_RECEIPT_SCHEMA_LITERAL),
  signatureId: SignatureId,
  transition: ActivationTransition,
  fromMode: ReleaseMode,
  toMode: ReleaseMode,
  candidateId: S.NullOr(CandidateId),
  restoredCandidateId: S.NullOr(CandidateId),
  reason: S.String.check(S.isMinLength(1), S.isMaxLength(2000)),
  at: DseTimestamp,
});
export type ActivationReceipt = typeof ActivationReceipt.Type;

/** How a small-sample uncertainty record was produced. */
export const UncertaintyMethod = S.Literals(["normal_approx_ci", "small_sample_note"]);
export type UncertaintyMethod = typeof UncertaintyMethod.Type;

/**
 * The uncertainty record that must accompany a small-dataset promotion. It pins
 * the baseline and candidate holdout scores, the delta, the sample size, and a
 * confidence interval (or an explicit small-sample note when the sample is too
 * small for a meaningful interval).
 */
export const UncertaintyRecord = S.Struct({
  schema: S.Literal(UNCERTAINTY_RECORD_SCHEMA_LITERAL),
  signatureId: SignatureId,
  candidateId: CandidateId,
  baselineHoldoutScore: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  candidateHoldoutScore: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  holdoutDelta: S.Number.check(S.isGreaterThanOrEqualTo(-1), S.isLessThanOrEqualTo(1)),
  sampleSize: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  method: UncertaintyMethod,
  ciLow: S.Number.check(S.isGreaterThanOrEqualTo(-1), S.isLessThanOrEqualTo(1)),
  ciHigh: S.Number.check(S.isGreaterThanOrEqualTo(-1), S.isLessThanOrEqualTo(1)),
  note: S.String.check(S.isMinLength(1), S.isMaxLength(2000)),
});
export type UncertaintyRecord = typeof UncertaintyRecord.Type;

const decodeBaselinePointer = S.decodeUnknownSync(BaselinePointer);

/**
 * Content-address a hand-written baseline behavior. The digest covers the exact
 * prompt bytes so the baseline is a verifiable shadow and rollback target.
 */
export const makeBaselinePointer = (args: {
  readonly signatureId: typeof SignatureId.Type;
  readonly baselineRef: string;
  readonly bytes: string;
  readonly description: string;
}): BaselinePointer =>
  decodeBaselinePointer({
    schema: BASELINE_POINTER_SCHEMA_LITERAL,
    signatureId: args.signatureId,
    baselineRef: args.baselineRef,
    digest: sha256Hex(canonicalStringify({ ref: args.baselineRef, bytes: args.bytes })),
    description: args.description,
  });
