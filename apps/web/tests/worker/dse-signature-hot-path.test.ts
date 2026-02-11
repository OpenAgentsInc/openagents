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
  recordFeatureRequestCalls: [] as any[],
  cancelRequested: new Set<string>(),
  nextRun: 1,
  lastUserText: "hi",
  canary: null as any,
  artifacts: new Map<string, any>(),
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

      if (
        isRecord(args) &&
        typeof args.threadId === "string" &&
        !("runId" in args) &&
        !("maxMessages" in args) &&
        !("signatureId" in args)
      ) {
        state.blueprintCalls.push({ ref, args });
        const status = String((state as any).bootstrapStatus ?? "pending");
        const stage = status === "complete" ? null : String((state as any).bootstrapStage ?? "ask_user_handle");
        return {
          ok: true,
          blueprint: {
            bootstrapState: { status, stage },
            docs: {
              user: { addressAs: "Ada" },
              identity: { name: "Autopilot" },
            },
          },
        };
      }

      // Canary query (signatureId + threadId).
      if (isRecord(args) && typeof args.signatureId === "string" && typeof args.threadId === "string" && !("compiled_id" in args)) {
        return { ok: true, canary: state.canary };
      }

      // Active pointer query (signatureId only).
      if (isRecord(args) && typeof args.signatureId === "string" && !("compiled_id" in args) && !("threadId" in args)) {
        state.dseGetActiveCalls.push({ ref, args });
        return { ok: true, compiled_id: null, updatedAtMs: null };
      }

      if (isRecord(args) && typeof args.signatureId === "string" && typeof args.compiled_id === "string") {
        const key = `${String(args.signatureId)}::${String(args.compiled_id)}`;
        return { ok: true, artifact: state.artifacts.get(key) ?? null };
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

      // Bootstrap advancement mutations (Convex-first).
      if (isRecord(args) && typeof args.threadId === "string" && typeof args.handle === "string") {
        ;((state as any).applyBootstrapCalls ?? ((state as any).applyBootstrapCalls = [])).push({ ref, args })
        return { ok: true, applied: true, updatedAtMs: 1 }
      }
      if (isRecord(args) && typeof args.threadId === "string" && typeof args.name === "string") {
        ;((state as any).applyBootstrapCalls ?? ((state as any).applyBootstrapCalls = [])).push({ ref, args })
        return { ok: true, applied: true, updatedAtMs: 1 }
      }
      if (isRecord(args) && typeof args.threadId === "string" && typeof args.vibe === "string") {
        ;((state as any).applyBootstrapCalls ?? ((state as any).applyBootstrapCalls = [])).push({ ref, args })
        return { ok: true, applied: true, updatedAtMs: 1 }
      }
      if (isRecord(args) && typeof args.threadId === "string" && "boundaries" in args) {
        ;((state as any).applyBootstrapCalls ?? ((state as any).applyBootstrapCalls = [])).push({ ref, args })
        return { ok: true, applied: true, updatedAtMs: 1 }
      }

      if (
        isRecord(args) &&
        typeof args.threadId === "string" &&
        typeof args.runId === "string" &&
        typeof args.messageId === "string" &&
        typeof args.capabilityKey === "string" &&
        isRecord(args.source)
      ) {
        state.recordFeatureRequestCalls.push({ ref, args });
        return { ok: true, featureRequestId: "fr_test_1", existed: false, updatedAtMs: 1 };
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

describe("apps/web worker autopilot: Stage 3 bootstrap advancement in hot path", () => {
  it("advances bootstrap by persisting the user handle (ask_user_handle -> ask_agent_name)", async () => {
    state.createRunCalls.length = 0;
    state.appendPartsCalls.length = 0;
    state.finalizeRunCalls.length = 0;
    state.snapshotCalls.length = 0;
    state.blueprintCalls.length = 0;
    state.dseGetActiveCalls.length = 0;
    state.recordPredictReceiptCalls.length = 0;
    state.recordFeatureRequestCalls.length = 0;
    state.canary = null;
    state.artifacts.clear();
    (state as any).applyBootstrapCalls = [];
    (state as any).bootstrapStatus = "pending";
    (state as any).bootstrapStage = "ask_user_handle";

    const ORIGIN = "http://example.com";

    const request = new Request(`${ORIGIN}/api/autopilot/send`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ threadId: "thread-1", anonKey: "anon-key-1", text: "Ada" }),
    });

    const ctx = createExecutionContext();
    const envWithAi = Object.assign(Object.create(env as any), {
      AI: {
        run: async () => ({
          choices: [{ message: { content: "ok" } }],
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

    // Bootstrap advancement is persisted to Convex (best-effort).
    const applyCalls = ((state as any).applyBootstrapCalls ?? []) as any[];
    expect(applyCalls.length).toBeGreaterThan(0);
    const last = applyCalls.at(-1)?.args as any;
    expect(last?.threadId).toBe("thread-1");
    expect(last?.handle).toBe("Ada");

    // No DSE signature parts are appended into the messageParts stream (bootstrap-only in hot path).
    const appended = state.appendPartsCalls.flatMap((c) => (Array.isArray(c.args.parts) ? c.args.parts : []));
    const hasSigParts = appended.some((p: any) => p?.part?.type === "dse.signature");
    expect(hasSigParts).toBe(false);
  });

  it("advances bootstrap by persisting the agent name (ask_agent_name -> ask_vibe)", async () => {
    state.createRunCalls.length = 0;
    state.appendPartsCalls.length = 0;
    state.finalizeRunCalls.length = 0;
    state.snapshotCalls.length = 0;
    state.blueprintCalls.length = 0;
    state.dseGetActiveCalls.length = 0;
    state.recordPredictReceiptCalls.length = 0;
    state.recordFeatureRequestCalls.length = 0;
    state.artifacts.clear();
    (state as any).applyBootstrapCalls = [];
    (state as any).bootstrapStatus = "pending";
    (state as any).bootstrapStage = "ask_agent_name";

    const ORIGIN = "http://example.com";
    const request = new Request(`${ORIGIN}/api/autopilot/send`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ threadId: "thread-2", anonKey: "anon-key-1", text: "Autopilot" }),
    });

    const ctx = createExecutionContext();
    const envWithAi = Object.assign(Object.create(env as any), {
      AI: {
        run: async () => ({
          choices: [{ message: { content: "ok" } }],
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

    const applyCalls = ((state as any).applyBootstrapCalls ?? []) as any[];
    expect(applyCalls.length).toBeGreaterThan(0);
    const last = applyCalls.at(-1)?.args as any;
    expect(last?.threadId).toBe("thread-2");
    expect(last?.name).toBe("Autopilot");
  });

  it("runs DetectUpgradeRequest signature after bootstrap completion and records a feature request in Convex", async () => {
    state.createRunCalls.length = 0;
    state.appendPartsCalls.length = 0;
    state.finalizeRunCalls.length = 0;
    state.snapshotCalls.length = 0;
    state.blueprintCalls.length = 0;
    state.dseGetActiveCalls.length = 0;
    state.recordPredictReceiptCalls.length = 0;
    state.recordFeatureRequestCalls.length = 0;
    state.artifacts.clear();
    (state as any).applyBootstrapCalls = [];
    (state as any).bootstrapStatus = "complete";
    (state as any).bootstrapStage = null;

    const ORIGIN = "http://example.com";
    const request = new Request(`${ORIGIN}/api/autopilot/send`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        threadId: "thread-3",
        anonKey: "anon-key-1",
        text: "Connect to my GitHub and run Codex remotely in the cloud.",
      }),
    });

    const ctx = createExecutionContext();
    const envWithAi = Object.assign(Object.create(env as any), {
      AI: {
        run: async () => ({
          choices: [
            {
              message: {
                content: JSON.stringify({
                  isUpgradeRequest: true,
                  capabilityKey: "github_cloud_codex",
                  capabilityLabel: "GitHub + cloud Codex execution",
                  summary: "User asks for GitHub connection and remote cloud Codex execution.",
                  notifyWhenAvailable: false,
                  confidence: 0.97,
                }),
              },
            },
          ],
          usage: { prompt_tokens: 20, completion_tokens: 30 },
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

    const appended = state.appendPartsCalls.flatMap((c) => (Array.isArray(c.args.parts) ? c.args.parts : []));
    const signatureParts = appended
      .map((p: any) => p?.part)
      .filter((part: any) => part?.type === "dse.signature" && part?.signatureId === "@openagents/autopilot/feedback/DetectUpgradeRequest.v1");
    expect(signatureParts.length).toBeGreaterThan(0);
    expect(signatureParts.some((part: any) => part?.state === "ok")).toBe(true);
    expect(signatureParts.some((part: any) => part?.outputPreview?.isUpgradeRequest === true)).toBe(true);

    const toolParts = appended
      .map((p: any) => p?.part)
      .filter((part: any) => part?.type === "dse.tool" && part?.toolName === "record_feature_request");
    expect(toolParts.length).toBeGreaterThan(0);
    expect(toolParts.some((part: any) => part?.state === "ok")).toBe(true);

    expect(state.recordFeatureRequestCalls.length).toBeGreaterThan(0);
    const recorded = state.recordFeatureRequestCalls.at(-1)?.args as any;
    expect(recorded?.threadId).toBe("thread-3");
    expect(recorded?.runId).toMatch(/^run-/);
    expect(recorded?.capabilityKey).toBe("github_cloud_codex");
    expect(recorded?.source?.signatureId).toBe("@openagents/autopilot/feedback/DetectUpgradeRequest.v1");
  });

  it("falls back to heuristic capability classification when DetectUpgradeRequest decode fails", async () => {
    state.createRunCalls.length = 0;
    state.appendPartsCalls.length = 0;
    state.finalizeRunCalls.length = 0;
    state.snapshotCalls.length = 0;
    state.blueprintCalls.length = 0;
    state.dseGetActiveCalls.length = 0;
    state.recordPredictReceiptCalls.length = 0;
    state.recordFeatureRequestCalls.length = 0;
    state.artifacts.clear();
    (state as any).applyBootstrapCalls = [];
    (state as any).bootstrapStatus = "complete";
    (state as any).bootstrapStage = null;

    const ORIGIN = "http://example.com";
    const request = new Request(`${ORIGIN}/api/autopilot/send`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        threadId: "thread-4",
        anonKey: "anon-key-1",
        text: "Please notify me when you can connect to my GitHub and deploy automatically.",
      }),
    });

    const ctx = createExecutionContext();
    const envWithAi = Object.assign(Object.create(env as any), {
      AI: {
        // DSE predict/repair attempts receive non-JSON here -> decode failure path.
        run: async () => ({
          choices: [{ message: { content: "This is definitely an upgrade request." } }],
          usage: { prompt_tokens: 20, completion_tokens: 30 },
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

    const appended = state.appendPartsCalls.flatMap((c) => (Array.isArray(c.args.parts) ? c.args.parts : []));
    const signatureParts = appended
      .map((p: any) => p?.part)
      .filter((part: any) => part?.type === "dse.signature" && part?.signatureId === "@openagents/autopilot/feedback/DetectUpgradeRequest.v1");

    expect(signatureParts.length).toBeGreaterThan(0);
    expect(signatureParts.some((part: any) => part?.state === "ok")).toBe(true);
    expect(signatureParts.some((part: any) => part?.fallbackUsed === true)).toBe(true);
    expect(signatureParts.some((part: any) => typeof part?.errorText === "string" && part.errorText.length > 0)).toBe(true);
    expect(signatureParts.some((part: any) => part?.outputPreview?.isUpgradeRequest === true)).toBe(true);

    expect(state.recordFeatureRequestCalls.length).toBeGreaterThan(0);
    const recorded = state.recordFeatureRequestCalls.at(-1)?.args as any;
    expect(recorded?.threadId).toBe("thread-4");
    expect(recorded?.notifyWhenAvailable).toBe(true);
    expect(recorded?.capabilityKey).toBe("github_integration_and_auto_deploy");
    expect(recorded?.source?.signatureId).toBe("@openagents/autopilot/feedback/DetectUpgradeRequest.v1");
  });
});
