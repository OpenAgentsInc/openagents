import { describe, expect, test } from "vite-plus/test";

import {
  BIP39_LANGUAGE,
  BIP39_PASSPHRASE_MODE,
  DERIVATION_PROFILE_ID,
  DIVERGENCE_TEST_PASSPHRASE,
  EMPTY_BIP39_PASSPHRASE,
  HISTORICAL_FORMAT_FIXTURES,
  HISTORICAL_FORMAT_IDS,
  LOCAL_IDENTITY_MANIFEST_SCHEMA,
  LOCAL_IDENTITY_MIGRATION_RECEIPT_SCHEMA,
  LOCAL_IDENTITY_SECRET_SCHEMA,
  NOSTR_DERIVATION_PATH,
  PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE,
  PUBLIC_TEST_IDENTITY_NONEMPTY_PASSPHRASE,
  PUBLIC_TEST_MNEMONIC,
  SECRET_STORE_SERVICE,
  SPARK_DERIVATION_PATH,
  decodeLocalIdentityManifest,
  decodeLocalIdentityMigrationReceipt,
  decodeLocalIdentitySecret,
  deriveSovereignIdentityPublic,
  extractDecodableMnemonic,
  isValidEnglishMnemonic,
  secretStoreAccount,
} from "./index.ts";

describe("frozen derivation profile", () => {
  test("the profile constants are frozen", () => {
    expect(NOSTR_DERIVATION_PATH).toBe("m/44'/1237'/0'/0/0");
    expect(SPARK_DERIVATION_PATH).toBe("m/44'/0'/0'/0/0");
    expect(BIP39_LANGUAGE).toBe("english");
    expect(BIP39_PASSPHRASE_MODE).toBe("empty");
    expect(EMPTY_BIP39_PASSPHRASE).toBe("");
    expect(DERIVATION_PROFILE_ID).toBe("openagents.legacy_unified_nostr_spark.v1");
  });

  test("the public test mnemonic is a valid English BIP-39 phrase", () => {
    expect(isValidEnglishMnemonic(PUBLIC_TEST_MNEMONIC)).toBe(true);
  });

  test("empty-passphrase derivation matches the frozen public vector", () => {
    const identity = deriveSovereignIdentityPublic(PUBLIC_TEST_MNEMONIC);
    expect(identity.npub).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.npub);
    expect(identity.nostrPublicKeyHex).toBe(
      PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.nostrPublicKeyHex,
    );
    expect(identity.sparkPublicKeyHex).toBe(
      PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.sparkPublicKeyHex,
    );
    expect(identity.sparkBip32FingerprintHex).toBe(
      PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.sparkBip32FingerprintHex,
    );
    expect(identity.derivationProfile).toBe(DERIVATION_PROFILE_ID);
    expect(identity.nostrDerivationPath).toBe(NOSTR_DERIVATION_PATH);
    expect(identity.sparkDerivationPath).toBe(SPARK_DERIVATION_PATH);
  });

  test("derivation is deterministic", () => {
    const a = deriveSovereignIdentityPublic(PUBLIC_TEST_MNEMONIC);
    const b = deriveSovereignIdentityPublic(`  ${PUBLIC_TEST_MNEMONIC}  `);
    expect(a).toEqual(b);
  });

  test("an invalid mnemonic is rejected", () => {
    expect(() => deriveSovereignIdentityPublic("not a real mnemonic phrase at all")).toThrow();
  });
});

describe("empty-passphrase rule", () => {
  test("a non-empty passphrase yields DIFFERENT keys", () => {
    const empty = deriveSovereignIdentityPublic(PUBLIC_TEST_MNEMONIC, EMPTY_BIP39_PASSPHRASE);
    const withPassphrase = deriveSovereignIdentityPublic(
      PUBLIC_TEST_MNEMONIC,
      DIVERGENCE_TEST_PASSPHRASE,
    );

    // The passphrase variant matches its own frozen vector.
    expect(withPassphrase.npub).toBe(PUBLIC_TEST_IDENTITY_NONEMPTY_PASSPHRASE.npub);
    expect(withPassphrase.sparkPublicKeyHex).toBe(
      PUBLIC_TEST_IDENTITY_NONEMPTY_PASSPHRASE.sparkPublicKeyHex,
    );

    // Every public identifier diverges from the empty-passphrase identity.
    expect(withPassphrase.npub).not.toBe(empty.npub);
    expect(withPassphrase.nostrPublicKeyHex).not.toBe(empty.nostrPublicKeyHex);
    expect(withPassphrase.sparkPublicKeyHex).not.toBe(empty.sparkPublicKeyHex);
    expect(withPassphrase.sparkBip32FingerprintHex).not.toBe(empty.sparkBip32FingerprintHex);
  });
});

describe("secret-store identifiers", () => {
  test("the service name is frozen and the account is derived", () => {
    expect(SECRET_STORE_SERVICE).toBe("com.openagents.identity.root.v1");
    expect(secretStoreAccount("owner-root")).toBe("identity:owner-root");
  });
});

