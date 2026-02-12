import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test";
import { getFunctionName } from "convex/server";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  authed: true,
  userId: "owner-1",
  deployments: [
    {
      deploymentId: "dep_1",
      paywallId: "pw_1",
      ownerId: "owner-1",
      configHash: "cfg_1",
      status: "applied",
      createdAtMs: 1_733_000_000_000,
      updatedAtMs: 1_733_000_000_200,
    },
    {
      deploymentId: "dep_2",
      paywallId: "pw_1",
      ownerId: "owner-1",
      configHash: "cfg_2",
      status: "failed",
      diagnostics: { reason: "health_check_failed" },
      createdAtMs: 1_733_000_000_100,
      updatedAtMs: 1_733_000_000_100,
    },
    {
      deploymentId: "dep_other",
      paywallId: "pw_other",
      ownerId: "owner-2",
      configHash: "cfg_other",
      status: "pending",
      createdAtMs: 1_733_000_000_300,
      updatedAtMs: 1_733_000_000_300,
    },
  ],
  events: [
    {
      eventId: "evt_1",
      paywallId: "pw_1",
      ownerId: "owner-1",
      eventType: "gateway_reconcile_ok",
      level: "info",
      requestId: "req_evt_1",
      createdAtMs: 1_733_000_000_250,
    },
    {
      eventId: "evt_2",
      paywallId: "pw_1",
      ownerId: "owner-1",
      eventType: "gateway_reconcile_failed",
      level: "error",
      requestId: "req_evt_2",
      createdAtMs: 1_733_000_000_150,
    },
    {
      eventId: "evt_other",
      paywallId: "pw_other",
      ownerId: "owner-2",
      eventType: "gateway_reconcile_ok",
      level: "info",
      requestId: "req_evt_other",
      createdAtMs: 1_733_000_000_350,
    },
  ],
  queryCalls: [] as Array<{ readonly ref: unknown; readonly args: unknown }>,
  mutationCalls: [] as Array<{ readonly ref: unknown; readonly args: unknown }>,
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
            user: { id: state.userId, email: "owner@example.com", firstName: "Owner", lastName: "One" },
            sessionId: "sess-deployments-1",
            accessToken: "token-deployments-1",
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

  const query = (ref: unknown, args: unknown) =>
    Effect.sync(() => {
      state.queryCalls.push({ ref, args });
      const fn = getFunctionName(ref as never);
      const payload = (args ?? {}) as Record<string, unknown>;

      if (fn === "lightning/ops:listOwnerGatewayDeployments") {
        const limit = typeof payload.limit === "number" ? Math.max(1, Math.floor(payload.limit)) : 50;
        const beforeUpdatedAtMs =
          typeof payload.beforeUpdatedAtMs === "number" ? Math.floor(payload.beforeUpdatedAtMs) : undefined;
        const status = typeof payload.status === "string" ? payload.status : undefined;
        const paywallId = typeof payload.paywallId === "string" ? payload.paywallId : undefined;

        const rows = state.deployments
          .filter((row) => row.ownerId === state.userId)
          .filter((row) => (status ? row.status === status : true))
          .filter((row) => (paywallId ? row.paywallId === paywallId : true))
          .filter((row) => (beforeUpdatedAtMs !== undefined ? row.updatedAtMs < beforeUpdatedAtMs : true))
          .sort((a, b) => b.updatedAtMs - a.updatedAtMs)
          .slice(0, limit);

        return {
          ok: true as const,
          deployments: rows,
          nextCursor: rows.length === limit ? rows[rows.length - 1]?.updatedAtMs ?? null : null,
        };
      }

      if (fn === "lightning/ops:listOwnerGatewayEvents") {
        const limit = typeof payload.limit === "number" ? Math.max(1, Math.floor(payload.limit)) : 50;
        const beforeCreatedAtMs =
          typeof payload.beforeCreatedAtMs === "number" ? Math.floor(payload.beforeCreatedAtMs) : undefined;
        const level = typeof payload.level === "string" ? payload.level : undefined;
        const paywallId = typeof payload.paywallId === "string" ? payload.paywallId : undefined;

        const rows = state.events
          .filter((row) => row.ownerId === state.userId)
          .filter((row) => (level ? row.level === level : true))
          .filter((row) => (paywallId ? row.paywallId === paywallId : true))
          .filter((row) => (beforeCreatedAtMs !== undefined ? row.createdAtMs < beforeCreatedAtMs : true))
          .sort((a, b) => b.createdAtMs - a.createdAtMs)
          .slice(0, limit);

        return {
          ok: true as const,
          events: rows,
          nextCursor: rows.length === limit ? rows[rows.length - 1]?.createdAtMs ?? null : null,
        };
      }

      throw new Error(`Unexpected query function: ${fn}`);
    });

  const mutation = (ref: unknown, args: unknown) =>
    Effect.sync(() => {
      state.mutationCalls.push({ ref, args });
      throw new Error("convex.mutation not expected");
    });

  const action = (_ref: unknown, _args: unknown) =>
    Effect.sync(() => {
      state.actionCalls += 1;
      throw new Error("convex.action not expected");
    });

  const subscribeQuery = (_ref: unknown, _args: unknown) => Stream.fail(new Error("convex.subscribeQuery not used"));

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, {
    query,
    mutation,
    action,
    subscribeQuery,
  });

  return { ...actual, ConvexServiceLive };
});

