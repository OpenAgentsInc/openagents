import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  // Records of calls made via ConvexService (worker-side).
  createRunCalls: [] as any[],
  appendPartsCalls: [] as any[],
  finalizeRunCalls: [] as any[],
  snapshotCalls: [] as any[],
  blueprintCalls: [] as any[],
  dseGetActiveCalls: [] as any[],
  recordPredictReceiptCalls: [] as any[],
  cancelRequested: new Set<string>(),
  nextRun: 1,
  lastUserText: "hi",
}));

vi.mock("@effect/ai/LanguageModel", async (importOriginal) => {
  const actual = await importOriginal<any>();
  const { Stream } = await import("effect");

  const streamText = (_args: any) => {
    const textId = "text-1";
    const meta = {};
    const marker = { ["~effect/ai/Content/Part"]: "~effect/ai/Content/Part" } as const;

    const parts = [
      { type: "text-start", id: textId, metadata: meta, ...marker },
      { type: "text-delta", id: textId, delta: "ok", metadata: meta, ...marker },
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

  const query = (ref: any, args: any) =>
    Effect.sync(() => {
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
            {
              messageId: "m-welcome",
              role: "assistant",
              status: "final",
              text: "Autopilot online.",
              runId: null,
              createdAtMs: 1,
              updatedAtMs: 1,
            },
            {
              messageId: "m-user-1",
              role: "user",
              status: "final",
              text: state.lastUserText,
              runId: null,
              createdAtMs: 2,
              updatedAtMs: 2,
            },
          ],
          parts: [],
        };
      }

      if (isRecord(args) && typeof args.threadId === "string" && !("runId" in args) && !("maxMessages" in args)) {
        state.blueprintCalls.push({ ref, args });
        return {
          ok: true,
          blueprint: {
            docs: {
              user: { addressAs: "Ada" },
              identity: { name: "Autopilot" },
            },
          },
        };
      }

      if (isRecord(args) && typeof args.signatureId === "string" && !("compiled_id" in args)) {
        state.dseGetActiveCalls.push({ ref, args });
        return { ok: true, compiled_id: null, updatedAtMs: null };
      }

      if (isRecord(args) && typeof args.signatureId === "string" && typeof args.compiled_id === "string") {
        return { ok: true, artifact: null };
      }

      if (isRecord(args) && typeof args.threadId === "string" && typeof args.runId === "string") {
        const runId = String(args.runId ?? "");
        return { ok: true, cancelRequested: state.cancelRequested.has(runId) };
      }

      throw new Error(`Unexpected Convex query in tests: ${String(ref?.name ?? ref)}`);
    });

  const mutation = (ref: any, args: any) =>
    Effect.sync(() => {
      if (isRecord(args) && Array.isArray(args.parts)) {
        state.appendPartsCalls.push({ ref, args });
        return { ok: true, inserted: Array.isArray(args.parts) ? args.parts.length : 0 };
      }

      if (isRecord(args) && typeof args.threadId === "string" && typeof args.runId === "string" && isRecord(args.receipt)) {
        state.recordPredictReceiptCalls.push({ ref, args });
        return { ok: true };
      }

      if (isRecord(args) && typeof args.status === "string" && typeof args.runId === "string") {
        state.finalizeRunCalls.push({ ref, args });
        return { ok: true };
      }

      if (isRecord(args) && typeof args.threadId === "string" && typeof args.runId === "string") {
        const runId = String(args.runId ?? "");
        if (runId) state.cancelRequested.add(runId);
        return { ok: true };
      }

      if (isRecord(args) && typeof args.threadId === "string" && typeof args.text === "string") {
        state.createRunCalls.push({ ref, args });
        state.lastUserText = String(args.text);
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

describe("apps/web worker autopilot: Stage 3 DSE signature in hot path", () => {
  it("records a DSE predict receipt and appends dse.signature parts into the chat stream", async () => {
    state.createRunCalls.length = 0;
    state.appendPartsCalls.length = 0;
    state.finalizeRunCalls.length = 0;
    state.snapshotCalls.length = 0;
    state.blueprintCalls.length = 0;
    state.dseGetActiveCalls.length = 0;
    state.recordPredictReceiptCalls.length = 0;

    const ORIGIN = "http://example.com";

    const request = new Request(`${ORIGIN}/api/autopilot/send`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ threadId: "thread-1", anonKey: "anon-key-1", text: "What can you do?" }),
    });

    const ctx = createExecutionContext();
    const envWithAi = Object.assign(Object.create(env as any), {
      AI: {
        run: async () => ({
          // DSE signature expects strict JSON, no markdown.
          choices: [{ message: { content: "{\"action\":\"none\"}" } }],
          usage: { prompt_tokens: 1, completion_tokens: 1 },
        }),
      },
    }) as any;

    const response = await worker.fetch(request, envWithAi, ctx);
    if (response.status !== 200) {
      throw new Error(`Unexpected status ${response.status}: ${await response.text()}`);
    }

    const json = (await response.json()) as any;
    expect(json.ok).toBe(true);

    await waitOnExecutionContext(ctx);

    // Receipt is recorded to Convex.
    expect(state.recordPredictReceiptCalls.length).toBeGreaterThan(0);
    const recorded = state.recordPredictReceiptCalls.at(-1)?.args as any;
    expect(recorded?.receipt?.signatureId).toBe("@openagents/autopilot/blueprint/SelectTool.v1");
    expect(typeof recorded?.receipt?.compiled_id).toBe("string");
    expect(String(recorded?.receipt?.compiled_id ?? "").length).toBeGreaterThan(0);

    // DSE signature parts are appended into the messageParts stream.
    const appended = state.appendPartsCalls.flatMap((c) => (Array.isArray(c.args.parts) ? c.args.parts : []));
    const sigParts = appended
      .map((p: any) => p?.part)
      .filter((p: any) => p?.type === "dse.signature" && p?.signatureId === "@openagents/autopilot/blueprint/SelectTool.v1");

    expect(sigParts.some((p: any) => p?.state === "start")).toBe(true);

    const ok = sigParts.find((p: any) => p?.state === "ok");
    expect(ok).toBeTruthy();
    expect(ok?.compiled_id).toBe(recorded.receipt.compiled_id);
    expect(ok?.receiptId).toBe(recorded.receipt.receiptId);
    expect(ok?.outputPreview?.action).toBe("none");
  });
});
