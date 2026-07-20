/**
 * IDR-01 public manifest writer contract and migration receipt production.
 *
 * The public manifest and the migration receipt hold PUBLIC data only. They
 * never hold the mnemonic, the `nsec`, a raw private key, a seed, or wallet
 * entropy. This module builds them from the frozen IDR-00 schemas and defines
 * the `ManifestStore` port a host implements with a real public-data file store
 * (IDR-05). It ships one real adapter: an in-memory store for tests.
 *
 * The build functions stamp the frozen schema literal and the frozen derivation
 * profile, then validate through the frozen decoders. A caller can never write a
 * manifest with a wrong schema, a wrong profile, or a secret-shaped field,
 * because the frozen schema rejects it.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Context, Effect, Layer, Ref, Schema as S } from "effect";
import {
  DERIVATION_PROFILE_ID,
  type IdentityRef,
  LOCAL_IDENTITY_MANIFEST_SCHEMA,
  LOCAL_IDENTITY_MIGRATION_RECEIPT_SCHEMA,
  LOCAL_IDENTITY_PLAINTEXT_RETIREMENT_RECEIPT_SCHEMA,
  LocalIdentityManifest,
  LocalIdentityMigrationReceipt,
  LocalIdentityPlaintextRetirementReceipt,
  type SecretStoreLocatorType,
  type SparkAdapterFingerprint,
} from "../contract/index.ts";

/** A typed manifest-store failure. It never carries secret material. */
export class ManifestStoreError extends S.TaggedErrorClass<ManifestStoreError>()(
  "sovereign-identity.ManifestStoreError",
  {
    reason: S.Literals([
      "storage_unavailable",
      "invalid_manifest",
      "invalid_receipt",
      "invalid_retirement_receipt",
      "write_failed",
    ]),
  },
) {}

/** The public inputs a manifest needs. The schema and profile are stamped for you. */
export interface BuildManifestInput {
  readonly identityRef: string;
  readonly npub: string;
  readonly nostrPublicKeyHex: string;
  readonly sparkFingerprints: ReadonlyArray<SparkAdapterFingerprint>;
  readonly secretStoreLocatorType: SecretStoreLocatorType;
  readonly receiptRefs: ReadonlyArray<string>;
  readonly backupState: "none" | "portable_backup_written" | "restore_verified";
  readonly createdAt: string;
  readonly migratedAt?: string;
}

/** The public inputs a migration receipt needs. */
export interface BuildMigrationReceiptInput {
  readonly receiptRef: string;
  readonly identityRef: string;
  readonly npub: string;
  readonly nostrPublicKeyHex: string;
  readonly sparkFingerprints: ReadonlyArray<SparkAdapterFingerprint>;
  readonly sourceLabels: ReadonlyArray<string>;
  readonly sourceFormatVersions: ReadonlyArray<string>;
  readonly outcome: "imported" | "no_match" | "owner_selection_required" | "blocked";
  readonly createdAt: string;
}

/** The public inputs a plaintext-retirement receipt needs (IDR-09). */
export interface BuildRetirementReceiptInput {
  readonly receiptRef: string;
  readonly identityRef: string;
  readonly npub: string;
  readonly nostrPublicKeyHex: string;
  /** Public labels of the retired legacy plaintext sources, never raw private paths. */
  readonly retiredSourceLabels: ReadonlyArray<string>;
  /** The public label of the verified remaining backup. */
  readonly verifiedBackupLabel: string;
  /** A PUBLIC reference to the owner confirmation, never the confirmation token. */
  readonly ownerConfirmationRef: string;
  readonly retiredAt: string;
}

const decodeManifest = S.decodeUnknownEffect(LocalIdentityManifest);
const decodeReceipt = S.decodeUnknownEffect(LocalIdentityMigrationReceipt);
const decodeRetirementReceipt = S.decodeUnknownEffect(LocalIdentityPlaintextRetirementReceipt);

