import { Effect, Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  type CustodyKeyExportInterface,
  DerivationProfileProof,
  type NostrSignerPort,
  PublicIdentityManifest,
  SignEventTemplate,
  SignedNostrEvent,
  type SovereignSignerInterface,
  type SparkSecretMaterialInterface,
} from "./signer.ts";

/**
 * IDR-01 defines the signer PORT SHAPE. The narrowed real implementation and the
 * static "normal code cannot import a secret export" test arrive in IDR-06. Here
 * the port shape is proven: the boundary exposes exactly the admitted public
 * operations, structurally matches the `nostr-effect` `LocalSignerPort`, and has
 * no secret-returning method.
 */

const SECRET_RETURNING_NAMES = [
  "mnemonic",
  "nsec",
  "privateKey",
  "rawPrivateKey",
  "seed",
  "entropy",
  "exportSecret",
  "exportNsec",
  "exportPrivateKeyBytes",
  "getMnemonic",
] as const;

describe("sovereign signer port shape", () => {
  test("the Nostr signer port matches the nostr-effect LocalSignerPort surface", () => {
    // A mock typed as the aligned port. The compiler rejects any extra or missing
    // member, so the runtime key set below is the exact aligned surface.
    const mock: NostrSignerPort = {
      getPublicKey: () => Effect.die("not implemented at IDR-01"),
      signEvent: () => Effect.die("not implemented at IDR-01"),
      nip44Encrypt: () => Effect.die("not implemented at IDR-01"),
      nip44Decrypt: () => Effect.die("not implemented at IDR-01"),
      createHttpAuthToken: () => Effect.die("not implemented at IDR-01"),
      toPublicManifest: () => Effect.die("not implemented at IDR-01"),
    };
    expect([...Object.keys(mock)].sort()).toEqual([
      "createHttpAuthToken",
      "getPublicKey",
      "nip44Decrypt",
      "nip44Encrypt",
      "signEvent",
      "toPublicManifest",
    ]);
    for (const name of SECRET_RETURNING_NAMES) {
      expect(Object.keys(mock)).not.toContain(name);
    }
  });

  test("the sovereign signer adds only proveDerivationProfile and no secret method", () => {
    const mock: SovereignSignerInterface = {
      getPublicKey: () => Effect.die("not implemented at IDR-01"),
      signEvent: () => Effect.die("not implemented at IDR-01"),
      nip44Encrypt: () => Effect.die("not implemented at IDR-01"),
      nip44Decrypt: () => Effect.die("not implemented at IDR-01"),
      createHttpAuthToken: () => Effect.die("not implemented at IDR-01"),
      toPublicManifest: () => Effect.die("not implemented at IDR-01"),
      proveDerivationProfile: () => Effect.die("not implemented at IDR-01"),
    };
    expect(Object.keys(mock)).toContain("proveDerivationProfile");
    for (const name of SECRET_RETURNING_NAMES) {
      expect(Object.keys(mock)).not.toContain(name);
    }
  });

  test("the custody export port is the only place a secret-returning method exists", () => {
    const mock: CustodyKeyExportInterface = {
      exportPrivateKeyBytes: () => Effect.die("custody/recovery only, not at IDR-01"),
      exportNsec: () => Effect.die("custody/recovery only, not at IDR-01"),
    };
    expect([...Object.keys(mock)].sort()).toEqual(["exportNsec", "exportPrivateKeyBytes"]);
  });

  test("the Spark secret-material port exposes only a bounded callback", () => {
    const mock: SparkSecretMaterialInterface = {
      withSeedMaterial: () => Effect.die("not implemented at IDR-01"),
    };
    expect(Object.keys(mock)).toEqual(["withSeedMaterial"]);
  });

  test("the sign template carries no author or signature and allows optional created_at", () => {
    const decoded = S.decodeUnknownSync(SignEventTemplate)({
      kind: 1,
      tags: [["t", "openagents"]],
      content: "hello",
    });
    expect(decoded).not.toHaveProperty("pubkey");
    expect(decoded).not.toHaveProperty("id");
    expect(decoded).not.toHaveProperty("sig");
    expect(decoded).not.toHaveProperty("created_at");
  });

  test("the signed event, manifest, and profile-proof schemas accept public fields only", () => {
    const event = S.decodeUnknownSync(SignedNostrEvent)({
      id: "aa",
      pubkey: "bb",
      created_at: 1_700_000_000,
      kind: 1,
      tags: [],
      content: "hi",
      sig: "cc",
    });
    expect(event.sig).toBe("cc");
    const manifest = S.decodeUnknownSync(PublicIdentityManifest)({
      pubkey: "e8bcf3823669444d0b49ad45d65088635d9fd8500a75b5f20b59abefa56a144f",
      npub: "npub1az708q3kd9zy6z6f44zav5ygvdwelkzspf6mtusttx47lft2z38sghk0w7",
      profileId: "openagents.legacy_unified_nostr_spark.v1",
    });
    expect(manifest.profileId).toBe("openagents.legacy_unified_nostr_spark.v1");
    const proof = S.decodeUnknownSync(DerivationProfileProof)({
      derivationProfile: "openagents.legacy_unified_nostr_spark.v1",
      nostrPublicKeyHex: "ab",
    });
    expect(proof.derivationProfile).toBe("openagents.legacy_unified_nostr_spark.v1");
  });
});
