import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  queryCalls: [] as any[],
  mutationCalls: [] as any[],
  recordReceiptCalls: [] as any[],
}));

vi.mock("../../src/effuse-host/dseAdminSecret", async (importOriginal) => {
  const actual = await importOriginal<any>();
  return {
    ...actual,
    isDseAdminSecretAuthorized: (request: Request, env: any) => {
      const secret = env?.OA_DSE_ADMIN_SECRET;
      const h = request.headers.get("authorization") ?? "";
      return Boolean(secret) && h.trim() === `Bearer ${secret}`;
    },
    withDseAdminSecretServices: (_env: any, _convexUrl: string, program: any) => program,
  };
});

vi.mock("@workos/authkit-session", async (importOriginal) => {
  const actual = await importOriginal<any>();
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

vi.mock("../../src/effect/convex", async (importOriginal) => {
  const actual = await importOriginal<any>();
  const { Effect, Layer, Stream } = await import("effect");

  const isRecord = (u: unknown): u is Record<string, any> => Boolean(u) && typeof u === "object";

  const query = (ref: any, args: any) =>
    Effect.sync(() => {
      state.queryCalls.push({ ref, args });

      // Canary status: /api/dse/canary/status uses api.dse.canary.getCanary.
      if (isRecord(args) && typeof args.signatureId === "string" && !("limit" in args) && !("split" in args) && !("threadId" in args)) {
        return {
          ok: true,
          canary: {
            signatureId: args.signatureId,
            enabled: true,
            control_compiled_id: "sha256:control",
            canary_compiled_id: "sha256:canary",
            rolloutPct: 20,
            salt: "salt",
            okCount: 3,
            errorCount: 0,
            minSamples: 3,
            maxErrorRate: 0.2,
            createdAtMs: 1,
            updatedAtMs: 2,
          },
        };
      }

      // Exercise predict: listExamples
      if (isRecord(args) && typeof args.signatureId === "string" && typeof args.limit === "number") {
        return {
          ok: true,
          examples: [
            {
              signatureId: args.signatureId,
              exampleId: "ex1",
              inputJson: {
                message: "What can you do?",
                blueprintHint: { userHandle: "Ada", agentName: "Autopilot" },
              },
              expectedJson: { action: "none" },
              split: "train",
              tags: ["seed"],
              source: "test",
              createdAtMs: 1,
              updatedAtMs: 1,
            },
          ],
        };
      }

      // Policy registry: canary.getCanary (thread-scoped) => no canary for predict test
      if (isRecord(args) && typeof args.signatureId === "string" && typeof args.threadId === "string") {
        return { ok: true, canary: null };
      }

      throw new Error(`Unexpected Convex query in tests: ${String(ref?.name ?? ref)}`);
    });

  const mutation = (ref: any, args: any) =>
    Effect.sync(() => {
      state.mutationCalls.push({ ref, args });

      // Ensure thread endpoint
      if (isRecord(args) && Object.keys(args).length === 0) {
        return { ok: true, threadId: "thread_ops" };
      }

      // Receipt recording from exercise predict
      if (isRecord(args) && typeof args.threadId === "string" && typeof args.runId === "string" && "receipt" in args) {
        state.recordReceiptCalls.push({ ref, args });
        return { ok: true };
      }

      throw new Error(`Unexpected Convex mutation in tests: ${String(ref?.name ?? ref)}`);
    });

  const action = (_ref: any, _args: any) => Effect.fail(new Error("convex.action not used in Worker tests"));
  const subscribeQuery = (_ref: any, _args: any) =>
    Stream.fail(new Error("convex.subscribeQuery not used in Worker tests"));

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, { query, mutation, action, subscribeQuery });
  return { ...actual, ConvexServiceLive };
});

const { default: worker } = await import("../../src/effuse-host/worker");

describe("apps/web worker DSE exercise endpoints (Phase 5)", () => {
  it("GET /api/dse/canary/status rejects missing admin secret", async () => {
    const ORIGIN = "http://example.com";
    const req = new Request(`${ORIGIN}/api/dse/canary/status?signatureId=@openagents/autopilot/blueprint/SelectTool.v1`, {
      method: "GET",
    });

    const env0 = Object.assign(Object.create(env as any), {
      OA_DSE_ADMIN_SECRET: "secret",
    }) as any;

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, env0, ctx);
    await waitOnExecutionContext(ctx);
    expect(res.status).toBe(401);
  });

  it("GET /api/dse/canary/status returns canary counters (admin-secret gated)", async () => {
    state.queryCalls.length = 0;

    const ORIGIN = "http://example.com";
    const req = new Request(`${ORIGIN}/api/dse/canary/status?signatureId=@openagents/autopilot/blueprint/SelectTool.v1`, {
      method: "GET",
      headers: { authorization: "Bearer secret" },
    });

    const env0 = Object.assign(Object.create(env as any), {
      OA_DSE_ADMIN_SECRET: "secret",
    }) as any;

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, env0, ctx);
    await waitOnExecutionContext(ctx);
    expect(res.status).toBe(200);
    const json = (await res.json()) as any;
    expect(json.ok).toBe(true);
    expect(json.canary.signatureId).toBe("@openagents/autopilot/blueprint/SelectTool.v1");
    expect(json.canary.okCount).toBe(3);
  });

  it("POST /api/dse/exercise/thread/ensure returns thread id (admin-secret gated)", async () => {
    state.mutationCalls.length = 0;

    const ORIGIN = "http://example.com";
    const req = new Request(`${ORIGIN}/api/dse/exercise/thread/ensure`, {
      method: "POST",
      headers: { "content-type": "application/json", authorization: "Bearer secret" },
      body: JSON.stringify({}),
    });

    const env0 = Object.assign(Object.create(env as any), {
      OA_DSE_ADMIN_SECRET: "secret",
    }) as any;

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, env0, ctx);
    await waitOnExecutionContext(ctx);
    expect(res.status).toBe(200);
    const json = (await res.json()) as any;
    expect(json.ok).toBe(true);
    expect(typeof json.threadId).toBe("string");
  });

  it("POST /api/dse/exercise/predict runs N predictions and records receipts", async () => {
    state.recordReceiptCalls.length = 0;

    const ORIGIN = "http://example.com";
    const req = new Request(`${ORIGIN}/api/dse/exercise/predict`, {
      method: "POST",
      headers: { "content-type": "application/json", authorization: "Bearer secret" },
      body: JSON.stringify({
        signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
        threadId: "thread_ops",
        count: 2,
        split: "train",
      }),
    });

    const envWithAi = Object.assign(Object.create(env as any), {
      OA_DSE_ADMIN_SECRET: "secret",
      AI: {
        run: async () => ({
          choices: [{ message: { content: "{\"action\":\"none\"}" } }],
          usage: { prompt_tokens: 1, completion_tokens: 1 },
        }),
      },
    }) as any;

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, envWithAi, ctx);
    await waitOnExecutionContext(ctx);
    expect(res.status).toBe(200);
    const json = (await res.json()) as any;
    expect(json.ok).toBe(true);
    expect(json.count).toBe(2);
    expect(state.recordReceiptCalls.length).toBe(2);
  });
});
