/* oxlint-disable openagents/no-manual-effect-runtime-in-tests -- @effect/vitest is not installed on the repository Effect 4 line. */
import { Effect } from "effect";
import { describe, expect, it, vi } from "vite-plus/test";

import { ManagedSandboxPrivatePreviewTargetError } from "./managed-sandbox-private-preview-target";
import {
  MANAGED_SANDBOX_PRIVATE_INGRESS_PATH_PREFIX,
  makeManagedSandboxPrivatePreviewRoutes,
} from "./managed-sandbox-private-preview-routes";

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

const preview = (content = "<h1>Private preview</h1>") => ({
  schemaVersion: "openagents.managed_sandbox_private_preview.v1" as const,
  capabilityRef: capability.capabilityRef,
  audienceRef: capability.audienceRef,
  sandboxRef: capability.sandboxRef,
  resourceGeneration: 1,
  preview: {
    schemaVersion: "openagents.managed_sandbox_guest_io.v1" as const,
    action: "read_file" as const,
    operationRef: "operation.preview.test",
    sandboxRef: capability.sandboxRef,
    resourceGeneration: 1,
    encoding: "utf8" as const,
    content,
    contentDigest: `sha256:${"b".repeat(64)}`,
    byteLength: content.length,
    binary: false,
    receipt: {
      schemaVersion: "openagents.managed_sandbox_guest_io_receipt.v1" as const,
      receiptRef: "receipt.preview.test",
      operationRef: "operation.preview.test",
      sandboxRef: capability.sandboxRef,
      resourceGeneration: 1,
      capabilityRef: capability.capabilityRef,
      action: "read_file" as const,
      outcome: "succeeded" as const,
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
      pathPolicy: "resolved_beneath_workspace_root" as const,
      symlinkTraversal: false as const,
      secretScan: "clean" as const,
      evidenceRefs: ["evidence.preview.test"],
    },
  },
});

const request = (audience = capability.audienceRef) =>
  new Request(
    `https://openagents.com${MANAGED_SANDBOX_PRIVATE_INGRESS_PATH_PREFIX}${capability.capabilityRef}?path=%2Fworkspace%2Fpreview.html`,
    { headers: { "x-test-audience": audience } },
  );

const run = <Bindings>(
  routes: ReturnType<typeof makeManagedSandboxPrivatePreviewRoutes<Bindings>>,
  input: Request,
  env: Bindings,
) => routes.route(input, env, {} as ExecutionContext)!;

describe("managed sandbox private preview route", () => {
  it("serves one authenticated audience preview with restrictive response policy", async () => {
    const usePreview = vi.fn(async () => preview());
    const routes = makeManagedSandboxPrivatePreviewRoutes({
      authenticateAudience: async (input) => ({
        userId: input.headers.get("x-test-audience") ?? "",
      }),
      enabled: () => true,
      readCapability: async (_env, input) =>
        input.audienceRef === capability.audienceRef ? capability : undefined,
      usePreview,
      now: () => new Date("2026-07-22T01:01:00.000Z"),
      makeRequestRef: () => "operation.sbx10.preview.test",
      accessUrlDigest: async () => capability.accessUrlDigest,
    });

    const response = await Effect.runPromise(run(routes, request(), {}));
    expect(response.status).toBe(200);
    expect(await response.text()).toBe("<h1>Private preview</h1>");
    expect(response.headers.get("content-security-policy")).toContain("sandbox");
    expect(response.headers.get("cache-control")).toBe("no-store, private");
    expect(response.headers.get("x-openagents-receipt-ref")).toBe("receipt.preview.test");
    expect(usePreview).toHaveBeenCalledWith(
      {},
      expect.objectContaining({
        audienceRef: capability.audienceRef,
        capability,
        path: "/workspace/preview.html",
      }),
    );
  });

  it("hides a capability from the wrong audience and does not call the target", async () => {
    const usePreview = vi.fn(async () => preview());
    const routes = makeManagedSandboxPrivatePreviewRoutes({
      authenticateAudience: async () => ({ userId: "audience.other" }),
      enabled: () => true,
      readCapability: async () => undefined,
      usePreview,
      now: () => new Date("2026-07-22T01:01:00.000Z"),
      accessUrlDigest: async () => capability.accessUrlDigest,
    });
    const response = await Effect.runPromise(run(routes, request("audience.other"), {}));
    expect(response.status).toBe(404);
    expect(usePreview).not.toHaveBeenCalled();
  });

  it("fails closed before target use after expiry", async () => {
    const usePreview = vi.fn(async () => preview());
    const routes = makeManagedSandboxPrivatePreviewRoutes({
      authenticateAudience: async () => ({ userId: capability.audienceRef }),
      enabled: () => true,
      readCapability: async () => capability,
      usePreview,
      now: () => new Date(capability.expiresAt),
      accessUrlDigest: async () => capability.accessUrlDigest,
    });
    const response = await Effect.runPromise(run(routes, request(), {}));
    expect(response.status).toBe(410);
    expect(usePreview).not.toHaveBeenCalled();
  });

  it("propagates native revocation and refuses topology-bearing content", async () => {
    const common = {
      authenticateAudience: async () => ({ userId: capability.audienceRef }),
      enabled: () => true,
      readCapability: async () => capability,
      now: () => new Date("2026-07-22T01:01:00.000Z"),
      accessUrlDigest: async () => capability.accessUrlDigest,
    };
    const revoked = makeManagedSandboxPrivatePreviewRoutes({
      ...common,
      usePreview: async () => {
        throw new ManagedSandboxPrivatePreviewTargetError(410, "capability_terminal");
      },
    });
    expect((await Effect.runPromise(run(revoked, request(), {}))).status).toBe(410);

    const leaking = makeManagedSandboxPrivatePreviewRoutes({
      ...common,
      usePreview: async () => preview("internal host 10.128.0.4"),
    });
    const response = await Effect.runPromise(run(leaking, request(), {}));
    expect(response.status).toBe(502);
    expect(await response.text()).not.toContain("10.128.0.4");
  });
});
