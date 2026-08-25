import { afterEach, describe, expect, it, vi } from "vitest";

import type { ReplyChunk } from "../src/coder-session.js";
import { openThread, resolveProxyUrl, ThreadUnavailable, type ThreadReplySource } from "../src/coder-thread.js";
import { ThreadTranscriptWriter } from "../src/coder-transcript.js";

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

/**
 * The conversation as the model receives it, without the session's anchor.
 *
 * Every session now opens with a system message naming what it is and what
 * tools it has. These assertions are about the shape of the conversation that
 * follows it, so they read past it rather than restating it; `anchorOf` is
 * where the anchor itself is checked.
 */
const conversation = (body: unknown) =>
  (body as Array<Record<string, unknown>>).filter((message) => message["role"] !== "system");

const anchorOf = (body: unknown) =>
  (body as Array<Record<string, unknown>>).find((message) => message["role"] === "system");

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

  it("sends the repository beside the objective, so resume can filter structurally", async () => {
    const calls = stub({});
    await openThread({
      origin: ORIGIN,
      token: ACCOUNT_TOKEN,
      objective: "openagents coder in OpenAgentsInc/openagents.com on main",
      repository: "OpenAgentsInc/openagents.com",
    });

    expect(calls[0]?.body).toEqual({
      objective: "openagents coder in OpenAgentsInc/openagents.com on main",
      repository: "OpenAgentsInc/openagents.com",
    });
  });

  it("omits the repository key entirely when none is named", async () => {
    const calls = stub({});
    await open();

    expect(calls[0]?.body).toEqual({ objective: "coder in repo on main" });
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
    expect(conversation(spends[1]?.body["messages"])).toEqual([
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
    expect(conversation(second?.body["messages"])).toEqual([
      { role: "user", content: "hello" },
      {
        role: "assistant",
        content: "",
        tool_calls: [
          {
            id: "call-1",
            type: "function",
            function: { name: "delegate", arguments: `{"prompt":"add tests"}` },
          },
        ],
      },
      { role: "tool", tool_call_id: "call-1", content: "2 of 2 children completed." },
    ]);
  });

  it("translates reasoning deltas into reasoning chunks, in stream order", async () => {
    stub({
      proxy: [
        sse([
          [
            `data: {"choices":[{"delta":{"reasoning":"Let me think."},"index":0}]}`,
            `data: {"choices":[{"delta":{"reasoning":" Two files."},"index":0}]}`,
            `data: {"choices":[{"delta":{"content":"Answer."},"index":0}]}`,
            `data: [DONE]`,
            "",
          ].join("\n\n"),
        ]),
      ],
    });

    expect(await chunks(await open())).toEqual([
      { type: "reasoning", value: "Let me think." },
      { type: "reasoning", value: " Two files." },
      { type: "text", value: "Answer." },
    ]);
  });

  it("keeps reasoning off the wire transcript: the next turn replays only what was said", async () => {
    const calls = stub({
      proxy: [
        sse([
          [
            `data: {"choices":[{"delta":{"reasoning":"Thinking."},"index":0}]}`,
            `data: {"choices":[{"delta":{"content":"Answer."},"index":0}]}`,
            `data: [DONE]`,
            "",
          ].join("\n\n"),
        ]),
        sse([LIVE_SSE]),
      ],
    });

    const source = await open();
    await chunks(source, "first");
    await chunks(source, "second");

    const spends = calls.filter((call) => call.url.endsWith("/api/inference/proxy"));
    expect(conversation(spends[1]?.body["messages"])).toEqual([
      { role: "user", content: "first" },
      { role: "assistant", content: "Answer." },
      { role: "user", content: "second" },
    ]);
  });

  it("replays two calls of one round in call order with their raw arguments", async () => {
    // The second call's arguments carry spacing the model chose. A parse and
    // re-serialize would normalize it; the wire must not.
    const rawFirst = `{"command":  "ls -la"}`;
    const rawSecond = `{"pattern": "thread",  "limit": 2}`;
    const round = [
      `data: ${JSON.stringify({
        choices: [
          {
            index: 0,
            delta: {
              tool_calls: [
                {
                  index: 0,
                  id: "call-a",
                  type: "function",
                  function: { name: "shell", arguments: rawFirst },
                },
                {
                  index: 1,
                  id: "call-b",
                  type: "function",
                  function: { name: "grep", arguments: rawSecond },
                },
              ],
            },
          },
        ],
      })}`,
      `data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}`,
      `data: [DONE]`,
      "",
    ].join("\n\n");

    const calls = stub({ proxy: [sse([round]), sse([LIVE_SSE])] });
    const source = await open();

    // The first tool finishes last, so completion order is the reverse of
    // call order, and the transcript must not care.
    let releaseFirst: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });
    source.useTools([
      {
        name: "shell",
        description: "run a command",
        parameters: { type: "object" },
        run: async () => {
          await gate;
          return "shell output";
        },
      },
      {
        name: "grep",
        description: "search",
        parameters: { type: "object" },
        run: async () => {
          releaseFirst();
          return "grep output";
        },
      },
    ]);

    await chunks(source, "run both");

    const spends = calls.filter((call) => call.url.endsWith("/api/inference/proxy"));
    expect(conversation(spends[1]?.body["messages"])).toEqual([
      { role: "user", content: "run both" },
      {
        role: "assistant",
        content: "",
        tool_calls: [
          { id: "call-a", type: "function", function: { name: "shell", arguments: rawFirst } },
          { id: "call-b", type: "function", function: { name: "grep", arguments: rawSecond } },
        ],
      },
      { role: "tool", tool_call_id: "call-a", content: "shell output" },
      { role: "tool", tool_call_id: "call-b", content: "grep output" },
    ]);
  });

  it("carries the words an assistant said alongside the calls it made", async () => {
    const round = [
      `data: {"choices":[{"delta":{"content":"Looking now."},"index":0}]}`,
      `data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"shell","arguments":"{}"}}]}}]}`,
      `data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}`,
      `data: [DONE]`,
      "",
    ].join("\n\n");
    const calls = stub({ proxy: [sse([round]), sse([LIVE_SSE])] });
    const source = await open();
    source.useTools([withTool(async () => "done")]);

    await chunks(source, "look");

    const spends = calls.filter((call) => call.url.endsWith("/api/inference/proxy"));
    const messages = conversation(spends[1]?.body["messages"]);
    expect(messages[1]).toMatchObject({ role: "assistant", content: "Looking now." });
    expect(messages[1]?.["tool_calls"]).toHaveLength(1);
  });

  it("bounds a tool result on the wire while the record keeps the fuller copy", async () => {
    const round = [
      `data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"shell","arguments":"{}"}}]}}]}`,
      `data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}`,
      `data: [DONE]`,
      "",
    ].join("\n\n");
    const calls = stub({ proxy: [sse([round]), sse([LIVE_SSE])] });
    const source = await open();
    const sink = recorder();
    source.useTranscript(sink);
    source.useTools([withTool(async () => "x".repeat(10_000))]);

    await chunks(source, "dump it");

    const spends = calls.filter((call) => call.url.endsWith("/api/inference/proxy"));
    const messages = spends[1]?.body["messages"] as Array<Record<string, unknown>>;
    const result = messages.find((message) => message["role"] === "tool");
    const wire = result?.["content"] as string;
    // The 4,000-char context budget still holds on the way to the model...
    expect(wire.length).toBeLessThan(4_200);
    expect(wire).toContain("characters omitted");
    // ...while the durable event keeps the result whole, as before.
    const ran = sink.events.find((event) => event.eventType === "tool.ran");
    expect(ran?.payload["output"]).toBe("x".repeat(10_000));
  });

  it("drops the calls past the step limit and asks for an answer without tools", async () => {
    const round = [
      `data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"shell","arguments":"{}"}}]}}]}`,
      `data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}`,
      `data: [DONE]`,
      "",
    ].join("\n\n");
    // Steps 0..100 each ask for a tool; step 100 is past the limit, so its
    // call is dropped and step 101 must answer in words.
    const calls = stub({
      proxy: [...Array.from({ length: 101 }, () => sse([round])), sse([LIVE_SSE])],
    });
    const source = await open();
    source.useTools([withTool(async () => "ok")]);

    expect(textOf(await chunks(source, "loop"))).toBe("Hello! Nice");

    const spends = calls.filter((call) => call.url.endsWith("/api/inference/proxy"));
    expect(spends).toHaveLength(102);
    const last = spends[101];
    // The tools are withheld for the answering round.
    expect(last?.body["tools"]).toBeUndefined();
    const messages = last?.body["messages"] as Array<Record<string, unknown>>;
    // The dropped round left no orphan: every assistant `tool_calls` message
    // is answered by a `tool` message, and the nudge closes the transcript.
    const asked = messages.filter((message) => Array.isArray(message["tool_calls"])).length;
    const answered = messages.filter((message) => message["role"] === "tool").length;
    expect(asked).toBe(100);
    expect(answered).toBe(100);
    expect(messages[messages.length - 1]?.["content"]).toContain(
      "You have reached this turn's limit on tool calls.",
    );
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

/** A sink that just remembers, standing in for the writer. */
const recorder = () => {
  const events: Array<{ eventType: string; payload: Record<string, unknown> }> = [];
  return {
    events,
    record(eventType: string, payload: Record<string, unknown>) {
      events.push({ eventType, payload });
    },
  };
};

const withTool = (run: (args: Record<string, unknown>) => Promise<string>) => ({
  name: "shell",
  description: "run a command",
  parameters: { type: "object" },
  run,
});

describe("the thread's durable transcript", () => {
  const TOOL_ROUND = [
    `data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"shell","arguments":"{\\"command\\":\\"ls\\"}"}}]}}]}`,
    `data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}`,
    `data: {"choices":[],"usage":{"completion_tokens":5,"prompt_tokens":40,"total_tokens":45}}`,
    `data: [DONE]`,
    "",
  ].join("\n\n");

  const ANSWER_ROUND = [
    `data: {"choices":[{"delta":{"content":"Two files."},"index":0}]}`,
    `data: {"choices":[],"usage":{"completion_tokens":11,"prompt_tokens":60,"total_tokens":71}}`,
    `data: [DONE]`,
    "",
  ].join("\n\n");

  it("records the turn in order: what was asked, each tool run, the answer", async () => {
    stub({ proxy: [sse([TOOL_ROUND]), sse([ANSWER_ROUND])] });
    const source = await open();
    const sink = recorder();
    source.useTranscript(sink);
    source.useTools([withTool(async () => "README.md\nsrc")]);

    await chunks(source, "what is in this repo?");

    expect(sink.events.map((event) => event.eventType)).toEqual([
      "turn.user",
      "tool.ran",
      "turn.assistant",
    ]);
    expect(sink.events[0]?.payload).toEqual({ text: "what is in this repo?" });
    expect(sink.events[1]?.payload).toEqual({
      call_id: "call-1",
      tool: "shell",
      arguments: `{"command":"ls"}`,
      status: "succeeded",
      output: "README.md\nsrc",
    });
  });

  it("records the answer with the turn's summed usage and its call count", async () => {
    stub({ proxy: [sse([TOOL_ROUND]), sse([ANSWER_ROUND])] });
    const source = await open();
    const sink = recorder();
    source.useTranscript(sink);
    source.useTools([withTool(async () => "README.md\nsrc")]);

    await chunks(source, "what is in this repo?");

    const answer = sink.events.find((event) => event.eventType === "turn.assistant");
    // The turn took two model calls; the record holds their sum with the
    // count, not the last call's figures presented as the turn's.
    expect(answer?.payload).toEqual({
      text: "Two files.",
      usage: { prompt_tokens: 100, completion_tokens: 16, total_tokens: 116, calls: 2 },
      tool_calls: 1,
    });
  });

  it("yields the turn's usage so the session's export can carry it", async () => {
    stub({ proxy: [sse([TOOL_ROUND]), sse([ANSWER_ROUND])] });
    const source = await open();
    source.useTools([withTool(async () => "README.md\nsrc")]);

    const received = await chunks(source, "what is in this repo?");

    // One usage chunk, at the end of the turn, summing both model calls —
    // the same report the local lane yields, so the ATIF export's step
    // metrics and final totals hold on every lane.
    const usage = received.filter((chunk) => chunk.type === "usage");
    expect(usage).toEqual([
      { type: "usage", promptTokens: 100, completionTokens: 16, calls: 2 },
    ]);
  });

  it("records a failed tool as one event carrying its error", async () => {
    stub({ proxy: [sse([TOOL_ROUND]), sse([ANSWER_ROUND])] });
    const source = await open();
    const sink = recorder();
    source.useTranscript(sink);
    source.useTools([
      withTool(async () => {
        throw new Error("permission denied");
      }),
    ]);

    await chunks(source, "try it");

    const ran = sink.events.find((event) => event.eventType === "tool.ran");
    expect(ran?.payload).toEqual({
      call_id: "call-1",
      tool: "shell",
      arguments: `{"command":"ls"}`,
      status: "failed",
      error: "permission denied",
    });
  });

  it("bounds a tool result on its way into the record", async () => {
    stub({ proxy: [sse([TOOL_ROUND]), sse([ANSWER_ROUND])] });
    const source = await open();
    const sink = recorder();
    source.useTranscript(sink);
    source.useTools([withTool(async () => "x".repeat(200_000))]);

    await chunks(source, "dump it");

    const ran = sink.events.find((event) => event.eventType === "tool.ran");
    const output = ran?.payload["output"] as string;
    // Kept at both ends around a marker, and far above the model-wire bound,
    // so every result a real session has produced is stored whole.
    expect(output.length).toBeLessThan(70_000);
    expect(output).toContain("characters omitted");
  });

  it("records a message steered into the running turn as the reader's", async () => {
    stub({ proxy: [sse([TOOL_ROUND]), sse([ANSWER_ROUND])] });
    const source = await open();
    const sink = recorder();
    source.useTranscript(sink);
    source.useTools([
      withTool(async () => {
        source.steer("only the top level");
        return "README.md\nsrc";
      }),
    ]);

    await chunks(source, "list the files");

    expect(sink.events.map((event) => event.eventType)).toEqual([
      "turn.user",
      "tool.ran",
      "turn.user",
      "turn.assistant",
    ]);
    expect(sink.events[2]?.payload).toEqual({ text: "only the top level", steered: true });
  });

  it("records the turn's reasoning whole, one event per block", async () => {
    stub({
      proxy: [
        sse([
          [
            `data: {"choices":[{"delta":{"reasoning":"Let me think."},"index":0}]}`,
            `data: {"choices":[{"delta":{"reasoning":" Two files."},"index":0}]}`,
            `data: {"choices":[{"delta":{"content":"Two files."},"index":0}]}`,
            `data: [DONE]`,
            "",
          ].join("\n\n"),
        ]),
      ],
    });
    const source = await open();
    const sink = recorder();
    source.useTranscript(sink);

    await chunks(source, "what is in this repo?");

    expect(sink.events.map((event) => event.eventType)).toEqual([
      "turn.user",
      "turn.reasoning",
      "turn.assistant",
    ]);
    // The deltas are how the thinking arrived, not what it is: one event
    // carries the block whole.
    expect(sink.events[1]?.payload).toEqual({ text: "Let me think. Two files." });
  });

  it("records nothing extra for a turn without tools", async () => {
    stub({});
    const source = await open();
    const sink = recorder();
    source.useTranscript(sink);

    await chunks(source, "hello");

    expect(sink.events.map((event) => event.eventType)).toEqual(["turn.user", "turn.assistant"]);
    expect(sink.events[1]?.payload).toMatchObject({ text: "Hello! Nice", tool_calls: 0 });
  });

  it("still answers when every transcript post fails", async () => {
    // The real writer against a server that refuses the events route: the
    // turn loop must neither throw nor wait on it.
    vi.stubGlobal(
      "fetch",
      vi.fn(async (target: URL | string, init?: RequestInit) => {
        const url = typeof target === "string" ? target : target.toString();
        if (url.endsWith("/events")) throw new Error("socket closed");
        if (url.endsWith("/api/inference/proxy")) return sse([LIVE_SSE]);
        if ((init?.method ?? "GET") === "POST") return json(201, CREATED);
        return json(200, { thread: {}, grant: {} });
      }),
    );

    const source = await openThread({
      origin: ORIGIN,
      token: ACCOUNT_TOKEN,
      objective: "coder in repo on main",
    });
    const notices: string[] = [];
    const writer = new ThreadTranscriptWriter({
      origin: ORIGIN,
      threadId: source.threadId,
      token: ACCOUNT_TOKEN,
      retryDelaysMs: [0, 0, 10],
      onTrouble: (message) => {
        notices.push(message);
      },
    });
    source.useTranscript(writer);

    expect(textOf(await chunks(source))).toBe("Hello! Nice");
    await writer.close(100);
    expect(notices).toHaveLength(1);
  });
});

describe("a provider failure mid-session", () => {
  const proxyCalls = (calls: ReadonlyArray<Call>) =>
    calls.filter((call) => call.url.endsWith("/api/inference/proxy"));

  it("carries the server's failure class into the sentence a reader sees", async () => {
    const calls = stub({
      proxy: [
        json(502, { error: { code: "provider_failed", reason: "context_length_exceeded" } }),
        json(502, { error: { code: "provider_failed", reason: "context_length_exceeded" } }),
        json(502, { error: { code: "provider_failed", reason: "context_length_exceeded" } }),
      ],
    });
    const source = await open();

    await expect(chunks(source)).rejects.toThrow(/context_length_exceeded/);
    // Non-vacuous: the generic sentence is still there, with the class beside it.
    expect(proxyCalls(calls)).toHaveLength(3);
  });

  it("retries a provider failure rather than losing the turn to one hiccup", async () => {
    const calls = stub({
      proxy: [json(502, { error: { code: "provider_failed" } }), sse([LIVE_SSE])],
    });
    const source = await open();

    expect(textOf(await chunks(source))).toBe("Hello! Nice");
    expect(proxyCalls(calls)).toHaveLength(2);
  });

  it("gives up after a bounded number of attempts", async () => {
    const calls = stub({
      proxy: [
        json(502, { error: { code: "provider_failed" } }),
        json(502, { error: { code: "provider_failed" } }),
        json(502, { error: { code: "provider_failed" } }),
        sse([LIVE_SSE]),
      ],
    });
    const source = await open();

    await expect(chunks(source)).rejects.toBeInstanceOf(ThreadUnavailable);
    // Stops at three; the fourth response, which would have succeeded, is never
    // asked for. A retry re-spends budget, so the ceiling has to bite.
    expect(proxyCalls(calls)).toHaveLength(3);
  });

  it("does not retry a settled grant", async () => {
    for (const [status, code] of [
      [403, "grant_revoked"],
      [403, "grant_expired"],
      [429, "grant_exhausted"],
      [401, "invalid_grant"],
    ] as const) {
      const calls = stub({
        proxy: [json(status, { error: { code } }), sse([LIVE_SSE])],
      });
      const source = await open();

      await expect(chunks(source)).rejects.toBeInstanceOf(ThreadUnavailable);
      // Retrying a settled refusal tells the reader the same thing three times.
      expect(proxyCalls(calls)).toHaveLength(1);
      vi.unstubAllGlobals();
    }
  });
});

describe("the session's anchor on the thread lane", () => {
  const proxied = (calls: ReadonlyArray<Call>) =>
    calls.filter((call) => call.url.endsWith("/api/inference/proxy"));

  const tool = (name: string) => ({
    name,
    description: `the ${name} tool`,
    parameters: { type: "object" as const },
    run: async () => "done",
  });

  it("says what the session is, so the model does not answer with the name underneath", async () => {
    // The gap this closes: asked "who are you", a thread session answered
    // "I'm ChatGPT" — the hosted model's own name — because nothing on this
    // lane had ever told it otherwise.
    const calls = stub({});
    const source = await open();
    await chunks(source);

    const anchor = anchorOf(proxied(calls)[0]?.body["messages"]);
    expect(anchor?.["content"]).toContain("`openagents coder`");
    expect(anchor?.["content"]).toContain("inference proxy");
  });

  it("names the declared tools as a closed list", async () => {
    const calls = stub({});
    const source = await open();
    source.useTools([tool("shell"), tool("delegate")]);
    await chunks(source);

    const content = String(anchorOf(proxied(calls)[0]?.body["messages"])?.["content"]);
    expect(content).toContain("You have 2 tools, and no others:");
    expect(content).toContain("`shell`");
    expect(content).toContain("`delegate`");
  });

  it("carries the standing context, rather than gluing it to what the reader typed", async () => {
    const calls = stub({});
    const source = await open();
    source.useContext("This session is working in /repo.");
    await chunks(source, "hello");

    const messages = proxied(calls)[0]?.body["messages"] as Array<Record<string, unknown>>;
    expect(String(anchorOf(messages)?.["content"])).toContain("This session is working in /repo.");
    // The reader's turn is the reader's words and nothing else. Prefixed onto
    // the prompt, the preamble read as something they had typed.
    expect(conversation(messages)).toEqual([{ role: "user", content: "hello" }]);
  });

  it("sends the anchor once, not on every turn", async () => {
    const calls = stub({});
    const source = await open();
    await chunks(source, "first");
    await chunks(source, "second");

    const messages = proxied(calls)[1]?.body["messages"] as Array<Record<string, unknown>>;
    expect(messages.filter((message) => message["role"] === "system")).toHaveLength(1);
    expect(messages[0]?.["role"]).toBe("system");
  });

  it("shows the reader the same text it sends", async () => {
    stub({});
    const source = await open();
    source.useTools([tool("shell")]);
    source.useContext("Workspace facts.");

    // `/system` reads the thing the model read. It used to say the server
    // composed this, which was both untrue and hiding that nothing was sent.
    const shown = source.describeContext();
    expect(shown).toContain("`openagents coder`");
    expect(shown).toContain("Workspace facts.");
    expect(shown).not.toContain("composed by the server");
  });
});

describe("resolveProxyUrl", () => {
  it("resolves the grant's path against the client's origin", () => {
    expect(
      resolveProxyUrl("http://localhost:4000/api/inference/proxy", "http://host.docker.internal:4000"),
    ).toBe("http://host.docker.internal:4000/api/inference/proxy");
  });

  it("keeps a same-origin grant URL intact", () => {
    expect(
      resolveProxyUrl("https://openagents.com/api/inference/proxy", "https://openagents.com"),
    ).toBe("https://openagents.com/api/inference/proxy");
  });
});
