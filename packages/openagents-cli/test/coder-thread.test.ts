import { afterEach, describe, expect, it, vi } from "vitest";

import type { ReplyChunk } from "../src/coder-session.js";
import { openThread, ThreadUnavailable, type ThreadReplySource } from "../src/coder-thread.js";

const ORIGIN = "https://openagents.test";
const ACCOUNT_TOKEN = "oa_pat_account";
const GRANT_TOKEN = "oa_grant_secret";
const THREAD_ID = "9bb19447-ecf4-4f1b-b44e-6b128664da9c";

const json = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

const CREATED = {
  thread: { id: THREAD_ID, status: "open" },
  grant: {
    token: GRANT_TOKEN,
    url: `${ORIGIN}/api/inference/proxy`,
    model: "gpt-5.6-luna",
    expires_at: "2026-08-23T23:59:59Z",
    limits: { max_calls: 256, max_total_tokens: 1_000_000, max_cost_microusd: 2_000_000 },
  },
};

/**
 * The body the running server returns, byte for byte, for a one-sentence
 * prompt. Every assertion about the translation is against this shape rather
 * than against one invented for the test.
 */
const LIVE_SSE = [
  `data: {"choices":[{"delta":{"content":"Hello"},"index":0}]}`,
  `data: {"choices":[{"delta":{"content":"!"},"index":0}]}`,
  `data: {"choices":[{"delta":{"content":" Nice"},"index":0}]}`,
  `data: {"choices":[{"delta":{},"finish_reason":"stop","index":0}]}`,
  `data: {"choices":[],"usage":{"completion_tokens":11,"prompt_tokens":12,"total_tokens":23}}`,
  `data: [DONE]`,
  "",
].join("\n\n");

/** An SSE response whose body arrives in the given pieces, in order. */
const sse = (pieces: ReadonlyArray<string>) =>
  new Response(
    new ReadableStream<Uint8Array>({
      start(controller) {
        const encoder = new TextEncoder();
        for (const piece of pieces) controller.enqueue(encoder.encode(piece));
        controller.close();
      },
    }),
    { status: 200, headers: { "content-type": "text/event-stream" } },
  );

interface Call {
  readonly method: string;
  readonly url: string;
  readonly authorization: string;
  readonly body: Record<string, unknown>;
}

/**
 * Answer the three routes a session uses: open, spend, read.
 *
 * `proxy` is consumed one response per call, so a test can give one turn a
 * body and the next a refusal.
 */
const stub = (options: {
  readonly create?: Response;
  readonly proxy?: ReadonlyArray<Response>;
  readonly show?: Response;
  readonly remove?: Response;
}) => {
  const calls: Call[] = [];
  const proxy = [...(options.proxy ?? [])];

  vi.stubGlobal(
    "fetch",
    vi.fn(async (target: URL | string, init?: RequestInit) => {
      const url = typeof target === "string" ? target : target.toString();
      const headers = (init?.headers ?? {}) as Record<string, string>;
      const raw = typeof init?.body === "string" ? init.body : "{}";
      calls.push({
        method: init?.method ?? "GET",
        url,
        authorization: headers["authorization"] ?? "",
        body: JSON.parse(raw) as Record<string, unknown>,
      });

      if (url.endsWith("/api/inference/proxy")) return proxy.shift() ?? sse([LIVE_SSE]);
      if ((init?.method ?? "GET") === "POST") return (options.create ?? json(201, CREATED)).clone();
      if ((init?.method ?? "GET") === "DELETE") return (options.remove ?? json(200, {})).clone();
      return (options.show ?? json(200, { thread: {}, grant: {} })).clone();
    }),
  );

  return calls;
};

const open = () =>
  openThread({ origin: ORIGIN, token: ACCOUNT_TOKEN, objective: "coder in repo on main" });

const chunks = async (source: ThreadReplySource, prompt = "hello") => {
  const out: ReplyChunk[] = [];
  for await (const chunk of source.reply(prompt, new AbortController().signal)) out.push(chunk);
  return out;
};

