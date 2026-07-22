import { describe, expect, test } from "vite-plus/test";

import { makePylonOwnerManagedEnrollmentClient } from "../src/portable-owner-managed-enrollment-client.js";

const options = {
  agentToken: "agent-token",
  baseUrl: "https://openagents.test",
  pylonRef: "pylon.owner-managed.1",
  targetRef: "target.owner-managed.1",
  workerInstanceRef: "worker.owner-managed.1",
  adapterRef: "adapter.pylon.owner-managed.v1",
  compatibilityRef: "compatibility.portable-session.v1",
  isolation: "owner_host_process" as const,
  checkpointKeyRef: "key.owner-managed.1",
  regionRef: "region.owner-managed.1",
  networkDestinationRefs: ["network.openagents.sync"],
  dataDestinationRefs: ["data.owner-managed.checkpoint"],
  retentionSeconds: 3_600,
  costPolicyRef: "cost.owner-managed.owner-paid.v1",
  generation: 1,
  evidenceRefs: ["evidence.owner-managed.1"],
};

const enrollment = (revision: number, state: "active" | "revoked") => ({
  schema: "openagents.owner_managed_environment_enrollment.v1",
  enrollmentRef: "enrollment.owner-managed.1",
  ownerRef: "owner.1",
  targetRef: options.targetRef,
  pylonRef: options.pylonRef,
  workerInstanceRef: options.workerInstanceRef,
  targetClass: "owner_managed",
  adapterRef: options.adapterRef,
  compatibilityRef: options.compatibilityRef,
  isolation: options.isolation,
  dataPosture: "owner_managed_region",
  custodyPolicy: "owner_held_key",
  checkpointKeyRef: options.checkpointKeyRef,
  regionRef: options.regionRef,
  networkDestinationRefs: options.networkDestinationRefs,
  dataDestinationRefs: options.dataDestinationRefs,
  retentionSeconds: options.retentionSeconds,
  costPolicyRef: options.costPolicyRef,
  generation: options.generation,
  revision,
  state,
  health: state === "active" ? "ready" : "revoked",
  evidenceRefs: options.evidenceRefs,
  observedAt: "2026-07-22T09:00:00.000Z",
  expiresAt: "2026-07-22T09:05:00.000Z",
  revokedAt: state === "active" ? null : "2026-07-22T09:01:00.000Z",
});

describe("Pylon owner-managed enrollment client", () => {
  test("admits, renews, and revokes without sending key bytes", async () => {
    const requests: Array<{ method: string; body: string }> = [];
    const client = makePylonOwnerManagedEnrollmentClient({
      ...options,
      fetchImpl: async (_input, init) => {
        const body = String(init?.body);
        requests.push({ method: init?.method ?? "", body });
        const revision = requests.length;
        return new Response(
          JSON.stringify({
            enrollment: enrollment(revision, init?.method === "DELETE" ? "revoked" : "active"),
          }),
          { status: revision === 1 ? 201 : 200 },
        );
      },
    });
    await expect(client.admitOrRenew()).resolves.toMatchObject({ revision: 1, state: "active" });
    await expect(client.admitOrRenew()).resolves.toMatchObject({ revision: 2, state: "active" });
    await expect(client.revoke()).resolves.toMatchObject({ revision: 3, state: "revoked" });
    expect(requests.map(({ method }) => method)).toEqual(["POST", "POST", "DELETE"]);
    expect(requests.map(({ body }) => body).join("\n")).not.toMatch(/keyBytes|privateKey|Bearer/u);
  });
});
