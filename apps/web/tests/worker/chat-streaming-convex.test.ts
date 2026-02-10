import {
  env,
  createExecutionContext,
  waitOnExecutionContext,
} from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  mode: "fast" as "fast" | "slow",
  failSnapshot: false,
  // Records of calls made via ConvexService (worker-side).
  createRunCalls: [] as any[],
  appendPartsCalls: [] as any[],
  finalizeRunCalls: [] as any[],
  cancelCalls: [] as any[],
  snapshotCalls: [] as any[],
  cancelRequested: new Set<string>(),
  nextRun: 1,
}));

vi.mock("@effect/ai/LanguageModel", async (importOriginal) => {
  const actual = await importOriginal<any>();
  const { Effect, Stream } = await import("effect");

  const streamText = (_args: any) => {
    const textId = "text-1";
    const meta = {};
    const marker = { ["~effect/ai/Content/Part"]: "~effect/ai/Content/Part" } as const;

    if (state.mode === "slow") {
      // Slow enough to cancel.
      return Stream.asyncPush((emit) => {
        let i = 0;
        const tick = () => {
          if (i === 0) emit.single({ type: "text-start", id: textId, metadata: meta, ...marker });
          if (i < 50) emit.single({ type: "text-delta", id: textId, delta: "x", metadata: meta, ...marker });
          if (i === 50) {
            emit.single({ type: "text-end", id: textId, metadata: meta, ...marker });
            emit.single({
              type: "finish",
              reason: "stop",
              usage: { inputTokens: 1, outputTokens: 1, totalTokens: 2 },
              metadata: meta,
              ...marker,
            });
            emit.end();
            return;
          }
          i++;
          setTimeout(tick, 10);
        };
        setTimeout(tick, 0);
        return Effect.sync(() => {});
      });
    }

    // Fast deterministic stream: emits many small deltas to validate batching.
    const deltas = Array.from({ length: 100 }, () => "a");
    const parts = [
      { type: "text-start", id: textId, metadata: meta, ...marker },
      ...deltas.map((d) => ({ type: "text-delta", id: textId, delta: d, metadata: meta, ...marker })),
      { type: "text-end", id: textId, metadata: meta, ...marker },
      {
        type: "finish",
        reason: "stop",
        usage: { inputTokens: 1, outputTokens: 1, totalTokens: 2 },
        metadata: meta,
        ...marker,
      },
    ];
    return Stream.fromIterable(parts);
  };

  return { ...actual, streamText };
});

vi.mock("../../src/effect/convex", async (importOriginal) => {
  const actual = await importOriginal<any>();
  const { Effect, Layer, Stream } = await import("effect");

  const isRecord = (u: unknown): u is Record<string, any> => Boolean(u) && typeof u === "object";

  const query = (ref: any, args: any) => {
    if (
      state.failSnapshot &&
      isRecord(args) &&
      typeof args.threadId === "string" &&
      typeof args.maxMessages === "number" &&
      typeof args.maxParts === "number"
    ) {
      return Effect.fail(new Error("snapshot_fail"));
    }

    return Effect.sync(() => {
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
          messages: [
            // Welcome assistant message (static).
            {
              messageId: "m-welcome",
              role: "assistant",
              status: "final",
              text: "Autopilot online.",
              runId: null,
              createdAtMs: 1,
              updatedAtMs: 1,
            },
            // User message created by createRun (not needed for prompt correctness in this suite).
          ],
          parts: [],
        };
      }

      if (isRecord(args) && typeof args.threadId === "string" && typeof args.runId === "string") {
        const runId = String(args.runId ?? "");
        return { ok: true, cancelRequested: state.cancelRequested.has(runId) };
      }

      // getBlueprint(threadId, anonKey?) — used for bootstrap-aware prompt and DSE hint.
      if (isRecord(args) && typeof args.threadId === "string" && !("maxMessages" in args) && !("runId" in args)) {
        return { ok: true, blueprint: null, updatedAtMs: 0 };
      }

      throw new Error(`Unexpected Convex query in tests: ${String(ref?.name ?? ref)}`);
    });
  };

  const mutation = (ref: any, args: any) =>
    Effect.sync(() => {
      if (isRecord(args) && Array.isArray(args.parts)) {
        state.appendPartsCalls.push({ ref, args });
        return { ok: true, inserted: Array.isArray(args.parts) ? args.parts.length : 0 };
      }

      if (isRecord(args) && typeof args.status === "string" && typeof args.runId === "string") {
        state.finalizeRunCalls.push({ ref, args });
        return { ok: true };
      }

      if (isRecord(args) && typeof args.threadId === "string" && typeof args.runId === "string") {
        state.cancelCalls.push({ ref, args });
        const runId = String(args.runId ?? "");
        if (runId) state.cancelRequested.add(runId);
        return { ok: true };
      }

      // applyBootstrapUserHandle(threadId, anonKey?, handle)
      if (isRecord(args) && typeof args.threadId === "string" && typeof args.handle === "string") {
        return { ok: true, applied: true, updatedAtMs: Date.now() };
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

      throw new Error(`Unexpected Convex mutation in tests: ${String(ref?.name ?? ref)}`);
    });

  const action = (_ref: any, _args: any) => Effect.fail(new Error("convex.action not used in tests"));
  const subscribeQuery = (_ref: any, _args: any) =>
    Stream.fail(new Error("convex.subscribeQuery not used in Worker tests"));

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, { query, mutation, action, subscribeQuery });

  return { ...actual, ConvexServiceLive };
});

