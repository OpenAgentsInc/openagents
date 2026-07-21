import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { Effect, Fiber, Schema as S, Stream } from "effect";
import { afterEach, beforeEach, describe, expect, test } from "vite-plus/test";

import {
  CONTEXT_ENVELOPE_SCHEMA_LITERAL,
  TurnProviderRef,
  TurnRequestRef,
  TurnThreadRef,
  WorkContextEnvelope,
} from "@openagentsinc/agent-runtime-schema";
import { ProviderStreamEvent, type ProviderStartInput } from "@openagentsinc/agent-turn-runtime";

import { makeThreadStore } from "../thread-store.ts";
import {
  HOSTED_KHALA_MAX_MESSAGES,
  HOSTED_KHALA_MAX_MESSAGE_CHARS,
  HOSTED_KHALA_MAX_TOTAL_CHARS,
  HOSTED_KHALA_PROVIDER_REF,
  boundedHostedKhalaMessages,
  makeHostedKhalaDescriptor,
  makeHostedKhalaProviderRegistry,
} from "./desktop-hosted-khala-provider.ts";

const decodeContext = S.decodeUnknownSync(WorkContextEnvelope);
const decodeRequestRef = S.decodeUnknownSync(TurnRequestRef);
const decodeThreadRef = S.decodeUnknownSync(TurnThreadRef);
const decodeProviderRef = S.decodeUnknownSync(TurnProviderRef);

const contextFor = (threadRef: string): WorkContextEnvelope =>
  decodeContext({
    schema: CONTEXT_ENVELOPE_SCHEMA_LITERAL,
    manifestRef: `context.${threadRef}`,
    threadRef,
    generation: { state: "unknown", reason: "not_observed" },
    createdAt: "2026-07-21T08:00:00.000Z",
    items: [],
    totalByteLength: 0,
    byteLimit: 0,
    truncated: false,
    redacted: false,
  });

const startInput = (threadRef: string, message: string): ProviderStartInput => ({
  providerRef: decodeProviderRef(HOSTED_KHALA_PROVIDER_REF),
  requestRef: decodeRequestRef("request.hosted.1"),
  threadRef: decodeThreadRef(threadRef),
  intent: { _tag: "Ask", text: message },
  context: contextFor(threadRef),
});

const sse = (frames: ReadonlyArray<readonly [string, unknown]>): string =>
  frames
    .map(([event, payload]) => `event: ${event}\ndata: ${JSON.stringify(payload)}\n\n`)
    .join("");

/** Scripted fetch: records the request and serves the given body/status. */
const scriptedFetch = (
  body: string,
  status = 200,
): {
  readonly fetchImpl: typeof fetch;
  readonly requests: Array<{ url: string; body: unknown }>;
} => {
  const requests: Array<{ url: string; body: unknown }> = [];
  const fetchImpl: typeof fetch = async (url, init) => {
    requests.push({ url: String(url), body: JSON.parse(String(init?.body)) });
    return new Response(body, { status, headers: { "content-type": "text/event-stream" } });
  };
  return { fetchImpl, requests };
};

