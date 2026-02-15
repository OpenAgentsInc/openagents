import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

type TerminalMode = "completed" | "blocked";

const state = vi.hoisted(() => ({
  modelStreamCalls: 0,
  nextRun: 1,
  nextTask: 1,
  terminalMode: "blocked" as TerminalMode,
  createRunCalls: [] as any[],
  appendPartsCalls: [] as any[],
  finalizeRunCalls: [] as any[],
  lightningCreateTaskCalls: [] as any[],
  lightningGetTaskCalls: [] as any[],
  lightningListTaskEventsCalls: [] as any[],
  lightningTransitionTaskCalls: [] as any[],
  lastUserTextByThread: new Map<string, string>(),
  lightningTasks: new Map<string, any>(),
  lightningEvents: new Map<string, any[]>(),
}));

vi.mock("@effect/ai/LanguageModel", async (importOriginal) => {
  const actual = await importOriginal<any>();
  const { Stream } = await import("effect");

  const streamText = (_args: any) => {
    state.modelStreamCalls += 1;
    const marker = { ["~effect/ai/Content/Part"]: "~effect/ai/Content/Part" } as const;
    return Stream.fromIterable([
      { type: "text-start", id: "text-1", metadata: {}, ...marker },
      { type: "text-delta", id: "text-1", delta: "fallback", metadata: {}, ...marker },
      { type: "text-end", id: "text-1", metadata: {}, ...marker },
      {
        type: "finish",
        reason: "stop",
        usage: { inputTokens: 1, outputTokens: 1, totalTokens: 2 },
        metadata: {},
        ...marker,
      },
    ]);
  };

  return { ...actual, streamText };
});

