import type {
  PortablePhaseOperationClaimRequest,
  PortablePhaseOperationRecord,
  PortablePhaseOperationRenewRequest,
  PortablePhaseOperationResultRequest,
  SyncSql,
} from "@openagentsinc/khala-sync-server";
import { describe, expect, test } from "vitest";

import {
  makePortablePhaseOperationRoutes,
  resolvePortablePhaseTarget,
} from "./portable-phase-operation-routes";

const pylonRef = "pylon.ide13.owner";
const targetRef = "target.ide13.owner-local";
const basePath = `/api/pylons/${pylonRef}/portable-targets/${targetRef}/phase-operations`;
const now = "2026-07-20T13:00:00.000Z";

const claimBody: PortablePhaseOperationClaimRequest = {
  schema: "openagents.portable_phase_operation.v1",
  operationRef: "operation.ide13.quiesce.1",
  claimRef: "claim.ide13.phase.1",
  sessionRef: "session.ide13.1",
  attachmentRef: "attachment.ide13.1",
  attachmentGeneration: 4,
  pylonRef,
  targetRef,
  workerInstanceRef: "worker.ide13.pylon.1",
  leaseExpiresAt: "2026-07-20T13:01:00.000Z",
};

const renewBody: PortablePhaseOperationRenewRequest = {
  schema: "openagents.portable_phase_operation.v1",
  claimRef: claimBody.claimRef,
  sessionRef: claimBody.sessionRef,
  attachmentRef: claimBody.attachmentRef,
  attachmentGeneration: claimBody.attachmentGeneration,
  pylonRef,
  targetRef,
  workerInstanceRef: claimBody.workerInstanceRef,
  claimGeneration: 1,
  expectedLeaseRevision: 1,
  leaseExpiresAt: "2026-07-20T13:02:00.000Z",
};

const completeBody: PortablePhaseOperationResultRequest = {
  schema: "openagents.portable_phase_operation.v1",
  claimRef: claimBody.claimRef,
  sessionRef: claimBody.sessionRef,
  attachmentRef: claimBody.attachmentRef,
  attachmentGeneration: claimBody.attachmentGeneration,
  pylonRef,
  targetRef,
  workerInstanceRef: claimBody.workerInstanceRef,
  claimGeneration: 1,
  expectedLeaseRevision: 2,
  resultRef: "result.ide13.phase.1",
  resultStatus: "completed",
  checkpointRef: null,
  checkpointObjectRef: null,
  checkpointDigest: null,
  evidenceRefs: ["evidence.ide13.phase.1"],
  errorRef: null,
  completedAt: now,
};

const record = (
  state: PortablePhaseOperationRecord["state"] = "pending",
): PortablePhaseOperationRecord => ({
  request: {
    schema: "openagents.portable_phase_operation.v1",
    operationRef: claimBody.operationRef,
    commandRef: "command.ide13.1",
    commandExecutionClaimRef: "claim.ide13.command.1",
    ownerRef: "owner.ide13.1",
    sessionRef: claimBody.sessionRef,
    attachmentRef: claimBody.attachmentRef,
    attachmentGeneration: claimBody.attachmentGeneration,
    targetRef,
    pylonRef,
    kind: "quiesce",
    checkpointRef: null,
    checkpointObjectRef: null,
    checkpointDigest: null,
    evidenceRefs: ["evidence.ide13.request.1"],
    expiresAt: "2026-07-20T13:05:00.000Z",
  },
  requestFingerprint: `sha256:${"1".repeat(64)}`,
  state,
  claimRef: state === "pending" ? null : claimBody.claimRef,
  claimFingerprint: state === "pending" ? null : `sha256:${"2".repeat(64)}`,
  workerInstanceRef: state === "pending" ? null : claimBody.workerInstanceRef,
  claimGeneration: state === "pending" ? null : 1,
  leaseRevision: state === "pending" ? null : 1,
  claimedAt: state === "pending" ? null : now,
  leaseExpiresAt: state === "pending" ? null : claimBody.leaseExpiresAt,
  resultRef: null,
  resultFingerprint: null,
  resultStatus: null,
  resultCheckpointRef: null,
  resultCheckpointObjectRef: null,
  resultCheckpointDigest: null,
  resultEvidenceRefs: [],
  errorRef: null,
  completedAt: null,
  updatedAt: now,
});

type Calls = {
  claim: Array<PortablePhaseOperationClaimRequest>;
  renew: Array<PortablePhaseOperationRenewRequest>;
  complete: Array<PortablePhaseOperationResultRequest>;
  pending: Array<Readonly<{ pylonRef: string; targetRef: string; limit: number }>>;
  resolved: Array<Readonly<{ ownerUserId: string; pylonRef: string; targetRef: string }>>;
};

