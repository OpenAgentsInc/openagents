import type {
  PortableOwnerLocalCapabilityOperationClaimRequest,
  PortableOwnerLocalCapabilityOperationRecord,
  PortableOwnerLocalCapabilityOperationRenewRequest,
  PortableOwnerLocalCapabilityOperationResultRequest,
} from "@openagentsinc/khala-sync-server";
import { describe, expect, test } from "vitest";

import { makePortableOwnerLocalCapabilityOperationRoutes } from "./portable-owner-local-capability-operation-routes";

const schema = "openagents.portable_owner_local_capability_operation.v1" as const;
const pylonRef = "pylon.ide13.capability";
const targetRef = "target.ide13.capability";
const basePath = `/api/pylons/${pylonRef}/portable-targets/${targetRef}/capability-operations`;
const now = "2026-07-20T15:00:00.000Z";

const claimBody: PortableOwnerLocalCapabilityOperationClaimRequest = {
  schema,
  operationRef: `operation.owner-local-capability.${"1".repeat(64)}`,
  claimRef: "claim.ide13.capability.1",
  pylonRef,
  targetRef,
  sessionRef: "session.ide13.capability.1",
  attachmentRef: "attachment.ide13.capability.1",
  attachmentGeneration: 2,
  workerInstanceRef: "worker.ide13.capability.1",
  leaseExpiresAt: "2026-07-20T15:01:00.000Z",
};

const renewBody: PortableOwnerLocalCapabilityOperationRenewRequest = {
  schema,
  claimRef: claimBody.claimRef,
  pylonRef,
  targetRef,
  sessionRef: claimBody.sessionRef,
  attachmentRef: claimBody.attachmentRef,
  attachmentGeneration: claimBody.attachmentGeneration,
  workerInstanceRef: claimBody.workerInstanceRef,
  claimGeneration: 1,
  expectedLeaseRevision: 1,
  leaseExpiresAt: "2026-07-20T15:02:00.000Z",
};

const completeBody: PortableOwnerLocalCapabilityOperationResultRequest = {
  schema,
  claimRef: claimBody.claimRef,
  pylonRef,
  targetRef,
  sessionRef: claimBody.sessionRef,
  attachmentRef: claimBody.attachmentRef,
  attachmentGeneration: claimBody.attachmentGeneration,
  workerInstanceRef: claimBody.workerInstanceRef,
  claimGeneration: 1,
  expectedLeaseRevision: 2,
  resultRef: "result.ide13.capability.1",
  resultStatus: "completed",
  receiptRef: "receipt.ide13.capability.1",
  evidenceRefs: ["evidence.ide13.capability.1"],
  errorRef: null,
  completedAt: now,
};

const record = (
  state: PortableOwnerLocalCapabilityOperationRecord["state"] = "pending",
): PortableOwnerLocalCapabilityOperationRecord => ({
  request: {
    schema,
    operationRef: claimBody.operationRef,
    action: "install",
    capability: "provider",
    commandExecutionClaimRef: "claim.ide13.command.1",
    ownerRef: "owner.ide13.capability.1",
    pylonRef,
    sessionRef: claimBody.sessionRef,
    attachmentRef: claimBody.attachmentRef,
    attachmentGeneration: claimBody.attachmentGeneration,
    targetRef,
    sourceLeaseRef: "lease.ide13.source.1",
    sourceGrantRef: "grant.ide13.source.1",
    destinationLeaseRef: "lease.ide13.destination.1",
    destinationGrantRef: "grant.ide13.destination.1",
    installationRef: null,
    permissionRefs: ["permission.provider.use"],
    permissionFingerprint: `sha256:${"2".repeat(64)}`,
    expiresAt: "2026-07-20T15:05:00.000Z",
  },
  requestFingerprint: `sha256:${"3".repeat(64)}`,
  state,
  claimRef: state === "pending" ? null : claimBody.claimRef,
  claimFingerprint: state === "pending" ? null : `sha256:${"4".repeat(64)}`,
  workerInstanceRef: state === "pending" ? null : claimBody.workerInstanceRef,
  claimGeneration: state === "pending" ? null : 1,
  leaseRevision: state === "pending" ? null : 1,
  claimedAt: state === "pending" ? null : now,
  leaseExpiresAt: state === "pending" ? null : claimBody.leaseExpiresAt,
  resultRef: null,
  resultFingerprint: null,
  resultStatus: null,
  receiptRef: null,
  resultEvidenceRefs: [],
  errorRef: null,
  completedAt: null,
  updatedAt: now,
});

type Calls = Readonly<{
  claim: Array<
    Readonly<{ ownerRef: string; body: PortableOwnerLocalCapabilityOperationClaimRequest }>
  >;
  renew: Array<
    Readonly<{ ownerRef: string; body: PortableOwnerLocalCapabilityOperationRenewRequest }>
  >;
  complete: Array<
    Readonly<{ ownerRef: string; body: PortableOwnerLocalCapabilityOperationResultRequest }>
  >;
  pending: Array<
    Readonly<{ ownerRef: string; pylonRef: string; targetRef: string; limit: number }>
  >;
  read: Array<
    Readonly<{ ownerRef: string; pylonRef: string; targetRef: string; operationRef: string }>
  >;
}>;

