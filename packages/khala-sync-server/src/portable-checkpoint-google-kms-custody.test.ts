import { createCipheriv, createDecipheriv, createHash } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";
import {
  PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  type PortableCheckpointCustodyObjectManifest,
} from "@openagentsinc/portable-session-contract";
import { describe, expect, test, vi } from "vite-plus/test";

import {
  createPortableCheckpointGoogleKmsCustodyDecryptor,
  PortableCheckpointGoogleKmsCustodyError,
  type GoogleKmsDecryptAuthority,
} from "./portable-checkpoint-google-kms-custody.js";

const key = Uint8Array.from({ length: 32 }, (_, index) => index + 1);
const nonce = Uint8Array.from({ length: 12 }, (_, index) => index + 10);
const plaintext = new TextEncoder().encode("portable checkpoint private payload");
const objectRef = "checkpoint-custody:ide13-google-kms";
const keyRef = "gcp-kms.key.ide13-google-kms";

const sha256 = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;

const header = (policy: "owner_managed" | "openagents_managed" = "openagents_managed") => ({
  schema: "openagents.portable_checkpoint_artifact_custody_encrypted.v2" as const,
  algorithm: "aes-256-gcm" as const,
  objectRef,
  policy,
  keyRef,
});

const encrypt = (policy: "owner_managed" | "openagents_managed" = "openagents_managed") => {
  const aad = new TextEncoder().encode(canonicalJson(header(policy)));
  const cipher = createCipheriv("aes-256-gcm", key, nonce, { authTagLength: 16 });
  cipher.setAAD(aad);
  const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final()]);
  const envelope = {
    ...header(policy),
    nonceBase64: Buffer.from(nonce).toString("base64"),
    authTagBase64: cipher.getAuthTag().toString("base64"),
    ciphertextBase64: ciphertext.toString("base64"),
  };
  const encryptedObjectBytes = new TextEncoder().encode(canonicalJson(envelope));
  aad.fill(0);
  return { envelope, ciphertext: Uint8Array.from(ciphertext), encryptedObjectBytes };
};

const manifestFor = (
  encrypted: ReturnType<typeof encrypt>,
  policy: "owner_managed" | "openagents_managed" = "openagents_managed",
): PortableCheckpointCustodyObjectManifest => ({
  schema: "openagents.portable_checkpoint_custody_object_manifest.v1",
  objectRef,
  objectDigest: sha256(encrypted.encryptedObjectBytes),
  artifactRef: "artifact.ide13.google-kms",
  artifactDigest: `sha256:${"1".repeat(64)}`,
  checkpointRef: "checkpoint.ide13.google-kms",
  checkpointDigest: `sha256:${"2".repeat(64)}`,
  bundleDigest: `sha256:${"3".repeat(64)}`,
  ciphertextDigest: sha256(encrypted.ciphertext),
  commandClaim: {
    schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
    claimRef: "claim.ide13.google-kms",
    commandRef: "command.ide13.google-kms",
    ownerRef: "owner.ide13.google-kms",
    sessionRef: "session.ide13.google-kms",
    commandKind: "move",
    commandFingerprint: `sha256:${"4".repeat(64)}`,
    claimFingerprint: `sha256:${"5".repeat(64)}`,
    sourceAttachmentRef: "attachment.ide13.google-kms",
    sourceGeneration: 1,
    destinationTargetRef: "target.ide13.google-kms",
    executorEnvironmentRef: "environment.ide13.google-kms",
    workerInstanceRef: "worker.ide13.google-kms",
    claimGeneration: 1,
    leaseRevision: 1,
    state: "claimed",
    claimedAt: "2026-07-20T12:00:00.000Z",
    leaseExpiresAt: "2026-07-20T12:10:00.000Z",
    updatedAt: "2026-07-20T12:00:00.000Z",
    terminalStatus: null,
    pendingReconcileRef: null,
    outcomeRef: null,
    evidenceRefs: [],
  },
  ownerRef: "owner.ide13.google-kms",
  sourcePylonRef: "pylon.ide13.google-kms",
  targetRef: "target.ide13.google-kms",
  sessionRef: "session.ide13.google-kms",
  sourceAttachmentRef: "attachment.ide13.google-kms",
  sourceGeneration: 1,
  custodyPolicy: policy,
  keyRef,
  byteLimit: 1024 * 1024,
  createdAt: "2026-07-20T12:00:00.000Z",
  expiresAt: "2026-07-20T12:09:00.000Z",
  retentionSeconds: 540,
  secretMaterial: "excluded",
});

const localKmsAuthority = () => {
  let authorityPlaintext: Uint8Array | undefined;
  let authorityInput: Parameters<GoogleKmsDecryptAuthority["decryptAes256Gcm"]>[0] | undefined;
  const decryptAes256Gcm = vi.fn<GoogleKmsDecryptAuthority["decryptAes256Gcm"]>(async (input) => {
    authorityInput = input;
    const decipher = createDecipheriv("aes-256-gcm", key, input.nonce, { authTagLength: 16 });
    decipher.setAAD(input.additionalAuthenticatedData);
    decipher.setAuthTag(input.authTag);
    const decrypted = Buffer.concat([decipher.update(input.ciphertext), decipher.final()]);
    authorityPlaintext = Uint8Array.from(decrypted);
    decrypted.fill(0);
    return authorityPlaintext;
  });
  return {
    authority: { decryptAes256Gcm } satisfies GoogleKmsDecryptAuthority,
    decryptAes256Gcm,
    input: () => authorityInput,
    plaintext: () => authorityPlaintext,
  };
};

