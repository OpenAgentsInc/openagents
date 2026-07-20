import { createHash } from "node:crypto";

import type {
  PortableCheckpointCustodyObjectManifest,
  PortableCommandExecutionClaim,
  PortablePhaseOperationRecord,
} from "@openagentsinc/portable-session-contract";
import { describe, expect, test } from "vitest";

import {
  makePortableCheckpointArtifactRoutes,
  type PortableCheckpointArtifactRouteDependencies,
} from "./portable-checkpoint-artifact-routes";
import type {
  PortableCheckpointArtifactAuthority,
  PortableCheckpointArtifactBucket,
} from "./portable-checkpoint-artifact-service";

const now = "2026-07-20T13:00:00.000Z";
const ownerRef = "owner.ide13.1";
const agentRef = "agent.ide13.1";
const sourcePylonRef = "pylon.ide13.source.1";
const destinationPylonRef = "pylon.ide13.destination.1";
const sourceTargetRef = "target.ide13.source.1";
const destinationTargetRef = "target.ide13.destination.1";
const sourceOperationRef = "operation.ide13.checkpoint-create.1";
const stageOperationRef = "operation.ide13.checkpoint-stage.1";
const cleanupOperationRef = "operation.ide13.source-cleanup.1";

const digest = (bytes: Uint8Array | string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(bytes).digest("hex")}`;

const ciphertext = new TextEncoder().encode(
  JSON.stringify({
    schema: "openagents.portable_checkpoint_artifact_custody_encrypted.v2",
    ciphertextBase64: Buffer.from("opaque-checkpoint-ciphertext").toString("base64"),
  }),
);
const objectDigest = digest(ciphertext);
const ciphertextDigest = digest(Buffer.from("opaque-checkpoint-ciphertext"));
const fixedDigest = `sha256:${"a".repeat(64)}` as const;

const commandClaim: PortableCommandExecutionClaim = {
  schema: "openagents.portable_command_execution.v1",
  claimRef: "claim.ide13.command.1",
  commandRef: "command.ide13.move.1",
  ownerRef,
  sessionRef: "session.ide13.1",
  commandKind: "move",
  commandFingerprint: fixedDigest,
  claimFingerprint: `sha256:${"b".repeat(64)}`,
  sourceAttachmentRef: "attachment.ide13.source.1",
  sourceGeneration: 4,
  destinationTargetRef,
  executorEnvironmentRef: sourceTargetRef,
  workerInstanceRef: "worker.ide13.command.1",
  claimGeneration: 1,
  leaseRevision: 1,
  state: "claimed",
  claimedAt: "2026-07-20T12:59:00.000Z",
  leaseExpiresAt: "2026-07-20T13:10:00.000Z",
  updatedAt: "2026-07-20T12:59:00.000Z",
  terminalStatus: null,
  pendingReconcileRef: null,
  outcomeRef: null,
  evidenceRefs: [],
};

const manifest: PortableCheckpointCustodyObjectManifest = {
  schema: "openagents.portable_checkpoint_custody_object_manifest.v1",
  objectRef: "checkpoint-custody.object.1",
  objectDigest,
  artifactRef: "artifact.ide13.checkpoint.1",
  artifactDigest: fixedDigest,
  checkpointRef: "checkpoint.ide13.1",
  checkpointDigest: `sha256:${"c".repeat(64)}`,
  bundleDigest: `sha256:${"d".repeat(64)}`,
  ciphertextDigest,
  commandClaim,
  ownerRef,
  sourcePylonRef,
  targetRef: destinationTargetRef,
  sessionRef: commandClaim.sessionRef,
  sourceAttachmentRef: commandClaim.sourceAttachmentRef,
  sourceGeneration: commandClaim.sourceGeneration,
  custodyPolicy: "openagents_managed",
  keyRef: "key.ide13.managed.1",
  byteLimit: 4096,
  createdAt: "2026-07-20T12:59:00.000Z",
  expiresAt: "2026-07-20T13:10:00.000Z",
  retentionSeconds: 660,
  secretMaterial: "excluded",
};

const phase = (
  operationRef: string,
  kind: PortablePhaseOperationRecord["request"]["kind"],
  pylonRef: string,
  targetRef: string,
): PortablePhaseOperationRecord => {
  const source = kind === "checkpoint-create" || kind === "source-cleanup";
  const checkpoint =
    kind === "checkpoint-create"
      ? { checkpointRef: manifest.checkpointRef, checkpointObjectRef: null, checkpointDigest: null }
      : kind === "checkpoint-stage"
        ? {
            checkpointRef: manifest.checkpointRef,
            checkpointObjectRef: manifest.objectRef,
            checkpointDigest: manifest.checkpointDigest,
          }
        : { checkpointRef: null, checkpointObjectRef: null, checkpointDigest: null };
  return {
    request: {
      schema: "openagents.portable_phase_operation.v1",
      operationRef,
      commandRef: commandClaim.commandRef,
      commandExecutionClaimRef: commandClaim.claimRef,
      ownerRef,
      sessionRef: commandClaim.sessionRef,
      attachmentRef: source ? commandClaim.sourceAttachmentRef : "attachment.ide13.destination.1",
      attachmentGeneration: source
        ? commandClaim.sourceGeneration
        : commandClaim.sourceGeneration + 1,
      targetRef,
      pylonRef,
      kind,
      ...checkpoint,
      evidenceRefs: [],
      expiresAt: "2026-07-20T13:10:00.000Z",
    },
    requestFingerprint: fixedDigest,
    state: "claimed",
    claimRef: `claim.phase.${operationRef}`,
    claimFingerprint: fixedDigest,
    workerInstanceRef: `worker.phase.${operationRef}`,
    claimGeneration: 1,
    leaseRevision: 1,
    claimedAt: "2026-07-20T12:59:30.000Z",
    leaseExpiresAt: "2026-07-20T13:05:00.000Z",
    resultRef: null,
    resultFingerprint: null,
    resultStatus: null,
    resultCheckpointRef: null,
    resultCheckpointObjectRef: null,
    resultCheckpointDigest: null,
    resultCheckpointManifestDigest: null,
    resultDestinationActivationReceipt: null,
    resultEvidenceRefs: [],
    errorRef: null,
    completedAt: null,
    updatedAt: "2026-07-20T12:59:30.000Z",
  };
};

const sourceAuthority: PortableCheckpointArtifactAuthority = {
  operation: phase(sourceOperationRef, "checkpoint-create", sourcePylonRef, sourceTargetRef),
  commandClaim,
};
const stageAuthority: PortableCheckpointArtifactAuthority = {
  operation: phase(
    stageOperationRef,
    "checkpoint-stage",
    destinationPylonRef,
    destinationTargetRef,
  ),
  commandClaim,
};
const cleanupAuthority: PortableCheckpointArtifactAuthority = {
  operation: phase(cleanupOperationRef, "source-cleanup", sourcePylonRef, sourceTargetRef),
  commandClaim,
};

class MemoryBucket implements PortableCheckpointArtifactBucket {
  readonly values = new Map<string, Uint8Array>();
  readonly events: Array<string> = [];

  async get(key: string) {
    const bytes = this.values.get(key);
    return bytes === undefined
      ? null
      : { size: bytes.byteLength, bytes: async () => Uint8Array.from(bytes) };
  }

  async putIfAbsent(key: string, bytes: Uint8Array) {
    if (this.values.has(key)) return "exists" as const;
    this.events.push(`put:${key}`);
    this.values.set(key, Uint8Array.from(bytes));
    return "created" as const;
  }

  async delete(key: string) {
    this.events.push(`delete:${key}`);
    this.values.delete(key);
  }
}

const bearerRequest = (
  path: string,
  options: Readonly<{
    body?: unknown;
    bytes?: Uint8Array;
    method?: string;
    operationRef?: string;
  }> = {},
): Request =>
  new Request(`https://openagents.com${path}`, {
    method: options.method ?? "POST",
    headers: {
      authorization: "Bearer oa_agent_test",
      ...(options.bytes === undefined
        ? { "content-type": "application/json" }
        : { "content-type": "application/octet-stream" }),
      ...(options.operationRef === undefined
        ? {}
        : { "x-openagents-operation-ref": options.operationRef }),
    },
    ...(options.bytes !== undefined
      ? { body: Uint8Array.from(options.bytes).buffer }
      : options.body === undefined
        ? {}
        : { body: JSON.stringify(options.body) }),
  });