const textOf = (out: ReadonlyArray<ReplyChunk>) =>
  out.map((chunk) => (chunk.type === "text" ? chunk.value : "")).join("");

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("openThread", () => {
  it("opens a thread with an objective and reports the model its grant pins", async () => {
    const calls = stub({});
    const source = await openThread({
      origin: ORIGIN,
      token: ACCOUNT_TOKEN,
      objective: "coder in repo on main",
      reasoning: "high",
    });

    expect(source.threadId).toBe(THREAD_ID);
    expect(source.model).toBe("gpt-5.6-luna");
    expect(calls[0]?.method).toBe("POST");
    expect(calls[0]?.url).toBe(`${ORIGIN}/api/v3/threads`);
    expect(calls[0]?.authorization).toBe(`Bearer ${ACCOUNT_TOKEN}`);
    expect(calls[0]?.body).toEqual({ objective: "coder in repo on main", reasoning: "high" });
  });

  it("names the model the thread's grant should pin, so children can run on another", async () => {
    const calls = stub({});
    await openThread({
      origin: ORIGIN,
      token: ACCOUNT_TOKEN,
      objective: "delegated children",
      model: "ox-alpha",
    });

    expect(calls[0]?.body).toEqual({ objective: "delegated children", model: "ox-alpha" });
  });

  it("lends children the model their own thread pinned, not the conversation's", async () => {
    stub({
      create: json(201, {
        ...CREATED,
        grant: { ...CREATED.grant, model: "ox-alpha" },
      }),
    });

    const source = await openThread({
      origin: ORIGIN,
      token: ACCOUNT_TOKEN,
      objective: "delegated children",
      model: "ox-alpha",
    });

    expect(source.childGrant.model).toBe("ox-alpha");
  });

  it("starts with the ceilings the grant was minted with", async () => {
    stub({});
    expect((await open()).budget).toBe("256 calls · 1.0M tok · $2.00");
  });

  it("carries the server's typed refusal when the account holds its last thread", async () => {
    const sentence =
      "This account holds 8 open threads and the configured maximum is 8. " +
      "Revoke a thread with DELETE /api/v3/threads/{thread_id} before opening another.";
    stub({
      create: json(429, {
        message: sentence,
        code: "thread_quota_reached",
        status: 429,
        errors: { threads: [sentence] },
      }),
    });

    await expect(open()).rejects.toMatchObject({
      code: "thread_quota_reached",
      status: 429,
      message: sentence,
    });
  });

  it("names the scope a token is missing rather than the status it returned", async () => {
    stub({ create: json(403, { code: "forbidden" }) });
    await expect(open()).rejects.toMatchObject({ code: "scope_missing" });
  });
});

