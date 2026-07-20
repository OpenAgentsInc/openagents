import { describe, expect, test } from "vite-plus/test";

import {
  deriveIdentityRef,
  deriveSovereignIdentityPublic,
  PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE,
  PUBLIC_TEST_MNEMONIC,
} from "../contract/index.ts";
import { resolveLocalIdentityPublic } from "./resolved-identity.ts";

/**
 * IDR-08 shared resolver proof. LOCAL-ONLY, fixture-only (IDR-00 public
 * mnemonic). No network, no Keychain, no secret in output.
 */
describe("IDR-08 the ONE resolved local identity", () => {
  test("resolves the frozen public identity and the canonical identityRef", () => {
    const resolved = resolveLocalIdentityPublic(PUBLIC_TEST_MNEMONIC);
    expect(resolved.npub).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.npub);
    expect(resolved.publicKey).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.nostrPublicKeyHex);
    expect(resolved.sparkFingerprint).toBe(
      PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.sparkBip32FingerprintHex,
    );
    // The identityRef is the npub under the frozen profile.
    expect(resolved.identityRef).toBe(resolved.npub);
    expect(resolved.identityRef).toBe(deriveIdentityRef(PUBLIC_TEST_MNEMONIC));
  });

  test("is deterministic across repeated resolution", () => {
    const a = resolveLocalIdentityPublic(PUBLIC_TEST_MNEMONIC);
    const b = resolveLocalIdentityPublic(PUBLIC_TEST_MNEMONIC);
    expect(a.identityRef).toBe(b.identityRef);
    expect(a.npub).toBe(b.npub);
    expect(a.sparkFingerprint).toBe(b.sparkFingerprint);
  });

  test("carries public data and a signer only — no secret shape", () => {
    const resolved = resolveLocalIdentityPublic(PUBLIC_TEST_MNEMONIC);
    // The projection has only the documented public keys plus the signer.
    expect(Object.keys(resolved).sort()).toEqual([
      "accountPath",
      "identityRef",
      "npub",
      "profileId",
      "publicKey",
      "signer",
      "sparkFingerprint",
    ]);
    // Serializing the public projection (the signer is a function, dropped by
    // JSON) exposes no mnemonic/nsec/seed.
    const serialized = JSON.stringify(resolved);
    expect(serialized.includes(PUBLIC_TEST_MNEMONIC)).toBe(false);
    expect(serialized.includes("nsec")).toBe(false);
    expect(serialized.includes("mnemonic")).toBe(false);
  });

  test("the identityRef anchors the same identity as the frozen Spark vector", () => {
    const resolved = resolveLocalIdentityPublic(PUBLIC_TEST_MNEMONIC);
    const spark = deriveSovereignIdentityPublic(PUBLIC_TEST_MNEMONIC);
    expect(resolved.sparkFingerprint).toBe(spark.sparkBip32FingerprintHex);
  });
});