const drainEvents = async (
  registry: ReturnType<typeof makeHostedKhalaProviderRegistry>,
  input: ProviderStartInput,
): Promise<ReadonlyArray<ProviderStreamEvent>> =>
  Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const run = yield* registry.start(input);
        return yield* Stream.runCollect(run.events);
      }),
    ),
  );

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(path.join(tmpdir(), "hosted-khala-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

describe("hosted Khala provider descriptor (#9145)", () => {
  test("declares the always-ready hosted lane honestly", () => {
    const descriptor = makeHostedKhalaDescriptor();
    expect(descriptor.candidate).toBe("hosted_khala");
    expect(descriptor.providerRef).toBe(HOSTED_KHALA_PROVIDER_REF);
    expect(descriptor.model).toBe("openagents/khala");
    expect(descriptor.placement).toBe("openagents_managed");
    expect(descriptor.dataDestination).toBe("openagents_managed_remote");
    expect(descriptor.costClass).toBe("managed_metered");
    expect(descriptor.supportsStreaming).toBe(true);
    expect(descriptor.readiness).toEqual({ state: "ready" });
  });

  test("describe is ALWAYS ready — reachability is never a readiness hole", async () => {
    const registry = makeHostedKhalaProviderRegistry({
      fetchImpl: async () => {
        throw new Error("offline");
      },
    });
    const descriptors = await Effect.runPromise(registry.describe);
    expect(descriptors).toHaveLength(1);
    expect(descriptors[0]!.readiness).toEqual({ state: "ready" });
  });
});

describe("hosted Khala SSE turn (#9145)", () => {
  test("delta/reasoning/meta/done stream to a Completed answer with served-model provenance", async () => {
    const { fetchImpl, requests } = scriptedFetch(
      sse([
        ["delta", { text: "Hello " }],
        ["reasoning", { text: "thinking about the greeting" }],
        ["delta", { text: "from the hosted mix." }],
        [
          "meta",
          {
            finishReason: "stop",
            servedModel: "khala-served-model",
            usage: { promptTokens: 3, completionTokens: 5, totalTokens: 8 },
            traceRef: "trace.khala_chat.req_1",
          },
        ],
        ["done", { done: true }],
      ]),
    );
    const store = makeThreadStore(path.join(dir, "threads.json"));
    const thread = store.newThread("Hosted chat");
    store.append(thread.id, { key: "u1", role: "user", text: "Say hello", timestamp: "18:00" });
    const registry = makeHostedKhalaProviderRegistry({
      fetchImpl,
      getThreadStore: () => store,
      nextId: () => "1",
      now: () => 1000,
    });
    const events = await drainEvents(registry, startInput(thread.id, "Say hello"));

    // The wire request carried the bounded thread window ending on the user turn.
    expect(requests).toHaveLength(1);
    expect(requests[0]!.url).toBe("https://openagents.com/api/khala/chat");
    expect(requests[0]!.body).toEqual({ messages: [{ role: "user", content: "Say hello" }] });

    // Streaming honesty: Progress + latest-snapshot Chain events precede the
    // terminal Completed, and the growing assistant entry streamed the text.
    expect(events.some((event) => event._tag === "Progress")).toBe(true);
    const chains = events.filter((event) => event._tag === "Chain");
    expect(chains.length).toBeGreaterThanOrEqual(2);
    const lastChain = chains[chains.length - 1]!;
    if (lastChain._tag !== "Chain") throw new Error("expected chain");
    expect(
      lastChain.entries.some(
        (entry) => entry.role === "assistant" && entry.text === "Hello from the hosted mix.",
      ),
    ).toBe(true);
    expect(
      lastChain.entries.some(
        (entry) => entry.role === "system" && entry.text === "thinking about the greeting",
      ),
    ).toBe(true);

    const terminal = events[events.length - 1]!;
    if (terminal._tag !== "Completed") throw new Error(`expected Completed, got ${terminal._tag}`);
    expect(terminal.candidate.kind).toBe("answer");
    if (terminal.candidate.kind !== "answer") throw new Error("expected answer");
    expect(terminal.candidate.text).toBe("Hello from the hosted mix.");
    expect(terminal.candidate.provenance.candidate).toBe("hosted_khala");
    expect(terminal.candidate.provenance.model).toBe("khala-served-model");
    expect(terminal.candidate.provenance.usageTruth).toBe("exact");
    expect(terminal.candidate.provenance.dataDestination).toBe("openagents_managed_remote");
  });

  test("a turn without a meta frame keeps the declared model and downgrades usage truth honestly", async () => {
    const { fetchImpl } = scriptedFetch(
      sse([
        ["delta", { text: "Answer." }],
        ["done", { done: true }],
      ]),
    );
    const registry = makeHostedKhalaProviderRegistry({ fetchImpl, nextId: () => "1" });
    const events = await drainEvents(registry, startInput("thread-meta-less", "Question?"));
    const terminal = events[events.length - 1]!;
    if (terminal._tag !== "Completed" || terminal.candidate.kind !== "answer") {
      throw new Error("expected Completed answer");
    }
    expect(terminal.candidate.provenance.model).toBe("openagents/khala");
    expect(terminal.candidate.provenance.usageTruth).toBe("unknown");
  });

  test("a terminal error frame becomes a typed Failed with bounded safe detail", async () => {
    const { fetchImpl } = scriptedFetch(
      sse([
        ["delta", { text: "partial" }],
        [
          "error",
          { code: "stream_failed", reason: "upstream reset", traceRef: "trace.khala_chat.req_2" },
        ],
      ]),
    );
    const registry = makeHostedKhalaProviderRegistry({ fetchImpl });
    const events = await drainEvents(registry, startInput("thread-error", "Question?"));
    const terminal = events[events.length - 1]!;
    if (terminal._tag !== "Failed") throw new Error(`expected Failed, got ${terminal._tag}`);
    expect(terminal.detail).toContain("stream_failed");
    expect(terminal.detail).toContain("upstream reset");
    expect(terminal.detail.length).toBeLessThanOrEqual(240);
  });

  test("a pre-stream 429 becomes a typed rate-limited Failed, never a start error", async () => {
    const { fetchImpl } = scriptedFetch(JSON.stringify({ error: "rate_limited" }), 429);
    const registry = makeHostedKhalaProviderRegistry({ fetchImpl });
    const events = await drainEvents(registry, startInput("thread-429", "Question?"));
    expect(events).toHaveLength(1);
    const terminal = events[0]!;
    if (terminal._tag !== "Failed") throw new Error(`expected Failed, got ${terminal._tag}`);
    expect(terminal.detail).toContain("rate_limited");
  });

  test("a network failure is a typed Failed with no body/token leakage", async () => {
    const registry = makeHostedKhalaProviderRegistry({
      fetchImpl: async () => {
        throw new Error("getaddrinfo ENOTFOUND openagents.com");
      },
    });
    const events = await drainEvents(registry, startInput("thread-net", "Question?"));
    expect(events).toHaveLength(1);
    const terminal = events[0]!;
    if (terminal._tag !== "Failed") throw new Error(`expected Failed, got ${terminal._tag}`);
    expect(terminal.detail).toContain("hosted khala unreachable");
  });

  test("a stream that ends without done/error is an honest stream_interrupted Failed", async () => {
    const { fetchImpl } = scriptedFetch(sse([["delta", { text: "half an ans" }]]));
    const registry = makeHostedKhalaProviderRegistry({ fetchImpl });
    const events = await drainEvents(registry, startInput("thread-cut", "Question?"));
    const terminal = events[events.length - 1]!;
    if (terminal._tag !== "Failed") throw new Error(`expected Failed, got ${terminal._tag}`);
    expect(terminal.detail).toContain("stream_interrupted");
  });

  test("cancellation aborts the in-flight fetch (scope teardown propagates)", async () => {
    let observed: AbortSignal | null = null;
    let sawRequest: () => void = () => {};
    const requested = new Promise<void>((resolve) => {
      sawRequest = resolve;
    });
    const fetchImpl: typeof fetch = (_url, init) => {
      observed = init?.signal ?? null;
      sawRequest();
      // Never resolves on its own; rejects only when aborted.
      return new Promise((_resolve, reject) => {
        init?.signal?.addEventListener("abort", () => reject(new Error("aborted")));
      });
    };
    const registry = makeHostedKhalaProviderRegistry({ fetchImpl });
    const fiber = Effect.runFork(
      Effect.scoped(
        Effect.gen(function* () {
          const run = yield* registry.start(startInput("thread-abort", "Question?"));
          yield* Stream.runDrain(run.events);
        }),
      ),
    );
    await requested;
    await Effect.runPromise(Fiber.interrupt(fiber));
    expect(observed).not.toBeNull();
    expect(observed!.aborted).toBe(true);
  });

  test("an empty intent prompt refuses to start (typed ProviderStartError)", async () => {
    const registry = makeHostedKhalaProviderRegistry({ fetchImpl: scriptedFetch("").fetchImpl });
    const outcome = await Effect.runPromise(
      Effect.scoped(registry.start(startInput("thread-empty", "   "))).pipe(
        Effect.map(() => "started" as const),
        Effect.catch((error) => Effect.succeed(error.reason)),
      ),
    );
    expect(outcome).toBe("unavailable");
  });
});

describe("hosted Khala message-window bounds (#9145)", () => {
  test("keeps the newest messages when the thread exceeds the 40-message bound", () => {
    const window = Array.from({ length: 90 }, (_, index) => ({
      role: index % 2 === 0 ? "user" : "assistant",
      text: `turn ${index}`,
    }));
    // Ends on assistant (index 89) → the fallback user prompt closes the window.
    const messages = boundedHostedKhalaMessages(window, "current question");
    expect(messages.length).toBeLessThanOrEqual(HOSTED_KHALA_MAX_MESSAGES);
    expect(messages[messages.length - 1]).toEqual({ role: "user", content: "current question" });
    // Oldest turns are the ones dropped.
    expect(messages.some((message) => message.content === "turn 0")).toBe(false);
    expect(messages.some((message) => message.content === "turn 88")).toBe(true);
  });

  test("slices an oversized message and truncates OLDEST first for the total budget", () => {
    const big = "x".repeat(HOSTED_KHALA_MAX_MESSAGE_CHARS + 500);
    const filler = "y".repeat(7_000);
    const window = [
      { role: "user", text: big },
      { role: "assistant", text: filler },
      { role: "user", text: filler },
      { role: "assistant", text: filler },
      { role: "user", text: "the newest question" },
    ];
    const messages = boundedHostedKhalaMessages(window, "the newest question");
    const total = messages.reduce((sum, message) => sum + message.content.length, 0);
    expect(total).toBeLessThanOrEqual(HOSTED_KHALA_MAX_TOTAL_CHARS);
    expect(
      messages.every((message) => message.content.length <= HOSTED_KHALA_MAX_MESSAGE_CHARS),
    ).toBe(true);
    // The newest user message ALWAYS survives; the oldest (oversized) one is gone.
    expect(messages[messages.length - 1]).toEqual({ role: "user", content: "the newest question" });
    expect(messages.some((message) => message.content.startsWith("xxx"))).toBe(false);
  });

  test("system notes and empty notes never cross the wire", () => {
    const messages = boundedHostedKhalaMessages(
      [
        { role: "system", text: "internal system note" },
        { role: "user", text: "   " },
        { role: "user", text: "real question" },
      ],
      "real question",
    );
    expect(messages).toEqual([{ role: "user", content: "real question" }]);
  });
});
