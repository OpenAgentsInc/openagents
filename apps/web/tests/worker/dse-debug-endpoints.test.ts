import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  getReceiptCalls: [] as any[],
  getBlobCalls: [] as any[],
}));

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

  const receiptJson = {
    format: "openagents.dse.predict_receipt",
    formatVersion: 1,
    receiptId: "r1",
    runId: "r1",
    createdAt: new Date().toISOString(),
    signatureId: "@openagents/test/Sig.v1",
    compiled_id: "c1",
    strategyId: "rlm_lite.v1",
    hashes: {
      inputSchemaHash: "sha256:in",
      outputSchemaHash: "sha256:out",
      promptIrHash: "sha256:prompt",
      paramsHash: "sha256:params",
    },
    model: {},
    timing: { startedAtMs: 1, endedAtMs: 2, durationMs: 1 },
    rlmTrace: { blob: { id: "sha256:trace", hash: "sha256:trace", size: 1 }, eventCount: 3 },
    result: { _tag: "Ok" },
  };

  const traceText = JSON.stringify({
    format: "openagents.dse.rlm_trace",
    formatVersion: 1,
    signatureId: receiptJson.signatureId,
    receiptId: receiptJson.receiptId,
    strategyId: receiptJson.strategyId,
    events: [{ _tag: "Iteration", i: 1 }, { _tag: "Final", output: { ok: true } }],
  });

  const query = (ref: any, args: any) =>
    Effect.sync(() => {
      if (isRecord(args) && typeof args.receiptId === "string") {
        state.getReceiptCalls.push({ ref, args });
        return { ok: true, receipt: { threadId: "t1", runId: "run1", json: receiptJson, createdAtMs: 1 } };
      }

      if (isRecord(args) && typeof args.threadId === "string" && typeof args.runId === "string" && typeof args.blobId === "string") {
        state.getBlobCalls.push({ ref, args });
        return { ok: true, text: traceText };
      }

      throw new Error(`Unexpected Convex query in tests: ${String(ref?.name ?? ref)}`);
    });

  const mutation = (_ref: any, _args: any) => Effect.fail(new Error("convex.mutation not used in this test"));
  const action = (_ref: any, _args: any) => Effect.fail(new Error("convex.action not used in this test"));
  const subscribeQuery = (_ref: any, _args: any) =>
    Stream.fail(new Error("convex.subscribeQuery not used in Worker tests"));

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, { query, mutation, action, subscribeQuery });
  return { ...actual, ConvexServiceLive };
});

const { default: worker } = await import("../../src/effuse-host/worker");

describe("apps/web worker DSE debug read endpoints (Phase D)", () => {
  it("GET /api/dse/receipt/:receiptId returns validated receipt JSON", async () => {
    state.getReceiptCalls.length = 0;
    state.getBlobCalls.length = 0;

    const ORIGIN = "http://example.com";
    const req = new Request(`${ORIGIN}/api/dse/receipt/r1`, { method: "GET" });

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, env as any, ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(200);
    const j = (await res.json()) as any;
    expect(j.ok).toBe(true);
    expect(j.receipt?.receiptId).toBe("r1");
    expect(j.receipt?.format).toBe("openagents.dse.predict_receipt");
    expect(state.getReceiptCalls.length).toBe(1);
  });

  it("GET /api/dse/blob/:receiptId/:blobId returns raw blob text scoped by receipt", async () => {
    state.getReceiptCalls.length = 0;
    state.getBlobCalls.length = 0;

    const ORIGIN = "http://example.com";
    const req = new Request(`${ORIGIN}/api/dse/blob/r1/sha256:trace`, { method: "GET" });

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, env as any, ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(200);
    const text = await res.text();
    expect(text).toContain("\"format\":\"openagents.dse.rlm_trace\"");
    expect(state.getReceiptCalls.length).toBe(1);
    expect(state.getBlobCalls.length).toBe(1);
  });
});

