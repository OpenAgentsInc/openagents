import { Schema } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  PORTABLE_CHECKPOINT_CUSTODY_OBJECT_MANIFEST_SCHEMA_VERSION,
  PortableCheckpointCustodyEncryptedV2Schema,
  PortableCheckpointCustodyEncryptedV3Schema,
  PortableCheckpointCustodyObjectManifestSchema,
} from "./checkpoint-custody-transport.js";

const digest = `sha256:${"a".repeat(64)}` as const;
const decodeManifest = Schema.decodeUnknownSync(PortableCheckpointCustodyObjectManifestSchema);
const decodeV2 = Schema.decodeUnknownSync(PortableCheckpointCustodyEncryptedV2Schema);
const decodeV3 = Schema.decodeUnknownSync(PortableCheckpointCustodyEncryptedV3Schema);
const manifest = {
  schema: PORTABLE_CHECKPOINT_CUSTODY_OBJECT_MANIFEST_SCHEMA_VERSION,
  objectRef: "checkpoint-custody.object.1",
  objectDigest: digest,
  artifactRef: "artifact.checkpoint.1",
  artifactDigest: digest,
  checkpointRef: "checkpoint.portable.1",
  checkpointDigest: digest,
  bundleDigest: digest,
  ciphertextDigest: digest,
  commandClaim: {
    schema: "openagents.portable_command_execution.v1" as const,
    claimRef: "claim.portable.1",
    commandRef: "command.portable.1",
    ownerRef: "owner.portable.1",
    sessionRef: "session.portable.1",
    commandKind: "move" as const,
    commandFingerprint: digest,
    claimFingerprint: digest,
    sourceAttachmentRef: "attachment.portable.source.1",
    sourceGeneration: 1,
    destinationTargetRef: "target.portable.destination.1",
    executorEnvironmentRef: "pylon.portable.source.1",
    workerInstanceRef: "worker.portable.1",
    claimGeneration: 1,
    leaseRevision: 1,
    state: "claimed" as const,
    claimedAt: "2026-07-20T12:00:00.000Z",
    leaseExpiresAt: "2026-07-20T12:10:00.000Z",
    updatedAt: "2026-07-20T12:00:00.000Z",
    terminalStatus: null,
    pendingReconcileRef: null,
    outcomeRef: null,
    evidenceRefs: [],
  },
  ownerRef: "owner.portable.1",
  sourcePylonRef: "pylon.portable.source.1",
  targetRef: "target.portable.destination.1",
  sessionRef: "session.portable.1",
  sourceAttachmentRef: "attachment.portable.source.1",
  sourceGeneration: 1,
  custodyPolicy: "owner_managed" as const,
  keyRef: "key.portable.1",
  byteLimit: 1_024,
  createdAt: "2026-07-20T12:00:00.000Z",
  expiresAt: "2026-07-20T12:10:00.000Z",
  retentionSeconds: 600,
  secretMaterial: "excluded" as const,
};

describe("PortableCheckpointCustodyObjectManifestSchema", () => {
  it("decodes a complete refs-only custody transport manifest", () => {
    expect(decodeManifest(manifest)).toEqual(manifest);
  });

  it("rejects secret material and an incomplete digest binding", () => {
    expect(() =>
      decodeManifest({
        ...manifest,
        secretMaterial: "included",
      }),
    ).toThrow();
    const { ciphertextDigest: _ciphertextDigest, ...incomplete } = manifest;
    expect(() => decodeManifest(incomplete)).toThrow();
  });
});

describe("checkpoint custody envelope versions", () => {
  const encryptedFields = {
    objectRef: "checkpoint-custody.object.1",
    keyRef: "key.portable.1",
    nonceBase64: Buffer.alloc(12).toString("base64"),
    authTagBase64: Buffer.alloc(16).toString("base64"),
    ciphertextBase64: Buffer.from("ciphertext").toString("base64"),
  };

  it("keeps v2 owner-managed and requires wrapped-DEK v3 for OpenAgents-managed", () => {
    expect(() =>
      decodeV2({
        ...encryptedFields,
        schema: "openagents.portable_checkpoint_artifact_custody_encrypted.v2",
        algorithm: "aes-256-gcm",
        policy: "openagents_managed",
      }),
    ).toThrow();
    expect(
      decodeV3({
        ...encryptedFields,
        schema: "openagents.portable_checkpoint_artifact_custody_encrypted.v3",
        algorithm: "aes-256-gcm+google-kms-wrapped-dek",
        policy: "openagents_managed",
        wrappedKeyBase64: Buffer.from("wrapped-dek").toString("base64"),
      }),
    ).toBeDefined();
  });
});
