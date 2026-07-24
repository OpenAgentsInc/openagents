import { bytesToHex } from "@noble/hashes/utils";
import { describe, expect, it } from "vite-plus/test";

import {
  FORBIDDEN_SARAH_NOSTR_SECRET_FIELDS,
  SARAH_NOSTR_IDENTITY_SECRET_ENV,
  SARAH_NOSTR_IDENTITY_SECRET_ID,
  SARAH_NOSTR_PRINCIPAL,
  SarahNostrSecretLeakError,
  assertLifecycleTransition,
  assertSarahNostrPublicSafe,
  buildArchiveRequestTemplate,
  buildAttestedAuthTemplate,
  createSealedSarahNostrSigner,
  generateSarahNostrSigner,
  generateSecretKeyBytes,
  loadSarahNostrSignerFromSecretManagerMount,
  publicKeyFromSecret,
  signOwnerAuthTag,
  toPublicSafeJson,
  verifyOwnerAuthTag,
  verifySignedEvent,
} from "./index.ts";

describe("SARAH-NR-04 Sarah Nostr identity", () => {
  it("sealed signer signs verifiable events and never exports a key method", () => {
    const signer = generateSarahNostrSigner();
    const pubkey = signer.getPublicKey();
    expect(pubkey).toMatch(/^[0-9a-f]{64}$/);

    const identity = signer.getPublicIdentity();
    expect(identity.principal).toBe(SARAH_NOSTR_PRINCIPAL);
    expect(identity.pubkey).toBe(pubkey);
    expect(identity.custodySecretId).toBe(SARAH_NOSTR_IDENTITY_SECRET_ID);
    expect(identity.lifecycle).toBe("active");

    // Port surface: no secret escape hatches.
    const keys = Object.keys(signer).sort();
    expect(keys).toEqual(["getPublicIdentity", "getPublicKey", "signEvent"]);

    const signed = signer.signEvent({
      kind: 1,
      created_at: 1_700_000_000,
      tags: [["alt", "sarah nostr identity test"]],
      content: "hello from sealed sarah",
    });
    expect(signed.pubkey).toBe(pubkey);
    expect(verifySignedEvent(signed)).toBe(true);
    assertSarahNostrPublicSafe(signed);
  });

  it("loads from Secret Manager mount env and clears the slot", () => {
    const sk = generateSecretKeyBytes();
    const hex = bytesToHex(sk);
    process.env[SARAH_NOSTR_IDENTITY_SECRET_ENV] = hex;

    const signer = loadSarahNostrSignerFromSecretManagerMount();
    expect(process.env[SARAH_NOSTR_IDENTITY_SECRET_ENV]).toBeUndefined();
    expect(signer.getPublicKey()).toBe(publicKeyFromSecret(sk));

    const signed = signer.signEvent({
      kind: 22242,
      created_at: 1_700_000_001,
      tags: [
        ["relay", "ws://127.0.0.1:18765"],
        ["challenge", "abc"],
      ],
      content: "",
    });
    expect(verifySignedEvent(signed)).toBe(true);
  });

  it("rejects secret-shaped public payloads", () => {
    expect(() =>
      assertSarahNostrPublicSafe({ pubkey: "a".repeat(64), privateKey: "x" }),
    ).toThrow(SarahNostrSecretLeakError);
    expect(() =>
      assertSarahNostrPublicSafe({ nsec: "nsec1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq" }),
    ).toThrow(SarahNostrSecretLeakError);
    expect(FORBIDDEN_SARAH_NOSTR_SECRET_FIELDS).toContain("SARAH_NOSTR_IDENTITY_SECRET");
  });

  it("binds NIP-OA owner attestation and builds NIP-AA AUTH template", () => {
    const sarah = generateSarahNostrSigner();
    const ownerSk = bytesToHex(generateSecretKeyBytes());
    const authTag = signOwnerAuthTag({
      agentPubkey: sarah.getPublicKey(),
      conditions: "",
      ownerSeckeyHex: ownerSk,
    });
    expect(authTag[0]).toBe("auth");
    expect(verifyOwnerAuthTag(authTag, sarah.getPublicKey())).toBe(true);
    // Self-attestation must fail
    expect(verifyOwnerAuthTag(authTag, authTag[1])).toBe(false);

    const template = buildAttestedAuthTemplate({
      challenge: "relay-challenge-1",
      relayUrl: "ws://127.0.0.1:18765",
      ownerAuthTag: authTag,
    });
    expect(template.kind).toBe(22242);
    const authTags = template.tags.filter((t) => t[0] === "auth");
    expect(authTags).toHaveLength(1);
    expect(authTags[0]?.[1]).toBe(authTag[1]);

    const signedAuth = sarah.signEvent(template);
    expect(verifySignedEvent(signedAuth)).toBe(true);
    assertSarahNostrPublicSafe(signedAuth);
    expect(toPublicSafeJson(signedAuth)).toContain(signedAuth.id);
  });

  it("builds NIP-IA archive request for rotation and enforces lifecycle table", () => {
    const oldPubkey = generateSarahNostrSigner().getPublicKey();
    const archive = buildArchiveRequestTemplate({
      targetPubkey: oldPubkey,
      reason: "rotated",
    });
    expect(archive.kind).toBe(9035);
    expect(archive.tags.some((t) => t[0] === "p" && t[1] === oldPubkey)).toBe(true);
    expect(archive.tags.some((t) => t[0] === "reason" && t[1] === "rotated")).toBe(
      true,
    );

    assertLifecycleTransition("active", "rotating");
    assertLifecycleTransition("rotating", "archived");
    assertLifecycleTransition("active", "revoked");
    assertLifecycleTransition("revoked", "archived");
    expect(() => assertLifecycleTransition("archived", "active")).toThrow(
      /illegal lifecycle/,
    );
    expect(() => assertLifecycleTransition("revoked", "active")).toThrow(
      /illegal lifecycle/,
    );
  });

  it("revoked signer refuses to sign", () => {
    const sk = generateSecretKeyBytes();
    const signer = createSealedSarahNostrSigner({
      secretKey: sk,
      lifecycle: "revoked",
    });
    expect(() =>
      signer.signEvent({
        kind: 1,
        created_at: 1,
        tags: [],
        content: "nope",
      }),
    ).toThrow(/cannot sign/);
  });
});
