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
  settlements: [
    {
      settlementId: "set_1",
      paywallId: "pw_1",
      ownerId: "owner-1",
      invoiceId: "inv_1",
      amountMsats: 1000,
      paymentProofRef: "lightning_preimage:aaaa",
      requestId: "req_1",
      createdAtMs: 1_733_000_000_100,
      metadata: {
        receipt: {
          payment_proof: { type: "lightning_preimage", value: "a".repeat(64) },
          correlation: { task_id: "task_1", request_id: "req_1" },
        },
      },
    },
    {
      settlementId: "set_2",
      paywallId: "pw_2",
      ownerId: "owner-1",
      invoiceId: "inv_2",
      amountMsats: 2000,
      paymentProofRef: "lightning_preimage:bbbb",
      requestId: "req_2",
      createdAtMs: 1_733_000_000_000,
      metadata: {
        receipt: {
          payment_proof: { type: "lightning_preimage", value: "b".repeat(64) },
          correlation: { task_id: "task_2", request_id: "req_2" },
        },
      },
    },
    {
      settlementId: "set_other",
      paywallId: "pw_3",
      ownerId: "owner-2",
      invoiceId: "inv_3",
      amountMsats: 3000,
      paymentProofRef: "lightning_preimage:cccc",
      requestId: "req_3",
      createdAtMs: 1_733_000_000_200,
      metadata: {
        receipt: {
          payment_proof: { type: "lightning_preimage", value: "c".repeat(64) },
          correlation: { task_id: "task_3", request_id: "req_3" },
        },
      },
    },
  ],
  paywalls: [
    { paywallId: "pw_1", ownerId: "owner-1" },
    { paywallId: "pw_2", ownerId: "owner-1" },
    { paywallId: "pw_3", ownerId: "owner-2" },
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
            sessionId: "sess-settlement-1",
            accessToken: "token-settlement-1",
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

      if (fn === "lightning/settlements:listOwnerSettlements") {
        const limit = typeof payload.limit === "number" ? Math.max(1, Math.floor(payload.limit)) : 50;
        const before = typeof payload.beforeCreatedAtMs === "number" ? Math.floor(payload.beforeCreatedAtMs) : undefined;

        const rows = state.settlements
          .filter((row) => row.ownerId === state.userId)
          .filter((row) => (before !== undefined ? row.createdAtMs < before : true))
          .sort((a, b) => b.createdAtMs - a.createdAtMs)
          .slice(0, limit);

        return {
          ok: true as const,
          settlements: rows,
          nextCursor: rows.length === limit ? rows[rows.length - 1]?.createdAtMs ?? null : null,
        };
      }

      if (fn === "lightning/settlements:listPaywallSettlements") {
        const paywallId = String(payload.paywallId ?? "");
        const paywall = state.paywalls.find((row) => row.paywallId === paywallId);
        if (!paywall) throw new Error("paywall_not_found");
        if (paywall.ownerId !== state.userId) throw new Error("forbidden");

        const limit = typeof payload.limit === "number" ? Math.max(1, Math.floor(payload.limit)) : 50;
        const before = typeof payload.beforeCreatedAtMs === "number" ? Math.floor(payload.beforeCreatedAtMs) : undefined;

        const rows = state.settlements
          .filter((row) => row.ownerId === state.userId && row.paywallId === paywallId)
          .filter((row) => (before !== undefined ? row.createdAtMs < before : true))
          .sort((a, b) => b.createdAtMs - a.createdAtMs)
          .slice(0, limit);

        return {
          ok: true as const,
          settlements: rows,
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

describe("apps/web worker lightning settlements endpoints", () => {
  it("requires auth for GET /api/lightning/settlements", async () => {
    state.authed = false;
    state.queryCalls.length = 0;

    const req = new Request("http://example.com/api/lightning/settlements", {
      method: "GET",
      headers: {
        "x-oa-request-id": "req-auth-settlements-1",
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

  it("lists owner settlements with pagination and request correlation", async () => {
    state.authed = true;
    state.userId = "owner-1";
    state.queryCalls.length = 0;

    const req = new Request("http://example.com/api/lightning/settlements?limit=1", {
      method: "GET",
      headers: {
        "x-oa-request-id": "req-owner-settlements-1",
      },
    });

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, makeEnv(), ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      ok: boolean;
      requestId: string | null;
      settlements: Array<{ settlementId: string; ownerId: string; paymentProofRef: string }>;
      nextCursor: number | null;
    };

    expect(body.ok).toBe(true);
    expect(body.requestId).toBe("req-owner-settlements-1");
    expect(body.settlements).toHaveLength(1);
    expect(body.settlements[0]?.ownerId).toBe("owner-1");
    expect(body.settlements[0]?.paymentProofRef.startsWith("lightning_preimage:")).toBe(true);
    expect(body.nextCursor).toEqual(expect.any(Number));
  });

  it("lists paywall settlements with owner scoping", async () => {
    state.authed = true;
    state.userId = "owner-1";
    state.queryCalls.length = 0;

    const req = new Request("http://example.com/api/lightning/paywalls/pw_1/settlements?limit=10", {
      method: "GET",
      headers: {
        "x-oa-request-id": "req-paywall-settlements-1",
      },
    });

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, makeEnv(), ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      ok: boolean;
      settlements: Array<{ settlementId: string; paywallId: string }>;
      requestId: string | null;
    };

    expect(body.ok).toBe(true);
    expect(body.requestId).toBe("req-paywall-settlements-1");
    expect(body.settlements).toHaveLength(1);
    expect(body.settlements[0]?.paywallId).toBe("pw_1");

    state.userId = "owner-2";
    const forbiddenReq = new Request("http://example.com/api/lightning/paywalls/pw_1/settlements", {
      method: "GET",
      headers: {
        "x-oa-request-id": "req-paywall-settlements-forbidden-1",
      },
    });

    const forbiddenCtx = createExecutionContext();
    const forbiddenRes = await worker.fetch(forbiddenReq, makeEnv(), forbiddenCtx);
    await waitOnExecutionContext(forbiddenCtx);

    expect(forbiddenRes.status).toBe(403);
    const forbiddenBody = (await forbiddenRes.json()) as { ok: boolean; error: string };
    expect(forbiddenBody.ok).toBe(false);
    expect(forbiddenBody.error).toContain("forbidden");
  });
});
