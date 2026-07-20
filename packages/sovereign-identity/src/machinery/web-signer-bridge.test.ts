import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { Schema } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { PUBLIC_TEST_MNEMONIC } from "../contract/index.ts";
import { resolveLocalIdentityPublic } from "./resolved-identity.ts";
import {
  assertWebBridgePayloadPublicSafe,
  FORBIDDEN_WEB_SECRET_FIELDS,
  Nip46RemoteSignerConfig,
  WebBridgeRawKeyRefusedError,
  WebPublicIdentity,
  webSignerBridgeFromSigner,
} from "./web-signer-bridge.ts";

/**
 * IDR-08 web boundary proof: the browser receives PUBLIC identity and signer
 * OPERATIONS only — never a raw mnemonic, `nsec`, private key, or seed. This is
 * a LOCAL-ONLY, fixture-only test (IDR-00 public mnemonic); it opens no network
 * and no Keychain.
 */
describe("IDR-08 the web signer bridge carries no raw key", () => {
  test("the web public identity schema admits only public fields", () => {
    const resolved = resolveLocalIdentityPublic(PUBLIC_TEST_MNEMONIC);
    const identity = Schema.decodeUnknownSync(WebPublicIdentity)({
      identityRef: resolved.identityRef,
      npub: resolved.npub,
      pubkey: resolved.publicKey,
      profileId: resolved.profileId,
    });
    expect(identity.identityRef).toBe(resolved.npub);
    // A payload carrying an nsec/mnemonic is not part of the schema; the excess
    // key is dropped, so the decoded value can never carry a secret field.
    const decoded = Schema.decodeUnknownSync(WebPublicIdentity)({
      identityRef: resolved.identityRef,
      npub: resolved.npub,
      pubkey: resolved.publicKey,
      profileId: resolved.profileId,
      nsec: "nsec1shouldneverpass",
      mnemonic: PUBLIC_TEST_MNEMONIC,
    } as Record<string, unknown>);
    expect(Object.keys(decoded).sort()).toEqual(["identityRef", "npub", "profileId", "pubkey"]);
    expect(JSON.stringify(decoded).includes("nsec")).toBe(false);
    expect(JSON.stringify(decoded).includes(PUBLIC_TEST_MNEMONIC)).toBe(false);
  });

  test("the guard refuses every raw-key-shaped field, at any depth", () => {
    for (const field of FORBIDDEN_WEB_SECRET_FIELDS) {
      expect(() => assertWebBridgePayloadPublicSafe({ [field]: "x" })).toThrow(
        WebBridgeRawKeyRefusedError,
      );
      expect(() => assertWebBridgePayloadPublicSafe({ nested: { deep: { [field]: "x" } } })).toThrow(
        WebBridgeRawKeyRefusedError,
      );
    }
    // A purely public payload passes.
    expect(() =>
      assertWebBridgePayloadPublicSafe({ npub: "npub1abc", pubkey: "ff", profileId: "p" }),
    ).not.toThrow();
  });

  test("the bridge forwards signer operations and exposes no secret method", async () => {
    const resolved = resolveLocalIdentityPublic(PUBLIC_TEST_MNEMONIC);
    const identity = Schema.decodeUnknownSync(WebPublicIdentity)({
      identityRef: resolved.identityRef,
      npub: resolved.npub,
      pubkey: resolved.publicKey,
      profileId: resolved.profileId,
    });
    const bridge = webSignerBridgeFromSigner(identity, resolved.signer);
    // The signer works through the bridge without ever exposing the key.
    expect(await bridge.getPublicKey()).toBe(resolved.publicKey);
    // The bridge surface names only public identity + signer operations.
    expect(Object.keys(bridge).sort()).toEqual([
      "getPublicKey",
      "identity",
      "nip44Decrypt",
      "nip44Encrypt",
      "signEvent",
    ]);
    for (const field of FORBIDDEN_WEB_SECRET_FIELDS) {
      expect(field in bridge).toBe(false);
    }
  });

  test("the NIP-46 config carries public routing data and a public identity only", () => {
    const resolved = resolveLocalIdentityPublic(PUBLIC_TEST_MNEMONIC);
    const config = Schema.decodeUnknownSync(Nip46RemoteSignerConfig)({
      transport: "nip46",
      relay: "wss://relay.example",
      remoteSignerPubkey: resolved.publicKey,
      identity: {
        identityRef: resolved.identityRef,
        npub: resolved.npub,
        pubkey: resolved.publicKey,
        profileId: resolved.profileId,
      },
    });
    expect(config.transport).toBe("nip46");
    // No secret shape survives the decode.
    for (const field of FORBIDDEN_WEB_SECRET_FIELDS) {
      expect(JSON.stringify(config).includes(field === "seed" ? "\"seed\"" : field)).toBe(false);
    }
  });

  test("static: the web bridge module names no secret-export escape hatch", () => {
    const modulePath = path.join(import.meta.dirname, "web-signer-bridge.ts");
    expect(existsSync(modulePath)).toBe(true);
    const source = readFileSync(modulePath, "utf8");
    // The module may LIST forbidden field names (that is its guard), but it must
    // never NAME a key-export escape hatch or reach the isolated custody module.
    for (const symbol of [
      "exportPrivateKeyBytes",
      "exportNsec",
      "makeCustodyKeyExport",
      "custodyKeyExportLayer",
      "asLocalKeySigner",
    ]) {
      expect(source.includes(symbol)).toBe(false);
    }
    expect(source.includes("custody")).toBe(false);
  });
});
