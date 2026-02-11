import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test";
import { getFunctionName } from "convex/server";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const FIXED_NOW = 1_733_000_000_000;

type PaywallPolicy = {
  readonly paywallId: string;
  readonly ownerId: string;
  readonly pricingMode: "fixed";
  readonly fixedAmountMsats: number;
  readonly maxPerRequestMsats?: number;
  readonly allowedHosts?: ReadonlyArray<string>;
  readonly blockedHosts?: ReadonlyArray<string>;
  readonly quotaPerMinute?: number;
  readonly quotaPerDay?: number;
  readonly killSwitch: boolean;
  readonly createdAtMs: number;
  readonly updatedAtMs: number;
};

type PaywallRoute = {
  readonly routeId: string;
  readonly paywallId: string;
  readonly ownerId: string;
  readonly hostPattern: string;
  readonly pathPattern: string;
  readonly upstreamUrl: string;
  readonly protocol: "http" | "https";
  readonly timeoutMs: number;
  readonly priority: number;
  readonly createdAtMs: number;
  readonly updatedAtMs: number;
};

type PaywallRecord = {
  readonly paywallId: string;
  readonly ownerId: string;
  readonly name: string;
  readonly description?: string;
  readonly status: "active" | "paused" | "archived";
  readonly requestId?: string;
  readonly metadata?: unknown;
  readonly createdAtMs: number;
  readonly updatedAtMs: number;
  readonly policy: PaywallPolicy;
  readonly routes: ReadonlyArray<PaywallRoute>;
};

const state = vi.hoisted(() => ({
  authed: true,
  userId: "user-paywall-1",
  nextPaywall: 1,
  nextRoute: 1,
  paywalls: [] as Array<PaywallRecord>,
  mutationCalls: [] as Array<{ readonly ref: unknown; readonly args: unknown }>,
  queryCalls: [] as Array<{ readonly ref: unknown; readonly args: unknown }>,
  actionCalls: 0,
}));

vi.mock("@workos/authkit-session", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@workos/authkit-session")>();
  return {
    ...actual,
    createAuthService: () => ({
      withAuth: async (_request: Request) => {
        if (!state.authed) return { auth: { user: null }, refreshedSessionData: undefined };
        return {
          auth: {
            user: { id: state.userId, email: "paywall@example.com", firstName: "Pay", lastName: "Wall" },
            sessionId: "sess-paywall-1",
            accessToken: "token-paywall-1",
          },
          refreshedSessionData: undefined,
        };
      },
      saveSession: async (_auth: unknown, _sessionData: string) => ({ headers: {} as Record<string, string> }),
    }),
  };
});

