import { Schema } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  PORTABLE_CHECKPOINT_CUSTODY_OBJECT_MANIFEST_SCHEMA_VERSION,
  PortableCheckpointCustodyObjectManifestSchema,
} from "./checkpoint-custody-transport.js";

const digest = `sha256:${"a".repeat(64)}` as const;
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
    expect(
      Schema.decodeUnknownSync(PortableCheckpointCustodyObjectManifestSchema)(manifest),
    ).toEqual(manifest);
  });

  it("rejects secret material and an incomplete digest binding", () => {
    expect(() =>
      Schema.decodeUnknownSync(PortableCheckpointCustodyObjectManifestSchema)({
        ...manifest,
        secretMaterial: "included",
      }),
    ).toThrow();
    const { ciphertextDigest: _ciphertextDigest, ...incomplete } = manifest;
    expect(() =>
      Schema.decodeUnknownSync(PortableCheckpointCustodyObjectManifestSchema)(incomplete),
    ).toThrow();
  });
});
