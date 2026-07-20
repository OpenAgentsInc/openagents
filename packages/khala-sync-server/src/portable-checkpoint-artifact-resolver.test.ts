import { createHash } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";
import {
  PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  type PortableCheckpointCustodyObjectManifest,
  type PortableCommandExecutionClaim,
} from "@openagentsinc/portable-session-contract";
import { describe, expect, test, vi } from "vite-plus/test";

import {
  PortableCommittedCheckpointArtifactResolver,
  PortableCommittedCheckpointArtifactError,
  type PortablePrivateCheckpointArtifactObject,
} from "./portable-checkpoint-artifact-resolver.js";
import { computePortableAgentGraphDigest } from "./portable-session-authority.js";
import {
  computePortableCheckpointDigest,
  type PortableCheckpointBundle,
} from "./portable-session-move.js";
import type { SyncSql } from "./sql.js";

const now = "2026-07-20T12:01:00.000Z";
const claimedAt = "2026-07-20T12:00:00.000Z";
const leaseExpiresAt = "2026-07-20T12:10:00.000Z";
const payloadExpiresAt = "2026-07-20T12:09:00.000Z";
const ownerRef = "owner.ide13.artifact";
const sessionRef = "session.ide13.artifact";
const sourceTargetRef = "target.ide13.artifact.source";
const sourcePylonRef = "pylon.ide13.artifact.source";
const destinationTargetRef = "target.ide13.artifact.destination";
const sourceAttachmentRef = "attachment.ide13.artifact.source";
const checkpointRef = "checkpoint.ide13.artifact";
const objectRef = "checkpoint-custody:ide13-artifact";
const artifactRef = "artifact.ide13.checkpoint";
const operationRef = "operation.ide13.checkpoint-create";
const resultRef = "result.ide13.checkpoint-create";

const sha256 = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;

const claim: PortableCommandExecutionClaim = {
  schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  claimRef: "claim.ide13.artifact",
  commandRef: "command.ide13.artifact",
  ownerRef,
  sessionRef,
  commandKind: "move",
  commandFingerprint: `sha256:${"1".repeat(64)}`,
  claimFingerprint: `sha256:${"2".repeat(64)}`,
  sourceAttachmentRef,
  sourceGeneration: 3,
  destinationTargetRef,
  executorEnvironmentRef: sourceTargetRef,
  workerInstanceRef: "worker.ide13.artifact",
  claimGeneration: 1,
  leaseRevision: 1,
  state: "claimed",
  claimedAt,
  leaseExpiresAt,
  updatedAt: claimedAt,
  terminalStatus: null,
  pendingReconcileRef: null,
  outcomeRef: null,
  evidenceRefs: [],
};

const graph: PortableCheckpointBundle["graph"] = {
  rootAgentRef: "agent.ide13.artifact.root",
  nodes: [
    {
      agentRef: "agent.ide13.artifact.root",
      threadRef: "thread.ide13.artifact.root",
      transcriptRef: "transcript.ide13.artifact.root",
      activityCursor: 5,
      lifecycle: "waiting",
      attachmentGeneration: 3,
    },
  ],
};

const checkpointPayload = {
  schema: "openagents.portable_checkpoint.v1" as const,
  checkpointRef,
  sessionRef,
  sourceAttachmentRef,
  sourceGeneration: 3,
  repositoryRef: "repository.ide13.artifact",
  repositoryRevisionRef: "commit.ide13.artifact",
  repositoryPostImageDigest: `sha256:${"3".repeat(64)}`,
  diffDigest: `sha256:${"4".repeat(64)}`,
  eventLogCursor: 8,
  catalogGenerationRef: "catalog.ide13.artifact",
  graphDigest: computePortableAgentGraphDigest(graph),
  approvalRefs: [],
  artifactRefs: [],
  receiptRefs: [],
  secretMaterial: "excluded" as const,
  processState: "excluded" as const,
};

