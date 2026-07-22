import { Schema } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { OwnerManagedEnvironmentEnrollmentSchema } from "./owner-managed-enrollment.js";

const enrollment = {
  schema: "openagents.owner_managed_environment_enrollment.v1",
  enrollmentRef: "enrollment.owner-managed.1",
  ownerRef: "owner.1",
  targetRef: "target.owner-managed.1",
  pylonRef: "pylon.owner-managed.1",
  workerInstanceRef: "worker.owner-managed.1",
  targetClass: "owner_managed",
  adapterRef: "adapter.pylon.owner-managed.v1",
  compatibilityRef: "compatibility.portable-session.v1",
  isolation: "owner_host_process",
  dataPosture: "owner_managed_region",
  custodyPolicy: "owner_held_key",
  checkpointKeyRef: "key.owner-managed.1",
  regionRef: "region.owner-managed.1",
  networkDestinationRefs: ["network.openagents.sync"],
  dataDestinationRefs: ["data.owner-managed.checkpoint"],
  retentionSeconds: 3_600,
  costPolicyRef: "cost.owner-managed.owner-paid.v1",
  generation: 1,
  revision: 1,
  state: "active",
  health: "ready",
  evidenceRefs: ["evidence.owner-managed.enrollment.1"],
  observedAt: "2026-07-22T09:00:00.000Z",
  expiresAt: "2026-07-22T09:05:00.000Z",
  revokedAt: null,
} as const;

describe("owner-managed environment enrollment", () => {
  test("contains refs and policy facts only", () => {
    const decoded = Schema.decodeUnknownSync(OwnerManagedEnvironmentEnrollmentSchema)(enrollment, {
      onExcessProperty: "error",
    });
    expect(decoded).toEqual(enrollment);
    expect(JSON.stringify(decoded)).not.toMatch(/private key|BEGIN|Bearer|\/Users\//u);
  });

  test.each(["checkpointKey", "credential", "endpoint", "hostPath"])(
    "rejects forbidden extra field %s",
    (field) => {
      expect(() =>
        Schema.decodeUnknownSync(OwnerManagedEnvironmentEnrollmentSchema)(
          { ...enrollment, [field]: "private" },
          { onExcessProperty: "error" },
        ),
      ).toThrow();
    },
  );
});
