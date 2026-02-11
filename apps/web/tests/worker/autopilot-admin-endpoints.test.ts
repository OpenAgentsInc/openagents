import {
  createExecutionContext,
  env,
  waitOnExecutionContext,
} from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  ensureOwnedThreadCalls: [] as Array<{ readonly ref: unknown; readonly args: unknown }>,
  createRunCalls: [] as Array<{ readonly ref: unknown; readonly args: unknown }>,
  resetCalls: [] as Array<{ readonly ref: unknown; readonly args: unknown }>,
  snapshotCalls: [] as Array<{ readonly ref: unknown; readonly args: unknown }>,
  traceCalls: [] as Array<{ readonly ref: unknown; readonly args: unknown }>,
  appendPartsCalls: [] as Array<{ readonly ref: unknown; readonly args: unknown }>,
  finalizeRunCalls: [] as Array<{ readonly ref: unknown; readonly args: unknown }>,
  nextRun: 1,
}));

vi.mock("../../src/auth/e2eAuth", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../src/auth/e2eAuth")>();
  const { Effect } = await import("effect");

  const encodeB64Url = (value: unknown): string =>
    Buffer.from(JSON.stringify(value), "utf8").toString("base64url");

  return {
    ...actual,
    mintE2eJwt: (input: { readonly user: { readonly id: string }; readonly ttlSeconds?: number }) =>
      Effect.succeed(
        `test.${encodeB64Url({
          iss: actual.E2E_JWT_ISSUER,
          sub: input.user.id,
          exp: Math.floor(Date.now() / 1000) + (typeof input.ttlSeconds === "number" ? input.ttlSeconds : 900),
        })}.sig`,
      ),
  };
});

vi.mock("@workos/authkit-session", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@workos/authkit-session")>();
  return {
    ...actual,
    createAuthService: () => ({
      withAuth: async (_request: Request) => ({
        auth: {
          user: { id: "admin-1", email: "admin@example.com", firstName: "A", lastName: "D" },
          sessionId: "sess-1",
          accessToken: "token-1",
        },
        refreshedSessionData: undefined,
      }),
      saveSession: async (_auth: unknown, _sessionData: string) => ({ headers: {} as Record<string, string> }),
    }),
  };
});

vi.mock("@effect/ai/LanguageModel", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@effect/ai/LanguageModel")>();
  const { Stream } = await import("effect");

  const marker = { ["~effect/ai/Content/Part"]: "~effect/ai/Content/Part" } as const;
  const textId = "text-1";
  const metadata = {};

  return {
    ...actual,
    streamText: () =>
      Stream.fromIterable([
        { type: "text-start", id: textId, metadata, ...marker },
        { type: "text-delta", id: textId, delta: "ok", metadata, ...marker },
        { type: "text-end", id: textId, metadata, ...marker },
        {
          type: "finish",
          reason: "stop",
          usage: { inputTokens: 1, outputTokens: 1, totalTokens: 2 },
          metadata,
          ...marker,
        },
      ]),
  };
});