vi.mock("../../src/effect/convex", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../src/effect/convex")>();
  const { Effect, Layer, Stream } = await import("effect");

  const isRecord = (value: unknown): value is Record<string, any> =>
    Boolean(value) && typeof value === "object" && !Array.isArray(value);

  const makeTerminalEvent = (task: any) => {
    if (state.terminalMode === "completed") {
      return {
        toStatus: "completed",
        metadata: {
          proofReference: "preimage:abc123",
          paymentId: "pay-1",
          responseStatusCode: 200,
          amountMsats: Number(task?.request?.maxSpendMsats ?? 0),
          responseContentType: "application/json",
          responseBytes: 11,
          responseBodyTextPreview: '{"ok":true}',
          responseBodySha256: "4062edaf750fb8074e7e83e0c9028c94e32468a8b6f1614774328ef045150f93",
          cacheHit: false,
          paid: true,
          cacheStatus: "miss",
          paymentBackend: "spark",
        },
      };
    }
    return {
      toStatus: "blocked",
      reason: "policy_denied",
      errorMessage: "policy_denied",
      metadata: {
        denyReason: "policy_denied",
      },
    };
  };

  const query = (_ref: unknown, args: unknown) =>
    Effect.sync(() => {
      if (
        isRecord(args) &&
        typeof args.threadId === "string" &&
        typeof args.maxMessages === "number" &&
        typeof args.maxParts === "number"
      ) {
        const userText = state.lastUserTextByThread.get(args.threadId) ?? "";
        return {
          ok: true,
          threadId: args.threadId,
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
              messageId: `m-user-${args.threadId}`,
              role: "user",
              status: "final",
              text: userText,
              runId: null,
              createdAtMs: 2,
              updatedAtMs: 2,
            },
          ],
          parts: [],
        };
      }

      if (isRecord(args) && typeof args.threadId === "string" && !("maxMessages" in args) && !("runId" in args)) {
        return {
          ok: true,
          blueprint: {
            bootstrapState: {
              status: "complete",
              stage: null,
            },
          },
          updatedAtMs: 1,
        };
      }

      if (isRecord(args) && typeof args.threadId === "string" && typeof args.runId === "string") {
        return { ok: true, cancelRequested: false };
      }

      // lightning.presence.getLatestExecutorPresence ({})
      if (isRecord(args) && Object.keys(args).length === 0) {
        const now = Date.now();
        return {
          ok: true,
          presence: {
            ownerId: "user-1",
            deviceId: "desktop-1",
            lastSeenAtMs: now,
            version: "desktop-test",
            capabilities: ["l402_executor"],
            createdAtMs: now,
            updatedAtMs: now,
          },
        };
      }

      if (isRecord(args) && typeof args.taskId === "string" && !("limit" in args)) {
        state.lightningGetTaskCalls.push(args);
        const task = state.lightningTasks.get(args.taskId);
        if (!task) return { ok: true, task: null };
        // Simulate the desktop executor completing work after explicit approval.
        if (task.status === "approved") {
          task.status = state.terminalMode;
          task.updatedAtMs = Date.now();
          state.lightningTasks.set(args.taskId, task);
          state.lightningEvents.set(args.taskId, [makeTerminalEvent(task)]);
        }
        return { ok: true, task };
      }

      if (isRecord(args) && typeof args.taskId === "string" && typeof args.limit === "number") {
        state.lightningListTaskEventsCalls.push(args);
        return { ok: true, events: state.lightningEvents.get(args.taskId) ?? [] };
      }

      throw new Error(`Unexpected Convex query in lightning-tool-chat test: ${JSON.stringify(args)}`);
    });

  const mutation = (_ref: unknown, args: unknown) =>
    Effect.sync(() => {
      if (isRecord(args) && typeof args.status === "string" && typeof args.runId === "string") {
        state.finalizeRunCalls.push(args);
        return { ok: true };
      }

      if (isRecord(args) && typeof args.threadId === "string" && typeof args.text === "string") {
        state.createRunCalls.push(args);
        state.lastUserTextByThread.set(args.threadId, args.text);
        const runId = `run-${state.nextRun++}`;
        return {
          ok: true,
          runId,
          userMessageId: `m-user-${runId}`,
          assistantMessageId: `m-assistant-${runId}`,
        };
      }

      if (isRecord(args) && Array.isArray(args.parts)) {
        state.appendPartsCalls.push(args);
        return { ok: true, inserted: args.parts.length };
      }

      if (isRecord(args) && isRecord(args.request) && typeof args.request.url === "string") {
        state.lightningCreateTaskCalls.push(args);
        const taskId = `task-${state.nextTask++}`;
        const task = {
          taskId,
          ownerId: "user-1",
          status: "queued",
          request: args.request,
          attemptCount: 0,
          createdAtMs: Date.now(),
          updatedAtMs: Date.now(),
          lastTransitionAtMs: Date.now(),
        };
        state.lightningTasks.set(taskId, task);
        return { ok: true, existed: false, task };
      }

      if (isRecord(args) && typeof args.taskId === "string" && typeof args.toStatus === "string") {
        state.lightningTransitionTaskCalls.push(args);
        const task = state.lightningTasks.get(args.taskId);
        if (!task) return { ok: false };
        if (task.status === args.toStatus) {
          return {
            ok: true,
            changed: false,
            task,
            event: null,
          };
        }
        task.status = args.toStatus;
        task.lastErrorCode = args.errorCode ?? undefined;
        task.lastErrorMessage = args.errorMessage ?? undefined;
        task.updatedAtMs = Date.now();
        state.lightningTasks.set(args.taskId, task);
        const events = state.lightningEvents.get(args.taskId) ?? [];
        events.push({
          toStatus: args.toStatus,
          reason: args.reason,
          errorCode: args.errorCode,
          errorMessage: args.errorMessage,
          metadata: args.metadata,
        });
        state.lightningEvents.set(args.taskId, events);
        return {
          ok: true,
          changed: true,
          task,
          event: events[events.length - 1],
        };
      }

      throw new Error(`Unexpected Convex mutation in lightning-tool-chat test: ${JSON.stringify(args)}`);
    });

  const action = (_ref: unknown, _args: unknown) => Effect.fail(new Error("convex.action not used in tests"));
  const subscribeQuery = (_ref: unknown, _args: unknown) =>
    Stream.fail(new Error("convex.subscribeQuery not used in Worker tests"));

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, { query, mutation, action, subscribeQuery });
  return { ...actual, ConvexServiceLive };
});

const { default: worker } = await import("../../src/effuse-host/worker");