const setup = (
  options: Readonly<{
    registeredAgentRef?: string;
    command?: PortableCommandExecutionClaim;
  }> = {},
) => {
  const bucket = new MemoryBucket();
  const authorities = new Map<string, PortableCheckpointArtifactAuthority>([
    [sourceOperationRef, { ...sourceAuthority, commandClaim: options.command ?? commandClaim }],
    [stageOperationRef, { ...stageAuthority, commandClaim: options.command ?? commandClaim }],
    [cleanupOperationRef, { ...cleanupAuthority, commandClaim: options.command ?? commandClaim }],
  ]);
  const dependencies: PortableCheckpointArtifactRouteDependencies<object> = {
    authenticate: async () => ({ agentUserId: agentRef, ownerUserId: ownerRef }),
    readPylonOwnerAgentUserId: async () => options.registeredAgentRef ?? agentRef,
    resolveExactTarget: async () => "ready",
    bucket: () => bucket,
    readAuthority: async (_env, input) => {
      const authority = authorities.get(input.operationRef);
      if (authority === undefined) throw new Error("missing test authority");
      if (
        authority.operation.request.pylonRef !== input.pylonRef ||
        authority.operation.request.targetRef !== input.targetRef
      ) {
        throw new Error("cross-scope authority read");
      }
      return authority;
    },
    now: () => new Date(now),
  };
  return {
    bucket,
    route:
      makePortableCheckpointArtifactRoutes(dependencies).routePortableCheckpointArtifactRequest,
  };
};