vi.mock("../../src/effect/convex", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../src/effect/convex")>();
  const { Effect, Layer, Stream } = await import("effect");

  const isRecord = (value: unknown): value is Record<string, unknown> =>
    Boolean(value) && typeof value === "object";

  const query = (ref: unknown, args: unknown) =>
    Effect.sync(() => {
      if (
        isRecord(args) &&
        typeof args.threadId === "string" &&
        typeof args.maxMessages === "number" &&
        typeof args.maxParts === "number" &&
        typeof args.maxRuns === "number"
      ) {
        state.traceCalls.push({ ref, args });
        return {
          ok: true,
          thread: {
            threadId: String(args.threadId),
            ownerId: "user_autopilot_admin_test",
            createdAtMs: 1,
            updatedAtMs: 1,
          },
          blueprint: null,
          messages: [],
          parts: [],
          runs: [],
          receipts: [],
          featureRequests: [],
          dseBlobs: [],
          dseVars: [],
          summary: {
            messageCount: 0,
            partCount: 0,
            runCount: 0,
            receiptCount: 0,
            featureRequestCount: 0,
            dseBlobCount: 0,
            dseVarCount: 0,
          },
        };
      }

      if (
        isRecord(args) &&
        typeof args.threadId === "string" &&
        typeof args.maxMessages === "number" &&
        typeof args.maxParts === "number"
      ) {
        state.snapshotCalls.push({ ref, args });
        return {
          ok: true,
          threadId: String(args.threadId),
          messages: [],
          parts: [],
        };
      }

      if (isRecord(args) && typeof args.threadId === "string" && typeof args.runId === "string") {
        return { ok: true, cancelRequested: false };
      }

      if (
        isRecord(args) &&
        typeof args.threadId === "string" &&
        !("maxMessages" in args) &&
        !("runId" in args)
      ) {
        return { ok: true, blueprint: null, updatedAtMs: 0 };
      }

      return { ok: true };
    });

  const mutation = (ref: unknown, args: unknown) =>
    Effect.sync(() => {
      if (isRecord(args) && Object.keys(args).length === 0) {
        state.ensureOwnedThreadCalls.push({ ref, args });
        return { ok: true, threadId: "thread-admin-1" };
      }

      if (isRecord(args) && typeof args.threadId === "string" && typeof args.text === "string") {
        state.createRunCalls.push({ ref, args });
        const runId = `run-${state.nextRun++}`;
        return {
          ok: true,
          runId,
          userMessageId: `m-user-${runId}`,
          assistantMessageId: `m-assistant-${runId}`,
        };
      }

      if (isRecord(args) && Array.isArray(args.parts)) {
        state.appendPartsCalls.push({ ref, args });
        return { ok: true, inserted: args.parts.length };
      }

      if (isRecord(args) && typeof args.status === "string" && typeof args.runId === "string") {
        state.finalizeRunCalls.push({ ref, args });
        return { ok: true };
      }

      if (
        isRecord(args) &&
        typeof args.threadId === "string" &&
        !("runId" in args) &&
        !("text" in args) &&
        !("parts" in args) &&
        !("status" in args)
      ) {
        state.resetCalls.push({ ref, args });
        return { ok: true };
      }

      return { ok: true };
    });

  const action = (_ref: unknown, _args: unknown) =>
    Effect.fail(new Error("convex.action not used in tests"));
  const subscribeQuery = (_ref: unknown, _args: unknown) =>
    Stream.fail(new Error("convex.subscribeQuery not used in Worker tests"));
  const connectionState = () => Effect.succeed(null);
  const refreshAuth = () => Effect.void;

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, {
    query,
    mutation,
    action,
    subscribeQuery,
    connectionState,
    refreshAuth,
  });

  return { ...actual, ConvexServiceLive };
});

const { default: worker } = await import("../../src/effuse-host/worker");

const ORIGIN = "http://example.com";
const ADMIN_SECRET = "admin-secret";

const makeEnv = (): WorkerEnv =>
  Object.assign(Object.create(env as WorkerEnv), {
    AI: {},
    OA_AUTOPILOT_ADMIN_SECRET: ADMIN_SECRET,
    OA_E2E_JWT_PRIVATE_JWK: "test-jwk",
  });

const authHeaders = (): HeadersInit => ({
  authorization: `Bearer ${ADMIN_SECRET}`,
  "content-type": "application/json",
});