const bundle: PortableCheckpointBundle = {
  checkpoint: {
    ...checkpointPayload,
    digest: computePortableCheckpointDigest(checkpointPayload),
  },
  executionBinding: {
    schema: "openagents.portable_session_execution_binding.v1",
    sessionRef,
    ownerRef,
    runRef: "run.ide13.artifact",
    repositoryRef: "repository.ide13.artifact",
    pinnedBaseRef: "commit.ide13.artifact.base",
  },
  graph,
  threadCursors: [
    {
      threadRef: "thread.ide13.artifact.root",
      transcriptRef: "transcript.ide13.artifact.root",
      activityCursor: 5,
      eventCursor: 8,
    },
  ],
};

const artifactBytes = new TextEncoder().encode("private-checkpoint-archive-fixture");
const ciphertext = new TextEncoder().encode("opaque-ciphertext-fixture");
const envelope = {
  schema: "openagents.portable_checkpoint_artifact_custody_encrypted.v2" as const,
  algorithm: "aes-256-gcm" as const,
  objectRef,
  policy: "openagents_managed" as const,
  keyRef: "key.ide13.artifact",
  nonceBase64: Buffer.from("123456789012").toString("base64"),
  authTagBase64: Buffer.from("1234567890123456").toString("base64"),
  ciphertextBase64: Buffer.from(ciphertext).toString("base64"),
};
const encryptedObjectBytes = new TextEncoder().encode(canonicalJson(envelope));
const payloadBytes = new TextEncoder().encode(
  canonicalJson({
    schema: "openagents.portable_checkpoint_artifact_custody_payload.v3",
    checkpointRef,
    artifactRef,
    digest: sha256(artifactBytes),
    bundle,
    createdAt: claimedAt,
    expiresAt: payloadExpiresAt,
    bytesBase64: Buffer.from(artifactBytes).toString("base64"),
  }),
);

const manifest: PortableCheckpointCustodyObjectManifest = {
  schema: "openagents.portable_checkpoint_custody_object_manifest.v1",
  objectRef,
  objectDigest: sha256(encryptedObjectBytes),
  artifactRef,
  artifactDigest: sha256(artifactBytes),
  checkpointRef,
  checkpointDigest: bundle.checkpoint.digest,
  bundleDigest: sha256(canonicalJson(bundle)),
  ciphertextDigest: sha256(ciphertext),
  commandClaim: claim,
  ownerRef,
  sourcePylonRef,
  targetRef: destinationTargetRef,
  sessionRef,
  sourceAttachmentRef,
  sourceGeneration: 3,
  custodyPolicy: "openagents_managed",
  keyRef: "key.ide13.artifact",
  byteLimit: 1024 * 1024,
  createdAt: claimedAt,
  expiresAt: payloadExpiresAt,
  retentionSeconds: 540,
  secretMaterial: "excluded",
};

const sql = Object.assign(
  async (strings: TemplateStringsArray): Promise<ReadonlyArray<Record<string, unknown>>> => {
    const query = strings.join("?");
    if (query.includes("khala_sync_portable_command_executions")) return [{ claim_json: claim }];
    if (query.includes("khala_sync_portable_phase_operations")) {
      return [
        {
          operation_ref: operationRef,
          command_execution_claim_ref: claim.claimRef,
          owner_user_id: ownerRef,
          session_ref: sessionRef,
          attachment_ref: sourceAttachmentRef,
          attachment_generation: 3,
          target_ref: sourceTargetRef,
          pylon_ref: sourcePylonRef,
          checkpoint_ref: checkpointRef,
          expires_at: leaseExpiresAt,
          state: "completed",
          result_ref: resultRef,
          result_status: "completed",
          result_checkpoint_ref: checkpointRef,
          result_checkpoint_object_ref: objectRef,
          result_checkpoint_digest: bundle.checkpoint.digest,
          completed_at: now,
        },
      ];
    }
    throw new Error("unexpected SQL");
  },
  {
    begin: async () => {
      throw new Error("transaction is not used");
    },
  },
) as SyncSql;

