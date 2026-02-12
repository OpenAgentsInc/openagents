import { Effect, Option } from "effect";
import { describe, expect, it } from "vitest";

import {
  listPaywallControlPlaneStateImpl,
  recordGatewayCompileIntentImpl,
} from "../../convex/lightning/ops";
import { makeInMemoryDb } from "./inMemoryDb";

const run = <A>(effect: Effect.Effect<A>) => Effect.runPromise(effect);

const makeCtx = (db: any) => ({
  db,
  auth: {
    getUserIdentity: () => Effect.succeed(Option.none()),
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
});