/** Build a validated public manifest from public inputs. */
export const buildManifest = Effect.fn("SovereignIdentity.buildManifest")(function* (
  input: BuildManifestInput,
) {
  const base = {
    schema: LOCAL_IDENTITY_MANIFEST_SCHEMA,
    identityRef: input.identityRef,
    npub: input.npub,
    nostrPublicKeyHex: input.nostrPublicKeyHex,
    derivationProfile: DERIVATION_PROFILE_ID,
    sparkFingerprints: input.sparkFingerprints,
    secretStoreLocatorType: input.secretStoreLocatorType,
    receiptRefs: input.receiptRefs,
    backupState: input.backupState,
    createdAt: input.createdAt,
  };
  const candidate =
    input.migratedAt === undefined ? base : { ...base, migratedAt: input.migratedAt };
  return yield* decodeManifest(candidate).pipe(
    Effect.mapError(() => new ManifestStoreError({ reason: "invalid_manifest" })),
  );
});

/** Build a validated public-safe migration receipt from public inputs. */
export const buildMigrationReceipt = Effect.fn("SovereignIdentity.buildMigrationReceipt")(
  function* (input: BuildMigrationReceiptInput) {
    return yield* decodeReceipt({
      schema: LOCAL_IDENTITY_MIGRATION_RECEIPT_SCHEMA,
      receiptRef: input.receiptRef,
      identityRef: input.identityRef,
      npub: input.npub,
      nostrPublicKeyHex: input.nostrPublicKeyHex,
      derivationProfile: DERIVATION_PROFILE_ID,
      sparkFingerprints: input.sparkFingerprints,
      sourceLabels: input.sourceLabels,
      sourceFormatVersions: input.sourceFormatVersions,
      outcome: input.outcome,
      createdAt: input.createdAt,
    }).pipe(Effect.mapError(() => new ManifestStoreError({ reason: "invalid_receipt" })));
  },
);

/**
 * Build a validated public-safe plaintext-retirement receipt from public inputs
 * (IDR-09). The frozen schema stamps `custodyRestoreVerified: true`, so this
 * receipt exists only when the caller has already proven the custody restore.
 */
export const buildRetirementReceipt = Effect.fn("SovereignIdentity.buildRetirementReceipt")(
  function* (input: BuildRetirementReceiptInput) {
    return yield* decodeRetirementReceipt({
      schema: LOCAL_IDENTITY_PLAINTEXT_RETIREMENT_RECEIPT_SCHEMA,
      receiptRef: input.receiptRef,
      identityRef: input.identityRef,
      npub: input.npub,
      nostrPublicKeyHex: input.nostrPublicKeyHex,
      derivationProfile: DERIVATION_PROFILE_ID,
      retiredSourceLabels: input.retiredSourceLabels,
      verifiedBackupLabel: input.verifiedBackupLabel,
      custodyRestoreVerified: true,
      ownerConfirmationRef: input.ownerConfirmationRef,
      retiredAt: input.retiredAt,
    }).pipe(
      Effect.mapError(() => new ManifestStoreError({ reason: "invalid_retirement_receipt" })),
    );
  },
);

/**
 * The public manifest writer port. A host implements it with a real public-data
 * file store under `<OpenAgents-local-data>/identities/<identityRef>/manifest.json`
 * (IDR-05). The store keeps public data only.
 */
export interface ManifestStoreInterface {
  readonly writeManifest: (
    manifest: LocalIdentityManifest,
  ) => Effect.Effect<void, ManifestStoreError>;
  readonly readManifest: (
    identityRef: IdentityRef,
  ) => Effect.Effect<LocalIdentityManifest | null, ManifestStoreError>;
  readonly writeReceipt: (
    receipt: LocalIdentityMigrationReceipt,
  ) => Effect.Effect<void, ManifestStoreError>;
  readonly readReceipt: (
    receiptRef: string,
  ) => Effect.Effect<LocalIdentityMigrationReceipt | null, ManifestStoreError>;
  /** Write the public-safe plaintext-retirement receipt (IDR-09). */
  readonly writeRetirementReceipt: (
    receipt: LocalIdentityPlaintextRetirementReceipt,
  ) => Effect.Effect<void, ManifestStoreError>;
  /** Read a public-safe plaintext-retirement receipt by reference, or `null`. */
  readonly readRetirementReceipt: (
    receiptRef: string,
  ) => Effect.Effect<LocalIdentityPlaintextRetirementReceipt | null, ManifestStoreError>;
}

