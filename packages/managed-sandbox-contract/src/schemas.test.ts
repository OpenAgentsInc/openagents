import { Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import { ManagedSandboxCommandSchema, ManagedSandboxResourceSchema } from "./schemas.ts";

const target = {
  targetRef: "target.gcp.sbx.dev",
  targetClass: "openagents_managed" as const,
  provider: "google_cloud" as const,
  adapterRef: "adapter.gce.v1",
  region: "us-central1",
  isolation: "gce_vm" as const,
  dataPosture: "openagents_managed_region" as const,
};

const lease = {
  leaseRef: "lease.test.1",
  state: "active" as const,
  issuedAt: "2026-07-19T12:00:00.000Z",
  expiresAt: "2026-07-19T13:00:00.000Z",
  ttlSeconds: 3600,
  renewable: false,
};

const budget = {
  currency: "USD" as const,
  maxCostMicros: 2_000_000,
  maxCpuMillis: 3_600_000,
  maxNetworkBytes: 100_000_000,
  maxArtifactBytes: 10_000_000,
  maxLifetimeSeconds: 3600,
};

describe("managed sandbox boundary schemas", () => {
  it("decodes a resource whose lifecycle facts remain distinct", () => {
    const decoded = S.decodeUnknownSync(ManagedSandboxResourceSchema)({
      schema: "openagents.managed_sandbox.v1",
      sandboxRef: "sandbox.test.1",
      ownerRef: "owner.test",
      tenantRef: "tenant.test",
      programRef: "program.managed_agent_sandboxes",
      workUnitRef: "work.test.1",
      attachmentRef: "attachment.test.1",
      attachmentGeneration: 0,
      resourceGeneration: 0,
      version: 1,
      lastEventSequence: 1,
      target,
      imageDigest: `sha256:${"a".repeat(64)}`,
      profileRef: "profile.sbx.gce.cpu.v1",
      lease,
      budget,
      capabilities: [],
      facts: {
        lifecycle: "stopping",
        leaseState: "active",
        guestState: "stopping",
        filesystemState: "checkpointing",
        ingressState: "closed",
        runtimeState: "settled",
        acceptingWork: false,
        cleanupComplete: false,
      },
      createdAt: "2026-07-19T12:00:00.000Z",
      updatedAt: "2026-07-19T12:01:00.000Z",
    });

    expect(decoded.facts.lifecycle).toBe("stopping");
    expect(decoded.facts.filesystemState).toBe("checkpointing");
    expect(decoded.facts.guestState).toBe("stopping");
  });

  it("requires bounded owner, tenant, generation, lease, budget, and capability input on create", () => {
    const decoded = S.decodeUnknownSync(ManagedSandboxCommandSchema)({
      _tag: "Create",
      schema: "openagents.managed_sandbox_command.v1",
      commandRef: "command.test.1",
      requestedByRef: "principal.sol.sbx00",
      ownerRef: "owner.test",
      tenantRef: "tenant.test",
      idempotencyRef: "idem.test.1",
      requestedAt: "2026-07-19T12:00:00.000Z",
      workUnitRef: "work.test.1",
      attachmentRef: "attachment.test.1",
      target,
      imageDigest: `sha256:${"b".repeat(64)}`,
      profileRef: "profile.sbx.gce.cpu.v1",
      lease,
      budget,
      requestedCapabilities: [
        {
          capabilityRef: "capability.test.command",
          kind: "command",
          state: "active",
          expiresAt: "2026-07-19T13:00:00.000Z",
        },
      ],
    });

    if (!("target" in decoded)) throw new Error("expected Create");
    expect(decoded.target.provider).toBe("google_cloud");
  });

  it("refuses mutable image tags and zero-length leases", () => {
    expect(() =>
      S.decodeUnknownSync(ManagedSandboxCommandSchema)({
        _tag: "Create",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: "command.test.1",
        requestedByRef: "principal.sol.sbx00",
        ownerRef: "owner.test",
        tenantRef: "tenant.test",
        idempotencyRef: "idem.test.1",
        requestedAt: "2026-07-19T12:00:00.000Z",
        workUnitRef: "work.test.1",
        attachmentRef: "attachment.test.1",
        target,
        imageDigest: "latest",
        profileRef: "profile.sbx.gce.cpu.v1",
        lease: { ...lease, ttlSeconds: 0 },
        budget,
        requestedCapabilities: [],
      }),
    ).toThrow();

    expect(() =>
      S.decodeUnknownSync(ManagedSandboxCommandSchema)({
        _tag: "Create",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: "command.test.bad-ttl",
        requestedByRef: "principal.sol.sbx01",
        ownerRef: "owner.test",
        tenantRef: "tenant.test",
        idempotencyRef: "idem.test.bad-ttl",
        requestedAt: "2026-07-19T12:00:00.000Z",
        workUnitRef: "work.test.bad-ttl",
        attachmentRef: "attachment.test.bad-ttl",
        target,
        imageDigest: `sha256:${"c".repeat(64)}`,
        profileRef: "profile.sbx.gce.cpu.v1",
        lease: { ...lease, expiresAt: "2026-07-19T12:30:00.000Z" },
        budget,
        requestedCapabilities: [],
      }),
    ).toThrowError(/exact positive TTL/);
  });
});
