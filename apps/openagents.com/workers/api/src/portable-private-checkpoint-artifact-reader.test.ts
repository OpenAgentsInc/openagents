import { createHash } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";
import type { SyncSql } from "@openagentsinc/khala-sync-server";
import {
  PortableCheckpointCustodyObjectManifestSchema,
  type PortableCheckpointCustodyObjectManifest,
  type PortableCommandExecutionClaim,
} from "@openagentsinc/portable-session-contract";
import { Schema } from "effect";
import { describe, expect, test } from "vitest";

import {
  makePortableCheckpointArtifactService,
  type PortableCheckpointArtifactBucket,
} from "./portable-checkpoint-artifact-service";
import { makePortablePrivateCheckpointArtifactReader } from "./portable-private-checkpoint-artifact-reader";

const encoder = new TextEncoder();
const ownerRef = "owner.ide13.private-reader";
const sessionRef = "session.ide13.private-reader";
const sourcePylonRef = "pylon.ide13.private-reader.source";
const sourceTargetRef = "target.ide13.private-reader.source";
const destinationTargetRef = "target.ide13.private-reader.destination";
const objectRef = "checkpoint-custody:ide13-private-reader";
const operationRef = "operation.ide13.private-reader.create";
const resultRef = "result.ide13.private-reader.create";
const phaseClaimRef = "claim.ide13.private-reader.phase";
const now = "2026-07-20T12:00:00.000Z";
const expiresAt = "2026-07-20T12:10:00.000Z";

const digest = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;
const orderedEntries = (
  value: Readonly<Record<string, unknown>>,
): ReadonlyArray<readonly [string, unknown]> => {
  const result: Array<readonly [string, unknown]> = [];
  for (const entry of Object.entries(value)) {
    const index = result.findIndex(([key]) => entry[0].localeCompare(key) < 0);
    if (index === -1) result.push(entry);
    else result.splice(index, 0, entry);
  }
  return result;
};
const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${orderedEntries(value as Readonly<Record<string, unknown>>)
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
};

const claim: PortableCommandExecutionClaim = {
  schema: "openagents.portable_command_execution.v1",
  claimRef: "claim.ide13.private-reader.command",
  commandRef: "command.ide13.private-reader.move",
  ownerRef,
  sessionRef,
  commandKind: "move",
  commandFingerprint: `sha256:${"a".repeat(64)}`,
  claimFingerprint: `sha256:${"b".repeat(64)}`,
  sourceAttachmentRef: "attachment.ide13.private-reader.source",
  sourceGeneration: 2,
  destinationTargetRef,
  executorEnvironmentRef: sourceTargetRef,
  workerInstanceRef: "worker.ide13.private-reader.command",
  claimGeneration: 1,
  leaseRevision: 1,
  state: "claimed",
  claimedAt: now,
  leaseExpiresAt: expiresAt,
  updatedAt: now,
  terminalStatus: null,
  pendingReconcileRef: null,
  outcomeRef: null,
  evidenceRefs: [],
};

const ciphertext = encoder.encode("opaque-private-checkpoint-ciphertext");
const encryptedObjectBytes = encoder.encode(
  canonicalJson({
    schema: "openagents.portable_checkpoint_artifact_custody_encrypted.v2",
    ciphertextBase64: Buffer.from(ciphertext).toString("base64"),
  }),
);

