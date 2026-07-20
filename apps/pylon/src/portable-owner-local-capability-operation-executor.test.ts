import { expect, test, vi } from "vite-plus/test";

import type { PortableOwnerLocalCapabilityOperationRequest } from "@openagentsinc/portable-session-contract";

import {
  capabilityMaterialRequest,
  makePylonPortableOwnerLocalCapabilityMaterialClient,
  PylonPortableOwnerLocalCapabilityMaterialError,
} from "./portable-owner-local-capability-material-client.js";
import { makePylonPortableOwnerLocalCapabilityOperationExecutor } from "./portable-owner-local-capability-operation-executor.js";
import type { PylonPortableOwnerLocalCapabilityExecutionClaim } from "./portable-owner-local-capability-operation-worker.js";

const pylonRef = "pylon.ide13.executor";
const targetRef = "target.ide13.executor";
const claim: PylonPortableOwnerLocalCapabilityExecutionClaim = {
  claimRef: "claim.ide13.executor",
  workerInstanceRef: "worker.ide13.executor",
  claimGeneration: 1,
  expectedLeaseRevision: 2,
  expectedLeaseExpiresAt: "2026-07-20T12:01:00.000Z",
};
const installRequest = (): PortableOwnerLocalCapabilityOperationRequest => ({
  schema: "openagents.portable_owner_local_capability_operation.v1",
  operationRef: "operation.ide13.executor.install",
  action: "install",
  capability: "provider",
  commandExecutionClaimRef: "claim.ide13.executor.command",
  ownerRef: "owner.ide13.executor",
  pylonRef,
  sessionRef: "session.ide13.executor",
  attachmentRef: "attachment.ide13.executor",
  attachmentGeneration: 1,
  targetRef,
  sourceLeaseRef: "lease.ide13.executor.source",
  sourceGrantRef: "grant.ide13.executor.source",
  destinationLeaseRef: "lease.ide13.executor.destination",
  destinationGrantRef: "grant.ide13.executor.destination",
  installationRef: null,
  permissionRefs: ["permission.ide13.executor"],
  permissionFingerprint: `sha256:${"2".repeat(64)}`,
  expiresAt: "2026-07-20T12:10:00.000Z",
});

const materialClient = (fetchImpl: typeof fetch) =>
  makePylonPortableOwnerLocalCapabilityMaterialClient({
    agentToken: "private-agent-token",
    baseUrl: "https://openagents.example",
    pylonRef,
    targetRef,
    fetchImpl,
  });

test("redeems exact-bound octets and rejects wrong bindings", async () => {
  const fetchImpl = vi.fn<typeof fetch>(async (_input, init) => {
    expect(JSON.parse(String(init?.body))).toEqual(
      capabilityMaterialRequest(installRequest(), claim),
    );
    return new Response(new Uint8Array([1, 2, 3]), {
      headers: { "content-type": "application/octet-stream", "cache-control": "private, no-store" },
    });
  });
  const client = materialClient(fetchImpl);
  expect([
    ...(await client.redeem(
      capabilityMaterialRequest(installRequest(), claim),
      new AbortController().signal,
    )),
  ]).toEqual([1, 2, 3]);
  await expect(
    client.redeem(
      { ...capabilityMaterialRequest(installRequest(), claim), targetRef: "target.ide13.wrong" },
      new AbortController().signal,
    ),
  ).rejects.toEqual(new PylonPortableOwnerLocalCapabilityMaterialError("invalid_request"));
  expect(fetchImpl).toHaveBeenCalledTimes(1);
});

test("rejects non-octet, empty, and oversized material responses", async () => {
  const exact = capabilityMaterialRequest(installRequest(), claim);
  for (const response of [
    new Response("json", {
      headers: { "content-type": "application/json", "cache-control": "no-store" },
    }),
    new Response(new Uint8Array(), {
      headers: { "content-type": "application/octet-stream", "cache-control": "no-store" },
    }),
    new Response(null, {
      headers: {
        "content-type": "application/octet-stream",
        "cache-control": "no-store",
        "content-length": String(1024 * 1024 + 1),
      },
    }),
  ]) {
    const client = materialClient(async () => response);
    await expect(client.redeem(exact, new AbortController().signal)).rejects.toEqual(
      new PylonPortableOwnerLocalCapabilityMaterialError("bad_response"),
    );
  }
});

