import { Effect, Layer, Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { LocalSecretStore, inMemoryLocalSecretStoreLayer } from "@openagentsinc/local-secret-store";

import {
  IdentityRef,
  PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE,
  SECRET_STORE_SERVICE,
  secretStoreAccount,
} from "../contract/index.ts";
import {
  ManifestStoreError,
  buildManifest,
  buildMigrationReceipt,
  inMemoryManifestStoreLayer,
} from "./manifest.ts";
import { SovereignIdentity, rootSecretLocator, sovereignIdentityLayer } from "./service.ts";

const identityRef = S.decodeUnknownSync(IdentityRef)("test-ref");
const vector = PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE;
const createdAt = "2026-07-20T00:00:00Z";

const sparkFingerprints = [
  { adapter: "rust_spark" as const, fingerprintHex: vector.sparkBip32FingerprintHex },
];

/** Compose the service over shared in-memory ports, keeping the ports exposed. */
const testLayer = sovereignIdentityLayer.pipe(
  Layer.provideMerge(Layer.mergeAll(inMemoryLocalSecretStoreLayer, inMemoryManifestStoreLayer)),
);

const run = <A, E>(effect: Effect.Effect<A, E, SovereignIdentity | LocalSecretStore>): Promise<A> =>
  Effect.runPromise(Effect.provide(effect, testLayer));

describe("manifest and receipt production over the in-memory store", () => {
  test("buildManifest stamps the frozen schema and profile and round-trips", async () => {
    const readBack = await run(
      Effect.gen(function* () {
        const identity = yield* SovereignIdentity;
        const manifest = yield* buildManifest({
          identityRef,
          npub: vector.npub,
          nostrPublicKeyHex: vector.nostrPublicKeyHex,
          sparkFingerprints,
          secretStoreLocatorType: "in_memory_test",
          receiptRefs: [],
          backupState: "none",
          createdAt,
        });
        yield* identity.writeManifest(manifest);
        return yield* identity.manifestFor(identityRef);
      }),
    );
    expect(readBack?.schema).toBe("openagents.local_identity_manifest.v1");
    expect(readBack?.derivationProfile).toBe("openagents.legacy_unified_nostr_spark.v1");
    expect(readBack?.npub).toBe(vector.npub);
  });

  test("buildMigrationReceipt round-trips through the store", async () => {
    const readBack = await run(
      Effect.gen(function* () {
        const identity = yield* SovereignIdentity;
        const receipt = yield* buildMigrationReceipt({
          receiptRef: "receipt-1",
          identityRef,
          npub: vector.npub,
          nostrPublicKeyHex: vector.nostrPublicKeyHex,
          sparkFingerprints,
          sourceLabels: ["plain_mnemonic_file"],
          sourceFormatVersions: ["plain-text-v1"],
          outcome: "imported",
          createdAt,
        });
        yield* identity.recordMigrationReceipt(receipt);
        return yield* identity.receiptFor("receipt-1");
      }),
    );
    expect(readBack?.schema).toBe("openagents.local_identity_migration_receipt.v1");
    expect(readBack?.outcome).toBe("imported");
  });

  test("buildManifest rejects an invalid public identifier", async () => {
    const error = await Effect.runPromise(
      Effect.flip(
        buildManifest({
          identityRef,
          npub: "not-an-npub",
          nostrPublicKeyHex: vector.nostrPublicKeyHex,
          sparkFingerprints,
          secretStoreLocatorType: "in_memory_test",
          receiptRefs: [],
          backupState: "none",
          createdAt,
        }),
      ),
    );
    expect(error).toBeInstanceOf(ManifestStoreError);
    expect(error.reason).toBe("invalid_manifest");
  });

  test("manifestFor returns null before any manifest is written", async () => {
    const before = await run(
      Effect.gen(function* () {
        const identity = yield* SovereignIdentity;
        return yield* identity.manifestFor(identityRef);
      }),
    );
    expect(before).toBeNull();
  });
});

describe("sovereign-identity custody uses presence, never the secret bytes", () => {
  test("hasRootSecret and rootCustody follow the injected secret store", async () => {
    const result = await run(
      Effect.gen(function* () {
        const identity = yield* SovereignIdentity;
        const secrets = yield* LocalSecretStore;

        const before = yield* identity.hasRootSecret(identityRef);
        const custodyBefore = yield* identity.rootCustody(identityRef);

        // A higher layer writes the opaque root secret; the service never does.
        yield* secrets.set(rootSecretLocator(identityRef), new Uint8Array([9, 9, 9]));

        const after = yield* identity.hasRootSecret(identityRef);
        const custodyAfter = yield* identity.rootCustody(identityRef);
        return { before, custodyBefore, after, custodyAfter };
      }),
    );
    expect(result.before).toBe(false);
    expect(result.custodyBefore.present).toBe(false);
    expect(result.after).toBe(true);
    expect(result.custodyAfter.present).toBe(true);
    expect(result.custodyAfter.protection).toBe("in_memory_unprotected");
  });

  test("the canonical root locator uses the frozen IDR-00 identifiers", () => {
    const locator = rootSecretLocator(identityRef);
    expect(locator.service).toBe(SECRET_STORE_SERVICE);
    expect(locator.account).toBe(secretStoreAccount(identityRef));
    expect(locator.account).toBe("identity:test-ref");
  });
});
