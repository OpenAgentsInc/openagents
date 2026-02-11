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
  tasks: [] as Array<{
    readonly taskId: string;
    readonly ownerId: string;
    readonly status: string;
    readonly request: Record<string, unknown>;
    readonly idempotencyKey?: string;
    readonly source?: string;
    readonly requestId?: string;
    readonly metadata?: unknown;
    readonly attemptCount: number;
    readonly lastErrorCode?: string;
    readonly lastErrorMessage?: string;
    readonly createdAtMs: number;
    readonly updatedAtMs: number;
    readonly lastTransitionAtMs: number;
  }>,
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
      if (isRecord(args) && typeof args.taskId === "string" && typeof args.toStatus === "string") {
        const index = state.tasks.findIndex((task) => task.taskId === args.taskId);
        if (index === -1) {
          throw new Error("task_not_found");
        }

        const prev = state.tasks[index]!;
        const now = FIXED_NOW + state.mutationCalls.length;
        const next = {
          ...prev,
          status: args.toStatus,
          updatedAtMs: now,
          lastTransitionAtMs: now,
          attemptCount: args.toStatus === "running" ? prev.attemptCount + 1 : prev.attemptCount,
          lastErrorCode: typeof args.errorCode === "string" ? args.errorCode : undefined,
          lastErrorMessage: typeof args.errorMessage === "string" ? args.errorMessage : undefined,
        };
        state.tasks[index] = next;
        return {
          ok: true as const,
          changed: true,
          task: next,
          event: {
            taskId: next.taskId,
            ownerId: next.ownerId,
            fromStatus: prev.status,
            toStatus: next.status,
            actor: typeof args.actor === "string" ? args.actor : "desktop_executor",
            reason: typeof args.reason === "string" ? args.reason : undefined,
            requestId: typeof args.requestId === "string" ? args.requestId : undefined,
            errorCode: typeof args.errorCode === "string" ? args.errorCode : undefined,
            errorMessage: typeof args.errorMessage === "string" ? args.errorMessage : undefined,
            metadata: args.metadata,
            createdAtMs: now,
          },
        };
      }

      if (isRecord(args) && isRecord(args.request) && typeof args.request.url === "string") {
        const taskId = `task-${state.nextTask++}`;
        const task = {
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
        };
        state.tasks.push(task);
        return {
          ok: true as const,
          existed: false,
          task,
        };
      }
      throw new Error("Unexpected mutation payload");
    });

  const query = (ref: unknown, args: unknown) =>
    Effect.sync(() => {
      state.queryCalls.push({ ref, args });
      if (!isRecord(args)) return { ok: true as const, tasks: state.tasks };
      if (typeof args.status === "string") {
        return { ok: true as const, tasks: state.tasks.filter((task) => task.status === args.status) };
      }
      return { ok: true as const, tasks: state.tasks };
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

describe("apps/web worker lightning task endpoint", () => {
  it("requires auth for POST /api/lightning/l402/tasks", async () => {
    state.authed = false;
    state.mutationCalls.length = 0;
    state.queryCalls.length = 0;
    state.tasks.length = 0;

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
    expect(state.queryCalls).toHaveLength(0);
    expect(state.actionCalls).toBe(0);
  });

  it("enqueues a task with deterministic typed payload and request correlation", async () => {
    state.authed = true;
    state.nextTask = 1;
    state.tasks.length = 0;
    state.mutationCalls.length = 0;
    state.queryCalls.length = 0;
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
    expect(state.queryCalls).toHaveLength(0);
    const callArgs = state.mutationCalls[0]!.args as Record<string, unknown>;
    expect(callArgs.requestId).toBe("req-create-1");
    expect(callArgs.source).toBe("web_worker_api");
    expect(state.actionCalls).toBe(0);
  });

  it("supports GET listing and transition endpoint", async () => {
    state.authed = true;
    state.tasks.length = 0;
    state.nextTask = 1;
    state.mutationCalls.length = 0;
    state.queryCalls.length = 0;

    const createReq = new Request("http://example.com/api/lightning/l402/tasks", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-create-list-1",
      },
      body: JSON.stringify({
        request: {
          url: "https://api.example.com/premium",
          maxSpendMsats: 1_000,
        },
      }),
    });
    const createCtx = createExecutionContext();
    const createRes = await worker.fetch(createReq, makeEnv(), createCtx);
    await waitOnExecutionContext(createCtx);
    expect(createRes.status).toBe(200);

    const listReq = new Request("http://example.com/api/lightning/l402/tasks?status=queued&limit=10", {
      method: "GET",
      headers: { "x-oa-request-id": "req-list-1" },
    });
    const listCtx = createExecutionContext();
    const listRes = await worker.fetch(listReq, makeEnv(), listCtx);
    await waitOnExecutionContext(listCtx);
    expect(listRes.status).toBe(200);
    const listBody = (await listRes.json()) as {
      ok: boolean;
      requestId: string | null;
      tasks: Array<{ taskId: string; status: string }>;
    };
    expect(listBody.ok).toBe(true);
    expect(listBody.requestId).toBe("req-list-1");
    expect(listBody.tasks[0]?.status).toBe("queued");

    const transitionReq = new Request("http://example.com/api/lightning/l402/tasks/task-1/transition", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-transition-1",
      },
      body: JSON.stringify({
        toStatus: "running",
        actor: "desktop_executor",
      }),
    });
    const transitionCtx = createExecutionContext();
    const transitionRes = await worker.fetch(transitionReq, makeEnv(), transitionCtx);
    await waitOnExecutionContext(transitionCtx);
    expect(transitionRes.status).toBe(200);
    const transitionBody = (await transitionRes.json()) as {
      ok: boolean;
      changed: boolean;
      requestId: string | null;
      task: { taskId: string; status: string };
      event?: {
        requestId?: string;
        toStatus?: string;
      };
    };
    expect(transitionBody.ok).toBe(true);
    expect(transitionBody.changed).toBe(true);
    expect(transitionBody.requestId).toBe("req-transition-1");
    expect(transitionBody.task).toMatchObject({ taskId: "task-1", status: "running" });
    expect(transitionBody.event?.requestId).toBe("req-transition-1");
    expect(transitionBody.event?.toStatus).toBe("running");

    const paidTransitionReq = new Request("http://example.com/api/lightning/l402/tasks/task-1/transition", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-oa-request-id": "req-transition-2",
      },
      body: JSON.stringify({
        toStatus: "paid",
        actor: "desktop_executor",
        metadata: {
          proofReference: "preimage:abcd1234",
          paymentId: "hash:demo:1",
          responseStatusCode: 200,
        },
      }),
    });
    const paidTransitionCtx = createExecutionContext();
    const paidTransitionRes = await worker.fetch(paidTransitionReq, makeEnv(), paidTransitionCtx);
    await waitOnExecutionContext(paidTransitionCtx);
    expect(paidTransitionRes.status).toBe(200);
    const paidTransitionBody = (await paidTransitionRes.json()) as {
      ok: boolean;
      changed: boolean;
      requestId: string | null;
      task: { taskId: string; status: string };
      event?: {
        requestId?: string;
        toStatus?: string;
        metadata?: {
          proofReference?: string;
          paymentId?: string;
          responseStatusCode?: number;
        };
      };
    };
    expect(paidTransitionBody.ok).toBe(true);
    expect(paidTransitionBody.changed).toBe(true);
    expect(paidTransitionBody.requestId).toBe("req-transition-2");
    expect(paidTransitionBody.task).toMatchObject({ taskId: "task-1", status: "paid" });
    expect(paidTransitionBody.event?.requestId).toBe("req-transition-2");
    expect(paidTransitionBody.event?.metadata).toMatchObject({
      proofReference: "preimage:abcd1234",
      paymentId: "hash:demo:1",
      responseStatusCode: 200,
    });
  });

  it("rejects invalid payloads and unsupported methods", async () => {
    state.authed = true;
    state.tasks.length = 0;
    state.mutationCalls.length = 0;
    state.queryCalls.length = 0;

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

    const putReq = new Request("http://example.com/api/lightning/l402/tasks", {
      method: "PUT",
      headers: { "x-oa-request-id": "req-method-1" },
    });
    const putCtx = createExecutionContext();
    const putRes = await worker.fetch(putReq, makeEnv(), putCtx);
    await waitOnExecutionContext(putCtx);
    expect(putRes.status).toBe(405);

    const invalidTransitionReq = new Request("http://example.com/api/lightning/l402/tasks/task-1/transition", {
      method: "POST",
      headers: {
        "x-oa-request-id": "req-invalid-transition-1",
        "content-type": "application/json",
      },
      body: JSON.stringify({ toStatus: "not-a-status" }),
    });
    const invalidTransitionCtx = createExecutionContext();
    const invalidTransitionRes = await worker.fetch(
      invalidTransitionReq,
      makeEnv(),
      invalidTransitionCtx,
    );
    await waitOnExecutionContext(invalidTransitionCtx);
    expect(invalidTransitionRes.status).toBe(400);

    expect(state.mutationCalls).toHaveLength(0);
  });
});
