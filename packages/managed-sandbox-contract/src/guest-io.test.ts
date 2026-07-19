import { Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  ManagedSandboxGuestIoRequestSchema,
  ManagedSandboxGuestIoResponseSchema,
} from "./guest-io.ts";

const limits = {
  workspaceRootRef: "workspace.managed-sandbox",
  maxFileBytes: 1_048_576,
  maxArtifactBytes: 10_000_000,
  maxOutputBytes: 131_072,
  maxDurationMillis: 60_000,
  maxCpuMillis: 60_000,
  maxProcesses: 32,
  maxNetworkBytes: 0,
  networkPolicyRef: "network-policy.managed-sandbox.deny-all",
};

const common = {
  schemaVersion: "openagents.managed_sandbox_guest_io.v1" as const,
  operationRef: "operation.sbx05.1",
  idempotencyRef: "idempotency.sbx05.1",
  actorRef: "agent.sbx05",
  ownerRef: "owner.sbx05",
  tenantRef: "tenant.sbx05",
  programRef: "program.managed_agent_sandboxes",
  workUnitRef: "work.sbx05",
  sandboxRef: "sandbox.sbx05",
  resourceGeneration: 2,
  capabilityRef: "capability.sbx05.file",
  capabilityState: "active" as const,
  capabilityExpiresAt: "2026-07-19T21:00:00.000Z",
  requestedAt: "2026-07-19T20:00:00.000Z",
  limits,
};

test("decodes the closed guest I/O request vocabulary", () => {
  const decode = S.decodeUnknownSync(ManagedSandboxGuestIoRequestSchema);
  expect(
    decode({ ...common, action: "read_file", path: "workspace/a.txt", encoding: "utf8" }),
  ).toMatchObject({ action: "read_file", resourceGeneration: 2 });
  expect(
    decode({
      ...common,
      action: "write_file",
      path: "workspace/a.txt",
      encoding: "utf8",
      content: "hello",
      contentDigest: `sha256:${"a".repeat(64)}`,
    }),
  ).toMatchObject({ action: "write_file", content: "hello" });
  expect(
    decode({
      ...common,
      action: "execute_command",
      command: "pwd",
      commandDigest: `sha256:${"b".repeat(64)}`,
      cwd: "workspace",
      timeoutMillis: 1_000,
    }),
  ).toMatchObject({ action: "execute_command", timeoutMillis: 1_000 });
  expect(
    decode({
      ...common,
      action: "read_artifact",
      path: "workspace/a.txt",
      retentionUntil: "2026-07-20T20:00:00.000Z",
    }),
  ).toMatchObject({ action: "read_artifact" });
});

describe("guest I/O response receipts", () => {
  test("bind exact scope, cleanup, confinement, and artifact provenance", () => {
    const receipt = {
      schemaVersion: "openagents.managed_sandbox_guest_io_receipt.v1" as const,
      receiptRef: "receipt.sbx05.artifact",
      operationRef: common.operationRef,
      sandboxRef: common.sandboxRef,
      resourceGeneration: common.resourceGeneration,
      capabilityRef: common.capabilityRef,
      action: "read_artifact" as const,
      outcome: "succeeded" as const,
      pathDigest: `sha256:${"c".repeat(64)}`,
      startedAt: common.requestedAt,
      finishedAt: "2026-07-19T20:00:01.000Z",
      bytesRead: 5,
      bytesWritten: 0,
      cpuMillis: 1,
      networkBytes: 0,
      processTerminated: true,
      descendantsRemaining: 0,
      scratchCleaned: true,
      ingressClosed: true,
      egressDenied: true,
      pathPolicy: "resolved_beneath_workspace_root" as const,
      symlinkTraversal: false as const,
      secretScan: "clean" as const,
      evidenceRefs: ["evidence.sbx05.artifact"],
    };
    const response = S.decodeUnknownSync(ManagedSandboxGuestIoResponseSchema)({
      schemaVersion: common.schemaVersion,
      action: "read_artifact",
      operationRef: common.operationRef,
      sandboxRef: common.sandboxRef,
      resourceGeneration: common.resourceGeneration,
      contentBase64: "aGVsbG8=",
      receipt,
      artifact: {
        schemaVersion: "openagents.managed_sandbox_artifact_receipt.v1",
        artifactRef: `artifact.sha256.${"d".repeat(64)}`,
        contentDigest: `sha256:${"d".repeat(64)}`,
        byteLength: 5,
        sourceGeneration: common.resourceGeneration,
        sourcePathDigest: receipt.pathDigest,
        retentionUntil: "2026-07-20T20:00:00.000Z",
        contentType: "text/plain",
        evidenceRefs: receipt.evidenceRefs,
      },
    });
    expect(response.receipt).toMatchObject({
      processTerminated: true,
      descendantsRemaining: 0,
      symlinkTraversal: false,
      secretScan: "clean",
    });
    expect(response.action).toBe("read_artifact");
  });

  test("refuses optimistic or unbounded receipt shapes", () => {
    const decode = S.decodeUnknownSync(ManagedSandboxGuestIoResponseSchema);
    expect(() =>
      decode({
        schemaVersion: common.schemaVersion,
        action: "write_file",
        operationRef: common.operationRef,
        sandboxRef: common.sandboxRef,
        resourceGeneration: common.resourceGeneration,
        contentDigest: `sha256:${"d".repeat(64)}`,
        byteLength: 5,
        receipt: {
          schemaVersion: "openagents.managed_sandbox_guest_io_receipt.v1",
          receiptRef: "receipt.sbx05.invalid",
          operationRef: common.operationRef,
          sandboxRef: common.sandboxRef,
          resourceGeneration: common.resourceGeneration,
          capabilityRef: common.capabilityRef,
          action: "write_file",
          outcome: "succeeded",
          pathDigest: `sha256:${"c".repeat(64)}`,
          startedAt: common.requestedAt,
          finishedAt: common.requestedAt,
          bytesRead: 0,
          bytesWritten: 5,
          cpuMillis: 0,
          networkBytes: 0,
          processTerminated: false,
          descendantsRemaining: -1,
          scratchCleaned: false,
          ingressClosed: false,
          egressDenied: false,
          pathPolicy: "trusted_driver",
          symlinkTraversal: true,
          secretScan: "unknown",
          evidenceRefs: [],
        },
      }),
    ).toThrow();
  });
});
