import { describe, expect, test } from "vite-plus/test";

import { makeOwnerManagedEnvironmentEnrollmentRoutes } from "./owner-managed-environment-enrollment-routes.js";

const enrollment = {
  schema: "openagents.owner_managed_environment_enrollment.v1" as const,
  enrollmentRef: "enrollment.owner-managed.1",
  ownerRef: "owner.1",
  targetRef: "target.owner-managed.1",
  pylonRef: "pylon.owner-managed.1",
  workerInstanceRef: "worker.owner-managed.1",
  targetClass: "owner_managed" as const,
  adapterRef: "adapter.pylon.owner-managed.v1",
  compatibilityRef: "compatibility.portable-session.v1",
  isolation: "owner_host_process" as const,
  dataPosture: "owner_managed_region" as const,
  custodyPolicy: "owner_held_key" as const,
  checkpointKeyRef: "key.owner-managed.1",
  regionRef: "region.owner-managed.1",
  networkDestinationRefs: [],
  dataDestinationRefs: [],
  retentionSeconds: 3_600,
  costPolicyRef: "cost.owner-managed.owner-paid.v1",
  generation: 1,
  revision: 1,
  state: "active" as const,
  health: "ready" as const,
  evidenceRefs: ["evidence.owner-managed.1"],
  observedAt: "2026-07-22T09:00:00.000Z",
  expiresAt: "2026-07-22T09:05:00.000Z",
  revokedAt: null,
};

const body = {
  schema: "openagents.owner_managed_environment_enrollment.request.v1",
  workerInstanceRef: enrollment.workerInstanceRef,
  adapterRef: enrollment.adapterRef,
  compatibilityRef: enrollment.compatibilityRef,
  isolation: enrollment.isolation,
  checkpointKeyRef: enrollment.checkpointKeyRef,
  regionRef: enrollment.regionRef,
  networkDestinationRefs: [],
  dataDestinationRefs: [],
  retentionSeconds: enrollment.retentionSeconds,
  costPolicyRef: enrollment.costPolicyRef,
  generation: enrollment.generation,
  health: "ready",
  evidenceRefs: enrollment.evidenceRefs,
};

describe("owner-managed environment enrollment routes", () => {
  test("authenticates the Pylon and forwards refs-only enrollment authority", async () => {
    let observed: unknown;
    const routes = makeOwnerManagedEnvironmentEnrollmentRoutes<{}>({
      authenticate: async () => ({ ownerUserId: "owner.1", ownerAgentUserId: "agent.1" }),
      withStore: async (_env, use) =>
        use({
          admit: async (input) => {
            observed = input;
            return enrollment;
          },
          read: async () => enrollment,
          revoke: async () => ({ ...enrollment, revision: 2, state: "revoked", health: "revoked" }),
        }),
    });
    const response = await routes.route(
      new Request(
        "https://openagents.test/api/pylons/pylon.owner-managed.1/owner-managed-environments/target.owner-managed.1",
        {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "idempotency-key": "enroll-owner-managed-1",
          },
          body: JSON.stringify(body),
        },
      ),
      {},
    );
    expect(response?.status).toBe(201);
    expect(observed).toMatchObject({
      ownerUserId: "owner.1",
      ownerAgentUserId: "agent.1",
      pylonRef: "pylon.owner-managed.1",
      targetRef: "target.owner-managed.1",
      checkpointKeyRef: "key.owner-managed.1",
    });
    expect(JSON.stringify(observed)).not.toContain("checkpointKeyBytes");
  });

  test("refuses mutation without idempotency", async () => {
    const routes = makeOwnerManagedEnvironmentEnrollmentRoutes<{}>({
      authenticate: async () => ({ ownerUserId: "owner.1", ownerAgentUserId: "agent.1" }),
      withStore: async (_env, use) =>
        use({
          admit: async () => enrollment,
          read: async () => enrollment,
          revoke: async () => ({
            ...enrollment,
            revision: 2,
            state: "revoked",
            health: "revoked",
          }),
        }),
    });
    const response = await routes.route(
      new Request(
        "https://openagents.test/api/pylons/pylon.owner-managed.1/owner-managed-environments/target.owner-managed.1",
        { method: "POST", body: JSON.stringify(body) },
      ),
      {},
    );
    expect(response?.status).toBe(400);
  });
});
