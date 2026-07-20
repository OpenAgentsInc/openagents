/**
 * IDR-00 frozen Effect Schema contract for the OpenAgents sovereign identity.
 *
 * This module freezes three schemas:
 *
 * 1. `openagents.local_identity_secret.v1` — the PRIVATE secret payload shape
 *    that a platform secret store holds. It is defined here as a type contract
 *    only. The frozen contract never serializes it to Git, config, logs, or the
 *    cloud.
 * 2. `openagents.local_identity_manifest.v1` — the PUBLIC identity manifest. It
 *    contains public identifiers only and never a secret field.
 * 3. `openagents.local_identity_migration_receipt.v1` — the PUBLIC-safe
 *    migration receipt. It records public identifiers, source labels, and
 *    format versions only.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Schema as S } from "effect";
import { DERIVATION_PROFILE_ID } from "./derivation.ts";
import { SECRET_STORE_LOCATOR_TYPES } from "./secret-store.ts";

/** Stable schema identifiers. */
export const LOCAL_IDENTITY_SECRET_SCHEMA = "openagents.local_identity_secret.v1";
export const LOCAL_IDENTITY_MANIFEST_SCHEMA = "openagents.local_identity_manifest.v1";
export const LOCAL_IDENTITY_MIGRATION_RECEIPT_SCHEMA =
  "openagents.local_identity_migration_receipt.v1";

/** A stable identity reference. */
export const IdentityRef = S.String.check(S.isMinLength(1)).pipe(S.brand("SovereignIdentityRef"));
export type IdentityRef = typeof IdentityRef.Type;

/** A lowercase hex string. */
const HexString = S.String.check(S.isPattern(/^[0-9a-f]+$/));

/** A Nostr NIP-19 `npub`. */
export const Npub = S.String.check(S.isPattern(/^npub1[0-9a-z]+$/)).pipe(S.brand("Npub"));
export type Npub = typeof Npub.Type;

/** An RFC 3339 UTC timestamp. */
export const IsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/),
).pipe(S.brand("SovereignIsoTimestamp"));
export type IsoTimestamp = typeof IsoTimestamp.Type;

/** The admitted Spark comparison adapters. */
export const SparkAdapterKind = S.Literals(["rust_spark", "breez_spark", "ldk"]);
export type SparkAdapterKind = typeof SparkAdapterKind.Type;

/** A per-adapter Spark public fingerprint. It never carries private material. */
export const SparkAdapterFingerprint = S.Struct({
  adapter: SparkAdapterKind,
  /** The public fingerprint the adapter reports for this identity. */
  fingerprintHex: HexString,
  /** The optional compressed public key, when the adapter exposes one. */
  publicKeyHex: S.optionalKey(HexString),
});
export type SparkAdapterFingerprint = typeof SparkAdapterFingerprint.Type;

// ---------------------------------------------------------------------------
// 1. Private secret payload
// ---------------------------------------------------------------------------

/**
 * The private secret payload. A platform secret store supplies encryption and
 * access control. This type is a contract only. Do not write an instance to any
 * durable non-secret sink.
 */
export const LocalIdentitySecret = S.Struct({
  schema: S.Literal(LOCAL_IDENTITY_SECRET_SCHEMA),
  mnemonic: S.String.check(S.isMinLength(1)),
  language: S.Literal("english"),
  bip39PassphraseMode: S.Literal("empty"),
});
export type LocalIdentitySecret = typeof LocalIdentitySecret.Type;

// ---------------------------------------------------------------------------
// 2. Public manifest
// ---------------------------------------------------------------------------

/**
 * The public identity manifest. It holds public identifiers only. It never
 * holds the mnemonic, `nsec`, raw private key, seed, or wallet entropy, and it
 * never holds the secret-store account value when a locator type is enough.
 */
export const LocalIdentityManifest = S.Struct({
  schema: S.Literal(LOCAL_IDENTITY_MANIFEST_SCHEMA),
  identityRef: IdentityRef,
  npub: Npub,
  nostrPublicKeyHex: HexString,
  derivationProfile: S.Literal(DERIVATION_PROFILE_ID),
  sparkFingerprints: S.Array(SparkAdapterFingerprint),
  secretStoreLocatorType: S.Literals(SECRET_STORE_LOCATOR_TYPES),
  receiptRefs: S.Array(S.String),
  backupState: S.Literals(["none", "portable_backup_written", "restore_verified"]),
  createdAt: IsoTimestamp,
  migratedAt: S.optionalKey(IsoTimestamp),
});
export type LocalIdentityManifest = typeof LocalIdentityManifest.Type;

// ---------------------------------------------------------------------------
// 3. Public migration receipt
// ---------------------------------------------------------------------------

/**
 * The public-safe migration receipt. It records public identifiers, source
 * labels, and format versions. It never records a phrase, `nsec`, raw private
 * key, seed, decrypted backup data, or a private path when a public label is
 * enough.
 */
export const LocalIdentityMigrationReceipt = S.Struct({
  schema: S.Literal(LOCAL_IDENTITY_MIGRATION_RECEIPT_SCHEMA),
  receiptRef: S.String.check(S.isMinLength(1)),
  identityRef: IdentityRef,
  npub: Npub,
  nostrPublicKeyHex: HexString,
  derivationProfile: S.Literal(DERIVATION_PROFILE_ID),
  sparkFingerprints: S.Array(SparkAdapterFingerprint),
  /** Public source labels only, never private paths. */
  sourceLabels: S.Array(S.String),
  /** The decoded historical format versions the run touched. */
  sourceFormatVersions: S.Array(S.String),
  outcome: S.Literals(["imported", "no_match", "owner_selection_required", "blocked"]),
  createdAt: IsoTimestamp,
});
export type LocalIdentityMigrationReceipt = typeof LocalIdentityMigrationReceipt.Type;

// ---------------------------------------------------------------------------
// Decoders
// ---------------------------------------------------------------------------

export const decodeLocalIdentitySecret = S.decodeUnknownSync(LocalIdentitySecret);
export const decodeLocalIdentityManifest = S.decodeUnknownSync(LocalIdentityManifest);
export const decodeLocalIdentityMigrationReceipt = S.decodeUnknownSync(
  LocalIdentityMigrationReceipt,
);
