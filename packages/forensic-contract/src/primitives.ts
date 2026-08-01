import { Schema as S } from "effect";

export const ForensicRef = S.String.check(
  S.isMinLength(3),
  S.isMaxLength(512),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:/#-]*$/),
).annotate({ identifier: "ForensicRef" });
export type ForensicRef = typeof ForensicRef.Type;

export const ForensicPath = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(1_024),
  S.makeFilter((value) => !value.startsWith("/") && !value.includes(".."), {
    message: "forensic paths must be repository-relative and cannot traverse parents",
  }),
).annotate({ identifier: "ForensicPath" });
export type ForensicPath = typeof ForensicPath.Type;

export const CommitSha = S.String.check(S.isPattern(/^[a-f0-9]{40}$/)).annotate({
  identifier: "CommitSha",
});
export type CommitSha = typeof CommitSha.Type;

export const Sha256Digest = S.String.check(S.isPattern(/^sha256:[a-f0-9]{64}$/)).annotate({
  identifier: "Sha256Digest",
});
export type Sha256Digest = typeof Sha256Digest.Type;

export const ForensicTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/),
).annotate({ identifier: "ForensicTimestamp" });
export type ForensicTimestamp = typeof ForensicTimestamp.Type;

export const ShortText = S.String.check(S.isMinLength(1), S.isMaxLength(1_000)).annotate({
  identifier: "ForensicShortText",
});
export type ShortText = typeof ShortText.Type;
export const LongText = S.String.check(S.isMinLength(1), S.isMaxLength(8_000)).annotate({
  identifier: "ForensicLongText",
});
export type LongText = typeof LongText.Type;

export const NonNegativeInteger = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)).annotate({
  identifier: "ForensicNonNegativeInteger",
});
export type NonNegativeInteger = typeof NonNegativeInteger.Type;

export const PositiveInteger = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)).annotate({
  identifier: "ForensicPositiveInteger",
});
export type PositiveInteger = typeof PositiveInteger.Type;

export const PositiveDecimalString = S.String.check(
  S.isPattern(/^(?:0|[1-9][0-9]*)(?:\.[0-9]+)?$/),
).annotate({ identifier: "PositiveDecimalString" });
export type PositiveDecimalString = typeof PositiveDecimalString.Type;

export const Exactness = S.Literals(["exact", "estimated", "upper_bound", "unavailable"]);
export type Exactness = typeof Exactness.Type;

export const EvidenceTier = S.Literals([
  "hypothesis",
  "source_observed",
  "artifact_observed",
  "executed",
  "independently_verified",
]);
export type EvidenceTier = typeof EvidenceTier.Type;

export const ClaimKind = S.Literals([
  "source_flaw",
  "artifact_selected",
  "state_space_model",
  "owned_fixture_recovered",
  "program_fingerprint",
  "entity_cluster",
  "unauthorized_movement",
  "identity_attribution",
]);
export type ClaimKind = typeof ClaimKind.Type;

export const ClaimAdjudication = S.Literals([
  "unverified",
  "qualified",
  "verified",
  "retained",
  "rejected",
  "superseded",
]);
export type ClaimAdjudication = typeof ClaimAdjudication.Type;

export const EvidenceKind = S.Literals([
  "source_reference",
  "preprocessed_source",
  "symbol_provider",
  "firmware_digest",
  "fault_build",
  "generator_vector",
  "assumption_model",
  "sensitivity_receipt",
  "owned_fixture_authorization",
  "generator_trace",
  "wallet_match",
  "historical_chain_snapshot",
  "positive_control",
  "negative_control",
  "base_rate",
  "evidence_graph",
  "pooling_edge",
  "victim_testimony",
  "transaction_reference",
  "identity_evidence",
  "independent_review",
]);
export type EvidenceKind = typeof EvidenceKind.Type;

export const BoundedRefs = S.Array(ForensicRef).check(S.isMaxLength(256));
export const NonEmptyBoundedRefs = BoundedRefs.check(S.isMinLength(1));
export const BoundedDigests = S.Array(Sha256Digest).check(S.isMaxLength(256));
export const BoundedShortTexts = S.Array(ShortText).check(S.isMaxLength(128));

const AvailableUsageTruthSchema = S.Struct({
  inputTokens: NonNegativeInteger,
  cachedInputTokens: NonNegativeInteger,
  outputTokens: NonNegativeInteger,
  reasoningTokens: NonNegativeInteger,
  totalTokens: NonNegativeInteger,
  exactness: S.Literals(["exact", "estimated", "upper_bound"]),
  providerUsageRef: S.optionalKey(ForensicRef),
}).pipe(
  S.check(
    S.makeFilter(
      (usage) =>
        usage.totalTokens === usage.inputTokens + usage.cachedInputTokens + usage.outputTokens,
      { message: "available total tokens must equal input + cached input + output" },
    ),
  ),
);

const UnavailableUsageTruthSchema = S.Struct({
  exactness: S.Literal("unavailable"),
  unavailableReasonRef: ForensicRef,
});

export const UsageTruthSchema = S.Union([
  AvailableUsageTruthSchema,
  UnavailableUsageTruthSchema,
]).annotate({ identifier: "ForensicUsageTruth" });
export type UsageTruth = typeof UsageTruthSchema.Type;

export const DurationTruthSchema = S.Struct({
  milliseconds: NonNegativeInteger,
  exactness: Exactness,
});
export interface DurationTruth extends S.Schema.Type<typeof DurationTruthSchema> {}