describe("OpenAgents-managed Google KMS checkpoint custody", () => {
  test("passes exact bindings to the authority and clears all temporary buffers", async () => {
    const encrypted = encrypt();
    const manifest = manifestFor(encrypted);
    const kms = localKmsAuthority();
    const decryptor = createPortableCheckpointGoogleKmsCustodyDecryptor({
      authority: kms.authority,
    });

    const result = await decryptor.decrypt({
      manifest,
      encryptedObjectBytes: encrypted.encryptedObjectBytes,
    });

    expect(result).toEqual(plaintext);
    expect(kms.decryptAes256Gcm).toHaveBeenCalledOnce();
    expect(kms.input()).toMatchObject({
      manifest,
      manifestDigest: sha256(canonicalJson(manifest)),
      objectRef,
      policy: "openagents_managed",
      keyRef,
    });
    expect(kms.input()?.additionalAuthenticatedData.every((byte) => byte === 0)).toBe(true);
    expect(kms.input()?.nonce).toEqual(new Uint8Array(12));
    expect(kms.input()?.authTag).toEqual(new Uint8Array(16));
    expect(kms.input()?.ciphertext.every((byte) => byte === 0)).toBe(true);
    expect(kms.plaintext()?.every((byte) => byte === 0)).toBe(true);
    expect(result.every((byte) => byte === 0)).toBe(false);
  });

  test("refuses owner-managed custody before it calls OpenAgents authority", async () => {
    const encrypted = encrypt("owner_managed");
    const manifest = manifestFor(encrypted, "owner_managed");
    const authority = { decryptAes256Gcm: vi.fn() } as unknown as GoogleKmsDecryptAuthority;
    const decryptor = createPortableCheckpointGoogleKmsCustodyDecryptor({ authority });

    await expect(
      decryptor.decrypt({ manifest, encryptedObjectBytes: encrypted.encryptedObjectBytes }),
    ).rejects.toMatchObject({ code: "owner_managed_refused" });
    expect(authority.decryptAes256Gcm).not.toHaveBeenCalled();
  });

  test("rejects object, key, policy, and ciphertext binding mismatches before authority", async () => {
    const encrypted = encrypt();
    const authority = { decryptAes256Gcm: vi.fn() } as unknown as GoogleKmsDecryptAuthority;
    const decryptor = createPortableCheckpointGoogleKmsCustodyDecryptor({ authority });
    const cases: PortableCheckpointCustodyObjectManifest[] = [
      { ...manifestFor(encrypted), objectDigest: `sha256:${"0".repeat(64)}` },
      { ...manifestFor(encrypted), keyRef: "gcp-kms.key.other" },
      { ...manifestFor(encrypted), ciphertextDigest: `sha256:${"0".repeat(64)}` },
    ];

    await Promise.all(
      cases.map((manifest) =>
        expect(
          decryptor.decrypt({ manifest, encryptedObjectBytes: encrypted.encryptedObjectBytes }),
        ).rejects.toBeInstanceOf(PortableCheckpointGoogleKmsCustodyError),
      ),
    );
    expect(authority.decryptAes256Gcm).not.toHaveBeenCalled();
  });

  test("fails closed when authentication fails and enforces plaintext bounds", async () => {
    const encrypted = encrypt();
    const tamperedTag = Buffer.from(encrypted.envelope.authTagBase64, "base64");
    tamperedTag[0] ^= 1;
    const tamperedEnvelope = {
      ...encrypted.envelope,
      authTagBase64: tamperedTag.toString("base64"),
    };
    const tamperedBytes = new TextEncoder().encode(canonicalJson(tamperedEnvelope));
    const tampered = {
      ...encrypted,
      envelope: tamperedEnvelope,
      encryptedObjectBytes: tamperedBytes,
    };
    const kms = localKmsAuthority();
    const decryptor = createPortableCheckpointGoogleKmsCustodyDecryptor({
      authority: kms.authority,
    });

    await expect(
      decryptor.decrypt({ manifest: manifestFor(tampered), encryptedObjectBytes: tamperedBytes }),
    ).rejects.toMatchObject({ code: "authority_unavailable" });

    const bounded = createPortableCheckpointGoogleKmsCustodyDecryptor({
      authority: localKmsAuthority().authority,
      maxPlaintextBytes: plaintext.byteLength - 1,
    });
    await expect(
      bounded.decrypt({
        manifest: manifestFor(encrypted),
        encryptedObjectBytes: encrypted.encryptedObjectBytes,
      }),
    ).rejects.toMatchObject({ code: "plaintext_oversized" });
  });
});