describe("frozen schemas", () => {
  const identity = deriveSovereignIdentityPublic(PUBLIC_TEST_MNEMONIC);
  const now = "2026-07-20T00:00:00.000Z";

  test("the secret payload schema decodes and enforces the frozen rule", () => {
    const secret = decodeLocalIdentitySecret({
      schema: LOCAL_IDENTITY_SECRET_SCHEMA,
      mnemonic: PUBLIC_TEST_MNEMONIC,
      language: "english",
      bip39PassphraseMode: "empty",
    });
    expect(secret.bip39PassphraseMode).toBe("empty");

    // A non-empty passphrase mode is not a valid secret record.
    expect(() =>
      decodeLocalIdentitySecret({
        schema: LOCAL_IDENTITY_SECRET_SCHEMA,
        mnemonic: PUBLIC_TEST_MNEMONIC,
        language: "english",
        bip39PassphraseMode: "with_passphrase",
      }),
    ).toThrow();
  });

  test("the public manifest decodes and drops secret fields", () => {
    const manifest = decodeLocalIdentityManifest({
      schema: LOCAL_IDENTITY_MANIFEST_SCHEMA,
      identityRef: "owner-root",
      npub: identity.npub,
      nostrPublicKeyHex: identity.nostrPublicKeyHex,
      derivationProfile: DERIVATION_PROFILE_ID,
      sparkFingerprints: [
        {
          adapter: "rust_spark",
          fingerprintHex: identity.sparkBip32FingerprintHex,
          publicKeyHex: identity.sparkPublicKeyHex,
        },
      ],
      secretStoreLocatorType: "in_memory_test",
      receiptRefs: [],
      backupState: "none",
      createdAt: now,
      // Secret fields are supplied on purpose; the schema must exclude them.
      mnemonic: PUBLIC_TEST_MNEMONIC,
      nsec: "nsec1shouldneverappear",
      seed: "deadbeef",
      privateKeyHex: "deadbeef",
    });
    expect(manifest.npub).toBe(identity.npub);
    expect(Object.keys(manifest)).not.toContain("mnemonic");
    expect(Object.keys(manifest)).not.toContain("nsec");
    expect(Object.keys(manifest)).not.toContain("seed");
    expect(Object.keys(manifest)).not.toContain("privateKeyHex");
  });

  test("the migration receipt decodes", () => {
    const receipt = decodeLocalIdentityMigrationReceipt({
      schema: LOCAL_IDENTITY_MIGRATION_RECEIPT_SCHEMA,
      receiptRef: "receipt-1",
      identityRef: "owner-root",
      npub: identity.npub,
      nostrPublicKeyHex: identity.nostrPublicKeyHex,
      derivationProfile: DERIVATION_PROFILE_ID,
      sparkFingerprints: [
        { adapter: "rust_spark", fingerprintHex: identity.sparkBip32FingerprintHex },
      ],
      sourceLabels: ["plain_mnemonic_file"],
      sourceFormatVersions: ["plain-text-v1"],
      outcome: "imported",
      createdAt: now,
    });
    expect(receipt.outcome).toBe("imported");
    expect(Object.keys(receipt)).not.toContain("mnemonic");
  });
});

describe("historical-format fixtures", () => {
  test("there is exactly one fixture per frozen format", () => {
    expect(HISTORICAL_FORMAT_FIXTURES.map((fixture) => fixture.format)).toEqual([
      ...HISTORICAL_FORMAT_IDS,
    ]);
  });

  test("every fixture points at the one frozen public identity", () => {
    for (const fixture of HISTORICAL_FORMAT_FIXTURES) {
      expect(fixture.expectedPublicIdentity).toEqual(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE);
    }
  });

  test("every decodable fixture yields the frozen public identity in code", () => {
    let decodableCount = 0;
    for (const fixture of HISTORICAL_FORMAT_FIXTURES) {
      const mnemonic = extractDecodableMnemonic(fixture);
      if (fixture.decodability === "decodable_now") {
        decodableCount += 1;
        expect(mnemonic).not.toBeNull();
        // Tripwire: the only phrase embedded in a fixture is the public test one.
        expect(mnemonic).toBe(PUBLIC_TEST_MNEMONIC);
        const identity = deriveSovereignIdentityPublic(mnemonic as string);
        expect(identity.npub).toBe(fixture.expectedPublicIdentity.npub);
        expect(identity.sparkPublicKeyHex).toBe(fixture.expectedPublicIdentity.sparkPublicKeyHex);
      } else {
        // An envelope-shape fixture is not decodable offline and names its gap.
        expect(mnemonic).toBeNull();
        expect(typeof fixture.idr03Gap).toBe("string");
        expect((fixture.idr03Gap as string).length).toBeGreaterThan(0);
      }
    }
    expect(decodableCount).toBeGreaterThan(0);
  });
});