const manifest: PortableCheckpointCustodyObjectManifest = {
  schema: "openagents.portable_checkpoint_custody_object_manifest.v1",
  objectRef,
  objectDigest: digest(encryptedObjectBytes),
  artifactRef: "artifact.ide13.private-reader",
  artifactDigest: `sha256:${"c".repeat(64)}`,
  checkpointRef: "checkpoint.ide13.private-reader",
  checkpointDigest: `sha256:${"d".repeat(64)}`,
  bundleDigest: `sha256:${"e".repeat(64)}`,
  ciphertextDigest: digest(ciphertext),
  commandClaim: claim,
  ownerRef,
  sourcePylonRef,
  targetRef: destinationTargetRef,
  sessionRef,
  sourceAttachmentRef: claim.sourceAttachmentRef,
  sourceGeneration: claim.sourceGeneration,
  custodyPolicy: "openagents_managed",
  keyRef: "key.ide13.private-reader",
  byteLimit: 4096,
  createdAt: now,
  expiresAt,
  retentionSeconds: 600,
  secretMaterial: "excluded",
};
const decodedManifest = Schema.decodeUnknownSync(PortableCheckpointCustodyObjectManifestSchema)(
  manifest,
);
const manifestDigest = digest(canonical(decodedManifest));
const prefix = "portable-checkpoint-custody/v1/";
const manifestRoot = `${prefix}manifests/sha256/${manifestDigest.slice(7)}/`;
const preparedKey = `${manifestRoot}prepared.json`;
const committedKey = `${manifestRoot}committed.json`;
const deletionIntentKey = `${manifestRoot}deletion-intent.json`;
const deletionReceiptKey = `${manifestRoot}deleted.json`;
const objectKey = `${prefix}objects/sha256/${manifest.objectDigest.slice(7)}`;

class MemoryBucket implements PortableCheckpointArtifactBucket {
  readonly values = new Map<string, Readonly<{ bytes: Uint8Array; size: number }>>();

  async get(key: string) {
    const value = this.values.get(key);
    return value === undefined
      ? null
      : { size: value.size, bytes: async () => Uint8Array.from(value.bytes) };
  }

  async putIfAbsent(key: string, bytes: Uint8Array) {
    if (this.values.has(key)) return "exists" as const;
    this.values.set(key, { bytes: Uint8Array.from(bytes), size: bytes.byteLength });
    return "created" as const;
  }

  async delete(key: string) {
    this.values.delete(key);
  }

  seedJson(key: string, value: unknown) {
    const bytes = encoder.encode(canonicalJson(value));
    this.values.set(key, { bytes, size: bytes.byteLength });
  }

  seedBytes(key: string, bytes: Uint8Array, size = bytes.byteLength) {
    this.values.set(key, { bytes: Uint8Array.from(bytes), size });
  }
}

const phaseRow = (overrides: Record<string, unknown> = {}) => ({
  operation_ref: operationRef,
  state: "completed",
  result_ref: resultRef,
  result_status: "completed",
  result_checkpoint_object_ref: objectRef,
  result_checkpoint_manifest_digest: manifestDigest,
  ...overrides,
});

const sqlWith = (rows: ReadonlyArray<unknown>): SyncSql =>
  Object.assign(async () => rows, {
    begin: async () => {
      throw new Error("transaction is not used");
    },
  }) as SyncSql;

const seedCommitted = (
  options: Readonly<{
    manifest?: PortableCheckpointCustodyObjectManifest;
    objectBytes?: Uint8Array;
    objectSize?: number;
  }> = {},
) => {
  const bucket = new MemoryBucket();
  const storedManifest = options.manifest ?? manifest;
  bucket.seedJson(preparedKey, {
    schema: "openagents.portable_checkpoint_artifact_prepare.v1",
    state: "prepared",
    manifestDigest,
    manifest: storedManifest,
    operationRef,
    phaseClaimRef,
    phaseClaimGeneration: 1,
    phaseLeaseRevision: 1,
    pylonRef: sourcePylonRef,
    targetRef: sourceTargetRef,
    preparedAt: now,
  });
  bucket.seedJson(committedKey, {
    schema: "openagents.portable_checkpoint_artifact_commit.v1",
    state: "committed",
    manifestDigest,
    objectDigest: storedManifest.objectDigest,
    operationRef,
    phaseClaimRef,
    phaseClaimGeneration: 1,
    phaseLeaseRevision: 1,
    verifiedAt: now,
  });
  const bytes = options.objectBytes ?? encryptedObjectBytes;
  bucket.seedBytes(objectKey, bytes, options.objectSize);
  const artifacts = makePortableCheckpointArtifactService({
    bucket,
    readAuthority: async () => {
      throw new Error("authority read is not used by the private reader");
    },
  });
  return { artifacts, bucket };
};

