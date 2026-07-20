/**
 * IDR-05 import to platform custody.
 *
 * `importConfirmedIdentity` takes IDR-04's CONFIRMED reconciliation result plus
 * the bounded `RecoveredSecret` it confirmed, moves the canonical secret into the
 * injected platform secret store, verifies the imported custody re-derives the
 * SAME public identity, writes the public manifest (atomically, through the
 * injected `ManifestStore`), and records a public-safe migration receipt. It
 * returns a public-safe outcome. It NEVER returns, logs, or persists the
 * mnemonic; the phrase lives only inside the bounded `use` scope, and the encoded
 * payload buffer is zeroed after the store write.
 *
 * `restoreRootSecretPublicIdentity` reads the imported custody back and derives
 * the PUBLIC identity, so a restart+restore proof can show the same npub and
 * Spark fingerprint after a simulated restart. It returns public data only.
 *
 * FAIL-CLOSED. If the post-import re-derivation does not match the confirmed
 * identity, the import deletes the just-written secret and fails
 * `verification_failed`; it never leaves a wrong root in custody.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { LocalSecretStore, type SecretCustodyState } from "@openagentsinc/local-secret-store";
import { Effect, Schema as S } from "effect";
import {
  type IdentityRef,
  type IsoTimestamp,
  LOCAL_IDENTITY_SECRET_SCHEMA,
  type LocalIdentityManifest,
  type SecretStoreLocatorType,
  decodeLocalIdentitySecret,
  deriveSovereignIdentityPublic,
} from "../contract/index.ts";
import type { RecoveredSecret } from "../decode/boundary.ts";
import type { ReconciliationConfirmed } from "../reconcile/result.ts";
import { deriveLocalNostrIdentity } from "./local-signer.ts";
import { ManifestStore, buildManifest, buildMigrationReceipt } from "./manifest.ts";
import { rootSecretLocator } from "./service.ts";

/** A typed import failure. It never carries secret material. */
export class ImportError extends S.TaggedErrorClass<ImportError>()(
  "sovereign-identity.ImportError",
  {
    reason: S.Literals([
      // The re-derived custody did not match the confirmed public identity.
      "verification_failed",
      // The secret store reported no entry after the write.
      "custody_absent",
      // A restored payload could not be decoded to a valid secret.
      "restore_decode_failed",
    ]),
  },
) {}

/** Encode the canonical secret payload for the platform store. Opaque bytes out. */
const encodeSecretPayload = (mnemonic: string): Uint8Array =>
  new TextEncoder().encode(
    JSON.stringify({
      schema: LOCAL_IDENTITY_SECRET_SCHEMA,
      mnemonic,
      language: "english",
      bip39PassphraseMode: "empty",
    }),
  );

/** Decode a stored payload back to the validated secret shape. */
const decodeSecretPayload = (bytes: Uint8Array) =>
  decodeLocalIdentitySecret(JSON.parse(new TextDecoder().decode(bytes)) as unknown);

/** The public inputs an import needs. Every field is public; the secret is bounded. */
export interface ImportConfirmedIdentityInput {
  /** The stable identity reference the custody entry and manifest key on. */
  readonly identityRef: IdentityRef;
  /** IDR-04's CONFIRMED reconciliation result. Only a confirmed identity imports. */
  readonly confirmed: ReconciliationConfirmed;
  /** The bounded recovered secret the confirmed identity was derived from. */
  readonly secret: RecoveredSecret;
  /** The platform store the injected `LocalSecretStore` writes to. */
  readonly secretStoreLocatorType: SecretStoreLocatorType;
  /** The decoded historical format versions the recovery touched. */
  readonly sourceFormatVersions: ReadonlyArray<string>;
  /** The public reference for the migration receipt this import records. */
  readonly receiptRef: string;
  /** When the identity was first created (public). */
  readonly createdAt: IsoTimestamp;
  /** When this import ran (public). */
  readonly migratedAt: IsoTimestamp;
  /** The backup state to record on the manifest. */
  readonly backupState?: "none" | "portable_backup_written" | "restore_verified";
}

/** The public-safe outcome of an import. It carries no secret material. */
export interface ImportOutcome {
  readonly identityRef: IdentityRef;
  readonly npub: string;
  readonly nostrPublicKeyHex: string;
  readonly manifest: LocalIdentityManifest;
  readonly custody: SecretCustodyState;
}