const scope = {
  commandExecutionClaimRef: claim.claimRef,
  ownerRef,
  sessionRef,
  artifact: {
    checkpointRef,
    checkpointObjectRef: objectRef,
    checkpointDigest: bundle.checkpoint.digest,
  },
};

const privateObject = (
  overrides: Partial<PortablePrivateCheckpointArtifactObject> = {},
): PortablePrivateCheckpointArtifactObject => ({
  state: "committed",
  tombstoned: false,
  phaseOperationRef: operationRef,
  phaseResultRef: resultRef,
  manifest,
  encryptedObjectBytes: Uint8Array.from(encryptedObjectBytes),
  ...overrides,
});

describe("Sync committed checkpoint artifact resolver", () => {
  test("resolves a committed exact artifact through admitted custody and is retry-stable", async () => {
    const issuedEncrypted: Uint8Array[] = [];
    const issuedPlaintext: Uint8Array[] = [];
    const read = vi.fn(async () => {
      const object = privateObject();
      issuedEncrypted.push(object.encryptedObjectBytes);
      return object;
    });
    const decrypt = vi.fn(async () => {
      const plaintext = Uint8Array.from(payloadBytes);
      issuedPlaintext.push(plaintext);
      return plaintext;
    });
    const resolver = new PortableCommittedCheckpointArtifactResolver({
      sql,
      objects: { read },
      custody: { decrypt },
      now: () => now,
    });

    expect(await resolver.resolve(scope)).toEqual(bundle);
    expect(await resolver.resolve(scope)).toEqual(bundle);
    expect(read).toHaveBeenCalledTimes(2);
    expect(decrypt).toHaveBeenCalledTimes(2);
    expect(issuedEncrypted.every((bytes) => bytes.every((byte) => byte === 0))).toBe(true);
    expect(issuedPlaintext.every((bytes) => bytes.every((byte) => byte === 0))).toBe(true);
  });

  test.each([
    ["pending object", { state: "pending" as const }, "artifact_unavailable"],
    ["tombstoned object", { tombstoned: true }, "artifact_tombstoned"],
    ["foreign phase result", { phaseResultRef: "result.ide13.foreign" }, "manifest_mismatch"],
    [
      "source target substituted for source Pylon",
      {
        manifest: { ...manifest, sourcePylonRef: sourceTargetRef },
      },
      "manifest_mismatch",
    ],
    [
      "changed ciphertext",
      {
        encryptedObjectBytes: new TextEncoder().encode(
          canonicalJson({ ...envelope, ciphertextBase64: "YQ==" }),
        ),
      },
      "manifest_mismatch",
    ],
  ])("fails closed for a %s", async (_label, override, code) => {
    const resolver = new PortableCommittedCheckpointArtifactResolver({
      sql,
      objects: { read: async () => privateObject(override) },
      custody: { decrypt: async () => Uint8Array.from(payloadBytes) },
      now: () => now,
    });
    await expect(resolver.resolve(scope)).rejects.toMatchObject({ code });
  });

  test("returns the same public-safe failure ref when custody is unavailable", async () => {
    const resolver = new PortableCommittedCheckpointArtifactResolver({
      sql,
      objects: { read: async () => privateObject() },
      custody: {
        decrypt: async () => {
          throw new Error("private provider failure");
        },
      },
      now: () => now,
    });
    const failures = await Promise.all([
      resolver.resolve(scope).catch((error: unknown) => error),
      resolver.resolve(scope).catch((error: unknown) => error),
    ]);
    expect(failures).toEqual([
      expect.objectContaining({ code: "custody_unavailable", failureRef: expect.any(String) }),
      expect.objectContaining({ code: "custody_unavailable", failureRef: expect.any(String) }),
    ]);
    const failureRefs = failures.map((failure) => {
      if (!(failure instanceof PortableCommittedCheckpointArtifactError)) {
        throw new Error("expected a committed checkpoint artifact error");
      }
      return failure.failureRef;
    });
    expect(failureRefs[0]).toBe(failureRefs[1]);
    expect(JSON.stringify(failures)).not.toContain("private provider failure");
  });
});
