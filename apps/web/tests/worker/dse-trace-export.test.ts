import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  putExampleCalls: [] as any[],
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
    events: [
      { _tag: "Input", input: { question: "q?", blobs: [{ id: "sha256:b1", hash: "sha256:b1", size: 10 }] } },
      { iteration: 1, promptHash: "sha256:p", action: { _tag: "Final", output: { answer: "a", evidence: { blobId: "sha256:b1", quote: "line" } } } },
    ],
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

  const mutation = (ref: any, args: any) =>
    Effect.sync(() => {
      if (isRecord(args) && typeof args.signatureId === "string" && typeof args.exampleId === "string") {
        state.putExampleCalls.push({ ref, args });
        return { ok: true, existed: false };
      }

      throw new Error(`Unexpected Convex mutation in tests: ${String(ref?.name ?? ref)}`);
    });

  const action = (_ref: any, _args: any) => Effect.fail(new Error("convex.action not used in tests"));
  const subscribeQuery = (_ref: any, _args: any) =>
    Stream.fail(new Error("convex.subscribeQuery not used in Worker tests"));

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, { query, mutation, action, subscribeQuery });
  return { ...actual, ConvexServiceLive };
});

const { default: worker } = await import("../../src/effuse-host/worker");

describe("apps/web worker DSE trace export endpoint (Phase F)", () => {
  it("exports a candidate example from an RLM trace and upserts into dseExamples", async () => {
    state.putExampleCalls.length = 0;
    state.getReceiptCalls.length = 0;
    state.getBlobCalls.length = 0;

    const ORIGIN = "http://example.com";
    const req = new Request(`${ORIGIN}/api/dse/trace/export`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ receiptId: "r1", split: "train", tags: ["seed"] }),
    });

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, env as any, ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(200);
    const j = (await res.json()) as any;
    expect(j.ok).toBe(true);
    expect(j.dryRun).toBe(false);
    expect(j.signatureId).toBe("@openagents/test/Sig.v1");
    expect(j.exampleId).toBe("trace:r1");

    expect(state.getReceiptCalls.length).toBe(1);
    expect(state.getBlobCalls.length).toBe(1);
    expect(state.putExampleCalls.length).toBe(1);

    const call = state.putExampleCalls[0]?.args as any;
    expect(call.signatureId).toBe("@openagents/test/Sig.v1");
    expect(call.exampleId).toBe("trace:r1");
    expect(call.inputJson).toEqual({
      question: "q?",
      blobs: [{ id: "sha256:b1", hash: "sha256:b1", size: 10 }],
    });
    expect(call.expectedJson).toEqual({ answer: "a", evidence: { blobId: "sha256:b1", quote: "line" } });
    expect(call.split).toBe("train");
    expect(call.tags).toEqual(["trace_export", "strategy:rlm_lite.v1", "seed"]);
    expect(typeof call.source).toBe("string");
  });
});

