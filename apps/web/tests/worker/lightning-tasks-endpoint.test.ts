import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const FIXED_NOW = 1_732_000_000_000;

const state = vi.hoisted(() => ({
  authed: true,
  userId: "user-lightning-1",
  nextTask: 1,
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
            user: { id: state.userId, email: "user@example.com", firstName: "L", lastName: "N" },
            sessionId: "sess-lightning-1",
            accessToken: "token-lightning-1",
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

  const isRecord = (value: unknown): value is Record<string, unknown> =>
    Boolean(value) && typeof value === "object" && !Array.isArray(value);

  const mutation = (ref: unknown, args: unknown) =>
    Effect.sync(() => {
      state.mutationCalls.push({ ref, args });
      if (isRecord(args) && isRecord(args.request) && typeof args.request.url === "string") {
        const taskId = `task-${state.nextTask++}`;
        return {
          ok: true as const,
          existed: false,
          task: {
            taskId,
            ownerId: state.userId,
            status: "queued",
            request: args.request,
            idempotencyKey: typeof args.idempotencyKey === "string" ? args.idempotencyKey : undefined,
            source: typeof args.source === "string" ? args.source : undefined,
            requestId: typeof args.requestId === "string" ? args.requestId : undefined,
            metadata: args.metadata,
            attemptCount: 0,
            createdAtMs: FIXED_NOW,
            updatedAtMs: FIXED_NOW,
            lastTransitionAtMs: FIXED_NOW,
          },
        };
      }
      throw new Error("Unexpected mutation payload");
    });

  const query = (_ref: unknown, _args: unknown) => Effect.fail(new Error("convex.query not used in this test"));
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

describe("apps/web worker lightning task endpoint", () => {
  it("requires auth for POST /api/lightning/l402/tasks", async () => {
    state.authed = false;
    state.mutationCalls.length = 0;

    const req = new Request("http://example.com/api/lightning/l402/tasks", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-auth-1",
      },
      body: JSON.stringify({
        request: {
          url: "https://api.example.com/protected",
          maxSpendMsats: 1_000,
        },
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
    expect(state.actionCalls).toBe(0);
  });

  it("enqueues a task with deterministic typed payload and request correlation", async () => {
    state.authed = true;
    state.nextTask = 1;
    state.mutationCalls.length = 0;
    state.actionCalls = 0;

    const req = new Request("http://example.com/api/lightning/l402/tasks", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-create-1",
      },
      body: JSON.stringify({
        request: {
          url: "https://api.example.com/premium",
          method: "GET",
          maxSpendMsats: 2_500,
          scope: "demo",
        },
        idempotencyKey: "episode-212-demo-1",
      }),
    });

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, makeEnv(), ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      ok: boolean;
      existed: boolean;
      requestId: string | null;
      task: {
        taskId: string;
        ownerId: string;
        status: string;
        request: { url: string; method?: string; maxSpendMsats: number; scope?: string };
      };
    };

    expect(body).toMatchObject({
      ok: true,
      existed: false,
      requestId: "req-create-1",
      task: {
        taskId: "task-1",
        ownerId: "user-lightning-1",
        status: "queued",
        request: {
          url: "https://api.example.com/premium",
          method: "GET",
          maxSpendMsats: 2_500,
          scope: "demo",
        },
      },
    });

    expect(state.mutationCalls).toHaveLength(1);
    const callArgs = state.mutationCalls[0]!.args as Record<string, unknown>;
    expect(callArgs.requestId).toBe("req-create-1");
    expect(callArgs.source).toBe("web_worker_api");
    expect(state.actionCalls).toBe(0);
  });

  it("rejects invalid payloads and non-POST methods", async () => {
    state.authed = true;
    state.mutationCalls.length = 0;

    const invalidReq = new Request("http://example.com/api/lightning/l402/tasks", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-invalid-1",
      },
      body: JSON.stringify({ request: { url: "", maxSpendMsats: -1 } }),
    });

    const invalidCtx = createExecutionContext();
    const invalidRes = await worker.fetch(invalidReq, makeEnv(), invalidCtx);
    await waitOnExecutionContext(invalidCtx);
    expect(invalidRes.status).toBe(400);

    const getReq = new Request("http://example.com/api/lightning/l402/tasks", {
      method: "GET",
      headers: { "x-oa-request-id": "req-method-1" },
    });
    const getCtx = createExecutionContext();
    const getRes = await worker.fetch(getReq, makeEnv(), getCtx);
    await waitOnExecutionContext(getCtx);
    expect(getRes.status).toBe(405);
    expect(state.mutationCalls).toHaveLength(0);
  });
});
