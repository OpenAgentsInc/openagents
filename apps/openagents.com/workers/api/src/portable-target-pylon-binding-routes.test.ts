import type {
  PortableTargetPylonBindingRecord,
  PortableTargetPylonBindingWrite,
} from "@openagentsinc/khala-sync-server";
import { describe, expect, test } from "vitest";

import { makePortableTargetPylonBindingRoutes } from "./portable-target-pylon-binding-routes";

const pylonRef = "pylon.ide13.binding";
const targetRef = "target.ide13.binding";
const sessionRef = "session.ide13.binding";
const path = `/api/pylons/${pylonRef}/portable-target-bindings/${targetRef}`;

const binding = (
  revision = 1,
  state: "active" | "revoked" = "active",
): PortableTargetPylonBindingRecord => ({
  bindingRef: "binding.portable-target-pylon.ide13",
  ownerUserId: "owner.ide13.binding",
  ownerAgentUserId: "agent.ide13.binding",
  sessionRef,
  targetRef,
  pylonRef,
  workerInstanceRef: "worker.ide13.binding",
  bindingDigest: `sha256:${"a".repeat(64)}`,
  revision,
  state,
  health: state === "active" ? "ready" : "revoked",
  evidenceRefs: ["evidence.ide13.binding"],
  lastRenewedAt: "2026-07-20T13:00:00.000Z",
  expiresAt: "2026-07-20T13:05:00.000Z",
  revokedAt: state === "revoked" ? "2026-07-20T13:01:00.000Z" : null,
  createdAt: "2026-07-20T13:00:00.000Z",
  updatedAt: "2026-07-20T13:00:00.000Z",
});

const request = (method: string, body?: unknown, requestPath = path) =>
  new Request(`https://openagents.com${requestPath}`, {
    method,
    headers: {
      authorization: "Bearer agent-token",
      "content-type": "application/json",
      "idempotency-key": "portable-target-binding-test-1",
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });

const body = {
  schema: "openagents.portable_target_pylon_binding.request.v1",
  sessionRef,
  workerInstanceRef: "worker.ide13.binding",
  bindingDigest: `sha256:${"a".repeat(64)}`,
  health: "ready",
  evidenceRefs: ["evidence.ide13.binding"],
} as const;

describe("portable target Pylon binding routes", () => {
  test("does not intercept an unrelated Pylon route", () => {
    const routes = makePortableTargetPylonBindingRoutes({
      authenticate: async () => undefined,
      withStore: async () => {
        throw new Error("must not open storage");
      },
    });
    expect(routes.route(request("GET", undefined, "/api/pylons"), {})).toBeUndefined();
  });

  test("admits only the authenticated exact tuple", async () => {
    const admitted: Array<PortableTargetPylonBindingWrite> = [];
    const routes = makePortableTargetPylonBindingRoutes({
      authenticate: async (_request, _env, inputPylonRef) =>
        inputPylonRef === pylonRef
          ? { ownerUserId: "owner.ide13.binding", ownerAgentUserId: "agent.ide13.binding" }
          : undefined,
      withStore: async (_env, use) =>
        use({
          admit: async (input) => {
            admitted.push(input);
            return binding();
          },
          read: async () => undefined,
          revoke: async () => binding(2, "revoked"),
        }),
    });
    const response = await routes.route(request("POST", body), {});
    expect(response?.status).toBe(201);
    expect(admitted).toHaveLength(1);
    expect(admitted[0]).toMatchObject({
      ownerUserId: "owner.ide13.binding",
      ownerAgentUserId: "agent.ide13.binding",
      pylonRef,
      targetRef,
      sessionRef,
      idempotencyKeyHash: expect.stringMatching(/^sha256:[0-9a-f]{64}$/),
    });
  });

  test("reads owner-private state and requires CAS for revoke", async () => {
    const revoked: Array<PortableTargetPylonBindingWrite> = [];
    const routes = makePortableTargetPylonBindingRoutes({
      authenticate: async () => ({
        ownerUserId: "owner.ide13.binding",
        ownerAgentUserId: "agent.ide13.binding",
      }),
      withStore: async (_env, use) =>
        use({
          admit: async () => binding(),
          read: async () => binding(),
          revoke: async (input) => {
            revoked.push(input);
            return binding(2, "revoked");
          },
        }),
    });
    const read = await routes.route(
      request("GET", undefined, `${path}?sessionRef=${sessionRef}`),
      {},
    );
    expect(read?.status).toBe(200);
    expect(await read?.json()).toMatchObject({ binding: { pylonRef, targetRef, revision: 1 } });

    const revoke = await routes.route(request("DELETE", { ...body, expectedRevision: 1 }), {});
    expect(revoke?.status).toBe(200);
    expect(revoked[0]?.expectedRevision).toBe(1);
  });

  test("fails closed for an unowned Pylon", async () => {
    const routes = makePortableTargetPylonBindingRoutes({
      authenticate: async () => undefined,
      withStore: async () => {
        throw new Error("must not open storage");
      },
    });
    const response = await routes.route(request("POST", body), {});
    expect(response?.status).toBe(401);
  });
});