test("maps caller cancellation without returning material", async () => {
  const controller = new AbortController();
  const client = materialClient(async (_input, init) => {
    controller.abort("cancelled");
    throw init?.signal?.reason;
  });
  await expect(
    client.redeem(capabilityMaterialRequest(installRequest(), claim), controller.signal),
  ).rejects.toEqual(new PylonPortableOwnerLocalCapabilityMaterialError("cancelled"));
});

test("installs an exact destination lease and clears redeemed material", async () => {
  const material = new TextEncoder().encode("private-provider-material");
  let observed: Uint8Array | undefined;
  const install = vi.fn(
    async (
      input: Parameters<
        import("@openagentsinc/khala-sync-server/portable-capability-installation-ports").OwnerLocalPortableCapabilityInstallationPort["install"]
      >[0],
    ) => {
      observed = input.material;
      expect(input.lease).toMatchObject({
        leaseRef: "lease.ide13.executor.destination",
        ownerRef: "owner.ide13.executor",
        capability: "provider",
        state: "issued",
      });
      expect(input.permissions).toEqual(["permission.ide13.executor"]);
      return {
        installationRef: "installation.ide13.executor",
        evidenceRef: "evidence.ide13.executor",
      };
    },
  );
  const executor = makePylonPortableOwnerLocalCapabilityOperationExecutor({
    materialClient: { redeem: async () => material },
    installationPort: { install, wipe: async () => ({ wipeReceiptRef: "receipt.ide13.wipe" }) },
  });
  const result = await executor.execute(installRequest(), claim, new AbortController().signal);
  expect(result.outcome).toMatchObject({
    status: "completed",
    installationRef: "installation.ide13.executor",
    evidenceRefs: ["evidence.ide13.executor"],
  });
  expect(result.outcome.receiptRef).not.toBe("installation.ide13.executor");
  expect(observed).toBe(material);
  expect([...material]).toEqual(Array.from({ length: material.length }, () => 0));
});

test("clears redeemed material when installation fails", async () => {
  const material = new Uint8Array([9, 8, 7]);
  const executor = makePylonPortableOwnerLocalCapabilityOperationExecutor({
    materialClient: { redeem: async () => material },
    installationPort: {
      install: async () => {
        throw new Error("installation refused");
      },
      wipe: async () => ({ wipeReceiptRef: "receipt.ide13.wipe" }),
    },
  });
  await expect(
    executor.execute(installRequest(), claim, new AbortController().signal),
  ).rejects.toThrow("installation refused");
  expect([...material]).toEqual([0, 0, 0]);
});

test("wipes by source lease without material redemption and replays by operation ref", async () => {
  const redeem = vi.fn(async () => new Uint8Array([1]));
  const wipe = vi.fn(async () => ({ wipeReceiptRef: "receipt.ide13.executor.wiped" }));
  const executor = makePylonPortableOwnerLocalCapabilityOperationExecutor({
    materialClient: { redeem },
    installationPort: {
      install: async () => {
        throw new Error("unexpected install");
      },
      wipe,
    },
  });
  const request: PortableOwnerLocalCapabilityOperationRequest = {
    ...installRequest(),
    operationRef: "operation.ide13.executor.wipe",
    action: "wipe",
    capability: null,
    installationRef: "installation.ide13.executor",
    permissionRefs: [],
  };
  const first = await executor.execute(request, claim, new AbortController().signal);
  const replay = await executor.execute(request, claim, new AbortController().signal);
  expect(first).toEqual(replay);
  expect(first.outcome).toMatchObject({
    status: "completed",
    installationRef: null,
    receiptRef: "receipt.ide13.executor.wiped",
    evidenceRefs: [],
  });
  expect(wipe).toHaveBeenCalledTimes(2);
  expect(wipe).toHaveBeenCalledWith({
    leaseRef: "lease.ide13.executor.source",
    targetRef,
    attachmentRef: "attachment.ide13.executor",
    attachmentGeneration: 1,
    installationRef: "installation.ide13.executor",
  });
  expect(redeem).not.toHaveBeenCalled();
});