const resetState = () => {
  state.modelStreamCalls = 0;
  state.nextRun = 1;
  state.nextTask = 1;
  state.terminalMode = "blocked";
  state.createRunCalls.length = 0;
  state.appendPartsCalls.length = 0;
  state.finalizeRunCalls.length = 0;
  state.lightningCreateTaskCalls.length = 0;
  state.lightningGetTaskCalls.length = 0;
  state.lightningListTaskEventsCalls.length = 0;
  state.lightningTransitionTaskCalls.length = 0;
  state.lastUserTextByThread.clear();
  state.lightningTasks.clear();
  state.lightningEvents.clear();
};

const finalLightningToolPart = () => {
  const parts = state.appendPartsCalls.flatMap((call: any) => (Array.isArray(call.parts) ? call.parts : []));
  return [...parts]
    .map((row: any) => row?.part)
    .reverse()
    .find((part: any) => part?.type === "dse.tool" && part?.state !== "start");
};

const useWalletExecutorFetchMock = (input: {
  readonly endpointUrl: string;
  readonly quotedAmountMsats: number;
  readonly responseBody: string;
  readonly walletMode: "ok" | "fail";
}) => {
  let payCalls = 0;
  let endpointCalls = 0;

  const mockFetch = vi.fn(async (resource: RequestInfo | URL, init?: RequestInit) => {
    const url = typeof resource === "string" ? resource : resource instanceof URL ? resource.toString() : resource.url;
    if (url === "https://wallet-executor.example/pay-bolt11") {
      payCalls += 1;
      if (input.walletMode === "fail") {
        return new Response(
          JSON.stringify({
            ok: false,
            error: {
              code: "wallet_backend_unavailable",
              message: "wallet backend unavailable",
            },
          }),
          { status: 502, headers: { "content-type": "application/json" } },
        );
      }
      return new Response(
        JSON.stringify({
          ok: true,
          result: {
            payment: {
              paymentId: `pay-${payCalls}`,
              amountMsats: input.quotedAmountMsats,
              preimageHex: "ab".repeat(32),
              paidAtMs: Date.now(),
            },
            quotedAmountMsats: input.quotedAmountMsats,
            windowSpendMsatsAfterPayment: input.quotedAmountMsats,
          },
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    }

    if (url === input.endpointUrl) {
      endpointCalls += 1;
      const headers = new Headers(init?.headers as HeadersInit | undefined);
      const authorization = headers.get("Authorization");
      if (!authorization) {
        return new Response("", {
          status: 402,
          headers: {
            "www-authenticate": `L402 invoice="lnbcrt1invoice_worker_${input.quotedAmountMsats}", macaroon="mac_worker", amount_msats=${input.quotedAmountMsats}`,
          },
        });
      }
      return new Response(input.responseBody, {
        status: 200,
        headers: {
          "content-type": "application/json",
        },
      });
    }

    return new Response("not_found", { status: 404 });
  });

  vi.stubGlobal("fetch", mockFetch as typeof globalThis.fetch);
  return {
    getPayCalls: () => payCalls,
    getEndpointCalls: () => endpointCalls,
    mockFetch,
    restore: () => {
      vi.unstubAllGlobals();
    },
  };
};

describe("apps/web worker lightning_l402_fetch chat runtime", () => {
  it("invokes lightning_l402_fetch and returns queued approval intent by default", async () => {
    resetState();
    state.terminalMode = "blocked";

    const request = new Request("http://example.com/api/autopilot/send", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        threadId: "thread-lightning-1",
        text: 'lightning_l402_fetch({"url":"https://api.example.com/premium","maxSpendMsats":1200})',
      }),
    });

    const ctx = createExecutionContext();
    const envWithAi = Object.assign(Object.create(env as any), { AI: {} }) as any;
    const response = await worker.fetch(request, envWithAi, ctx);
    expect(response.status).toBe(200);
    await waitOnExecutionContext(ctx);

    expect(state.modelStreamCalls).toBe(0);
    expect(state.lightningCreateTaskCalls).toHaveLength(1);
    expect(state.lightningTransitionTaskCalls).toHaveLength(0);
    expect(state.lightningGetTaskCalls).toHaveLength(0);
    expect(state.lightningListTaskEventsCalls).toHaveLength(0);

    const parts = state.appendPartsCalls.flatMap((call: any) => (Array.isArray(call.parts) ? call.parts : []));
    const finalToolPart = [...parts]
      .map((row: any) => row?.part)
      .reverse()
      .find((part: any) => part?.type === "dse.tool" && part?.state !== "start");
    expect(finalToolPart?.toolName).toBe("lightning_l402_fetch");
    expect(finalToolPart?.state).toBe("approval-requested");
    expect(finalToolPart?.output?.status).toBe("queued");
    expect(finalToolPart?.output?.approvalRequired).toBe(true);
    expect(String(finalToolPart?.output?.taskId ?? "")).toMatch(/^task-/);

    const finalized = state.finalizeRunCalls.at(-1);
    expect(finalized?.status).toBe("final");
    expect(String(finalized?.text ?? "")).toContain("Approval required");
  });

  it("routes natural language sats4ai L402 request to lightning_l402_fetch (no model stream)", async () => {
    resetState();
    state.terminalMode = "blocked";

    const request = new Request("http://example.com/api/autopilot/send", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        threadId: "thread-lightning-natural-sats4ai",
        text: "Call the sats4ai L402 text generation endpoint with 'Tell me one short fact about Bitcoin'. Max spend 100 sats. Ask me to approve before paying.",
      }),
    });

    const ctx = createExecutionContext();
    const envWithAi = Object.assign(Object.create(env as any), { AI: {} }) as any;
    const response = await worker.fetch(request, envWithAi, ctx);
    expect(response.status).toBe(200);
    await waitOnExecutionContext(ctx);

    expect(state.modelStreamCalls).toBe(0);
    expect(state.lightningCreateTaskCalls).toHaveLength(1);

    const created = state.lightningCreateTaskCalls[0];
    expect(created?.request?.url).toBe("https://sats4ai.com/api/l402/text-generation");
    expect(created?.request?.method).toBe("POST");
    expect(created?.request?.maxSpendMsats).toBe(100_000);
    expect(created?.request?.headers?.["Content-Type"]).toBe("application/json");
    expect(typeof created?.request?.body).toBe("string");
    expect(String(created?.request?.body ?? "")).toContain("Tell me one short fact about Bitcoin");

    const finalToolPart = finalLightningToolPart();
    expect(finalToolPart?.toolName).toBe("lightning_l402_fetch");
    expect(finalToolPart?.state).toBe("approval-requested");
    expect(finalToolPart?.output?.status).toBe("queued");
    expect(finalToolPart?.output?.approvalRequired).toBe(true);
  });

  it("supports endpointPreset A (resolved by env) so prompts don't embed URLs", async () => {
    resetState();
    state.terminalMode = "blocked";

    const request = new Request("http://example.com/api/autopilot/send", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        threadId: "thread-lightning-preset-a",
        text: 'lightning_l402_fetch({"endpointPreset":"A","maxSpendMsats":1200})',
      }),
    });

    const ctx = createExecutionContext();
    const envWithAi = Object.assign(Object.create(env as any), {
      AI: {},
      OA_EP212_ENDPOINT_A_URL: "https://api.example.com/preset-a",
    }) as any;
    const response = await worker.fetch(request, envWithAi, ctx);
    expect(response.status).toBe(200);
    await waitOnExecutionContext(ctx);

    expect(state.lightningCreateTaskCalls).toHaveLength(1);
    expect(state.lightningCreateTaskCalls[0]?.request?.url).toBe("https://api.example.com/preset-a");
  });

  it("invokes lightning_l402_fetch and completes with blocked terminal state when approval is disabled", async () => {
    resetState();
    state.terminalMode = "blocked";

    const request = new Request("http://example.com/api/autopilot/send", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        threadId: "thread-lightning-2",
        text: 'lightning_l402_fetch({"url":"https://api.example.com/premium","maxSpendMsats":1200,"requireApproval":false})',
      }),
    });

    const ctx = createExecutionContext();
    const envWithAi = Object.assign(Object.create(env as any), { AI: {} }) as any;
    const response = await worker.fetch(request, envWithAi, ctx);
    expect(response.status).toBe(200);
    await waitOnExecutionContext(ctx);

    expect(state.modelStreamCalls).toBe(0);
    expect(state.lightningCreateTaskCalls).toHaveLength(1);
    expect(state.lightningTransitionTaskCalls).toHaveLength(1);
    expect(state.lightningGetTaskCalls.length).toBeGreaterThan(0);
    expect(state.lightningListTaskEventsCalls).toHaveLength(1);

    const parts = state.appendPartsCalls.flatMap((call: any) => (Array.isArray(call.parts) ? call.parts : []));
    const finalToolPart = [...parts]
      .map((row: any) => row?.part)
      .reverse()
      .find((part: any) => part?.type === "dse.tool" && part?.state !== "start");
    expect(finalToolPart?.toolName).toBe("lightning_l402_fetch");
    expect(finalToolPart?.output?.status).toBe("blocked");
    expect(finalToolPart?.output?.denyReason).toBe("policy_denied");
    expect(finalToolPart?.output?.approvalRequired).toBe(false);

    const finalized = state.finalizeRunCalls.at(-1);
    expect(finalized?.status).toBe("final");
    expect(String(finalized?.text ?? "")).toContain("blocked");
  });

  it("returns proof reference on completed terminal state", async () => {
    resetState();
    state.terminalMode = "completed";

    const request = new Request("http://example.com/api/autopilot/send", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        threadId: "thread-lightning-2",
        text: 'lightning_l402_fetch({"url":"https://api.example.com/paid","maxSpendMsats":900,"requireApproval":false})',
      }),
    });

    const ctx = createExecutionContext();
    const envWithAi = Object.assign(Object.create(env as any), { AI: {} }) as any;
    const response = await worker.fetch(request, envWithAi, ctx);
    expect(response.status).toBe(200);
    await waitOnExecutionContext(ctx);

    const parts = state.appendPartsCalls.flatMap((call: any) => (Array.isArray(call.parts) ? call.parts : []));
    const finalToolPart = [...parts]
      .map((row: any) => row?.part)
      .reverse()
      .find((part: any) => part?.type === "dse.tool" && part?.state !== "start");
    expect(finalToolPart?.output?.status).toBe("completed");
    expect(finalToolPart?.output?.proofReference).toBe("preimage:abc123");
    expect(finalToolPart?.output?.paymentId).toBe("pay-1");
    expect(finalToolPart?.output?.responseContentType).toBe("application/json");
    expect(finalToolPart?.output?.responseBytes).toBe(11);
    expect(finalToolPart?.output?.responseBodyTextPreview).toBe('{"ok":true}');
    expect(finalToolPart?.output?.responseBodySha256).toBe(
      "4062edaf750fb8074e7e83e0c9028c94e32468a8b6f1614774328ef045150f93",
    );
    expect(finalToolPart?.output?.cacheHit).toBe(false);
    expect(finalToolPart?.output?.paid).toBe(true);
    expect(finalToolPart?.output?.cacheStatus).toBe("miss");
    expect(finalToolPart?.output?.paymentBackend).toBe("spark");
    expect(finalToolPart?.output?.approvalRequired).toBe(false);
  });

  it("rejects invalid tool params via schema validation", async () => {
    resetState();

    const request = new Request("http://example.com/api/autopilot/send", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        threadId: "thread-lightning-3",
        text: 'lightning_l402_fetch({"url":"","maxSpendMsats":-1})',
      }),
    });

    const ctx = createExecutionContext();
    const envWithAi = Object.assign(Object.create(env as any), { AI: {} }) as any;
    const response = await worker.fetch(request, envWithAi, ctx);
    expect(response.status).toBe(200);
    await waitOnExecutionContext(ctx);

    expect(state.lightningCreateTaskCalls).toHaveLength(0);
    const parts = state.appendPartsCalls.flatMap((call: any) => (Array.isArray(call.parts) ? call.parts : []));
    const finalToolPart = [...parts]
      .map((row: any) => row?.part)
      .reverse()
      .find((part: any) => part?.type === "dse.tool" && part?.state !== "start");
    expect(finalToolPart?.output?.status).toBe("blocked");
    expect(finalToolPart?.output?.denyReason).toBe("invalid_params");
  });
});