const setup = (
  options?: Readonly<{
    authenticated?: boolean;
    registeredOwner?: string;
    targetState?: "ready" | "unavailable" | "not_found";
  }>,
) => {
  const calls: Calls = { claim: [], renew: [], complete: [], pending: [], resolved: [] };
  const routes = makePortablePhaseOperationRoutes({
    authenticate: async () =>
      options?.authenticated === false
        ? undefined
        : { agentUserId: "agent.ide13.1", ownerUserId: "owner.ide13.1" },
    readPylonOwnerAgentUserId: async () => options?.registeredOwner ?? "agent.ide13.1",
    resolveExactTarget: async (_env, input) => {
      calls.resolved.push(input);
      return options?.targetState ?? "ready";
    },
    withExchange: async (_env, use) =>
      use({
        pending: async (inputPylonRef, inputTargetRef, limit) => {
          calls.pending.push({
            pylonRef: String(inputPylonRef),
            targetRef: String(inputTargetRef),
            limit: limit ?? 32,
          });
          return [record()];
        },
        claim: async (input) => {
          calls.claim.push(input as PortablePhaseOperationClaimRequest);
          return { status: "replayed", operation: record("claimed") };
        },
        renew: async (input) => {
          calls.renew.push(input as PortablePhaseOperationRenewRequest);
          return { status: "renewed", operation: record("claimed") };
        },
        complete: async (input) => {
          calls.complete.push(input as PortablePhaseOperationResultRequest);
          return { status: "completed", operation: record("completed") };
        },
      }),
  });
  return { calls, route: routes.routePortablePhaseOperationRequest };
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

describe("portable phase operation Pylon routes (IDE-13)", () => {
  test("resolves the exact owner, Pylon, and target with parameterized SQL", async () => {
    const sql = ((_strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>) => {
      expect(values).toEqual([targetRef, "owner.ide13.1", "owner.ide13.1", pylonRef, targetRef]);
      return Promise.resolve([{ health: "ready" }]);
    }) as unknown as SyncSql;
    await expect(
      resolvePortablePhaseTarget(sql, {
        ownerUserId: "owner.ide13.1",
        pylonRef,
        targetRef,
      }),
    ).resolves.toBe("ready");
  });

  test("requires an authenticated registered Pylon owner", async () => {
    const unauthenticated = setup({ authenticated: false });
    const unauthenticatedResponse = await unauthenticated.route(request(), {});
    expect(unauthenticatedResponse?.status).toBe(401);
    expect(unauthenticated.calls.resolved).toHaveLength(0);

    const wrongOwner = setup({ registeredOwner: "agent.ide13.other" });
    const wrongOwnerResponse = await wrongOwner.route(request(), {});
    expect(wrongOwnerResponse?.status).toBe(403);
    expect(await wrongOwnerResponse?.json()).toMatchObject({ error: "pylon_not_owned" });
    expect(wrongOwner.calls.resolved).toHaveLength(0);
  });

  test("rejects a decoded path that is not a public ref", async () => {
    const { calls, route } = setup();
    const response = await route(
      request(
        `/api/pylons/${pylonRef}/portable-targets/${encodeURIComponent("target/private")}/phase-operations`,
      ),
      {},
    );
    expect(response?.status).toBe(400);
    expect(calls.resolved).toHaveLength(0);
  });

  test("fails closed unless the exact owner/Pylon/target tuple resolves ready", async () => {
    const notFound = setup({ targetState: "not_found" });
    const notFoundResponse = await notFound.route(request(), {});
    expect(notFoundResponse?.status).toBe(403);
    expect(notFound.calls.pending).toHaveLength(0);
    expect(notFound.calls.resolved).toEqual([
      { ownerUserId: "owner.ide13.1", pylonRef, targetRef },
    ]);

    const unavailable = setup({ targetState: "unavailable" });
    expect((await unavailable.route(request(), {}))?.status).toBe(409);
    expect(unavailable.calls.pending).toHaveLength(0);
  });

  test("lists only the exact target queue with a bounded limit", async () => {
    const { calls, route } = setup();
    const response = await route(request(`${basePath}?limit=7`), {});
    expect(response?.status).toBe(200);
    expect(calls.pending).toEqual([{ pylonRef, targetRef, limit: 7 }]);
    expect(await response?.json()).toMatchObject({
      schema: "openagents.portable_phase_operation_transport.v1",
      operations: [{ request: { pylonRef, targetRef }, state: "pending" }],
    });

    expect((await route(request(`${basePath}?limit=33`), {}))?.status).toBe(400);
  });

  test("claims exact bytes and returns an idempotent replay", async () => {
    const { calls, route } = setup();
    const response = await route(
      request(`${basePath}/claim`, { ...claimBody, rawPayload: "must-not-cross" }),
      {},
    );
    expect(response?.status).toBe(200);
    expect(await response?.json()).toMatchObject({ status: "replayed" });
    expect(calls.claim).toEqual([claimBody]);
    expect(calls.claim[0]).not.toHaveProperty("rawPayload");
  });

  test("rejects a path/body binding mismatch before the store", async () => {
    const { calls, route } = setup();
    const response = await route(
      request(`${basePath}/claim`, { ...claimBody, targetRef: "target.ide13.other" }),
      {},
    );
    expect(response?.status).toBe(409);
    expect(await response?.json()).toMatchObject({ error: "phase_scope_mismatch" });
    expect(calls.claim).toHaveLength(0);
  });

  test("renews and completes only exact claim bindings", async () => {
    const { calls, route } = setup();
    expect((await route(request(`${basePath}/renew`, renewBody), {}))?.status).toBe(200);
    expect((await route(request(`${basePath}/complete`, completeBody), {}))?.status).toBe(200);
    expect(calls.renew).toEqual([renewBody]);
    expect(calls.complete).toEqual([completeBody]);
  });

  test("does not expose an enqueue or takeover operation", async () => {
    const { route } = setup();
    expect(route(request(`${basePath}/enqueue`, claimBody), {})).toBeUndefined();
    expect(route(request(`${basePath}/takeover`, claimBody), {})).toBeUndefined();
  });
});