/**
 * Import the confirmed identity into platform custody, verify it, and write the
 * public manifest and migration receipt. It requires the injected
 * `LocalSecretStore` and `ManifestStore`.
 */
export const importConfirmedIdentity = Effect.fn("SovereignIdentity.importConfirmedIdentity")(
  function* (input: ImportConfirmedIdentityInput) {
    const secrets = yield* LocalSecretStore;
    const manifests = yield* ManifestStore;
    const locator = rootSecretLocator(input.identityRef);
    const expected = input.confirmed.identity;

    // 1. Move the secret into custody and re-derive the public identity, all
    //    inside the bounded scope. The payload buffer is zeroed after the write.
    const derived = yield* input.secret.use((mnemonic) =>
      Effect.gen(function* () {
        const payload = encodeSecretPayload(mnemonic);
        yield* secrets
          .set(locator, payload)
          .pipe(Effect.ensuring(Effect.sync(() => payload.fill(0))));
        return yield* Effect.try({
          try: () => {
            const identity = deriveLocalNostrIdentity(mnemonic);
            return { npub: identity.npub, nostrPublicKeyHex: identity.publicKey };
          },
          catch: () => new ImportError({ reason: "verification_failed" }),
        });
      }),
    );

    // 2. Fail closed: a mismatch deletes the just-written secret and stops.
    if (
      derived.npub !== expected.npub ||
      derived.nostrPublicKeyHex !== expected.nostrPublicKeyHex
    ) {
      yield* secrets.delete(locator);
      return yield* new ImportError({ reason: "verification_failed" });
    }

    // 3. Prove the entry is present, then read its custody state.
    const present = yield* secrets.presence(locator);
    if (!present) return yield* new ImportError({ reason: "custody_absent" });
    const custody = yield* secrets.custody(locator);

    // 4. Write the public manifest atomically through the injected store.
    const manifest = yield* buildManifest({
      identityRef: input.identityRef,
      npub: expected.npub,
      nostrPublicKeyHex: expected.nostrPublicKeyHex,
      sparkFingerprints: expected.sparkFingerprints,
      secretStoreLocatorType: input.secretStoreLocatorType,
      receiptRefs: [input.receiptRef],
      backupState: input.backupState ?? "none",
      createdAt: input.createdAt,
      migratedAt: input.migratedAt,
    });
    yield* manifests.writeManifest(manifest);

    // 5. Record the public-safe migration receipt.
    const receipt = yield* buildMigrationReceipt({
      receiptRef: input.receiptRef,
      identityRef: input.identityRef,
      npub: expected.npub,
      nostrPublicKeyHex: expected.nostrPublicKeyHex,
      sparkFingerprints: expected.sparkFingerprints,
      sourceLabels: expected.sourceLabels,
      sourceFormatVersions: input.sourceFormatVersions,
      outcome: "imported",
      createdAt: input.migratedAt,
    });
    yield* manifests.writeReceipt(receipt);

    return {
      identityRef: input.identityRef,
      npub: expected.npub,
      nostrPublicKeyHex: expected.nostrPublicKeyHex,
      manifest,
      custody,
    } satisfies ImportOutcome;
  },
);

/** The public identity a restore derives from the imported custody. */
export interface RestoredPublicIdentity {
  readonly npub: string;
  readonly nostrPublicKeyHex: string;
  readonly sparkPublicKeyHex: string;
  readonly sparkBip32FingerprintHex: string;
}

/**
 * Read the imported root secret back from the injected platform store and derive
 * its PUBLIC identity. It returns public data only. The decoded payload is used
 * inside one bounded step and never returned.
 */
export const restoreRootSecretPublicIdentity = Effect.fn(
  "SovereignIdentity.restoreRootSecretPublicIdentity",
)(function* (identityRef: IdentityRef) {
  const secrets = yield* LocalSecretStore;
  const bytes = yield* secrets.get(rootSecretLocator(identityRef));
  return yield* Effect.try({
    try: (): RestoredPublicIdentity => {
      const secret = decodeSecretPayload(bytes);
      const nostr = deriveLocalNostrIdentity(secret.mnemonic);
      const full = deriveSovereignIdentityPublic(secret.mnemonic);
      return {
        npub: nostr.npub,
        nostrPublicKeyHex: nostr.publicKey,
        sparkPublicKeyHex: full.sparkPublicKeyHex,
        sparkBip32FingerprintHex: full.sparkBip32FingerprintHex,
      };
    },
    catch: () => new ImportError({ reason: "restore_decode_failed" }),
  });
});