const base = (pylonRef: string, targetRef: string) =>
  `/api/pylons/${pylonRef}/portable-targets/${targetRef}/checkpoint-artifacts`;

describe("portable checkpoint artifact routes (IDE-13)", () => {
  test("prepares, uploads, commits, redeems, and downloads an exact immutable object", async () => {
    const { bucket, route } = setup();
    const sourceBase = base(sourcePylonRef, sourceTargetRef);
    const preparedResponse = await route(
      bearerRequest(`${sourceBase}/prepare`, {
        body: { operationRef: sourceOperationRef, manifest },
      }),
      {},
    );
    expect(preparedResponse?.status).toBe(200);
    const prepared = (await preparedResponse?.json()) as {
      manifestDigest: string;
      upload: { path: string };
    };
    expect(prepared.upload.path).toContain("/checkpoint-artifacts/");
    expect([...bucket.values.keys()]).toContain(
      `portable-checkpoint-custody/v1/manifests/sha256/${prepared.manifestDigest.slice(7)}/prepared.json`,
    );

    expect(
      (
        await route(
          bearerRequest(prepared.upload.path, {
            bytes: ciphertext,
            method: "PUT",
            operationRef: sourceOperationRef,
          }),
          {},
        )
      )?.status,
    ).toBe(200);
    expect(
      (
        await route(
          bearerRequest(`${sourceBase}/${prepared.manifestDigest.slice(7)}/commit`, {
            body: { operationRef: sourceOperationRef },
          }),
          {},
        )
      )?.status,
    ).toBe(200);

    const destinationBase = base(destinationPylonRef, destinationTargetRef);
    const redeemResponse = await route(
      bearerRequest(`${destinationBase}/${prepared.manifestDigest.slice(7)}/redeem`, {
        body: { operationRef: stageOperationRef },
      }),
      {},
    );
    expect(redeemResponse?.status).toBe(200);
    const redeemed = (await redeemResponse?.json()) as {
      redemptionRef: string;
      download: { path: string };
      manifest: PortableCheckpointCustodyObjectManifest;
    };
    expect(redeemed.manifest).toEqual(manifest);
    const downloadResponse = await route(
      bearerRequest(redeemed.download.path, {
        body: { operationRef: stageOperationRef, redemptionRef: redeemed.redemptionRef },
      }),
      {},
    );
    expect(downloadResponse?.status).toBe(200);
    expect(new Uint8Array(await downloadResponse!.arrayBuffer())).toEqual(ciphertext);

    const objectKeys = [...bucket.values.keys()].filter((key) => key.includes("/objects/sha256/"));
    expect(objectKeys).toEqual([
      `portable-checkpoint-custody/v1/objects/sha256/${objectDigest.slice(7)}`,
    ]);
  });

  test("rejects cross-Pylon scope, stale command claims, oversized data, and replay mismatch", async () => {
    const wrongOwner = setup({ registeredAgentRef: "agent.ide13.other" });
    expect(
      (
        await wrongOwner.route(
          bearerRequest(`${base(sourcePylonRef, sourceTargetRef)}/prepare`, {
            body: { operationRef: sourceOperationRef, manifest },
          }),
          {},
        )
      )?.status,
    ).toBe(403);

    const stale = setup({
      command: { ...commandClaim, state: "terminal", terminalStatus: "failed" },
    });
    const staleResponse = await stale.route(
      bearerRequest(`${base(sourcePylonRef, sourceTargetRef)}/prepare`, {
        body: { operationRef: sourceOperationRef, manifest },
      }),
      {},
    );
    expect(staleResponse?.status).toBe(409);
    expect(await staleResponse?.json()).toMatchObject({ error: "checkpoint_artifact_stale_claim" });

    const swappedSourceBinding = setup();
    const swappedSourceResponse = await swappedSourceBinding.route(
      bearerRequest(`${base(sourcePylonRef, sourceTargetRef)}/prepare`, {
        body: {
          operationRef: sourceOperationRef,
          manifest: { ...manifest, sourcePylonRef: sourceTargetRef },
        },
      }),
      {},
    );
    expect(swappedSourceResponse?.status).toBe(400);
    expect(await swappedSourceResponse?.json()).toMatchObject({
      error: "checkpoint_artifact_invalid",
    });

    const active = setup();
    const preparedResponse = await active.route(
      bearerRequest(`${base(sourcePylonRef, sourceTargetRef)}/prepare`, {
        body: { operationRef: sourceOperationRef, manifest },
      }),
      {},
    );
    const prepared = (await preparedResponse?.json()) as { upload: { path: string } };
    const wrongOperation = await active.route(
      bearerRequest(prepared.upload.path, {
        bytes: ciphertext,
        method: "PUT",
        operationRef: cleanupOperationRef,
      }),
      {},
    );
    expect(wrongOperation?.status).toBe(409);

    const oversized = setup();
    const oversizedManifest = { ...manifest, byteLimit: 3 };
    const oversizedPreparedResponse = await oversized.route(
      bearerRequest(`${base(sourcePylonRef, sourceTargetRef)}/prepare`, {
        body: { operationRef: sourceOperationRef, manifest: oversizedManifest },
      }),
      {},
    );
    const oversizedPrepared = (await oversizedPreparedResponse?.json()) as {
      upload: { path: string };
    };
    expect(
      (
        await oversized.route(
          bearerRequest(oversizedPrepared.upload.path, {
            bytes: ciphertext,
            method: "PUT",
            operationRef: sourceOperationRef,
          }),
          {},
        )
      )?.status,
    ).toBe(400);

    const conflictingPrepare = await active.route(
      bearerRequest(`${base(sourcePylonRef, sourceTargetRef)}/prepare`, {
        body: {
          operationRef: sourceOperationRef,
          manifest: { ...manifest, artifactRef: "artifact.ide13.other" },
        },
      }),
      {},
    );
    expect(conflictingPrepare?.status).toBe(409);
  });

  test("stores a deletion tombstone before delete and verifies absence", async () => {
    const { bucket, route } = setup();
    const sourceBase = base(sourcePylonRef, sourceTargetRef);
    const preparedResponse = await route(
      bearerRequest(`${sourceBase}/prepare`, {
        body: { operationRef: sourceOperationRef, manifest },
      }),
      {},
    );
    const prepared = (await preparedResponse?.json()) as {
      manifestDigest: string;
      upload: { path: string };
    };
    await route(
      bearerRequest(prepared.upload.path, {
        bytes: ciphertext,
        method: "PUT",
        operationRef: sourceOperationRef,
      }),
      {},
    );
    await route(
      bearerRequest(`${sourceBase}/${prepared.manifestDigest.slice(7)}/commit`, {
        body: { operationRef: sourceOperationRef },
      }),
      {},
    );

    const deleted = await route(
      bearerRequest(`${sourceBase}/${prepared.manifestDigest.slice(7)}/delete`, {
        body: { operationRef: cleanupOperationRef },
      }),
      {},
    );
    expect(deleted?.status).toBe(200);
    expect(await deleted?.json()).toMatchObject({ state: "deleted", verifiedAbsent: true });
    const intentIndex = bucket.events.findIndex((event) => event.includes("deletion-intent.json"));
    const deleteIndex = bucket.events.findIndex(
      (event) => event.includes("delete:") && event.includes("/objects/"),
    );
    expect(intentIndex).toBeGreaterThanOrEqual(0);
    expect(deleteIndex).toBeGreaterThan(intentIndex);
    expect(
      bucket.values.has(`portable-checkpoint-custody/v1/objects/sha256/${objectDigest.slice(7)}`),
    ).toBe(false);
  });
});