vi.mock("../../src/effect/convex", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../src/effect/convex")>();
  const { Effect, Layer, Stream } = await import("effect");

  const clonePaywall = <A>(value: A): A => structuredClone(value);

  const normalizeHost = (value: string): string => value.trim().toLowerCase().replace(/^https?:\/\//, "").replace(/\/+$/, "");

  const makePolicy = (
    paywallId: string,
    ownerId: string,
    input: Record<string, unknown>,
    now: number,
    previous?: PaywallPolicy,
  ): PaywallPolicy => {
    const fixedAmountMsats = Math.floor(Number(input.fixedAmountMsats));
    if (!Number.isFinite(fixedAmountMsats) || fixedAmountMsats < 0) {
      throw new Error("invalid_input");
    }

    const maxPerRequestMsats =
      input.maxPerRequestMsats === undefined ? previous?.maxPerRequestMsats : Math.floor(Number(input.maxPerRequestMsats));
    if (maxPerRequestMsats !== undefined && (!Number.isFinite(maxPerRequestMsats) || maxPerRequestMsats < 0)) {
      throw new Error("invalid_input");
    }
    if (maxPerRequestMsats !== undefined && fixedAmountMsats > maxPerRequestMsats) {
      throw new Error("policy_violation");
    }

    const allowedHostsInput = Array.isArray(input.allowedHosts)
      ? input.allowedHosts.map((host) => String(host))
      : previous?.allowedHosts;
    const blockedHostsInput = Array.isArray(input.blockedHosts)
      ? input.blockedHosts.map((host) => String(host))
      : previous?.blockedHosts;

    const allowedHosts =
      allowedHostsInput && allowedHostsInput.length > 0
        ? [...new Set(allowedHostsInput.map(normalizeHost).filter((host) => host.length > 0))].sort((a, b) =>
            a.localeCompare(b),
          )
        : undefined;
    const blockedHosts =
      blockedHostsInput && blockedHostsInput.length > 0
        ? [...new Set(blockedHostsInput.map(normalizeHost).filter((host) => host.length > 0))].sort((a, b) =>
            a.localeCompare(b),
          )
        : undefined;

    if (allowedHosts && blockedHosts) {
      const blocked = new Set(blockedHosts);
      if (allowedHosts.some((host) => blocked.has(host))) {
        throw new Error("policy_violation");
      }
    }

    const quotaPerMinute =
      input.quotaPerMinute === undefined
        ? previous?.quotaPerMinute
        : Math.floor(Number(input.quotaPerMinute));
    const quotaPerDay =
      input.quotaPerDay === undefined
        ? previous?.quotaPerDay
        : Math.floor(Number(input.quotaPerDay));

    if (quotaPerMinute !== undefined && (!Number.isFinite(quotaPerMinute) || quotaPerMinute <= 0)) {
      throw new Error("invalid_input");
    }
    if (quotaPerDay !== undefined && (!Number.isFinite(quotaPerDay) || quotaPerDay <= 0)) {
      throw new Error("invalid_input");
    }

    const killSwitch =
      input.killSwitch === undefined
        ? (previous?.killSwitch ?? false)
        : input.killSwitch === true;

    return {
      paywallId,
      ownerId,
      pricingMode: "fixed",
      fixedAmountMsats,
      maxPerRequestMsats,
      allowedHosts,
      blockedHosts,
      quotaPerMinute,
      quotaPerDay,
      killSwitch,
      createdAtMs: previous?.createdAtMs ?? now,
      updatedAtMs: now,
    };
  };

  const makeRoutes = (
    paywallId: string,
    ownerId: string,
    input: ReadonlyArray<Record<string, unknown>>,
    now: number,
    currentPaywallId?: string,
  ): ReadonlyArray<PaywallRoute> => {
    if (input.length === 0) throw new Error("invalid_input");

    const dedupe = new Set<string>();
    const routes = input.map((route, index) => {
      const hostPattern = String(route.hostPattern ?? "").trim();
      const pathPatternRaw = String(route.pathPattern ?? "").trim();
      const pathPattern = pathPatternRaw.startsWith("/") ? pathPatternRaw : `/${pathPatternRaw}`;
      const upstreamUrl = String(route.upstreamUrl ?? "").trim();
      if (!hostPattern || !pathPatternRaw || !upstreamUrl) throw new Error("invalid_input");

      const parsed = new URL(upstreamUrl);
      const protocol = parsed.protocol === "http:" ? "http" : parsed.protocol === "https:" ? "https" : null;
      if (!protocol) throw new Error("policy_violation");

      const timeoutMs =
        route.timeoutMs === undefined ? 15_000 : Math.floor(Number(route.timeoutMs));
      if (!Number.isFinite(timeoutMs) || timeoutMs < 100 || timeoutMs > 120_000) {
        throw new Error("invalid_input");
      }

      const priority =
        route.priority === undefined ? index + 1 : Math.floor(Number(route.priority));
      if (!Number.isFinite(priority) || priority <= 0) throw new Error("invalid_input");

      const key = `${hostPattern}::${pathPattern}`;
      if (dedupe.has(key)) throw new Error("route_conflict");
      dedupe.add(key);

      for (const paywall of state.paywalls) {
        if (paywall.paywallId === currentPaywallId) continue;
        const conflict = paywall.routes.some(
          (savedRoute) => savedRoute.hostPattern === hostPattern && savedRoute.pathPattern === pathPattern,
        );
        if (conflict) throw new Error("route_conflict");
      }

      return {
        routeId: `route-${state.nextRoute++}`,
        paywallId,
        ownerId,
        hostPattern,
        pathPattern,
        upstreamUrl,
        protocol,
        timeoutMs,
        priority,
        createdAtMs: now,
        updatedAtMs: now,
      } satisfies PaywallRoute;
    });

    return routes;
  };

  const assertPolicyAllowsRoutes = (policy: PaywallPolicy, routes: ReadonlyArray<PaywallRoute>) => {
    const allowed = policy.allowedHosts ? new Set(policy.allowedHosts) : null;
    const blocked = policy.blockedHosts ? new Set(policy.blockedHosts) : null;
    for (const route of routes) {
      const routeHost = normalizeHost(new URL(route.upstreamUrl).host);
      if (allowed && !allowed.has(routeHost)) throw new Error("policy_violation");
      if (blocked && blocked.has(routeHost)) throw new Error("policy_violation");
    }
  };

  const requirePaywall = (paywallId: string): PaywallRecord => {
    const paywall = state.paywalls.find((item) => item.paywallId === paywallId);
    if (!paywall) throw new Error("paywall_not_found");
    if (paywall.ownerId !== state.userId) throw new Error("forbidden");
    return paywall;
  };

  const mutation = (ref: unknown, args: unknown) =>
    Effect.sync(() => {
      state.mutationCalls.push({ ref, args });
      const functionName = getFunctionName(ref as never);
      const now = FIXED_NOW + state.mutationCalls.length;
      const payload = (args ?? {}) as Record<string, unknown>;

      if (functionName === "lightning/paywalls:createPaywall") {
        const paywallId = `pw-${state.nextPaywall++}`;
        const ownerId = state.userId;
        const policy = makePolicy(paywallId, ownerId, payload.policy as Record<string, unknown>, now);
        const routes = makeRoutes(paywallId, ownerId, (payload.routes as Array<Record<string, unknown>>) ?? [], now);
        assertPolicyAllowsRoutes(policy, routes);

        const statusInput = payload.status === "paused" ? "paused" : "active";
        const status = policy.killSwitch ? "paused" : statusInput;

        const paywall: PaywallRecord = {
          paywallId,
          ownerId,
          name: String(payload.name ?? ""),
          description: typeof payload.description === "string" ? payload.description : undefined,
          status,
          requestId: typeof payload.requestId === "string" ? payload.requestId : undefined,
          metadata: payload.metadata,
          createdAtMs: now,
          updatedAtMs: now,
          policy,
          routes,
        };

        state.paywalls.push(paywall);
        return {
          ok: true as const,
          paywall: clonePaywall(paywall),
        };
      }

      if (functionName === "lightning/paywalls:updatePaywall") {
        const paywall = requirePaywall(String(payload.paywallId ?? ""));
        if (paywall.status === "archived") throw new Error("invalid_transition");

        const policy = payload.policy
          ? makePolicy(paywall.paywallId, paywall.ownerId, payload.policy as Record<string, unknown>, now, paywall.policy)
          : paywall.policy;
        const routes = payload.routes
          ? makeRoutes(
              paywall.paywallId,
              paywall.ownerId,
              payload.routes as Array<Record<string, unknown>>,
              now,
              paywall.paywallId,
            )
          : paywall.routes;

        assertPolicyAllowsRoutes(policy, routes);

        const nextStatus = policy.killSwitch && paywall.status === "active" ? "paused" : paywall.status;

        const updated: PaywallRecord = {
          ...paywall,
          name: typeof payload.name === "string" ? payload.name : paywall.name,
          description: typeof payload.description === "string" ? payload.description : paywall.description,
          status: nextStatus,
          requestId: typeof payload.requestId === "string" ? payload.requestId : paywall.requestId,
          metadata: payload.metadata ?? paywall.metadata,
          updatedAtMs: now,
          policy,
          routes,
        };

        const index = state.paywalls.findIndex((item) => item.paywallId === paywall.paywallId);
        state.paywalls[index] = updated;

        return {
          ok: true as const,
          paywall: clonePaywall(updated),
        };
      }

      if (functionName === "lightning/paywalls:pausePaywall") {
        const paywall = requirePaywall(String(payload.paywallId ?? ""));
        if (paywall.status === "archived") throw new Error("invalid_transition");
        if (paywall.status === "paused") {
          return {
            ok: true as const,
            changed: false,
            paywall: clonePaywall(paywall),
          };
        }
        if (paywall.status !== "active") throw new Error("invalid_transition");

        const updated = {
          ...paywall,
          status: "paused" as const,
          updatedAtMs: now,
        };
        const index = state.paywalls.findIndex((item) => item.paywallId === paywall.paywallId);
        state.paywalls[index] = updated;

        return {
          ok: true as const,
          changed: true,
          paywall: clonePaywall(updated),
        };
      }

      if (functionName === "lightning/paywalls:resumePaywall") {
        const paywall = requirePaywall(String(payload.paywallId ?? ""));
        if (paywall.status === "archived") throw new Error("invalid_transition");
        if (paywall.policy.killSwitch) throw new Error("policy_violation");
        if (paywall.status === "active") {
          return {
            ok: true as const,
            changed: false,
            paywall: clonePaywall(paywall),
          };
        }
        if (paywall.status !== "paused") throw new Error("invalid_transition");

        const updated = {
          ...paywall,
          status: "active" as const,
          updatedAtMs: now,
        };
        const index = state.paywalls.findIndex((item) => item.paywallId === paywall.paywallId);
        state.paywalls[index] = updated;

        return {
          ok: true as const,
          changed: true,
          paywall: clonePaywall(updated),
        };
      }

      throw new Error(`Unexpected mutation function: ${functionName}`);
    });

  const query = (ref: unknown, args: unknown) =>
    Effect.sync(() => {
      state.queryCalls.push({ ref, args });
      const functionName = getFunctionName(ref as never);
      const payload = (args ?? {}) as Record<string, unknown>;

      if (functionName === "lightning/paywalls:getPaywall") {
        const paywall = requirePaywall(String(payload.paywallId ?? ""));
        return {
          ok: true as const,
          paywall: clonePaywall(paywall),
        };
      }

      if (functionName === "lightning/paywalls:listPaywalls") {
        const status = typeof payload.status === "string" ? payload.status : undefined;
        const limit =
          typeof payload.limit === "number" && Number.isFinite(payload.limit)
            ? Math.max(1, Math.min(200, Math.floor(payload.limit)))
            : 50;

        const filtered = state.paywalls
          .filter((paywall) => paywall.ownerId === state.userId)
          .filter((paywall) => (status ? paywall.status === status : true))
          .sort((a, b) => b.updatedAtMs - a.updatedAtMs)
          .slice(0, limit)
          .map((paywall) => clonePaywall(paywall));

        return {
          ok: true as const,
          paywalls: filtered,
        };
      }

      throw new Error(`Unexpected query function: ${functionName}`);
    });

  const action = (_ref: unknown, _args: unknown) =>
    Effect.sync(() => {
      state.actionCalls += 1;
      throw new Error("convex.action not expected");
    });

  const subscribeQuery = (_ref: unknown, _args: unknown) =>
    Stream.fail(new Error("convex.subscribeQuery not used in this test"));

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, { query, mutation, action, subscribeQuery });
  return { ...actual, ConvexServiceLive };
});