/** The `ManifestStore` service tag. IDR-05 supplies the real file-backed layer. */
export class ManifestStore extends Context.Service<ManifestStore, ManifestStoreInterface>()(
  "sovereign-identity.ManifestStore",
) {}

/**
 * The in-memory `ManifestStore` layer. It round-trips every record through the
 * frozen decoders, so a serialization defect fails a test the same way a real
 * public-data store would. It keeps public data only.
 */
export const inMemoryManifestStoreLayer = Layer.effect(
  ManifestStore,
  Effect.gen(function* () {
    const manifests = yield* Ref.make(new Map<string, unknown>());
    const receipts = yield* Ref.make(new Map<string, unknown>());
    const retirementReceipts = yield* Ref.make(new Map<string, unknown>());

    const writeManifest = Effect.fn("ManifestStore.writeManifest")(function* (
      manifest: LocalIdentityManifest,
    ) {
      const encoded = yield* S.encodeEffect(LocalIdentityManifest)(manifest).pipe(
        Effect.mapError(() => new ManifestStoreError({ reason: "invalid_manifest" })),
      );
      yield* Ref.update(manifests, (map) => new Map(map).set(manifest.identityRef, encoded));
    });

    const readManifest = Effect.fn("ManifestStore.readManifest")(function* (
      identityRef: IdentityRef,
    ) {
      const map = yield* Ref.get(manifests);
      const stored = map.get(identityRef);
      if (stored === undefined) return null;
      return yield* decodeManifest(stored).pipe(
        Effect.mapError(() => new ManifestStoreError({ reason: "invalid_manifest" })),
      );
    });

    const writeReceipt = Effect.fn("ManifestStore.writeReceipt")(function* (
      receipt: LocalIdentityMigrationReceipt,
    ) {
      const encoded = yield* S.encodeEffect(LocalIdentityMigrationReceipt)(receipt).pipe(
        Effect.mapError(() => new ManifestStoreError({ reason: "invalid_receipt" })),
      );
      yield* Ref.update(receipts, (map) => new Map(map).set(receipt.receiptRef, encoded));
    });

    const readReceipt = Effect.fn("ManifestStore.readReceipt")(function* (receiptRef: string) {
      const map = yield* Ref.get(receipts);
      const stored = map.get(receiptRef);
      if (stored === undefined) return null;
      return yield* decodeReceipt(stored).pipe(
        Effect.mapError(() => new ManifestStoreError({ reason: "invalid_receipt" })),
      );
    });

    const writeRetirementReceipt = Effect.fn("ManifestStore.writeRetirementReceipt")(function* (
      receipt: LocalIdentityPlaintextRetirementReceipt,
    ) {
      const encoded = yield* S.encodeEffect(LocalIdentityPlaintextRetirementReceipt)(receipt).pipe(
        Effect.mapError(() => new ManifestStoreError({ reason: "invalid_retirement_receipt" })),
      );
      yield* Ref.update(retirementReceipts, (map) =>
        new Map(map).set(receipt.receiptRef, encoded),
      );
    });

    const readRetirementReceipt = Effect.fn("ManifestStore.readRetirementReceipt")(function* (
      receiptRef: string,
    ) {
      const map = yield* Ref.get(retirementReceipts);
      const stored = map.get(receiptRef);
      if (stored === undefined) return null;
      return yield* decodeRetirementReceipt(stored).pipe(
        Effect.mapError(() => new ManifestStoreError({ reason: "invalid_retirement_receipt" })),
      );
    });

    return ManifestStore.of({
      writeManifest,
      readManifest,
      writeReceipt,
      readReceipt,
      writeRetirementReceipt,
      readRetirementReceipt,
    });
  }),
);
