import { createHash } from "node:crypto";

import type { PortableCheckpointCustodyObjectManifest } from "@openagentsinc/portable-session-contract";
import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  makePylonPortableCheckpointArtifactClient,
  PylonPortableCheckpointArtifactTransportError,
} from "./portable-checkpoint-artifact-client.js";

const pylonRef = "pylon.ide13.source.1";
const targetRef = "target.ide13.source.1";
const operationRef = "operation.ide13.checkpoint.1";
const now = "2026-07-20T13:00:00.000Z";
const digest = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;
const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value as Readonly<Record<string, unknown>>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
};

const bytes = new TextEncoder().encode(
  JSON.stringify({ ciphertextBase64: Buffer.from("opaque").toString("base64") }),
);
const fixedDigest = `sha256:${"a".repeat(64)}` as const;
const manifest: PortableCheckpointCustodyObjectManifest = {
  schema: "openagents.portable_checkpoint_custody_object_manifest.v1",
  objectRef: "checkpoint-custody.object.1",
  objectDigest: digest(bytes),
  artifactRef: "artifact.ide13.1",
  artifactDigest: fixedDigest,
  checkpointRef: "checkpoint.ide13.1",
  checkpointDigest: `sha256:${"b".repeat(64)}`,
  bundleDigest: `sha256:${"c".repeat(64)}`,
  ciphertextDigest: digest("opaque"),
  commandClaim: {
    schema: "openagents.portable_command_execution.v1",
    claimRef: "claim.ide13.command.1",
    commandRef: "command.ide13.move.1",
    ownerRef: "owner.ide13.1",
    sessionRef: "session.ide13.1",
    commandKind: "move",
    commandFingerprint: fixedDigest,
    claimFingerprint: `sha256:${"d".repeat(64)}`,
    sourceAttachmentRef: "attachment.ide13.source.1",
    sourceGeneration: 1,
    destinationTargetRef: "target.ide13.destination.1",
    executorEnvironmentRef: targetRef,
    workerInstanceRef: "worker.ide13.1",
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
  },
  ownerRef: "owner.ide13.1",
  sourcePylonRef: targetRef,
  targetRef: "target.ide13.destination.1",
  sessionRef: "session.ide13.1",
  sourceAttachmentRef: "attachment.ide13.source.1",
  sourceGeneration: 1,
  custodyPolicy: "openagents_managed",
  keyRef: "key.ide13.1",
  byteLimit: 4096,
  createdAt: "2026-07-20T12:59:00.000Z",
  expiresAt: "2026-07-20T13:10:00.000Z",
  retentionSeconds: 660,
  secretMaterial: "excluded",
};
const manifestDigest = digest(canonical(manifest));

