import {
  ManagedSandboxCommandSchema,
  type ManagedSandboxCommand,
} from "@openagentsinc/managed-sandbox-contract";
import type { ManagedSandboxCommandReservation } from "@openagentsinc/khala-sync-server";
import { Effect, Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { makeManagedSandboxBroker } from "./managed-sandbox-broker";
import type { BoxV1NativeStore, BoxV1Policy } from "./managed-sandbox-box-v1-routes";
import { unavailableBoxV1Runtime } from "./managed-sandbox-box-v1-routes";

const now = "2026-07-19T16:00:00.000Z";
const imageDigest = `sha256:${"b".repeat(64)}`;
const policy: BoxV1Policy = {
  target: {
    targetRef: "target.gcp.managed-sandbox.us-central1",
    targetClass: "openagents_managed",
    provider: "google_cloud",
    adapterRef: "adapter.oa-codex-control.gce.v1",
    region: "us-central1",
    isolation: "gce_vm",
    dataPosture: "openagents_managed_region",
  },
  imageDigest,
  profileRef: "profile.sbx.gce.e2-small.v1",
  defaultTtlSeconds: 3_600,
  maxTtlSeconds: 86_400,
  maxActiveBoxes: 2,
  maxCostMicros: 10_000,
  maxCpuMillis: 3_600_000,
  maxNetworkBytes: 100_000_000,
  maxArtifactBytes: 10_000_000,
};

const createCommand = (overrides: Record<string, unknown> = {}): ManagedSandboxCommand =>
  S.decodeUnknownSync(ManagedSandboxCommandSchema)({
    _tag: "Create",
    schema: "openagents.managed_sandbox_command.v1",
    commandRef: "command.native.create.fixture",
    requestedByRef: "principal.desktop",
    ownerRef: "owner.fixture",
    tenantRef: "owner.fixture",
    idempotencyRef: "idempotency.native.create.fixture",
    requestedAt: now,
    workUnitRef: "work.native.fixture",
    attachmentRef: "attachment.native.fixture",
    target: policy.target,
    imageDigest,
    profileRef: policy.profileRef,
    lease: {
      leaseRef: "lease.native.fixture",
      state: "active",
      issuedAt: now,
      expiresAt: "2026-07-19T17:00:00.000Z",
      ttlSeconds: 3_600,
      renewable: true,
    },
    budget: {
      currency: "USD",
      maxCostMicros: 10_000,
      maxCpuMillis: 3_600_000,
      maxNetworkBytes: 100_000_000,
      maxArtifactBytes: 10_000_000,
      maxLifetimeSeconds: 3_600,
    },
    requestedCapabilities: [
      {
        capabilityRef: "capability.native.fixture.agent_turn",
        kind: "agent_turn",
        state: "active",
        expiresAt: "2026-07-19T17:00:00.000Z",
      },
    ],
    ...overrides,
  });

const fixture = () => {
  let reserveCount = 0;
  let reserved: ManagedSandboxCommand | null = null;
  let reservation: ManagedSandboxCommandReservation | undefined;
  const store = {
    reservation: () => Effect.succeed(reservation),
    reserve: ({ command, initialResource }: Parameters<BoxV1NativeStore["reserve"]>[0]) => {
      reserveCount += 1;
      reserved = command;
      if (initialResource === undefined) throw new Error("missing initial resource");
      reservation = {
        disposition: "reserved" as const,
        status: "pending" as const,
        command,
        resource: initialResource,
      };
      return Effect.succeed(reservation);
    },
  } as unknown as BoxV1NativeStore;
  const broker = makeManagedSandboxBroker({
    principal: {
      actorRef: "principal.desktop",
      ownerRef: "owner.fixture",
      tenantRef: "owner.fixture",
      login: "Desktop",
      email: null,
    },
    policy,
    store,
    runtime: unavailableBoxV1Runtime,
    now: () => new Date(now),
  });
  return { broker, reserveCount: () => reserveCount, reserved: () => reserved };
};

describe("native managed-sandbox broker", () => {
  test("creates one canonical resource and returns an explicit accepted target receipt", async () => {
    const testbed = fixture();
    const result = await Effect.runPromise(
      testbed.broker.execute(createCommand(), { attachmentGeneration: 7 }),
    );
    expect(result.resource).toMatchObject({
      ownerRef: "owner.fixture",
      tenantRef: "owner.fixture",
      programRef: "program.managed_agent_sandboxes",
      workUnitRef: "work.native.fixture",
      attachmentGeneration: 7,
      imageDigest,
      profileRef: policy.profileRef,
      facts: { lifecycle: "provisioning", cleanupComplete: false },
    });
    expect(result.receipt).toMatchObject({
      commandRef: "command.native.create.fixture",
      outcome: "accepted",
      lifecycle: "provisioning",
    });
    expect(testbed.reserveCount()).toBe(1);
    expect(testbed.reserved()).toMatchObject({
      idempotencyRef: "idempotency.native.create.fixture",
    });
  });

  test("rejects cross-owner and target substitution before durable effects", async () => {
    for (const command of [
      createCommand({ ownerRef: "owner.other", tenantRef: "owner.other" }),
      createCommand({ imageDigest: `sha256:${"c".repeat(64)}` }),
    ]) {
      const testbed = fixture();
      await expect(
        Effect.runPromise(testbed.broker.execute(command, { attachmentGeneration: 7 })),
      ).rejects.toMatchObject({ code: "conflict" });
      expect(testbed.reserveCount()).toBe(0);
    }
  });

  test("reuses durable request time on replay and rejects changed semantic bytes", async () => {
    const testbed = fixture();
    const first = await Effect.runPromise(
      testbed.broker.execute(createCommand(), { attachmentGeneration: 7 }),
    );
    const replay = await Effect.runPromise(
      testbed.broker.execute(createCommand({ requestedAt: "2026-07-19T16:05:00.000Z" }), {
        attachmentGeneration: 7,
      }),
    );

    expect(replay.command.requestedAt).toBe(now);
    expect(replay.receipt.observedAt).toBe(now);
    expect(replay.receipt).toEqual(first.receipt);
    expect(testbed.reserveCount()).toBe(1);

    await expect(
      Effect.runPromise(
        testbed.broker.execute(
          createCommand({
            requestedAt: "2026-07-19T16:10:00.000Z",
            workUnitRef: "work.native.substituted",
          }),
          { attachmentGeneration: 7 },
        ),
      ),
    ).rejects.toMatchObject({ code: "conflict" });
    expect(testbed.reserveCount()).toBe(1);
  });

  test("requires the exact attachment generation before reserving create", async () => {
    const testbed = fixture();
    await expect(Effect.runPromise(testbed.broker.execute(createCommand()))).rejects.toMatchObject({
      code: "validation_failed",
    });
    expect(testbed.reserveCount()).toBe(0);
  });
});
