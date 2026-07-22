import { describe, expect, it, vi } from "vite-plus/test";

import {
  ManagedSandboxPrivatePreviewTargetError,
  makeManagedSandboxPrivatePreviewTarget,
} from "./managed-sandbox-private-preview-target";

const capability = {
  _tag: "Active" as const,
  schema: "openagents.managed_sandbox_private_ingress.v1" as const,
  capabilityRef: "capability.sbx10.ingress.test",
  sandboxRef: "sandbox.test",
  resourceGeneration: 1,
  ownerRef: "owner.test",
  audienceRef: "audience.test",
  kind: "preview" as const,
  issuedAt: "2026-07-22T01:00:00.000Z",
  expiresAt: "2026-07-22T01:05:00.000Z",
  ttlSeconds: 300,
  accessUrlDigest: `sha256:${"a".repeat(64)}`,
  accessUrlAtRest: "redacted" as const,
  audiencePolicy: "owner_scoped_explicit_audience" as const,
  publicAccess: false as const,
  permanentRoute: false as const,
  vnc: "unsupported" as const,
  auditRefs: ["audit.ingress.create.test"],
};

const targetResponse = (content = "preview") => ({
  schemaVersion: "openagents.managed_sandbox_private_preview.v1",
  capabilityRef: capability.capabilityRef,
  audienceRef: capability.audienceRef,
  sandboxRef: capability.sandboxRef,
  resourceGeneration: 1,
  preview: {
    schemaVersion: "openagents.managed_sandbox_guest_io.v1",
    action: "read_file",
    operationRef: "operation.preview.test",
    sandboxRef: capability.sandboxRef,
    resourceGeneration: 1,
    encoding: "utf8",
    content,
    contentDigest: `sha256:${"b".repeat(64)}`,
    byteLength: content.length,
    binary: false,
    receipt: {
      schemaVersion: "openagents.managed_sandbox_guest_io_receipt.v1",
      receiptRef: "receipt.preview.test",
      operationRef: "operation.preview.test",
      sandboxRef: capability.sandboxRef,
      resourceGeneration: 1,
      capabilityRef: capability.capabilityRef,
      action: "read_file",
      outcome: "succeeded",
      pathDigest: `sha256:${"c".repeat(64)}`,
      startedAt: "2026-07-22T01:01:00.000Z",
      finishedAt: "2026-07-22T01:01:00.100Z",
      bytesRead: content.length,
      bytesWritten: 0,
      cpuMillis: 1,
      networkBytes: 0,
      processTerminated: true,
      descendantsRemaining: 0,
      scratchCleaned: true,
      ingressClosed: true,
      egressDenied: true,
      pathPolicy: "resolved_beneath_workspace_root",
      symlinkTraversal: false,
      secretScan: "clean",
      evidenceRefs: ["evidence.preview.test"],
    },
  },
});

const input = {
  requestRef: "operation.sbx10.preview.test",
  capability,
  audienceRef: capability.audienceRef,
  path: "/workspace/preview.html",
  encoding: "utf8" as const,
};

describe("managed sandbox private preview target", () => {
  it("sends no raw URL or topology and validates the exact target response", async () => {
    const fetch = vi.fn(async (_url: string | URL | Request, init?: RequestInit) => {
      const body = String(init?.body);
      expect(body).not.toContain("https://openagents.com/api/managed-sandboxes/private-ingress");
      expect(body).not.toContain("10.128.");
      expect(JSON.parse(body)).toMatchObject({
        capabilityRef: capability.capabilityRef,
        audienceRef: capability.audienceRef,
        capability: { accessUrlAtRest: "redacted" },
      });
      return new Response(JSON.stringify(targetResponse()));
    });
    const target = makeManagedSandboxPrivatePreviewTarget({
      baseUrl: "https://control.example.invalid",
      bearerToken: "test-control-token",
      fetch,
    });
    const result = await target.use(input);
    expect(result.preview.action).toBe("read_file");
    if (result.preview.action !== "read_file") throw new Error("expected read preview");
    expect(result.preview.content).toBe("preview");
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it("fails closed for terminal capabilities and mismatched or topology-bearing responses", async () => {
    const terminal = makeManagedSandboxPrivatePreviewTarget({
      baseUrl: "https://control.example.invalid",
      bearerToken: "test-control-token",
      fetch: async () => new Response("{}", { status: 410 }),
    });
    await expect(terminal.use(input)).rejects.toMatchObject({ status: 410 });

    const mismatch = makeManagedSandboxPrivatePreviewTarget({
      baseUrl: "https://control.example.invalid",
      bearerToken: "test-control-token",
      fetch: async () =>
        new Response(JSON.stringify({ ...targetResponse(), audienceRef: "audience.other" })),
    });
    await expect(mismatch.use(input)).rejects.toBeInstanceOf(
      ManagedSandboxPrivatePreviewTargetError,
    );

    const leak = makeManagedSandboxPrivatePreviewTarget({
      baseUrl: "https://control.example.invalid",
      bearerToken: "test-control-token",
      fetch: async () => new Response(JSON.stringify(targetResponse('"hostname":"vm.internal"'))),
    });
    await expect(leak.use(input)).rejects.toMatchObject({ reasonRef: "target_response_refused" });
  });
});