describe("apps/web worker lightning_l402_fetch with server wallet executor", () => {
  it("executes queued request after explicit approval and returns terminal receipt in approve output", async () => {
    resetState();
    const fetchMock = useWalletExecutorFetchMock({
      endpointUrl: "https://api.example.com/l402-approve",
      quotedAmountMsats: 1_500,
      responseBody: '{"approved":"ok"}',
      walletMode: "ok",
    });

    try {
      const envWithAi = Object.assign(Object.create(env as any), {
        AI: {},
        OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL: "https://wallet-executor.example",
      }) as any;

      const first = new Request("http://example.com/api/autopilot/send", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          threadId: "thread-lightning-wallet-approve",
          text: 'lightning_l402_fetch({"url":"https://api.example.com/l402-approve","maxSpendMsats":2000})',
        }),
      });
      const firstCtx = createExecutionContext();
      const firstRes = await worker.fetch(first, envWithAi, firstCtx);
      expect(firstRes.status).toBe(200);
      await waitOnExecutionContext(firstCtx);

      expect(fetchMock.getPayCalls()).toBe(0);
      const queuedPart = finalLightningToolPart();
      expect(queuedPart?.toolName).toBe("lightning_l402_fetch");
      expect(queuedPart?.output?.status).toBe("queued");
      const taskId = String(queuedPart?.output?.taskId ?? "");
      expect(taskId).toMatch(/^task-/);

      const second = new Request("http://example.com/api/autopilot/send", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          threadId: "thread-lightning-wallet-approve",
          text: `lightning_l402_approve(${JSON.stringify({ taskId })})`,
        }),
      });
      const secondCtx = createExecutionContext();
      const secondRes = await worker.fetch(second, envWithAi, secondCtx);
      expect(secondRes.status).toBe(200);
      await waitOnExecutionContext(secondCtx);

      expect(fetchMock.getPayCalls()).toBe(1);
      const approvePart = finalLightningToolPart();
      expect(approvePart?.toolName).toBe("lightning_l402_approve");
      expect(approvePart?.state).toBe("ok");
      expect(approvePart?.output?.ok).toBe(true);
      expect(approvePart?.output?.taskId).toBe(taskId);
      expect(approvePart?.output?.terminal?.status).toBe("completed");
      expect(approvePart?.output?.terminal?.paymentBackend).toBe("spark");
      expect(approvePart?.output?.terminal?.paid).toBe(true);
      expect(approvePart?.output?.terminal?.responseBodyTextPreview).toBe('{"approved":"ok"}');
      expect(typeof approvePart?.output?.terminal?.responseBodySha256).toBe("string");
    } finally {
      fetchMock.restore();
    }
  });

  it("executes paid request with wallet executor and returns receipt metadata", async () => {
    resetState();
    const fetchMock = useWalletExecutorFetchMock({
      endpointUrl: "https://api.example.com/l402-success",
      quotedAmountMsats: 1_500,
      responseBody: '{"premium":"ok"}',
      walletMode: "ok",
    });

    try {
      const request = new Request("http://example.com/api/autopilot/send", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          threadId: "thread-lightning-wallet-success",
          text: 'lightning_l402_fetch({"url":"https://api.example.com/l402-success","maxSpendMsats":2000,"requireApproval":false})',
        }),
      });

      const ctx = createExecutionContext();
      const envWithAi = Object.assign(Object.create(env as any), {
        AI: {},
        OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL: "https://wallet-executor.example",
      }) as any;
      const response = await worker.fetch(request, envWithAi, ctx);
      expect(response.status).toBe(200);
      await waitOnExecutionContext(ctx);

      expect(fetchMock.getPayCalls()).toBe(1);
      const finalToolPart = finalLightningToolPart();
      expect(finalToolPart?.output?.status).toBe("completed");
      expect(finalToolPart?.output?.paymentBackend).toBe("spark");
      expect(finalToolPart?.output?.paid).toBe(true);
      expect(finalToolPart?.output?.cacheStatus).toBe("miss");
      expect(finalToolPart?.output?.responseBodyTextPreview).toBe('{"premium":"ok"}');
      expect(typeof finalToolPart?.output?.responseBodySha256).toBe("string");
    } finally {
      fetchMock.restore();
    }
  });

  it("reuses cached credential on repeat request without a second payment", async () => {
    resetState();
    const fetchMock = useWalletExecutorFetchMock({
      endpointUrl: "https://api.example.com/l402-cache",
      quotedAmountMsats: 1_200,
      responseBody: '{"cached":true}',
      walletMode: "ok",
    });

    try {
      const envWithAi = Object.assign(Object.create(env as any), {
        AI: {},
        OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL: "https://wallet-executor.example",
      }) as any;

      const first = new Request("http://example.com/api/autopilot/send", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          threadId: "thread-lightning-wallet-cache",
          text: 'lightning_l402_fetch({"url":"https://api.example.com/l402-cache","maxSpendMsats":2000,"requireApproval":false})',
        }),
      });
      const firstCtx = createExecutionContext();
      const firstRes = await worker.fetch(first, envWithAi, firstCtx);
      expect(firstRes.status).toBe(200);
      await waitOnExecutionContext(firstCtx);
      expect(fetchMock.getPayCalls()).toBe(1);

      const second = new Request("http://example.com/api/autopilot/send", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          threadId: "thread-lightning-wallet-cache",
          text: 'lightning_l402_fetch({"url":"https://api.example.com/l402-cache","maxSpendMsats":2000,"requireApproval":false})',
        }),
      });
      const secondCtx = createExecutionContext();
      const secondRes = await worker.fetch(second, envWithAi, secondCtx);
      expect(secondRes.status).toBe(200);
      await waitOnExecutionContext(secondCtx);

      expect(fetchMock.getPayCalls()).toBe(1);
      const finalToolPart = finalLightningToolPart();
      expect(finalToolPart?.output?.status).toBe("cached");
      expect(finalToolPart?.output?.cacheHit).toBe(true);
      expect(finalToolPart?.output?.paid).toBe(false);
      expect(finalToolPart?.output?.cacheStatus).toBe("hit");
    } finally {
      fetchMock.restore();
    }
  });

  it("blocks over-cap invoice before payment using deterministic deny reason", async () => {
    resetState();
    const fetchMock = useWalletExecutorFetchMock({
      endpointUrl: "https://api.example.com/l402-overcap",
      quotedAmountMsats: 9_999,
      responseBody: '{"shouldNot":"happen"}',
      walletMode: "ok",
    });

    try {
      const request = new Request("http://example.com/api/autopilot/send", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          threadId: "thread-lightning-wallet-overcap",
          text: 'lightning_l402_fetch({"url":"https://api.example.com/l402-overcap","maxSpendMsats":1000,"requireApproval":false})',
        }),
      });

      const ctx = createExecutionContext();
      const envWithAi = Object.assign(Object.create(env as any), {
        AI: {},
        OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL: "https://wallet-executor.example",
      }) as any;
      const response = await worker.fetch(request, envWithAi, ctx);
      expect(response.status).toBe(200);
      await waitOnExecutionContext(ctx);

      expect(fetchMock.getPayCalls()).toBe(0);
      const finalToolPart = finalLightningToolPart();
      expect(finalToolPart?.output?.status).toBe("blocked");
      expect(finalToolPart?.output?.denyReasonCode).toBe("amount_over_cap");
      expect(finalToolPart?.output?.quotedAmountMsats).toBe(9_999);
      expect(finalToolPart?.output?.maxSpendMsats).toBe(1_000);
    } finally {
      fetchMock.restore();
    }
  });

  it("surfaces wallet executor failures as deterministic failed status", async () => {
    resetState();
    const fetchMock = useWalletExecutorFetchMock({
      endpointUrl: "https://api.example.com/l402-wallet-fail",
      quotedAmountMsats: 1_500,
      responseBody: '{"unused":true}',
      walletMode: "fail",
    });

    try {
      const request = new Request("http://example.com/api/autopilot/send", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          threadId: "thread-lightning-wallet-failure",
          text: 'lightning_l402_fetch({"url":"https://api.example.com/l402-wallet-fail","maxSpendMsats":2000,"requireApproval":false})',
        }),
      });

      const ctx = createExecutionContext();
      const envWithAi = Object.assign(Object.create(env as any), {
        AI: {},
        OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL: "https://wallet-executor.example",
      }) as any;
      const response = await worker.fetch(request, envWithAi, ctx);
      expect(response.status).toBe(200);
      await waitOnExecutionContext(ctx);

      expect(fetchMock.getPayCalls()).toBe(1);
      const finalToolPart = finalLightningToolPart();
      expect(finalToolPart?.output?.status).toBe("failed");
      expect(finalToolPart?.output?.denyReason).toContain("wallet_backend_unavailable");
    } finally {
      fetchMock.restore();
    }
  });
});