const { default: worker } = await import("../../src/effuse-host/worker");

describe("apps/web worker autopilot streaming (Convex-first)", () => {
  it("writes messageParts in a single chunked batch (no per-token writes)", async () => {
    state.mode = "fast";
    state.failSnapshot = false;
    state.createRunCalls.length = 0;
    state.appendPartsCalls.length = 0;
    state.finalizeRunCalls.length = 0;
    state.snapshotCalls.length = 0;

    const ORIGIN = "http://example.com";

    const request = new Request(`${ORIGIN}/api/autopilot/send`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ threadId: "thread-1", anonKey: "anon-key-1", text: "hi" }),
    });

    const ctx = createExecutionContext();
    // cloudflare:test env bindings are not guaranteed to be enumerable, so avoid
    // spreading (it can drop required vars like VITE_CONVEX_URL).
    const envWithAi = Object.assign(Object.create(env as any), { AI: {} }) as any;
    const response = await worker.fetch(request, envWithAi, ctx);
    if (response.status !== 200) {
      throw new Error(`Unexpected status ${response.status}: ${await response.text()}`);
    }

    const json = (await response.json()) as any;
    expect(json.ok).toBe(true);
    expect(typeof json.runId).toBe("string");

    await waitOnExecutionContext(ctx);

    // The model produced 100 tiny text-delta parts. We should not call appendParts 100 times.
    expect(state.appendPartsCalls.length).toBeGreaterThan(0);
    expect(state.appendPartsCalls.length).toBeLessThan(10);

    const appended = state.appendPartsCalls.flatMap((c) => (Array.isArray(c.args.parts) ? c.args.parts : []));
    const deltaParts = appended.filter((p: any) => p?.part?.type === "text-delta");
    expect(deltaParts.length).toBe(1);
    expect(String(deltaParts[0].part.delta).length).toBe(100);

    const finishParts = appended.filter((p: any) => p?.part?.type === "finish");
    expect(finishParts.length).toBeGreaterThan(0);
    expect(finishParts[0]?.part?.usage?.totalTokens).toBeGreaterThan(0);

    // Seq must be monotonic within the batch.
    const seqs = appended.map((p: any) => Number(p.seq));
    const sorted = [...seqs].sort((a, b) => a - b);
    expect(seqs).toEqual(sorted);

    // Terminal finalize recorded.
    expect(state.finalizeRunCalls.length).toBeGreaterThan(0);
    const lastFinalize = state.finalizeRunCalls.at(-1)?.args as any;
    expect(["final", "error", "canceled"]).toContain(String(lastFinalize.status));
  });

  it("cancel requests are persisted (best-effort) and finalize as canceled", async () => {
    state.mode = "slow";
    state.failSnapshot = false;
    state.cancelCalls.length = 0;
    state.appendPartsCalls.length = 0;
    state.finalizeRunCalls.length = 0;

    const ORIGIN = "http://example.com";

    const sendReq = new Request(`${ORIGIN}/api/autopilot/send`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ threadId: "thread-2", anonKey: "anon-key-2", text: "hi" }),
    });

    const ctxSend = createExecutionContext();
    const envWithAi = Object.assign(Object.create(env as any), { AI: {} }) as any;
    const sendRes = await worker.fetch(sendReq, envWithAi, ctxSend);
    if (sendRes.status !== 200) {
      throw new Error(`Unexpected status ${sendRes.status}: ${await sendRes.text()}`);
    }
    const sendJson = (await sendRes.json()) as any;
    const runId = String(sendJson.runId);

    const cancelReq = new Request(`${ORIGIN}/api/autopilot/cancel`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ threadId: "thread-2", anonKey: "anon-key-2", runId }),
    });

    const ctxCancel = createExecutionContext();
    const cancelRes = await worker.fetch(cancelReq, envWithAi, ctxCancel);
    if (cancelRes.status !== 200) {
      throw new Error(`Unexpected status ${cancelRes.status}: ${await cancelRes.text()}`);
    }
    await waitOnExecutionContext(ctxCancel);

    await waitOnExecutionContext(ctxSend);

    expect(state.cancelCalls.length).toBeGreaterThan(0);

    const lastFinalize = state.finalizeRunCalls.at(-1)?.args as any;
    expect(String(lastFinalize.status)).toBe("canceled");
  });

  it("finalizes as error if Convex snapshot load fails (no stuck streaming)", async () => {
    state.mode = "fast";
    state.failSnapshot = true;
    state.createRunCalls.length = 0;
    state.appendPartsCalls.length = 0;
    state.finalizeRunCalls.length = 0;
    state.snapshotCalls.length = 0;

    const ORIGIN = "http://example.com";

    const request = new Request(`${ORIGIN}/api/autopilot/send`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ threadId: "thread-err", anonKey: "anon-key-err", text: "hi" }),
    });

    const ctx = createExecutionContext();
    const envWithAi = Object.assign(Object.create(env as any), { AI: {} }) as any;
    const response = await worker.fetch(request, envWithAi, ctx);
    if (response.status !== 200) {
      throw new Error(`Unexpected status ${response.status}: ${await response.text()}`);
    }

    await waitOnExecutionContext(ctx);

    expect(state.finalizeRunCalls.length).toBeGreaterThan(0);
    const lastFinalize = state.finalizeRunCalls.at(-1)?.args as any;
    expect(String(lastFinalize.status)).toBe("error");
  });
});