describe("portable private checkpoint artifact reader", () => {
  test("returns newly owned encrypted bytes for one exact committed phase result", async () => {
    const { artifacts, bucket } = seedCommitted();
    const reader = makePortablePrivateCheckpointArtifactReader({
      sql: sqlWith([phaseRow()]),
      artifacts,
    });

    const result = await reader.read(objectRef);
    expect(result).toMatchObject({
      state: "committed",
      tombstoned: false,
      phaseOperationRef: operationRef,
      phaseResultRef: resultRef,
      manifest,
    });
    expect(result.phaseOperationRef).not.toBe(result.phaseResultRef);
    expect(result.encryptedObjectBytes).toEqual(encryptedObjectBytes);
    result.encryptedObjectBytes.fill(0);
    expect(bucket.values.get(objectKey)?.bytes).toEqual(encryptedObjectBytes);
  });

  test.each([
    [
      "pending object",
      () => seedCommitted(),
      (bucket: MemoryBucket) => bucket.values.delete(committedKey),
      "commit",
    ],
    [
      "delete-pending tombstone",
      () => seedCommitted(),
      (bucket: MemoryBucket) =>
        bucket.seedJson(deletionIntentKey, {
          schema: "openagents.portable_checkpoint_artifact_deletion_intent.v1",
          state: "delete_pending",
          manifestDigest,
          objectDigest: manifest.objectDigest,
          operationRef: "operation.ide13.private-reader.delete",
          phaseClaimRef,
          phaseClaimGeneration: 1,
          phaseLeaseRevision: 1,
          pylonRef: sourcePylonRef,
          targetRef: sourceTargetRef,
          recordedAt: now,
        }),
      "private_object_tombstoned",
    ],
    [
      "deleted tombstone",
      () => seedCommitted(),
      (bucket: MemoryBucket) =>
        bucket.seedJson(deletionReceiptKey, {
          schema: "openagents.portable_checkpoint_artifact_deletion_receipt.v1",
          state: "deleted",
          manifestDigest,
          objectDigest: manifest.objectDigest,
          operationRef: "operation.ide13.private-reader.delete",
          verifiedAbsent: true,
          deletedAt: now,
        }),
      "private_object_tombstoned",
    ],
    [
      "mismatched content",
      () => seedCommitted({ objectBytes: encoder.encode("different-encrypted-content") }),
      () => undefined,
      "object_digest",
    ],
    [
      "mismatched manifest",
      () => seedCommitted({ manifest: { ...manifest, objectRef: "checkpoint-custody:foreign" } }),
      () => undefined,
      "private_manifest_binding",
    ],
    [
      "oversized object",
      () => seedCommitted({ objectSize: manifest.byteLimit + 1 }),
      () => undefined,
      "object_read",
    ],
  ])("rejects a %s", async (_label, setup, mutate, operation) => {
    const { artifacts, bucket } = setup();
    mutate(bucket);
    const reader = makePortablePrivateCheckpointArtifactReader({
      sql: sqlWith([phaseRow()]),
      artifacts,
    });
    await expect(reader.read(objectRef)).rejects.toMatchObject({ operation });
  });

  test("rejects ambiguous and mismatched phase bindings", async () => {
    const { artifacts } = seedCommitted();
    const ambiguous = makePortablePrivateCheckpointArtifactReader({
      sql: sqlWith([phaseRow(), phaseRow({ result_ref: "result.ide13.private-reader.other" })]),
      artifacts,
    });
    await expect(ambiguous.read(objectRef)).rejects.toMatchObject({
      operation: "private_phase_binding",
    });

    const wrongOperation = makePortablePrivateCheckpointArtifactReader({
      sql: sqlWith([phaseRow({ operation_ref: "operation.ide13.private-reader.foreign" })]),
      artifacts,
    });
    await expect(wrongOperation.read(objectRef)).rejects.toMatchObject({
      operation: "private_phase_operation_binding",
    });
  });
});