describe("apps/web worker autopilot admin endpoints (fixed test user)", () => {
  it("rejects unauthorized admin send calls", async () => {
    const req = new Request(`${ORIGIN}/api/autopilot/admin/send`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ text: "hello" }),
    });
    const ctx = createExecutionContext();
    const res = await worker.fetch(req, makeEnv(), ctx);

    expect(res.status).toBe(401);
    const body = (await res.json()) as { ok: boolean; error: string };
    expect(body.ok).toBe(false);
    expect(body.error).toBe("unauthorized");
  });

  it("starts a run for the fixed test user via /api/autopilot/admin/send", async () => {
    state.ensureOwnedThreadCalls.length = 0;
    state.createRunCalls.length = 0;
    state.appendPartsCalls.length = 0;
    state.finalizeRunCalls.length = 0;

    const req = new Request(`${ORIGIN}/api/autopilot/admin/send`, {
      method: "POST",
      headers: authHeaders(),
      body: JSON.stringify({ text: "admin trigger message", resetThread: true }),
    });
    const ctx = createExecutionContext();
    const res = await worker.fetch(req, makeEnv(), ctx);

    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      ok: boolean;
      testUserId: string;
      threadId: string;
      runId: string;
      assistantMessageId: string;
    };
    expect(body.ok).toBe(true);
    expect(body.testUserId).toBe("user_autopilot_admin_test");
    expect(body.threadId).toBe("thread-admin-1");
    expect(body.runId.length).toBeGreaterThan(0);
    expect(state.ensureOwnedThreadCalls.length).toBeGreaterThan(0);
    expect(state.createRunCalls.length).toBeGreaterThan(0);

    await waitOnExecutionContext(ctx);
    expect(state.appendPartsCalls.length).toBeGreaterThan(0);
  });

  it("resets and snapshots the test-user thread with admin auth", async () => {
    state.resetCalls.length = 0;
    state.snapshotCalls.length = 0;

    const resetReq = new Request(`${ORIGIN}/api/autopilot/admin/reset`, {
      method: "POST",
      headers: authHeaders(),
      body: JSON.stringify({ threadId: "thread-admin-1" }),
    });
    const resetCtx = createExecutionContext();
    const resetRes = await worker.fetch(resetReq, makeEnv(), resetCtx);
    expect(resetRes.status).toBe(200);
    await waitOnExecutionContext(resetCtx);
    expect(state.resetCalls.length).toBeGreaterThan(0);

    const snapshotReq = new Request(
      `${ORIGIN}/api/autopilot/admin/snapshot?threadId=thread-admin-1&maxMessages=50&maxParts=500`,
      {
        method: "GET",
        headers: { authorization: `Bearer ${ADMIN_SECRET}` },
      },
    );
    const snapshotCtx = createExecutionContext();
    const snapshotRes = await worker.fetch(snapshotReq, makeEnv(), snapshotCtx);
    expect(snapshotRes.status).toBe(200);
    const snapshotBody = (await snapshotRes.json()) as {
      ok: boolean;
      snapshot: { threadId: string };
    };
    expect(snapshotBody.ok).toBe(true);
    expect(snapshotBody.snapshot.threadId).toBe("thread-admin-1");
    expect(state.snapshotCalls.length).toBeGreaterThan(0);
  });

  it("returns trace bundle via /api/autopilot/admin/trace", async () => {
    state.traceCalls.length = 0;

    const traceReq = new Request(
      `${ORIGIN}/api/autopilot/admin/trace?threadId=thread-admin-1&maxMessages=20&maxParts=100&maxRuns=10`,
      {
        method: "GET",
        headers: { authorization: `Bearer ${ADMIN_SECRET}` },
      },
    );
    const traceCtx = createExecutionContext();
    const traceRes = await worker.fetch(traceReq, makeEnv(), traceCtx);

    expect(traceRes.status).toBe(200);
    const traceBody = (await traceRes.json()) as {
      ok: boolean;
      trace: { summary: { runCount: number } };
      testUserId: string;
    };
    expect(traceBody.ok).toBe(true);
    expect(traceBody.testUserId).toBe("user_autopilot_admin_test");
    expect(typeof traceBody.trace.summary.runCount).toBe("number");
    expect(state.traceCalls.length).toBeGreaterThan(0);
  });
});
