import { createHash } from "node:crypto";

import type {
  PortableCapabilityLease,
  PortableTargetDescriptor,
  SecretMaterial,
} from "@openagentsinc/portable-session-contract";
import { describe, expect, test, vi } from "vite-plus/test";

import { managedCapabilityMarkerPath } from "./portable-capability-runtime-adapters.js";
import {
  createPostgresManagedPortableCommandInstallationPortResolver,
  PortableManagedCommandInstallationResolverError,
} from "./portable-managed-command-installation-resolver.js";
import type { SyncSql } from "./sql.js";

const claimRef = "claim.ide13.managed-installation";
const ownerRef = "owner.ide13.managed-installation";
const sessionRef = "session.ide13.managed-installation";
const targetRef = "target.ide13.managed-installation";
const adapterRef = "adapter.ide13.managed-installation";
const resourceRef = "resource.ide13.managed-installation";
const attachmentRef = "attachment.ide13.managed-installation";

const target: PortableTargetDescriptor = {
  targetRef,
  targetClass: "openagents_managed",
  adapterRef,
  ownerRef,
  compatibilityRef: "compatibility.ide13.managed-installation",
  isolation: "dedicated_microvm",
  dataPosture: "openagents_managed_region",
  health: "ready",
};

const authorityRow = () => ({
  target_ref: targetRef,
  target_owner_ref: ownerRef,
  target_class: "openagents_managed",
  adapter_ref: adapterRef,
  compatibility_ref: target.compatibilityRef,
  isolation: target.isolation,
  data_posture: target.dataPosture,
  health: "ready",
  claim_ref: claimRef,
  claim_owner_ref: ownerRef,
  session_ref: sessionRef,
  executor_environment_ref: "target.ide13.source",
  destination_target_ref: targetRef,
  claim_state: "claimed",
  terminal_status: null,
  lease_expires_at: "2026-07-20T12:10:00.000Z",
});

const resourceRow = () => ({
  owner_user_id: ownerRef,
  target_ref: targetRef,
  session_ref: sessionRef,
  attachment_ref: attachmentRef,
  generation: 2,
  resource_ref: resourceRef,
  state: "staged",
  accepting_work: false,
});

const sqlWith = (
  input: Readonly<{
    authority?: ReadonlyArray<ReturnType<typeof authorityRow>>;
    resources?: ReadonlyArray<ReturnType<typeof resourceRow>>;
  }> = {},
): SyncSql =>
  Object.assign(
    vi.fn(async (strings: TemplateStringsArray) => {
      const query = strings.join("?");
      if (query.includes("khala_sync_portable_command_executions AS claim")) {
        return input.authority ?? [authorityRow()];
      }
      if (query.includes("khala_sync_portable_managed_targets")) {
        return input.resources ?? [resourceRow()];
      }
      throw new Error("unexpected managed installation SQL");
    }),
    {
      begin: async () => {
        throw new Error("transaction is not used");
      },
    },
  ) as unknown as SyncSql;

const scope = (targetDescriptor: PortableTargetDescriptor = target) => ({
  commandExecutionClaimRef: claimRef,
  ownerRef,
  sessionRef,
  target: targetDescriptor,
  sourceAttachmentRef: "attachment.ide13.managed-installation.source",
  sourceGeneration: 1,
  destinationAttachmentRef: attachmentRef,
  destinationGeneration: 2,
  grantBindings: [
    {
      sourceLeaseRef: "lease.ide13.managed-installation.source",
      grantRef: "grant.ide13.managed-installation.source",
      ownerUserId: ownerRef,
      kind: "provider" as const,
      providerAccountRef: "account.ide13.managed-installation",
    },
  ],
  capabilityTransfers: [
    {
      sourceLeaseRef: "lease.ide13.managed-installation.source",
      destinationLeaseRef: "lease.ide13.managed-installation",
      destinationSourceGrantRef: "grant.ide13.managed-installation.destination",
      expiresAt: "2026-07-20T12:09:00.000Z",
    },
  ],
});

const lease: PortableCapabilityLease = {
  leaseRef: "lease.ide13.managed-installation",
  ownerRef,
  sessionRef,
  attachmentRef,
  attachmentGeneration: 2,
  targetRef,
  capability: "provider",
  accountRef: "account.ide13.managed-installation",
  expiresAt: "2026-07-20T12:09:00.000Z",
  state: "issued",
};