const { default: worker } = await import("../../src/effuse-host/worker");

const makeEnv = (): WorkerEnv =>
  Object.assign(Object.create(env as WorkerEnv), {
    AI: {},
  });

describe("apps/web worker lightning deployments endpoints", () => {
  it("requires auth for GET /api/lightning/deployments", async () => {
    state.authed = false;
    state.queryCalls.length = 0;

    const req = new Request("http://example.com/api/lightning/deployments", {
      method: "GET",
      headers: {
        "x-oa-request-id": "req-auth-deployments-1",
      },
    });

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, makeEnv(), ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(401);
    const body = (await res.json()) as { ok: boolean; error: string };
    expect(body.ok).toBe(false);
    expect(body.error).toContain("unauthorized");
    expect(state.queryCalls).toHaveLength(0);
  });

  it("lists owner deployments with status filters and request correlation", async () => {
    state.authed = true;
    state.userId = "owner-1";
    state.queryCalls.length = 0;

    const req = new Request("http://example.com/api/lightning/deployments?paywallId=pw_1&status=applied&limit=10", {
      method: "GET",
      headers: {
        "x-oa-request-id": "req-deployments-1",
      },
    });

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, makeEnv(), ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      ok: boolean;
      requestId: string | null;
      deployments: Array<{ deploymentId: string; status: string; configHash: string }>;
      nextCursor: number | null;
    };

    expect(body.ok).toBe(true);
    expect(body.requestId).toBe("req-deployments-1");
    expect(body.deployments).toHaveLength(1);
    expect(body.deployments[0]).toMatchObject({
      deploymentId: "dep_1",
      status: "applied",
      configHash: "cfg_1",
    });
    expect(body.nextCursor).toBeNull();
  });

  it("lists owner deployment events with level filter and pagination cursor", async () => {
    state.authed = true;
    state.userId = "owner-1";
    state.queryCalls.length = 0;

    const req = new Request("http://example.com/api/lightning/deployments/events?paywallId=pw_1&level=error&limit=10", {
      method: "GET",
      headers: {
        "x-oa-request-id": "req-deployment-events-1",
      },
    });

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, makeEnv(), ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      ok: boolean;
      requestId: string | null;
      events: Array<{ eventId: string; level: string; requestId?: string }>;
      nextCursor: number | null;
    };

    expect(body.ok).toBe(true);
    expect(body.requestId).toBe("req-deployment-events-1");
    expect(body.events).toHaveLength(1);
    expect(body.events[0]).toMatchObject({
      eventId: "evt_2",
      level: "error",
      requestId: "req_evt_2",
    });
    expect(body.nextCursor).toBeNull();
  });
});
