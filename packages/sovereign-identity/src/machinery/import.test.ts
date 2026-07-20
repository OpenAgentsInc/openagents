import { mkdtemp, readFile, readdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  LocalSecretStore,
  inMemoryLocalSecretStoreLayerWith,
} from "@openagentsinc/local-secret-store";
import { Effect, Layer, Result } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { deriveSovereignIdentityPublic } from "../contract/derivation.ts";
import { IdentityRef, IsoTimestamp, Npub } from "../contract/index.ts";
import { PUBLIC_TEST_MNEMONIC } from "../contract/vectors.ts";
import { RecoveredSecret } from "../decode/boundary.ts";
import type { DecodedCandidate } from "../decode/result.ts";
import { reconcileIdentities } from "../reconcile/reconcile.ts";
import type { ExpectedPublicIdentity, ReconciliationConfirmed } from "../reconcile/result.ts";
import { rustSparkComparisonAdapter } from "../reconcile/spark-adapter.ts";
import { fileManifestStoreLayer, manifestPath } from "./file-manifest-store.ts";
import {
  type ImportConfirmedIdentityInput,
  ImportError,
  importConfirmedIdentity,
  restoreRootSecretPublicIdentity,
} from "./import.ts";
import { deriveLocalNostrIdentity } from "./local-signer.ts";
import { ManifestStore, inMemoryManifestStoreLayer } from "./manifest.ts";
import { rootSecretLocator } from "./service.ts";

/**
 * IDR-05 import-to-custody tests.
 *
 * SAFETY: every secret is the PUBLIC published BIP-39 TEST mnemonic — never a
 * real secret. The restart+restore round-trip uses the IN-MEMORY adapter and
 * asserts `in_memory_unprotected` custody, so NO OS keychain call runs. The
 * manifest and receipt are asserted to carry NO mnemonic word.
 */

const IDENTITY_REF = IdentityRef.make("idr05-restart-restore");
const CREATED_AT = IsoTimestamp.make("2026-07-20T00:00:00Z");
const MIGRATED_AT = IsoTimestamp.make("2026-07-20T00:00:01Z");

const reference = deriveSovereignIdentityPublic(PUBLIC_TEST_MNEMONIC);
const referenceNpub = deriveLocalNostrIdentity(PUBLIC_TEST_MNEMONIC).npub;

const candidateFrom = (mnemonic: string, sourceLabel: string): DecodedCandidate => ({
  result: {
    format: "plain_mnemonic_file",
    formatLabel: "Plain BIP-39 mnemonic file",
    formatVersion: "1",
    sourcePathLabel: sourceLabel,
    status: "decoded",
    decoded: true,
    publicIdentity: null,
  },
  secret: new RecoveredSecret(mnemonic, "plain_mnemonic_file", "1"),
});

const expected: ExpectedPublicIdentity = {
  label: "public local records",
  npub: Npub.make(referenceNpub),
  sparkFingerprints: [
    {
      adapter: "rust_spark",
      fingerprintHex: reference.sparkBip32FingerprintHex,
      publicKeyHex: reference.sparkPublicKeyHex,
    },
  ],
};

/** Produce a genuine IDR-04 CONFIRMED result for the public test mnemonic. */
const confirmFixture = Effect.gen(function* () {
  const result = yield* reconcileIdentities({
    candidates: [candidateFrom(PUBLIC_TEST_MNEMONIC, "fixture: public test mnemonic")],
    expected,
    adapters: [rustSparkComparisonAdapter],
  });
  if (result._tag !== "confirmed") throw new Error(`expected confirmed, got ${result._tag}`);
  return result;
});

const importInput = (confirmed: ReconciliationConfirmed): ImportConfirmedIdentityInput => ({
  identityRef: IDENTITY_REF,
  confirmed,
  secret: new RecoveredSecret(PUBLIC_TEST_MNEMONIC, "plain_mnemonic_file", "1"),
  secretStoreLocatorType: "in_memory_test",
  sourceFormatVersions: ["plain_mnemonic_file@1"],
  receiptRef: "receipt:idr05-restart-restore",
  createdAt: CREATED_AT,
  migratedAt: MIGRATED_AT,
  backupState: "restore_verified",
});

