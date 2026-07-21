import { Schema as S } from "effect"

import { canonicalArtifact } from "./artifact.ts"
import { Digest, NonEmptyString, PositiveInteger, RelativePath, StableRef } from "./schema.ts"

/**
 * openagents.authority_decision_receipt.v1 (root AUTHORITY.md
 * authority-delegation-receipts). A bounded, public-safe record of one
 * authority-gated decision: who acted, under which grant and program, against
 * which target, with which conditions evaluated, and what outcome resulted.
 *
 * This receipt is emitted by the deterministic independent-admission verifier
 * when it admits (or refuses to admit) an AssuranceSpec revision under
 * `grant.independent_assurance`. It never carries raw secrets, raw test output,
 * or private prompts (`raw_secrets_forbidden`, `public_safe_only`); private
 * evidence is referenced by path only.
 */
export const AUTHORITY_DECISION_RECEIPT_SCHEMA_ID = "openagents.authority_decision_receipt.v1" as const

/** Timestamp in ISO 8601 UTC, e.g. `2026-07-21T00:00:00Z`. */
const IsoInstant = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/),
)

export const AUTHORITY_DECISION_OUTCOMES = [
  "succeeded",
  "failed",
  "refused",
  "rolled_back",
  "narrowed",
  "revoked",
  "needs_owner_reserved_action",
] as const

export const AuthorityConditionResultSchema = S.Struct({
  condition_ref: StableRef,
  result: S.Literals(["satisfied", "not_satisfied", "not_applicable"]),
  statement: NonEmptyString,
})
export type AuthorityConditionResult = typeof AuthorityConditionResultSchema.Type

/**
 * The reproduction tally the verifier observed, so a reader can reconcile the
 * decision without re-reading the whole evidence map. Counts are public-safe
 * integers, never raw oracle output.
 */
export const AuthorityReproductionSummarySchema = S.Struct({
  executable_criteria: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  executable_green: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  executable_failed: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  smoke_gated: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  receipt_backed: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  designed_only: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  total_criteria: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  batches: S.Array(S.Struct({
    batch_id: StableRef,
    command: NonEmptyString,
    cwd: RelativePath,
    exit_code: S.Number.check(S.isInt()),
    ok: S.Boolean,
    tests_passed: S.optionalKey(S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0))),
    tests_failed: S.optionalKey(S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0))),
    files: S.optionalKey(S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0))),
  })),
})
export type AuthorityReproductionSummary = typeof AuthorityReproductionSummarySchema.Type

export const AuthorityDecisionReceiptSchema = S.Struct({
  schema_id: S.Literal(AUTHORITY_DECISION_RECEIPT_SCHEMA_ID),
  receipt_ref: StableRef,
  profile_id: StableRef,
  profile_revision: PositiveInteger,
  program_ref: StableRef,
  grant_ref: StableRef,
  actor_role: StableRef,
  action: NonEmptyString,
  target_ref: RelativePath,
  target_digest: Digest,
  trigger_ref: NonEmptyString,
  independence: S.Struct({
    reviewer_ref: StableRef,
    producer_ref: StableRef,
    distinct: S.Boolean,
    statement: NonEmptyString,
  }),
  condition_results: S.Array(AuthorityConditionResultSchema).check(S.isMinLength(1)),
  reproduction_summary: AuthorityReproductionSummarySchema,
  scope_notes: S.Array(NonEmptyString),
  started_at: IsoInstant,
  settled_at: IsoInstant,
  outcome: S.Literals(AUTHORITY_DECISION_OUTCOMES),
  evidence_refs: S.Array(RelativePath),
  public_safety: S.Struct({
    classification: S.Literal("reviewed_public_safe"),
    contains_raw_output: S.Literal(false),
  }),
})
export type AuthorityDecisionReceipt = typeof AuthorityDecisionReceiptSchema.Type
export const decodeAuthorityDecisionReceipt = S.decodeUnknownSync(AuthorityDecisionReceiptSchema)

/** Canonical, byte-stable serialization for the receipt artifact. */
export const authorityDecisionReceiptArtifact = (receipt: AuthorityDecisionReceipt) => canonicalArtifact(receipt)
