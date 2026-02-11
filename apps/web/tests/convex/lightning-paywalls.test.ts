import { Effect, Option } from "effect";
import { describe, expect, it } from "vitest";

import {
  createPaywallImpl,
  getPaywallImpl,
  listPaywallsImpl,
  pausePaywallImpl,
  resumePaywallImpl,
  updatePaywallImpl,
} from "../../convex/lightning/paywalls";
import { makeInMemoryDb } from "./inMemoryDb";

const run = <A>(effect: Effect.Effect<A>) => Effect.runPromise(effect);

const authedCtx = (db: any, subject = "user-1") => ({
  db,
  auth: {
    getUserIdentity: () => Effect.succeed(Option.some({ subject })),
  },
});

describe("convex/lightning paywall control-plane", () => {
  it("creates, updates, gets, and lists hosted paywalls with typed shape", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "user-1");

    const created = await run(
      createPaywallImpl(ctx, {
        name: "Premium Weather API",
        description: "Paywalled weather feed",
        policy: {
          pricingMode: "fixed",
          fixedAmountMsats: 5_000,
          maxPerRequestMsats: 10_000,
          allowedHosts: ["api.example.com"],
          killSwitch: false,
        },
        routes: [
          {
            hostPattern: "openagents.com",
            pathPattern: "/api/weather",
            upstreamUrl: "https://api.example.com/weather",
            protocol: "https",
            timeoutMs: 5_000,
            priority: 1,
          },
        ],
      }),
    );

    expect(created.ok).toBe(true);
    expect(created.paywall.ownerId).toBe("user-1");
    expect(created.paywall.status).toBe("active");
    expect(created.paywall.routes).toHaveLength(1);
    expect(created.paywall.policy.fixedAmountMsats).toBe(5_000);

    const updated = await run(
      updatePaywallImpl(ctx, {
        paywallId: created.paywall.paywallId,
        name: "Premium Weather API v2",
        description: "Updated description",
      }),
    );

    expect(updated.paywall.name).toContain("v2");
    expect(updated.paywall.description).toContain("Updated");

    const fetched = await run(getPaywallImpl(ctx, { paywallId: created.paywall.paywallId }));
    expect(fetched.paywall.paywallId).toBe(created.paywall.paywallId);
    expect(fetched.paywall.routes[0]?.hostPattern).toBe("openagents.com");

    const listed = await run(listPaywallsImpl(ctx, { status: "active", limit: 10 }));
    expect(listed.ok).toBe(true);
    expect(listed.paywalls).toHaveLength(1);
    expect(listed.paywalls[0]?.paywallId).toBe(created.paywall.paywallId);
  });

  it("enforces owner-only access and deterministic route conflict denials", async () => {
    const db = makeInMemoryDb();
    const ownerCtx = authedCtx(db, "owner-1");
    const otherCtx = authedCtx(db, "owner-2");

    const created = await run(
      createPaywallImpl(ownerCtx, {
        name: "Owner one paywall",
        policy: {
          pricingMode: "fixed",
          fixedAmountMsats: 1_000,
          killSwitch: false,
        },
        routes: [
          {
            hostPattern: "openagents.com",
            pathPattern: "/api/owner-one",
            upstreamUrl: "https://owner-one.example.com/api",
          },
        ],
      }),
    );

    await expect(
      run(
        createPaywallImpl(otherCtx, {
          name: "conflicting route",
          policy: {
            pricingMode: "fixed",
            fixedAmountMsats: 2_000,
          },
          routes: [
            {
              hostPattern: "openagents.com",
              pathPattern: "/api/owner-one",
              upstreamUrl: "https://owner-two.example.com/api",
            },
          ],
        }),
      ),
    ).rejects.toThrow(/route_conflict/);

    await expect(run(getPaywallImpl(otherCtx, { paywallId: created.paywall.paywallId }))).rejects.toThrow(/forbidden/);
    await expect(
      run(
        updatePaywallImpl(otherCtx, {
          paywallId: created.paywall.paywallId,
          name: "forbidden update",
        }),
      ),
    ).rejects.toThrow(/forbidden/);
    await expect(run(pausePaywallImpl(otherCtx, { paywallId: created.paywall.paywallId }))).rejects.toThrow(/forbidden/);
    await expect(run(resumePaywallImpl(otherCtx, { paywallId: created.paywall.paywallId }))).rejects.toThrow(/forbidden/);
  });

  it("enforces pause/resume state graph and kill-switch behavior", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "user-1");

    const created = await run(
      createPaywallImpl(ctx, {
        name: "Lifecycle paywall",
        policy: {
          pricingMode: "fixed",
          fixedAmountMsats: 3_000,
          killSwitch: false,
        },
        routes: [
          {
            hostPattern: "openagents.com",
            pathPattern: "/api/lifecycle",
            upstreamUrl: "https://lifecycle.example.com/api",
          },
        ],
      }),
    );

    const paused = await run(
      pausePaywallImpl(ctx, {
        paywallId: created.paywall.paywallId,
      }),
    );
    expect(paused.changed).toBe(true);
    expect(paused.paywall.status).toBe("paused");

    const pausedAgain = await run(
      pausePaywallImpl(ctx, {
        paywallId: created.paywall.paywallId,
      }),
    );
    expect(pausedAgain.changed).toBe(false);
    expect(pausedAgain.paywall.status).toBe("paused");

    const resumed = await run(
      resumePaywallImpl(ctx, {
        paywallId: created.paywall.paywallId,
      }),
    );
    expect(resumed.changed).toBe(true);
    expect(resumed.paywall.status).toBe("active");

    const killSwitchUpdated = await run(
      updatePaywallImpl(ctx, {
        paywallId: created.paywall.paywallId,
        policy: {
          pricingMode: "fixed",
          fixedAmountMsats: 3_000,
          killSwitch: true,
        },
      }),
    );
    expect(killSwitchUpdated.paywall.status).toBe("paused");
    expect(killSwitchUpdated.paywall.policy.killSwitch).toBe(true);

    await expect(run(resumePaywallImpl(ctx, { paywallId: created.paywall.paywallId }))).rejects.toThrow(/policy_violation/);

    const paywallDoc = db.__tables.l402Paywalls.find((row) => row.paywallId === created.paywall.paywallId);
    if (!paywallDoc) throw new Error("missing test paywall row");
    paywallDoc.status = "archived";

    await expect(run(pausePaywallImpl(ctx, { paywallId: created.paywall.paywallId }))).rejects.toThrow(/invalid_transition/);
  });

  it("validates policy caps, allowlist/blocklist, and upstream host restrictions", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "user-1");

    await expect(
      run(
        createPaywallImpl(ctx, {
          name: "cap violation",
          policy: {
            pricingMode: "fixed",
            fixedAmountMsats: 11_000,
            maxPerRequestMsats: 10_000,
          },
          routes: [
            {
              hostPattern: "openagents.com",
              pathPattern: "/api/cap",
              upstreamUrl: "https://caps.example.com/api",
            },
          ],
        }),
      ),
    ).rejects.toThrow(/policy_violation/);

    await expect(
      run(
        createPaywallImpl(ctx, {
          name: "allowlist-blocklist overlap",
          policy: {
            pricingMode: "fixed",
            fixedAmountMsats: 1_000,
            allowedHosts: ["api.example.com"],
            blockedHosts: ["api.example.com"],
          },
          routes: [
            {
              hostPattern: "openagents.com",
              pathPattern: "/api/overlap",
              upstreamUrl: "https://api.example.com/feed",
            },
          ],
        }),
      ),
    ).rejects.toThrow(/policy_violation/);

    await expect(
      run(
        createPaywallImpl(ctx, {
          name: "allowlist host violation",
          policy: {
            pricingMode: "fixed",
            fixedAmountMsats: 1_000,
            allowedHosts: ["allowed.example.com"],
          },
          routes: [
            {
              hostPattern: "openagents.com",
              pathPattern: "/api/allowlist",
              upstreamUrl: "https://blocked.example.com/feed",
            },
          ],
        }),
      ),
    ).rejects.toThrow(/policy_violation/);

    await expect(
      run(
        createPaywallImpl(ctx, {
          name: "blocked host violation",
          policy: {
            pricingMode: "fixed",
            fixedAmountMsats: 1_000,
            blockedHosts: ["blocked.example.com"],
          },
          routes: [
            {
              hostPattern: "openagents.com",
              pathPattern: "/api/blocklist",
              upstreamUrl: "https://blocked.example.com/feed",
            },
          ],
        }),
      ),
    ).rejects.toThrow(/policy_violation/);
  });
});
