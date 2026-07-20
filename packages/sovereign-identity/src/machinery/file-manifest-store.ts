/**
 * IDR-05 file-backed `ManifestStore` with ATOMIC manifest writes.
 *
 * The public identity manifest lives at
 * `<OpenAgents-local-data>/identities/<identityRef>/manifest.json`, and the
 * public-safe migration receipts live beside it. This layer writes each record
 * ATOMICALLY: it encodes through the frozen schema, writes a temporary file with
 * owner-only permissions, then renames it over the target, so a reader never sees
 * a half-written manifest and a crash mid-write cannot corrupt the existing one.
 *
 * The store keeps PUBLIC data only. The frozen manifest and receipt schemas
 * reject a secret-shaped field, so this file store can never persist the
 * mnemonic, `nsec`, raw key, or seed. It uses `node:fs` for public data only;
 * the platform secret store, not this file, holds the secret.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { randomBytes } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import { Effect, Layer, Schema as S } from "effect";
import {
  type IdentityRef,
  LocalIdentityManifest,
  LocalIdentityMigrationReceipt,
  LocalIdentityPlaintextRetirementReceipt,
} from "../contract/index.ts";
import { ManifestStore, ManifestStoreError } from "./manifest.ts";

const decodeManifest = S.decodeUnknownEffect(LocalIdentityManifest);
const encodeManifest = S.encodeEffect(LocalIdentityManifest);
const decodeReceipt = S.decodeUnknownEffect(LocalIdentityMigrationReceipt);
const encodeReceipt = S.encodeEffect(LocalIdentityMigrationReceipt);
const decodeRetirementReceipt = S.decodeUnknownEffect(LocalIdentityPlaintextRetirementReceipt);
const encodeRetirementReceipt = S.encodeEffect(LocalIdentityPlaintextRetirementReceipt);

/** The manifest path for one identity, under the local data root. */
export const manifestPath = (rootDir: string, identityRef: string): string =>
  path.join(rootDir, "identities", encodeURIComponent(identityRef), "manifest.json");

/** The receipt path for one receipt reference, under the local data root. */
export const receiptPath = (rootDir: string, receiptRef: string): string =>
  path.join(rootDir, "identities", "_receipts", `${encodeURIComponent(receiptRef)}.json`);

/** The plaintext-retirement receipt path for one receipt reference (IDR-09). */
export const retirementReceiptPath = (rootDir: string, receiptRef: string): string =>
  path.join(
    rootDir,
    "identities",
    "_retirement_receipts",
    `${encodeURIComponent(receiptRef)}.json`,
  );

const isNotFound = (error: unknown): boolean =>
  typeof error === "object" && error !== null && (error as { code?: string }).code === "ENOENT";

/** Write JSON to a path atomically: temp file with owner-only mode, then rename. */
const atomicWriteJson = (filePath: string, json: string) =>
  Effect.tryPromise({
    try: async () => {
      const dir = path.dirname(filePath);
      await mkdir(dir, { recursive: true });
      const temp = path.join(
        dir,
        `.${path.basename(filePath)}.${randomBytes(6).toString("hex")}.tmp`,
      );
      await writeFile(temp, json, { encoding: "utf8", mode: 0o600 });
      await rename(temp, filePath);
    },
    catch: () => new ManifestStoreError({ reason: "write_failed" }),
  });

const readJson = (filePath: string) =>
  Effect.tryPromise({
    try: async (): Promise<unknown | null> => {
      try {
        return JSON.parse(await readFile(filePath, "utf8")) as unknown;
      } catch (error) {
        if (isNotFound(error)) return null;
        throw error;
      }
    },
    catch: () => new ManifestStoreError({ reason: "storage_unavailable" }),
  });

/**
 * The file-backed `ManifestStore` layer, rooted at a local data directory. It
 * writes manifests and receipts atomically and keeps public data only.
 */
export const fileManifestStoreLayer = (rootDir: string): Layer.Layer<ManifestStore> =>
  Layer.succeed(
    ManifestStore,
    ManifestStore.of({
      writeManifest: (manifest) =>
        Effect.gen(function* () {
          const encoded = yield* encodeManifest(manifest).pipe(
            Effect.mapError(() => new ManifestStoreError({ reason: "invalid_manifest" })),
          );
          yield* atomicWriteJson(
            manifestPath(rootDir, manifest.identityRef),
            JSON.stringify(encoded, null, 2),
          );
        }),
      readManifest: (identityRef: IdentityRef) =>
        Effect.gen(function* () {
          const stored = yield* readJson(manifestPath(rootDir, identityRef));
          if (stored === null) return null;
          return yield* decodeManifest(stored).pipe(
            Effect.mapError(() => new ManifestStoreError({ reason: "invalid_manifest" })),
          );
        }),
      writeReceipt: (receipt) =>
        Effect.gen(function* () {
          const encoded = yield* encodeReceipt(receipt).pipe(
            Effect.mapError(() => new ManifestStoreError({ reason: "invalid_receipt" })),
          );
          yield* atomicWriteJson(
            receiptPath(rootDir, receipt.receiptRef),
            JSON.stringify(encoded, null, 2),
          );
        }),
      readReceipt: (receiptRef: string) =>
        Effect.gen(function* () {
          const stored = yield* readJson(receiptPath(rootDir, receiptRef));
          if (stored === null) return null;
          return yield* decodeReceipt(stored).pipe(
            Effect.mapError(() => new ManifestStoreError({ reason: "invalid_receipt" })),
          );
        }),
      writeRetirementReceipt: (receipt) =>
        Effect.gen(function* () {
          const encoded = yield* encodeRetirementReceipt(receipt).pipe(
            Effect.mapError(
              () => new ManifestStoreError({ reason: "invalid_retirement_receipt" }),
            ),
          );
          yield* atomicWriteJson(
            retirementReceiptPath(rootDir, receipt.receiptRef),
            JSON.stringify(encoded, null, 2),
          );
        }),
      readRetirementReceipt: (receiptRef: string) =>
        Effect.gen(function* () {
          const stored = yield* readJson(retirementReceiptPath(rootDir, receiptRef));
          if (stored === null) return null;
          return yield* decodeRetirementReceipt(stored).pipe(
            Effect.mapError(
              () => new ManifestStoreError({ reason: "invalid_retirement_receipt" }),
            ),
          );
        }),
    }),
  );