describe("ThreadReplySource", () => {
  it("translates the proxy's content deltas into text chunks", async () => {
    stub({});
    const out = await chunks(await open());

    expect(out.every((chunk) => chunk.type === "text")).toBe(true);
    expect(textOf(out)).toBe("Hello! Nice");
  });

  it("parses a body split across reads in the middle of a frame", async () => {
    // The proxy sends the whole body at once today. This is the same body cut
    // where a chunked sender would cut it, so stage 4 needs no change here.
    const at = LIVE_SSE.indexOf("Nice") + 2;
    stub({ proxy: [sse([LIVE_SSE.slice(0, at), LIVE_SSE.slice(at)])] });

    expect(textOf(await chunks(await open()))).toBe("Hello! Nice");
  });

  it("spends the grant at the proxy and never sends the account token there", async () => {
    const calls = stub({});
    await chunks(await open());

    const spend = calls.find((call) => call.url.endsWith("/api/inference/proxy"));
    expect(spend?.authorization).toBe(`Bearer ${GRANT_TOKEN}`);
    expect(calls.filter((call) => call.authorization.includes(ACCOUNT_TOKEN))).toHaveLength(2);
  });

  it("answers the next turn against the thread's own transcript", async () => {
    const calls = stub({ proxy: [sse([LIVE_SSE]), sse([LIVE_SSE])] });
    const source = await open();
    await chunks(source, "first");
    await chunks(source, "second");

    const spends = calls.filter((call) => call.url.endsWith("/api/inference/proxy"));
    expect(spends[1]?.body["messages"]).toEqual([
      { role: "user", content: "first" },
      { role: "assistant", content: "Hello! Nice" },
      { role: "user", content: "second" },
    ]);
  });

  it("takes the turn's usage off the budget and then reads the server's figure", async () => {
    stub({
      show: json(200, {
        thread: { id: THREAD_ID },
        grant: {
          limits: { max_calls: 256, max_total_tokens: 1_000_000, max_cost_microusd: 2_000_000 },
          remaining: { calls: 255, total_tokens: 999_977, cost_microusd: 1_999_100 },
        },
      }),
    });

    const source = await open();
    await chunks(source);
    expect(source.budget).toBe("255 calls · 1.0M tok · $2.00");
  });

  it("assembles tool call fragments identified by their wire index", async () => {
    stub({
      proxy: [
        sse([
          [
            `data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"repo_grep","arguments":"{\\"pattern\\":"}}]}}]}`,
            `data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\\"thread\\"}"}}]}}]}`,
            `data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}`,
            `data: [DONE]`,
            "",
          ].join("\n\n"),
        ]),
      ],
    });

    // A call the session cannot run is still reported, and the turn continues
    // with the refusal on the thread, because the alternative is a turn that
    // ends on a tool row and never answers.
    expect(await chunks(await open())).toEqual([
      {
        type: "tool_call",
        callId: "call-1",
        name: "repo_grep",
        arguments: `{"pattern":"thread"}`,
      },
      {
        type: "tool_result",
        callId: "call-1",
        output: undefined,
        error: "This session has no `repo_grep` tool.",
      },
      { type: "text", value: "Hello" },
      { type: "text", value: "!" },
      { type: "text", value: " Nice" },
    ]);
  });

  it("runs a declared tool and answers from its result", async () => {
    const calls = stub({
      proxy: [
        sse([
          [
            `data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"delegate","arguments":"{\\"prompt\\":\\"add tests\\"}"}}]}}]}`,
            `data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}`,
            `data: [DONE]`,
            "",
          ].join("\n\n"),
        ]),
        sse([
          [
            `data: {"choices":[{"delta":{"content":"Two children finished."},"index":0}]}`,
            `data: [DONE]`,
            "",
          ].join("\n\n"),
        ]),
      ],
    });

    const source = await open();
    const seen: Array<Record<string, unknown>> = [];
    source.useTools([
      {
        name: "delegate",
        description: "run children",
        parameters: { type: "object" },
        run: async (args) => {
          seen.push(args);
          return "2 of 2 children completed.";
        },
      },
    ]);

    const out = await chunks(source);

    expect(seen).toEqual([{ prompt: "add tests" }]);
    expect(out).toContainEqual({
      type: "tool_result",
      callId: "call-1",
      output: "2 of 2 children completed.",
      error: undefined,
    });
    expect(textOf(out)).toBe("Two children finished.");

    // The tool declaration reaches the model, and the exchange is on the
    // thread, or the next turn cannot see what its own call returned.
    const proxied = calls.filter((call) => call.url.includes("/inference/proxy"));
    const first = proxied[0];
    expect(first?.body["tools"]).toEqual([
      {
        type: "function",
        function: { name: "delegate", description: "run children", parameters: { type: "object" } },
      },
    ]);
    const second = proxied[1];
    expect(second?.body["messages"]).toEqual([
      { role: "user", content: "hello" },
      { role: "assistant", content: `[tool call]\ndelegate({"prompt":"add tests"})` },
      { role: "user", content: "[tool result delegate]\n2 of 2 children completed." },
    ]);
  });

  it("lends the grant to children without handing over the token", async () => {
    stub({});
    const grant = (await open()).childGrant;
    expect(grant.model).toBe("gpt-5.6-luna");
    expect(grant.proxyUrl).toBe(`${ORIGIN}/api/inference/proxy`);
    // Redacted, so an interpolation into a config or a command line cannot
    // print it.
    expect(String(grant.token)).not.toContain(GRANT_TOKEN);
  });

  it("reports a revoked grant with a sentence rather than a status", async () => {
    stub({ proxy: [json(403, { error: { code: "grant_revoked" } })] });
    const failure = await chunks(await open()).catch((cause: unknown) => cause);

    expect(failure).toBeInstanceOf(ThreadUnavailable);
    expect(failure).toMatchObject({
      code: "grant_revoked",
      message: "This thread was revoked. Start a new session to open another.",
    });
  });

  it("stops without throwing when the turn is interrupted", async () => {
    stub({});
    const source = await open();
    const controller = new AbortController();
    controller.abort();

    const out: ReplyChunk[] = [];
    for await (const chunk of source.reply("hello", controller.signal)) out.push(chunk);
    expect(out).toEqual([]);
  });

  it("revokes the thread with the account token", async () => {
    const calls = stub({});
    await (await open()).revoke();

    const removal = calls.find((call) => call.method === "DELETE");
    expect(removal?.url).toBe(`${ORIGIN}/api/v3/threads/${THREAD_ID}`);
    expect(removal?.authorization).toBe(`Bearer ${ACCOUNT_TOKEN}`);
  });

  it("leaves the session usable when revoking cannot reach the server", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (target: URL | string, init?: RequestInit) => {
        if ((init?.method ?? "GET") === "DELETE") throw new Error("socket closed");
        return json(201, CREATED);
      }),
    );

    await expect((await open()).revoke()).resolves.toBeUndefined();
  });
});