const { default: worker } = await import("../../src/effuse-host/worker");

const makeEnv = (): WorkerEnv =>
  Object.assign(Object.create(env as WorkerEnv), {
    AI: {},
  });

describe("apps/web worker lightning paywall endpoints", () => {
  it("requires auth for POST /api/lightning/paywalls", async () => {
    state.authed = false;
    state.paywalls.length = 0;
    state.mutationCalls.length = 0;
    state.queryCalls.length = 0;
    state.actionCalls = 0;

    const req = new Request("http://example.com/api/lightning/paywalls", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-paywall-auth-1",
      },
      body: JSON.stringify({
        name: "Unauthorized create",
        policy: {
          pricingMode: "fixed",
          fixedAmountMsats: 1_000,
        },
        routes: [
          {
            hostPattern: "openagents.com",
            pathPattern: "/api/unauth",
            upstreamUrl: "https://example.com/unauth",
          },
        ],
      }),
    });

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, makeEnv(), ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(401);
    const body = (await res.json()) as { ok: boolean; error: string };
    expect(body.ok).toBe(false);
    expect(body.error).toContain("unauthorized");
    expect(state.mutationCalls).toHaveLength(0);
    expect(state.queryCalls).toHaveLength(0);
  });

  it("supports create/list/get/update with typed payloads and request correlation", async () => {
    state.authed = true;
    state.userId = "user-paywall-1";
    state.nextPaywall = 1;
    state.nextRoute = 1;
    state.paywalls.length = 0;
    state.mutationCalls.length = 0;
    state.queryCalls.length = 0;
    state.actionCalls = 0;

    const createReq = new Request("http://example.com/api/lightning/paywalls", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-paywall-create-1",
      },
      body: JSON.stringify({
        name: "Premium weather paywall",
        description: "Daily premium weather dataset",
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
            pathPattern: "/api/premium-weather",
            upstreamUrl: "https://api.example.com/weather",
            protocol: "https",
            timeoutMs: 5_000,
            priority: 1,
          },
        ],
      }),
    });

    const createCtx = createExecutionContext();
    const createRes = await worker.fetch(createReq, makeEnv(), createCtx);
    await waitOnExecutionContext(createCtx);

    expect(createRes.status).toBe(200);
    const createBody = (await createRes.json()) as {
      ok: boolean;
      requestId: string | null;
      paywall: {
        paywallId: string;
        ownerId: string;
        name: string;
        status: string;
        policy: { fixedAmountMsats: number };
        routes: Array<{ hostPattern: string; pathPattern: string }>;
      };
    };

    expect(createBody).toMatchObject({
      ok: true,
      requestId: "req-paywall-create-1",
      paywall: {
        paywallId: "pw-1",
        ownerId: "user-paywall-1",
        name: "Premium weather paywall",
        status: "active",
        policy: { fixedAmountMsats: 5_000 },
        routes: [{ hostPattern: "openagents.com", pathPattern: "/api/premium-weather" }],
      },
    });

    const listReq = new Request("http://example.com/api/lightning/paywalls?status=active&limit=10", {
      method: "GET",
      headers: { "x-oa-request-id": "req-paywall-list-1" },
    });
    const listCtx = createExecutionContext();
    const listRes = await worker.fetch(listReq, makeEnv(), listCtx);
    await waitOnExecutionContext(listCtx);

    expect(listRes.status).toBe(200);
    const listBody = (await listRes.json()) as {
      ok: boolean;
      requestId: string | null;
      paywalls: Array<{ paywallId: string; status: string }>;
    };
    expect(listBody.ok).toBe(true);
    expect(listBody.requestId).toBe("req-paywall-list-1");
    expect(listBody.paywalls).toHaveLength(1);
    expect(listBody.paywalls[0]).toMatchObject({ paywallId: "pw-1", status: "active" });

    const getReq = new Request("http://example.com/api/lightning/paywalls/pw-1", {
      method: "GET",
      headers: { "x-oa-request-id": "req-paywall-get-1" },
    });
    const getCtx = createExecutionContext();
    const getRes = await worker.fetch(getReq, makeEnv(), getCtx);
    await waitOnExecutionContext(getCtx);

    expect(getRes.status).toBe(200);
    const getBody = (await getRes.json()) as {
      ok: boolean;
      requestId: string | null;
      paywall: {
        paywallId: string;
        name: string;
      };
    };
    expect(getBody.ok).toBe(true);
    expect(getBody.requestId).toBe("req-paywall-get-1");
    expect(getBody.paywall.paywallId).toBe("pw-1");

    const updateReq = new Request("http://example.com/api/lightning/paywalls/pw-1", {
      method: "PATCH",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-paywall-update-1",
      },
      body: JSON.stringify({
        name: "Premium weather paywall v2",
        description: "Updated weather dataset",
      }),
    });
    const updateCtx = createExecutionContext();
    const updateRes = await worker.fetch(updateReq, makeEnv(), updateCtx);
    await waitOnExecutionContext(updateCtx);

    expect(updateRes.status).toBe(200);
    const updateBody = (await updateRes.json()) as {
      ok: boolean;
      requestId: string | null;
      paywall: { paywallId: string; name: string; description?: string };
    };
    expect(updateBody.ok).toBe(true);
    expect(updateBody.requestId).toBe("req-paywall-update-1");
    expect(updateBody.paywall).toMatchObject({
      paywallId: "pw-1",
      name: "Premium weather paywall v2",
      description: "Updated weather dataset",
    });

    expect(state.actionCalls).toBe(0);
  });

  it("enforces pause/resume transitions and kill-switch policy_violation", async () => {
    state.authed = true;
    state.userId = "user-paywall-1";
    state.nextPaywall = 1;
    state.nextRoute = 1;
    state.paywalls.length = 0;
    state.mutationCalls.length = 0;
    state.queryCalls.length = 0;

    const createReq = new Request("http://example.com/api/lightning/paywalls", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-paywall-lifecycle-create",
      },
      body: JSON.stringify({
        name: "Lifecycle paywall",
        policy: {
          pricingMode: "fixed",
          fixedAmountMsats: 4_000,
          killSwitch: false,
        },
        routes: [
          {
            hostPattern: "openagents.com",
            pathPattern: "/api/lifecycle",
            upstreamUrl: "https://api.example.com/lifecycle",
          },
        ],
      }),
    });

    const createCtx = createExecutionContext();
    const createRes = await worker.fetch(createReq, makeEnv(), createCtx);
    await waitOnExecutionContext(createCtx);
    expect(createRes.status).toBe(200);

    const pauseReq = new Request("http://example.com/api/lightning/paywalls/pw-1/pause", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-paywall-pause-1",
      },
      body: JSON.stringify({ reason: "maintenance" }),
    });
    const pauseCtx = createExecutionContext();
    const pauseRes = await worker.fetch(pauseReq, makeEnv(), pauseCtx);
    await waitOnExecutionContext(pauseCtx);

    expect(pauseRes.status).toBe(200);
    const pauseBody = (await pauseRes.json()) as {
      ok: boolean;
      changed: boolean;
      paywall: { status: string };
    };
    expect(pauseBody.ok).toBe(true);
    expect(pauseBody.changed).toBe(true);
    expect(pauseBody.paywall.status).toBe("paused");

    const pauseAgainCtx = createExecutionContext();
    const pauseAgainRes = await worker.fetch(pauseReq, makeEnv(), pauseAgainCtx);
    await waitOnExecutionContext(pauseAgainCtx);
    expect(pauseAgainRes.status).toBe(200);
    const pauseAgainBody = (await pauseAgainRes.json()) as { changed: boolean; paywall: { status: string } };
    expect(pauseAgainBody.changed).toBe(false);
    expect(pauseAgainBody.paywall.status).toBe("paused");

    const resumeReq = new Request("http://example.com/api/lightning/paywalls/pw-1/resume", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-paywall-resume-1",
      },
      body: JSON.stringify({ reason: "resume" }),
    });
    const resumeCtx = createExecutionContext();
    const resumeRes = await worker.fetch(resumeReq, makeEnv(), resumeCtx);
    await waitOnExecutionContext(resumeCtx);

    expect(resumeRes.status).toBe(200);
    const resumeBody = (await resumeRes.json()) as {
      ok: boolean;
      changed: boolean;
      paywall: { status: string };
    };
    expect(resumeBody.ok).toBe(true);
    expect(resumeBody.changed).toBe(true);
    expect(resumeBody.paywall.status).toBe("active");

    const killSwitchUpdateReq = new Request("http://example.com/api/lightning/paywalls/pw-1", {
      method: "PATCH",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-paywall-update-kill-switch-1",
      },
      body: JSON.stringify({
        policy: {
          pricingMode: "fixed",
          fixedAmountMsats: 4_000,
          killSwitch: true,
        },
      }),
    });

    const killSwitchCtx = createExecutionContext();
    const killSwitchRes = await worker.fetch(killSwitchUpdateReq, makeEnv(), killSwitchCtx);
    await waitOnExecutionContext(killSwitchCtx);
    expect(killSwitchRes.status).toBe(200);

    const blockedResumeReq = new Request("http://example.com/api/lightning/paywalls/pw-1/resume", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-paywall-resume-blocked-1",
      },
      body: JSON.stringify({ reason: "blocked by kill switch" }),
    });

    const blockedResumeCtx = createExecutionContext();
    const blockedResumeRes = await worker.fetch(blockedResumeReq, makeEnv(), blockedResumeCtx);
    await waitOnExecutionContext(blockedResumeCtx);

    expect(blockedResumeRes.status).toBe(422);
    const blockedBody = (await blockedResumeRes.json()) as { ok: boolean; error: string };
    expect(blockedBody.ok).toBe(false);
    expect(blockedBody.error).toContain("policy_violation");
  });

  it("returns deterministic conflict/validation statuses and method guards", async () => {
    state.authed = true;
    state.userId = "user-paywall-1";
    state.nextPaywall = 1;
    state.nextRoute = 1;
    state.paywalls.length = 0;
    state.mutationCalls.length = 0;
    state.queryCalls.length = 0;

    const firstReq = new Request("http://example.com/api/lightning/paywalls", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-paywall-conflict-1",
      },
      body: JSON.stringify({
        name: "First paywall",
        policy: {
          pricingMode: "fixed",
          fixedAmountMsats: 1_000,
        },
        routes: [
          {
            hostPattern: "openagents.com",
            pathPattern: "/api/conflict",
            upstreamUrl: "https://api.example.com/first",
          },
        ],
      }),
    });

    const firstCtx = createExecutionContext();
    const firstRes = await worker.fetch(firstReq, makeEnv(), firstCtx);
    await waitOnExecutionContext(firstCtx);
    expect(firstRes.status).toBe(200);

    const conflictReq = new Request("http://example.com/api/lightning/paywalls", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-paywall-conflict-2",
      },
      body: JSON.stringify({
        name: "Conflicting paywall",
        policy: {
          pricingMode: "fixed",
          fixedAmountMsats: 2_000,
        },
        routes: [
          {
            hostPattern: "openagents.com",
            pathPattern: "/api/conflict",
            upstreamUrl: "https://api.example.com/second",
          },
        ],
      }),
    });

    const conflictCtx = createExecutionContext();
    const conflictRes = await worker.fetch(conflictReq, makeEnv(), conflictCtx);
    await waitOnExecutionContext(conflictCtx);

    expect(conflictRes.status).toBe(409);
    const conflictBody = (await conflictRes.json()) as { ok: boolean; error: string };
    expect(conflictBody.ok).toBe(false);
    expect(conflictBody.error).toContain("route_conflict");

    const invalidStatusReq = new Request("http://example.com/api/lightning/paywalls?status=nope", {
      method: "GET",
      headers: { "x-oa-request-id": "req-paywall-invalid-status-1" },
    });
    const invalidStatusCtx = createExecutionContext();
    const invalidStatusRes = await worker.fetch(invalidStatusReq, makeEnv(), invalidStatusCtx);
    await waitOnExecutionContext(invalidStatusCtx);

    expect(invalidStatusRes.status).toBe(400);

    const putReq = new Request("http://example.com/api/lightning/paywalls", {
      method: "PUT",
      headers: { "x-oa-request-id": "req-paywall-method-1" },
    });
    const putCtx = createExecutionContext();
    const putRes = await worker.fetch(putReq, makeEnv(), putCtx);
    await waitOnExecutionContext(putCtx);
    expect(putRes.status).toBe(405);
  });
});