describe("Pylon portable checkpoint artifact client", () => {
  test("publishes and redeems exact bytes without placing the token in a URL", async () => {
    const requests: Array<Request> = [];
    const fetchImpl: typeof fetch = async (input, init) => {
      const request = new Request(input, init);
      requests.push(request.clone());
      const url = new URL(request.url);
      expect(url.search).toBe("");
      expect(url.username).toBe("");
      expect(url.password).toBe("");
      expect(request.headers.get("authorization")).toBe("Bearer private-token");
      if (url.pathname.endsWith("/prepare")) {
        const requestRoute = url.pathname.replace(/\/prepare$/u, "");
        return Response.json({
          schema: "openagents.portable_checkpoint_artifact_transport.v1",
          status: "prepared",
          manifestDigest,
          objectDigest: manifest.objectDigest,
          byteLimit: manifest.byteLimit,
          expiresAt: manifest.expiresAt,
          upload: {
            transport: "server_mediated",
            method: "PUT",
            path: `${requestRoute}/${manifestDigest.slice(7)}/upload`,
            contentType: "application/octet-stream",
            operationRefHeader: "x-openagents-operation-ref",
            expiresAt: manifest.expiresAt,
          },
        });
      }
      if (url.pathname.endsWith("/upload")) {
        expect(request.headers.get("x-openagents-operation-ref")).toBe(operationRef);
        expect(new Uint8Array(await request.arrayBuffer())).toEqual(bytes);
        return Response.json({
          schema: "openagents.portable_checkpoint_artifact_transport.v1",
          status: "uploaded",
          manifestDigest,
          objectDigest: manifest.objectDigest,
        });
      }
      if (url.pathname.endsWith("/commit")) {
        return Response.json({
          schema: "openagents.portable_checkpoint_artifact_transport.v1",
          status: "committed",
          manifestDigest,
          objectDigest: manifest.objectDigest,
        });
      }
      if (url.pathname.endsWith("/redeem")) {
        const requestRoute = url.pathname.replace(/\/[a-f0-9]{64}\/redeem$/u, "");
        return Response.json({
          schema: "openagents.portable_checkpoint_artifact_transport.v1",
          status: "redeemed",
          redemptionRef: "redemption.ide13.1",
          expiresAt: "2026-07-20T13:05:00.000Z",
          manifest,
          download: {
            transport: "server_mediated",
            method: "POST",
            path: `${requestRoute}/${manifestDigest.slice(7)}/download`,
          },
        });
      }
      if (url.pathname.endsWith("/download")) {
        return new Response(Uint8Array.from(bytes).buffer, {
          headers: { "content-length": String(bytes.byteLength) },
        });
      }
      throw new Error("unexpected request");
    };
    const sourceClient = makePylonPortableCheckpointArtifactClient({
      agentToken: "private-token",
      baseUrl: "https://openagents.test",
      pylonRef,
      targetRef,
      fetchImpl,
      now: () => new Date(now),
    });

    await expect(
      Effect.runPromise(sourceClient.publish({ operationRef, manifest, bytes })),
    ).resolves.toEqual({ manifestDigest });
    await expect(
      Effect.runPromise(sourceClient.publish({ operationRef, manifest, bytes })),
    ).resolves.toEqual({ manifestDigest });
    const destinationClient = makePylonPortableCheckpointArtifactClient({
      agentToken: "private-token",
      baseUrl: "https://openagents.test",
      pylonRef: "pylon.ide13.destination.1",
      targetRef: manifest.targetRef,
      fetchImpl,
      now: () => new Date(now),
    });
    await expect(
      Effect.runPromise(
        destinationClient.redeem({
          operationRef,
          manifestDigest,
          checkpointObjectRef: manifest.objectRef,
          checkpointDigest: manifest.checkpointDigest,
          commandClaimRef: manifest.commandClaim.claimRef,
        }),
      ),
    ).resolves.toEqual({ manifest, bytes });
    expect(requests).toHaveLength(8);
    expect(requests.every(request => !request.url.includes("private-token"))).toBe(true);
  });

  test("fails closed on expiry and manifest digest drift", async () => {
    const expired = makePylonPortableCheckpointArtifactClient({
      agentToken: "private-token",
      baseUrl: "https://openagents.test",
      pylonRef,
      targetRef,
      fetchImpl: async () => {
        throw new Error("must not fetch");
      },
      now: () => new Date("2026-07-20T13:11:00.000Z"),
    });
    await expect(
      Effect.runPromise(expired.publish({ operationRef, manifest, bytes })),
    ).rejects.toEqual(new PylonPortableCheckpointArtifactTransportError({ reason: "expired" }));

    const drift = makePylonPortableCheckpointArtifactClient({
      agentToken: "private-token",
      baseUrl: "https://openagents.test",
      pylonRef,
      targetRef,
      fetchImpl: async () =>
        Response.json({
          schema: "openagents.portable_checkpoint_artifact_transport.v1",
          status: "prepared",
          manifestDigest: fixedDigest,
          objectDigest: manifest.objectDigest,
          byteLimit: manifest.byteLimit,
          expiresAt: manifest.expiresAt,
          upload: {
            transport: "server_mediated",
            method: "PUT",
            path: `/wrong`,
            contentType: "application/octet-stream",
            operationRefHeader: "x-openagents-operation-ref",
            expiresAt: manifest.expiresAt,
          },
        }),
      now: () => new Date(now),
    });
    await expect(
      Effect.runPromise(drift.publish({ operationRef, manifest, bytes })),
    ).rejects.toEqual(new PylonPortableCheckpointArtifactTransportError({ reason: "bad_response" }));
  });
});
