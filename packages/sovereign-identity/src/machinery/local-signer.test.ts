import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { deriveSovereignIdentityPublic } from "../contract/derivation.ts";
import {
  DIVERGENCE_TEST_PASSPHRASE,
  PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE,
  PUBLIC_TEST_MNEMONIC,
} from "../contract/vectors.ts";
import { deriveLocalNostrIdentity, makeSovereignSignerFromMnemonic } from "./local-signer.ts";

/**
 * IDR-06 parity + signer-surface tests.
 *
 * SAFETY: every value comes from the ONE published BIP-39 TEST mnemonic in
 * `vectors.ts`. No real secret is used, and no secret is printed.
 *
 * The frozen IDR-00 reference derivation (`deriveSovereignIdentityPublic`) stays
 * an INDEPENDENT hand-rolled `@noble`/`@scure` computation, so these tests prove
 * the swapped `nostr-effect` engine matches the frozen reference rather than
 * checking the engine against itself.
 */
describe("IDR-06 nostr-effect engine matches the frozen IDR-00 reference", () => {
  test("derives the exact frozen npub, pubkey, and profile from the public test mnemonic", () => {
    const identity = deriveLocalNostrIdentity(PUBLIC_TEST_MNEMONIC);
    const reference = deriveSovereignIdentityPublic(PUBLIC_TEST_MNEMONIC);

    expect(identity.npub).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.npub);
    expect(identity.npub).toBe(reference.npub);
    expect(identity.publicKey).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.nostrPublicKeyHex);
    expect(identity.publicKey).toBe(reference.nostrPublicKeyHex);
    expect(identity.profileId).toBe("openagents.legacy_unified_nostr_spark.v1");
    expect(identity.accountPath).toBe("m/44'/1237'/0'/0/0");
  });

  test("the Spark public fingerprint stays on the frozen reference (nostr-effect is Nostr-only)", () => {
    // The engine swap is Nostr-only; the Spark branch keeps deriving from the
    // frozen reference, so the boot display's wallet fingerprint is unchanged.
    const reference = deriveSovereignIdentityPublic(PUBLIC_TEST_MNEMONIC);
    expect(reference.sparkBip32FingerprintHex).toBe(
      PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.sparkBip32FingerprintHex,
    );
  });
});

describe("IDR-06 signer surface is narrow (no secret-returning method)", () => {
  test("`asSigner()` exposes exactly the six LocalSignerPort operations", () => {
    const { signer } = deriveLocalNostrIdentity(PUBLIC_TEST_MNEMONIC);
    expect(Object.keys(signer).sort()).toEqual(
      [
        "createHttpAuthToken",
        "getPublicKey",
        "nip44Encrypt",
        "nip44Decrypt",
        "signEvent",
        "toPublicManifest",
      ].sort(),
    );
    // The escape hatches are NOT reachable through the signer.
    expect("exportPrivateKeyBytes" in signer).toBe(false);
    expect("exportNsec" in signer).toBe(false);
    expect("mnemonic" in signer).toBe(false);
  });

  test("the public identity object carries no mnemonic/nsec/private-key/seed field", () => {
    const identity = deriveLocalNostrIdentity(PUBLIC_TEST_MNEMONIC);
    const keys = Object.keys(identity);
    for (const forbidden of ["mnemonic", "nsec", "privateKey", "privateKeyHex", "privateKeyBytes", "seed"]) {
      expect(keys.includes(forbidden)).toBe(false);
    }
    // The whole serialized public identity never contains the test phrase.
    const serialized = JSON.stringify({ ...identity, signer: "[signer]" });
    expect(serialized.includes("abandon")).toBe(false);
  });
});

describe("IDR-06 Effect SovereignSigner real layer", () => {
  test("signs a NIP-98 token and reports the frozen public manifest + profile proof", async () => {
    const iface = makeSovereignSignerFromMnemonic(PUBLIC_TEST_MNEMONIC);

    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const pubkey = yield* iface.getPublicKey();
        const manifest = yield* iface.toPublicManifest();
        const proof = yield* iface.proveDerivationProfile();
        const token = yield* iface.createHttpAuthToken(
          "https://openagents.com/api/pylons/pylon.test/heartbeat",
          "POST",
          { includeAuthorizationScheme: true, body: '{"pylonRef":"pylon.test"}' },
        );
        const event = yield* iface.signEvent({ kind: 1, content: "hi", tags: [] });
        return { pubkey, manifest, proof, token, event };
      }),
    );

    expect(result.pubkey).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.nostrPublicKeyHex);
    expect(result.manifest.npub).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.npub);
    expect(result.proof.derivationProfile).toBe("openagents.legacy_unified_nostr_spark.v1");
    expect(result.proof.nostrPublicKeyHex).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.nostrPublicKeyHex);
    expect(result.token.startsWith("Nostr ")).toBe(true);
    expect(result.event.kind).toBe(1);
    expect(result.event.pubkey).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.nostrPublicKeyHex);
    expect(result.event.sig.length).toBeGreaterThan(0);
  });

  test("a non-empty passphrase would diverge — the legacy profile stays empty-passphrase", () => {
    // Guard the empty-passphrase rule at the engine seam: the divergence vector
    // is a different identity, proving the adopted engine keeps the frozen rule.
    const legacy = deriveLocalNostrIdentity(PUBLIC_TEST_MNEMONIC);
    const diverged = deriveSovereignIdentityPublic(PUBLIC_TEST_MNEMONIC, DIVERGENCE_TEST_PASSPHRASE);
    expect(legacy.npub).not.toBe(diverged.npub);
  });
});