describe("IDR-05 import to platform custody — CI-safe in-memory adapter", () => {
  test("restart + restore round-trips the SAME public identity with no OS keychain call", async () => {
    const backing = new Map<string, Uint8Array>();

    const outcome = await Effect.runPromise(
      Effect.provide(
        Effect.gen(function* () {
          const confirmed = yield* confirmFixture;
          return yield* importConfirmedIdentity(importInput(confirmed));
        }),
        Layer.mergeAll(inMemoryLocalSecretStoreLayerWith(backing), inMemoryManifestStoreLayer),
      ),
    );

    expect(outcome.npub).toBe(referenceNpub);
    expect(outcome.custody.present).toBe(true);
    expect(outcome.custody.platformKind).toBe("in_memory_test");
    // Proves no OS keychain: real Keychain custody would be platform_protected.
    expect(outcome.custody.protection).toBe("in_memory_unprotected");
    expect(outcome.manifest.secretStoreLocatorType).toBe("in_memory_test");
    expect(outcome.manifest.receiptRefs).toContain("receipt:idr05-restart-restore");

    // "Restart": a NEW layer over the SAME backing map = the persisted store.
    const restored = await Effect.runPromise(
      Effect.provide(
        restoreRootSecretPublicIdentity(IDENTITY_REF),
        inMemoryLocalSecretStoreLayerWith(backing),
      ),
    );

    expect(restored.npub).toBe(referenceNpub);
    expect(restored.nostrPublicKeyHex).toBe(reference.nostrPublicKeyHex);
    expect(restored.sparkPublicKeyHex).toBe(reference.sparkPublicKeyHex);
    expect(restored.sparkBip32FingerprintHex).toBe(reference.sparkBip32FingerprintHex);
  });

  test("the manifest and migration receipt carry NO mnemonic word (public-safe)", async () => {
    const backing = new Map<string, Uint8Array>();
    const { manifest, receipt } = await Effect.runPromise(
      Effect.provide(
        Effect.gen(function* () {
          const confirmed = yield* confirmFixture;
          const out = yield* importConfirmedIdentity(importInput(confirmed));
          const manifests = yield* ManifestStore;
          const stored = yield* manifests.readReceipt("receipt:idr05-restart-restore");
          return { manifest: out.manifest, receipt: stored } as const;
        }),
        Layer.mergeAll(inMemoryLocalSecretStoreLayerWith(backing), inMemoryManifestStoreLayer),
      ),
    );
    for (const blob of [JSON.stringify(manifest), JSON.stringify(receipt)]) {
      expect(blob).not.toContain("abandon");
      expect(blob).not.toContain(PUBLIC_TEST_MNEMONIC);
    }
    expect(receipt?.outcome).toBe("imported");
    expect(receipt?.sourceLabels).toContain("fixture: public test mnemonic");
  });

  test("a wrong confirmed identity fails verification_failed and rolls back custody", async () => {
    const backing = new Map<string, Uint8Array>();
    const wrongNpub = Npub.make(
      "npub1000000000000000000000000000000000000000000000000000000000000",
    );
    const result = await Effect.runPromise(
      Effect.provide(
        Effect.gen(function* () {
          const confirmed = yield* confirmFixture;
          const tampered: ReconciliationConfirmed = {
            ...confirmed,
            identity: { ...confirmed.identity, npub: wrongNpub },
          };
          const outcome = yield* Effect.result(
            importConfirmedIdentity({ ...importInput(confirmed), confirmed: tampered }),
          );
          const store = yield* LocalSecretStore;
          const present = yield* store.presence(rootSecretLocator(IDENTITY_REF));
          return { outcome, present } as const;
        }),
        Layer.mergeAll(inMemoryLocalSecretStoreLayerWith(backing), inMemoryManifestStoreLayer),
      ),
    );
    expect(Result.isFailure(result.outcome)).toBe(true);
    if (Result.isFailure(result.outcome)) {
      expect(result.outcome.failure).toBeInstanceOf(ImportError);
      expect((result.outcome.failure as ImportError).reason).toBe("verification_failed");
    }
    // Fail-closed rollback: the wrong secret was deleted, custody is empty.
    expect(result.present).toBe(false);
  });
});

describe("IDR-05 atomic file-backed manifest store", () => {
  test("import through the file store writes the manifest atomically with no temp leftovers", async () => {
    const root = await mkdtemp(path.join(tmpdir(), "idr05-manifest-"));
    const backing = new Map<string, Uint8Array>();
    await Effect.runPromise(
      Effect.provide(
        Effect.gen(function* () {
          const confirmed = yield* confirmFixture;
          return yield* importConfirmedIdentity(importInput(confirmed));
        }),
        Layer.mergeAll(inMemoryLocalSecretStoreLayerWith(backing), fileManifestStoreLayer(root)),
      ),
    );

    const file = manifestPath(root, IDENTITY_REF);
    const raw = await readFile(file, "utf8");
    const onDisk = JSON.parse(raw) as Record<string, unknown>;
    expect(onDisk.schema).toBe("openagents.local_identity_manifest.v1");
    expect(onDisk.npub).toBe(referenceNpub);
    // The public manifest carries no secret.
    expect(raw).not.toContain("abandon");
    // The atomic temp file was renamed away: no `.tmp` residue.
    const entries = await readdir(path.dirname(file));
    expect(entries.filter((name) => name.endsWith(".tmp"))).toEqual([]);
    expect(entries).toContain("manifest.json");
  });
});
