import { Effect, Option } from "effect";
import { describe, expect, it } from "vitest";

import {
  listOwnerGatewayDeploymentsImpl,
  listOwnerGatewayEventsImpl,
  listPaywallControlPlaneStateImpl,
  recordGatewayCompileIntentImpl,
  recordGatewayDeploymentEventImpl,
} from "../../convex/lightning/ops";
import { makeInMemoryDb } from "./inMemoryDb";

const run = <A>(effect: Effect.Effect<A>) => Effect.runPromise(effect);

const makeCtx = (db: any, subject?: string) => ({
  db,
  auth: {
    getUserIdentity: () =>
      Effect.succeed(subject ? Option.some({ subject }) : Option.none()),
  },
});

describe("convex/lightning ops control-plane", () => {
  it("lists paywall control-plane state with policy and routes when secret is valid", async () => {
    const db = makeInMemoryDb();
    const ctx = makeCtx(db);
    const prevSecret = process.env.OA_LIGHTNING_OPS_SECRET;
    process.env.OA_LIGHTNING_OPS_SECRET = "ops-secret";

    try {
      await db.insert("l402Paywalls", {
        paywallId: "pw_1",
        ownerId: "owner_1",
        name: "Premium Data",
        status: "active",
        createdAtMs: 1,
        updatedAtMs: 2,
      });
      await db.insert("l402PaywallPolicies", {
        paywallId: "pw_1",
        ownerId: "owner_1",
        pricingMode: "fixed",
        fixedAmountMsats: 2_500,
        killSwitch: false,
        createdAtMs: 1,
        updatedAtMs: 2,
      });
      await db.insert("l402PaywallRoutes", {
        routeId: "route_1",
        paywallId: "pw_1",
        ownerId: "owner_1",
        hostPattern: "openagents.com",
        pathPattern: "/api/premium",
        upstreamUrl: "https://api.example.com/premium",
        protocol: "https",
        timeoutMs: 5_000,
        priority: 1,
        createdAtMs: 1,
        updatedAtMs: 2,
      });

      const listed = await run(
        listPaywallControlPlaneStateImpl(ctx, {
          secret: "ops-secret",
          statuses: ["active"],
        }),
      );

      expect(listed.ok).toBe(true);
      expect(listed.paywalls).toHaveLength(1);
      expect(listed.paywalls[0]).toMatchObject({
        paywallId: "pw_1",
        ownerId: "owner_1",
        status: "active",
        policy: {
          pricingMode: "fixed",
          fixedAmountMsats: 2_500,
          killSwitch: false,
        },
      });
      expect(listed.paywalls[0]?.routes[0]).toMatchObject({
        routeId: "route_1",
        hostPattern: "openagents.com",
        pathPattern: "/api/premium",
      });
    } finally {
      process.env.OA_LIGHTNING_OPS_SECRET = prevSecret;
    }
  });

  it("rejects secret mismatches deterministically", async () => {
    const db = makeInMemoryDb();
    const ctx = makeCtx(db);
    const prevSecret = process.env.OA_LIGHTNING_OPS_SECRET;
    process.env.OA_LIGHTNING_OPS_SECRET = "ops-secret";

    try {
      await expect(
        run(
          listPaywallControlPlaneStateImpl(ctx, {
            secret: "wrong-secret",
          }),
        ),
      ).rejects.toThrow(/forbidden/);
    } finally {
      process.env.OA_LIGHTNING_OPS_SECRET = prevSecret;
    }
  });

  it("records compile intent with configHash and diagnostic metadata", async () => {
    const db = makeInMemoryDb();
    const ctx = makeCtx(db);
    const prevSecret = process.env.OA_LIGHTNING_OPS_SECRET;
    process.env.OA_LIGHTNING_OPS_SECRET = "ops-secret";

    try {
      const first = await run(
        recordGatewayCompileIntentImpl(ctx, {
          secret: "ops-secret",
          deploymentId: "dep_fixed_1",
          configHash: "cfg_abc123",
          status: "pending",
          diagnostics: [{ code: "duplicate_route", message: "duplicate route" }],
          metadata: { source: "lightning-ops" },
          requestId: "req_1",
        }),
      );

      expect(first.ok).toBe(true);
      expect(first.deployment).toMatchObject({
        deploymentId: "dep_fixed_1",
        configHash: "cfg_abc123",
        status: "pending",
      });
      expect((first.deployment.diagnostics as any)?.metadata?.source).toBe("lightning-ops");

      const second = await run(
        recordGatewayCompileIntentImpl(ctx, {
          secret: "ops-secret",
          deploymentId: "dep_fixed_1",
          configHash: "cfg_def456",
          status: "failed",
          diagnostics: [{ code: "missing_pricing", message: "pricing required" }],
          requestId: "req_2",
        }),
      );

      expect(second.deployment).toMatchObject({
        deploymentId: "dep_fixed_1",
        configHash: "cfg_def456",
        status: "failed",
      });
      expect(db.__tables.l402GatewayDeployments).toHaveLength(1);
    } finally {
      process.env.OA_LIGHTNING_OPS_SECRET = prevSecret;
    }
  });

  it("records gateway deployment events with correlation metadata", async () => {
    const db = makeInMemoryDb();
    const ctx = makeCtx(db);
    const prevSecret = process.env.OA_LIGHTNING_OPS_SECRET;
    process.env.OA_LIGHTNING_OPS_SECRET = "ops-secret";

    try {
      const result = await run(
        recordGatewayDeploymentEventImpl(ctx, {
          secret: "ops-secret",
          paywallId: "pw_1",
          ownerId: "owner_1",
          eventType: "gateway_reconcile_health_ok",
          level: "info",
          requestId: "req_health_1",
          deploymentId: "dep_health_1",
          configHash: "cfg_health_1",
          executionPath: "hosted-node",
          metadata: {
            statusCode: 200,
          },
        }),
      );

      expect(result.ok).toBe(true);
      expect(result.event).toMatchObject({
        paywallId: "pw_1",
        ownerId: "owner_1",
        eventType: "gateway_reconcile_health_ok",
        level: "info",
        requestId: "req_health_1",
      });

      const events = db.__tables.l402GatewayEvents;
      expect(events).toHaveLength(1);
      expect(events[0]?.metadata?.executionPath).toBe("hosted-node");
      expect(events[0]?.metadata?.deploymentId).toBe("dep_health_1");
      expect(events[0]?.metadata?.configHash).toBe("cfg_health_1");
    } finally {
      process.env.OA_LIGHTNING_OPS_SECRET = prevSecret;
    }
  });

  it("lists owner deployments and events with deterministic ordering and pagination", async () => {
    const db = makeInMemoryDb();
    const ctx = makeCtx(db, "owner_1");

    await db.insert("l402GatewayDeployments", {
      deploymentId: "dep_2",
      paywallId: "pw_1",
      ownerId: "owner_1",
      configHash: "cfg_2",
      status: "failed",
      createdAtMs: 10,
      updatedAtMs: 30,
    });
    await db.insert("l402GatewayDeployments", {
      deploymentId: "dep_1",
      paywallId: "pw_1",
      ownerId: "owner_1",
      configHash: "cfg_1",
      status: "applied",
      createdAtMs: 5,
      updatedAtMs: 40,
    });
    await db.insert("l402GatewayDeployments", {
      deploymentId: "dep_other",
      paywallId: "pw_other",
      ownerId: "owner_2",
      configHash: "cfg_other",
      status: "applied",
      createdAtMs: 3,
      updatedAtMs: 50,
    });

    await db.insert("l402GatewayEvents", {
      eventId: "evt_2",
      paywallId: "pw_1",
      ownerId: "owner_1",
      eventType: "gateway_reconcile_failed",
      level: "error",
      requestId: "req_2",
      createdAtMs: 120,
    });
    await db.insert("l402GatewayEvents", {
      eventId: "evt_1",
      paywallId: "pw_1",
      ownerId: "owner_1",
      eventType: "gateway_reconcile_ok",
      level: "info",
      requestId: "req_1",
      createdAtMs: 140,
    });
    await db.insert("l402GatewayEvents", {
      eventId: "evt_other",
      paywallId: "pw_other",
      ownerId: "owner_2",
      eventType: "gateway_reconcile_ok",
      level: "info",
      requestId: "req_other",
      createdAtMs: 150,
    });

    const deployments = await run(
      listOwnerGatewayDeploymentsImpl(ctx, {
        paywallId: "pw_1",
        limit: 1,
      }),
    );

    expect(deployments.ok).toBe(true);
    expect(deployments.deployments).toHaveLength(1);
    expect(deployments.deployments[0]?.deploymentId).toBe("dep_1");
    expect(deployments.nextCursor).toBe(40);

    const deploymentsPageTwo = await run(
      listOwnerGatewayDeploymentsImpl(ctx, {
        paywallId: "pw_1",
        beforeUpdatedAtMs: deployments.nextCursor ?? undefined,
        limit: 10,
      }),
    );
    expect(deploymentsPageTwo.deployments).toHaveLength(1);
    expect(deploymentsPageTwo.deployments[0]?.deploymentId).toBe("dep_2");

    const events = await run(
      listOwnerGatewayEventsImpl(ctx, {
        paywallId: "pw_1",
        limit: 1,
      }),
    );

    expect(events.ok).toBe(true);
    expect(events.events).toHaveLength(1);
    expect(events.events[0]?.eventId).toBe("evt_1");
    expect(events.nextCursor).toBe(140);

    const eventsPageTwo = await run(
      listOwnerGatewayEventsImpl(ctx, {
        paywallId: "pw_1",
        beforeCreatedAtMs: events.nextCursor ?? undefined,
        limit: 10,
      }),
    );

    expect(eventsPageTwo.events).toHaveLength(1);
    expect(eventsPageTwo.events[0]?.eventId).toBe("evt_2");
  });
});