const setup = (options: Readonly<{ authenticated?: boolean; registeredOwner?: string }> = {}) => {
  const calls: Calls = { claim: [], renew: [], complete: [], pending: [], read: [] };
  const routes = makePortableOwnerLocalCapabilityOperationRoutes({
    authenticate: async () =>
      options.authenticated === false
        ? undefined
        : { agentUserId: "agent.ide13.capability", ownerUserId: "owner.ide13.capability" },
    readPylonOwnerAgentUserId: async () => options.registeredOwner ?? "agent.ide13.capability",
    withExchange: async (_env, use) =>
      use({
        pending: async (ownerRef, inputPylonRef, inputTargetRef, limit) => {
          calls.pending.push({
            ownerRef: String(ownerRef),
            pylonRef: String(inputPylonRef),
            targetRef: String(inputTargetRef),
            limit: limit ?? 32,
          });
          return [record()];
        },
        read: async (ownerRef, inputPylonRef, inputTargetRef, operationRef) => {
          calls.read.push({
            ownerRef: String(ownerRef),
            pylonRef: String(inputPylonRef),
            targetRef: String(inputTargetRef),
            operationRef: String(operationRef),
          });
          return record("claimed");
        },
        claim: async (ownerRef, input) => {
          calls.claim.push({
            ownerRef: String(ownerRef),
            body: input as PortableOwnerLocalCapabilityOperationClaimRequest,
          });
          return { status: "replayed", operation: record("claimed") };
        },
        renew: async (ownerRef, input) => {
          calls.renew.push({
            ownerRef: String(ownerRef),
            body: input as PortableOwnerLocalCapabilityOperationRenewRequest,
          });
          return { status: "renewed", operation: record("claimed") };
        },
        complete: async (ownerRef, input) => {
          calls.complete.push({
            ownerRef: String(ownerRef),
            body: input as PortableOwnerLocalCapabilityOperationResultRequest,
          });
          return { status: "completed", operation: record("completed") };
        },
      }),
  });
  return { calls, route: routes.routePortableOwnerLocalCapabilityOperationRequest };
};

const request = (
  path = basePath,
  body?: unknown,
  method = body === undefined ? "GET" : "POST",
): Request =>
  new Request(`https://openagents.com${path}`, {
    method,
    headers: { authorization: "Bearer oa_agent_test", "content-type": "application/json" },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });

describe("owner-local capability operation Pylon routes", () => {
  test("requires an authenticated registered Pylon owner", async () => {
    const unauthenticated = setup({ authenticated: false });
    expect((await unauthenticated.route(request(), {}))?.status).toBe(401);
    expect(unauthenticated.calls.pending).toHaveLength(0);

    const wrongOwner = setup({ registeredOwner: "agent.ide13.other" });
    const response = await wrongOwner.route(request(), {});
    expect(response?.status).toBe(403);
    expect(await response?.json()).toMatchObject({ error: "pylon_not_owned" });
    expect(wrongOwner.calls.pending).toHaveLength(0);
  });

  test("polls and reconciles only the exact Pylon target path", async () => {
    const { calls, route } = setup();
    const poll = await route(request(`${basePath}?limit=7`), {});
    expect(poll?.status).toBe(200);
    expect(calls.pending).toEqual([
      { ownerRef: "owner.ide13.capability", pylonRef, targetRef, limit: 7 },
    ]);
    expect(await poll?.json()).toMatchObject({
      schema: "openagents.portable_owner_local_capability_operation_transport.v1",
      operations: [{ request: { pylonRef, targetRef }, state: "pending" }],
    });

    const reconciled = await route(request(`${basePath}/reconcile/${claimBody.operationRef}`), {});
    expect(reconciled?.status).toBe(200);
    expect(calls.read).toEqual([
      {
        ownerRef: "owner.ide13.capability",
        pylonRef,
        targetRef,
        operationRef: claimBody.operationRef,
      },
    ]);
  });

  test("decodes claim, renewal, and refs-only completion bodies", async () => {
    const { calls, route } = setup();
    const claimed = await route(
      request(`${basePath}/claim`, { ...claimBody, material: "must-not-cross" }),
      {},
    );
    expect(claimed?.status).toBe(200);
    expect(calls.claim).toEqual([{ ownerRef: "owner.ide13.capability", body: claimBody }]);
    expect(calls.claim[0]?.body).not.toHaveProperty("material");
    expect((await route(request(`${basePath}/renew`, renewBody), {}))?.status).toBe(200);
    expect((await route(request(`${basePath}/complete`, completeBody), {}))?.status).toBe(200);
    expect(calls.renew).toEqual([{ ownerRef: "owner.ide13.capability", body: renewBody }]);
    expect(calls.complete).toEqual([{ ownerRef: "owner.ide13.capability", body: completeBody }]);
  });

  test("rejects path/body drift and has no enqueue or material route", async () => {
    const { calls, route } = setup();
    const mismatch = await route(
      request(`${basePath}/claim`, { ...claimBody, targetRef: "target.ide13.other" }),
      {},
    );
    expect(mismatch?.status).toBe(409);
    expect(calls.claim).toHaveLength(0);
    expect(route(request(`${basePath}/enqueue`, claimBody), {})).toBeUndefined();
    expect(route(request(`${basePath}/material`, claimBody), {})).toBeUndefined();
  });
});