const material = (): SecretMaterial =>
  new TextEncoder().encode("managed-installation-fixture") as SecretMaterial;

const managedFetch = vi.fn(async (request: string | URL | Request, init?: RequestInit) => {
  const url = new URL(request instanceof Request ? request.url : request.toString());
  if (!url.pathname.endsWith("/capabilities/install")) throw new Error("unexpected endpoint");
  const headers = new Headers(init?.headers);
  const evidenceRef = headers.get("X-OA-Evidence-Ref")!;
  const leaseRef = headers.get("X-OA-Lease-Ref")!;
  return Response.json({
    installationRef: `installation.agent-computer.capability.${createHash("sha256")
      .update(`${resourceRef}|${leaseRef}`)
      .digest("hex")
      .slice(0, 16)}`,
    evidenceRef,
    resourceRef,
    marker: { leaseRef, evidenceRef },
    material: "excluded",
  });
});

const resolver = (sql: SyncSql, now: () => string = () => "2026-07-20T12:00:00.000Z") =>
  createPostgresManagedPortableCommandInstallationPortResolver({
    sql,
    baseUrl: "https://agent-computer.example",
    bearerToken: "managed-installation-service-bearer",
    fetch: managedFetch,
    now,
  });

describe("Postgres managed command installation-port resolver", () => {
  test("resolves and installs through the exact staged managed resource", async () => {
    managedFetch.mockClear();
    const sql = sqlWith();
    const resolution = await resolver(sql).resolve(scope());
    expect(resolution).toMatchObject({
      targetRef,
      targetClass: "openagents_managed",
      adapterRef,
    });
    const installed = await resolution!.port.install({
      lease,
      permissions: ["provider.turn.execute"],
      material: material(),
      managedMarkerPath: managedCapabilityMarkerPath(sessionRef, lease.leaseRef),
    });
    expect(installed).toMatchObject({
      installationRef: expect.stringMatching(/^installation\.agent-computer\.capability\./u),
      marker: { leaseRef: lease.leaseRef },
    });
    expect(managedFetch).toHaveBeenCalledOnce();
    expect(sql).toHaveBeenCalledTimes(3);
  });

  test.each([
    ["target", { target: { ...target, adapterRef: "adapter.ide13.wrong" } }],
    ["owner", { ownerRef: "owner.ide13.wrong" }],
    ["session", { sessionRef: "session.ide13.wrong" }],
  ])("rejects an exact %s mismatch", async (_name, patch) => {
    await expect(resolver(sqlWith()).resolve({ ...scope(), ...patch })).rejects.toBeInstanceOf(
      PortableManagedCommandInstallationResolverError,
    );
  });

  test("fails closed when the staged managed resource is missing", async () => {
    managedFetch.mockClear();
    const resolution = await resolver(sqlWith({ resources: [] })).resolve(scope());
    await expect(
      resolution!.port.install({
        lease,
        permissions: ["provider.turn.execute"],
        material: material(),
        managedMarkerPath: managedCapabilityMarkerPath(sessionRef, lease.leaseRef),
      }),
    ).rejects.toMatchObject({ code: "target_unavailable" });
    expect(managedFetch).not.toHaveBeenCalled();
  });

  test("rechecks claim expiry before it resolves the staged resource", async () => {
    managedFetch.mockClear();
    let current = "2026-07-20T12:00:00.000Z";
    const resolution = await resolver(sqlWith(), () => current).resolve(scope());
    current = "2026-07-20T12:11:00.000Z";
    await expect(
      resolution!.port.install({
        lease,
        permissions: ["provider.turn.execute"],
        material: material(),
        managedMarkerPath: managedCapabilityMarkerPath(sessionRef, lease.leaseRef),
      }),
    ).rejects.toMatchObject({ code: "target_unavailable" });
    expect(managedFetch).not.toHaveBeenCalled();
  });

  test.each(["owner_local", "owner_managed", "managed_provider"] as const)(
    "rejects unsupported %s targets without SQL",
    async (targetClass) => {
      const sql = sqlWith();
      await expect(resolver(sql).resolve(scope({ ...target, targetClass }))).resolves.toBeNull();
      expect(sql).not.toHaveBeenCalled();
    },
  );
});
