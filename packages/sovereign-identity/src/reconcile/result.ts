/**
 * IDR-04 public-safe reconciliation result and typed conflict.
 *
 * Reconciliation NEVER imports, creates, or writes anything. It only classifies:
 * it derives each candidate's PUBLIC identity, groups duplicates, compares them
 * against the public local records, and returns a typed result. Import is IDR-05.
 *
 * Every schema here carries PUBLIC identifiers only — an `npub`, a hex Nostr
 * public key, and per-adapter Spark fingerprints. None ever carries the
 * mnemonic, `nsec`, raw private key, or seed. A serialized result is safe to log
 * or persist.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Schema as S } from "effect";
import { Npub, SparkAdapterFingerprint, SparkAdapterKind } from "../contract/index.ts";

/** A lowercase hex string. */
const HexString = S.String.check(S.isPattern(/^[0-9a-f]+$/));

/**
 * The PUBLIC identity one decoded candidate derives: the Nostr `npub` (via the
 * IDR-06 `IdentityKeys` path) and the per-adapter Spark fingerprints. It records
 * the public source label and which adapters were deferred (registered but not
 * compared), for transparency.
 */
export const CandidatePublicIdentity = S.Struct({
  /** A PUBLIC source label, never a raw private path. */
  sourceLabel: S.String.check(S.isMinLength(1)),
  npub: Npub,
  nostrPublicKeyHex: HexString,
  sparkFingerprints: S.Array(SparkAdapterFingerprint),
  /** Adapters that were registered but deferred (LDK), so not compared. */
  deferredAdapters: S.Array(SparkAdapterKind),
});
export interface CandidatePublicIdentity extends S.Schema.Type<typeof CandidatePublicIdentity> {}

/**
 * A distinct identity after duplicate grouping. Several candidate LOCATIONS that
 * derive the same `npub` collapse into one `GroupedIdentity` carrying every
 * source label. This is how duplicates are grouped without keeping the secret.
 */
export const GroupedIdentity = S.Struct({
  npub: Npub,
  nostrPublicKeyHex: HexString,
  sparkFingerprints: S.Array(SparkAdapterFingerprint),
  /** Every candidate location that produced this identity, sorted. */
  sourceLabels: S.Array(S.String.check(S.isMinLength(1))),
});
export interface GroupedIdentity extends S.Schema.Type<typeof GroupedIdentity> {}

/**
 * The PUBLIC expected identity to match a candidate against. It is assembled from
 * the public local records the audit lists (the `npub` from `identity.json`, the
 * Spark fingerprints from `wallet_context.json`). Either field may be absent when
 * that public comparison source is unavailable.
 */
export const ExpectedPublicIdentity = S.Struct({
  /** A public label for the record set, for the result. */
  label: S.String.check(S.isMinLength(1)),
  /** The expected Nostr `npub`, when a Nostr record exists. */
  npub: S.optionalKey(Npub),
  /** The expected Spark fingerprints per adapter, when Spark records exist. */
  sparkFingerprints: S.Array(SparkAdapterFingerprint),
});
export interface ExpectedPublicIdentity extends S.Schema.Type<typeof ExpectedPublicIdentity> {}

// ---------------------------------------------------------------------------
// Result variants (public-safe, boundary-crossing tagged structs)
// ---------------------------------------------------------------------------

/** A clean match: one grouped identity that matched the public records. */
export const ReconciliationConfirmed = S.TaggedStruct("confirmed", {
  identity: GroupedIdentity,
});
export interface ReconciliationConfirmed
  extends S.Schema.Type<typeof ReconciliationConfirmed> {}

/**
 * The Nostr identifier matched but the Spark profile did not. The tool reports a
 * Spark-profile mismatch and stops; it never imports.
 */
export const NostrMatchesSparkMismatch = S.TaggedStruct("nostr_matches_spark_mismatch", {
  identity: GroupedIdentity,
  expected: ExpectedPublicIdentity,
});
export interface NostrMatchesSparkMismatch
  extends S.Schema.Type<typeof NostrMatchesSparkMismatch> {}

/**
 * The Spark profile matched but the Nostr identifier did not. The tool reports a
 * Nostr mismatch and stops; it never imports.
 */
export const SparkMatchesNostrMismatch = S.TaggedStruct("spark_matches_nostr_mismatch", {
  identity: GroupedIdentity,
  expected: ExpectedPublicIdentity,
});
export interface SparkMatchesNostrMismatch
  extends S.Schema.Type<typeof SparkMatchesNostrMismatch> {}

/**
 * Two or more DIFFERENT valid phrases derived valid-but-different identities. The
 * tool NEVER auto-picks; it stops and lists the distinct identities for the owner
 * to select.
 */
export const OwnerSelectionRequired = S.TaggedStruct("owner_selection_required", {
  candidates: S.Array(GroupedIdentity),
});
export interface OwnerSelectionRequired
  extends S.Schema.Type<typeof OwnerSelectionRequired> {}

/**
 * No public comparison source identified the intended root, or the single
 * candidate matched neither the Nostr nor the Spark record. The tool stops.
 */
export const NoPublicMatch = S.TaggedStruct("no_public_match", {
  candidates: S.Array(GroupedIdentity),
});
export interface NoPublicMatch extends S.Schema.Type<typeof NoPublicMatch> {}

/**
 * A typed reconciliation conflict: any non-confirmed outcome. It carries only the
 * class tag plus public identifiers. A caller (IDR-05) decides what to do; the
 * conflict itself never imports or writes.
 */
export const ReconciliationConflict = S.Union([
  NostrMatchesSparkMismatch,
  SparkMatchesNostrMismatch,
  OwnerSelectionRequired,
  NoPublicMatch,
]);
export type ReconciliationConflict = typeof ReconciliationConflict.Type;

/** The full reconciliation result: a confirmed identity or a typed conflict. */
export const ReconciliationResult = S.Union([
  ReconciliationConfirmed,
  NostrMatchesSparkMismatch,
  SparkMatchesNostrMismatch,
  OwnerSelectionRequired,
  NoPublicMatch,
]);
export type ReconciliationResult = typeof ReconciliationResult.Type;

/** The conflict tags, for exhaustive handling. */
export const RECONCILIATION_CONFLICT_TAGS = [
  "nostr_matches_spark_mismatch",
  "spark_matches_nostr_mismatch",
  "owner_selection_required",
  "no_public_match",
] as const;

/** Whether a reconciliation result is a conflict (any non-confirmed outcome). */
export const isReconciliationConflict = (
  result: ReconciliationResult,
): result is ReconciliationConflict => result._tag !== "confirmed";
