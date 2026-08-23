import { afterEach, describe, expect, it, vi } from "vitest";

import { CODER_BACKENDS, defaultBackend, findBackend } from "../src/coder-backends.js";
import { ChatApiReplySource, ChatApiUnavailable } from "../src/coder-chat-api.js";
import type { ReplyChunk } from "../src/coder-session.js";

const json = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

const chunks = async (source: ChatApiReplySource, prompt = "hello") => {
  const out: ReplyChunk[] = [];
  for await (const chunk of source.reply(prompt, new AbortController().signal)) out.push(chunk);
  return out;
};

/** The assistant text a turn produced, which most of these tests assert on. */
const collect = async (source: ChatApiReplySource, prompt = "hello") =>
  (await chunks(source, prompt))
    .map((chunk) => (chunk.type === "text" ? chunk.value : ""))
    .join("");

const source = () =>
  new ChatApiReplySource({ origin: "https://openagents.test", token: "test-token" });

afterEach(() => {
  vi.unstubAllGlobals();
});

/** Answer `GET /chat/events` from a script and `POST /chat/turns` with a run id. */
const stubFetch = (pages: ReadonlyArray<ReadonlyArray<unknown>>, submit = json(202, {})) => {
  let page = 0;
  const calls: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn((url: URL, init?: RequestInit) => {
      const path = url.pathname;
      calls.push(`${init?.method ?? "GET"} ${path}`);
      if (path.endsWith("/chat/turns")) return Promise.resolve(submit.clone());
      const events = pages[Math.min(page, pages.length - 1)] ?? [];
      page += 1;
      return Promise.resolve(json(200, { events }));
    }),
  );
  return calls;
};

describe("ChatApiReplySource", () => {
  it("yields text deltas for the submitted run and stops at completed", async () => {
    stubFetch(
      [
        [],
        [
          { run_id: "run-1", sequence: 1, type: "text_delta", payload: { value: "Hello" } },
          { run_id: "run-1", sequence: 2, type: "text_delta", payload: { value: " there" } },
          { run_id: "run-1", sequence: 3, type: "response_completed", payload: {} },
        ],
      ],
      json(202, { turn: { id: "run-1" } }),
    );

    expect(await collect(source())).toBe("Hello there");
  });

  it("does not replay events that existed before the turn was submitted", async () => {
    stubFetch(
      [
        [{ run_id: "run-0", sequence: 9, type: "text_delta", payload: { value: "OLD" } }],
        [
          { run_id: "run-0", sequence: 9, type: "text_delta", payload: { value: "OLD" } },
          { run_id: "run-1", sequence: 1, type: "text_delta", payload: { value: "new" } },
          { run_id: "run-1", sequence: 2, type: "response_completed", payload: {} },
        ],
      ],
      json(202, { turn: { id: "run-1" } }),
    );

    expect(await collect(source())).toBe("new");
  });

  it("does not repeat a delta already delivered on an earlier poll", async () => {
    stubFetch(
      [
        [],
        [{ run_id: "run-1", sequence: 1, type: "text_delta", payload: { value: "one" } }],
        [
          { run_id: "run-1", sequence: 1, type: "text_delta", payload: { value: "one" } },
          { run_id: "run-1", sequence: 2, type: "text_delta", payload: { value: " two" } },
          { run_id: "run-1", sequence: 3, type: "response_completed", payload: {} },
        ],
      ],
      json(202, { turn: { id: "run-1" } }),
    );

    expect(await collect(source())).toBe("one two");
  });

  it("yields reasoning deltas beside the text, in the order they were recorded", async () => {
    stubFetch(
      [
        [],
        [
          { run_id: "run-1", sequence: 1, type: "reasoning_delta", payload: { value: "thinking" } },
          { run_id: "run-1", sequence: 2, type: "text_delta", payload: { value: "said" } },
          { run_id: "run-1", sequence: 3, type: "response_completed", payload: {} },
        ],
      ],
      json(202, { turn: { id: "run-1" } }),
    );

    expect(await chunks(source())).toEqual([
      { type: "reasoning", value: "thinking" },
      { type: "text", value: "said" },
    ]);
  });

  it("surfaces a tool call from the projection the event carries", async () => {
    stubFetch(
      [
        [],
        [
          {
            run_id: "run-1",
            sequence: 1,
            type: "tool_call_started",
            payload: { call_id: "c1", name: "repo_grep", arguments: '{"pattern":"x"}' },
            tool_call: {
              call_id: "c1",
              name: "repo_grep",
              arguments: '{\n  "pattern": "x"\n}',
              output: null,
              error: null,
              status: "running",
            },
          },
          {
            run_id: "run-1",
            sequence: 2,
            type: "tool_call_completed",
            payload: { call_id: "c1", output: "{}" },
            tool_call: {
              call_id: "c1",
              name: "repo_grep",
              arguments: '{\n  "pattern": "x"\n}',
              output: '{\n  "matches": []\n}',
              error: null,
              status: "succeeded",
            },
          },
          { run_id: "run-1", sequence: 3, type: "response_completed", payload: {} },
        ],
      ],
      json(202, { turn: { id: "run-1" } }),
    );

    // The pretty-printed projection is what the browser shows, so the CLI
    // reads it rather than re-deriving the call from the raw payload.
    expect(await chunks(source())).toEqual([
      {
        type: "tool_call",
        callId: "c1",
        name: "repo_grep",
        arguments: '{\n  "pattern": "x"\n}',
      },
      { type: "tool_result", callId: "c1", output: '{\n  "matches": []\n}', error: undefined },
    ]);
  });

  it("reports a failed tool call with the server's message", async () => {
    stubFetch(
      [
        [],
        [
          {
            run_id: "run-1",
            sequence: 1,
            type: "tool_call_failed",
            payload: { call_id: "c1", error: "The tool is not authorized for this data scope." },
            tool_call: {
              call_id: "c1",
              name: "conversation_search",
              arguments: "{}",
              output: null,
              error: { code: null, message: "The tool is not authorized for this data scope." },
              status: "failed",
            },
          },
          { run_id: "run-1", sequence: 2, type: "response_completed", payload: {} },
        ],
      ],
      json(202, { turn: { id: "run-1" } }),
    );

    expect(await chunks(source())).toEqual([
      {
        type: "tool_result",
        callId: "c1",
        output: undefined,
        error: "The tool is not authorized for this data scope.",
      },
    ]);
  });

  it("reads a tool call from the raw payload when no projection is attached", async () => {
    stubFetch(
      [
        [],
        [
          {
            run_id: "run-1",
            sequence: 1,
            type: "tool_call_started",
            payload: { call_id: "c1", name: "repo_grep", arguments: '{"pattern":"x"}' },
          },
          { run_id: "run-1", sequence: 2, type: "response_completed", payload: {} },
        ],
      ],
      json(202, { turn: { id: "run-1" } }),
    );

    expect(await chunks(source())).toEqual([
      { type: "tool_call", callId: "c1", name: "repo_grep", arguments: '{"pattern":"x"}' },
    ]);
  });

  it("reports a missing scope rather than an empty reply", async () => {
    stubFetch([[]], json(401, { error: "invalid_api_token" }));
    await expect(collect(source())).rejects.toThrow(ChatApiUnavailable);
    await expect(collect(source())).rejects.toThrow(/chat:account/);
  });

  it("names the one-turn-at-a-time rule when the server refuses a concurrent turn", async () => {
    stubFetch([[]], json(409, { error: "turn_in_progress" }));
    await expect(collect(source())).rejects.toThrow(/one turn runs at a time/i);
  });

  it("surfaces a failed turn with the server's reason", async () => {
    stubFetch(
      [
        [],
        [
          {
            run_id: "run-1",
            sequence: 1,
            type: "response_failed",
            payload: { reason: "provider refused", code: "invalid_response" },
          },
        ],
      ],
      json(202, { turn: { id: "run-1" } }),
    );
    await expect(collect(source())).rejects.toThrow(/provider refused/);
  });

  it("sends the prompt and the reasoning effort the caller chose", async () => {
    const seen: Array<Record<string, unknown>> = [];
    let submitted = false;
    vi.stubGlobal(
      "fetch",
      vi.fn((url: URL, init?: RequestInit) => {
        if (url.pathname.endsWith("/chat/turns")) {
          seen.push(JSON.parse(String(init?.body)) as Record<string, unknown>);
          submitted = true;
          return Promise.resolve(json(202, { turn: { id: "run-1" } }));
        }
        // The run does not exist until it is submitted, which is what the
        // pre-submit snapshot relies on.
        return Promise.resolve(
          json(200, {
            events: submitted
              ? [{ run_id: "run-1", sequence: 1, type: "response_completed", payload: {} }]
              : [],
          }),
        );
      }),
    );

    const configured = new ChatApiReplySource({
      origin: "https://openagents.test",
      token: "test-token",
      reasoning: "high",
    });
    await collect(configured, "do the thing");

    expect(seen[0]).toEqual({
      message: "do the thing",
      model: defaultBackend().id,
      reasoning: "high",
    });
  });

  it("reports the label of the backend it is set to", () => {
    expect(source().model).toBe(defaultBackend().label);
    expect(source().backendId).toBe(defaultBackend().id);
  });

  it("sends the backend it was constructed with", async () => {
    const seen: Array<Record<string, unknown>> = [];
    let page = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn((url: URL, init?: RequestInit) => {
        if (url.pathname.endsWith("/chat/turns")) {
          seen.push(JSON.parse(String(init?.body)) as Record<string, unknown>);
          return Promise.resolve(json(202, { turn: { id: "run-1" } }));
        }
        // The source snapshots the log before it submits, so the first read
        // has to predate the turn or it counts the reply as already seen.
        const events =
          page++ === 0
            ? []
            : [
                { run_id: "run-1", sequence: 1, type: "text_delta", payload: { value: "hi" } },
                { run_id: "run-1", sequence: 2, type: "response_completed", payload: {} },
              ];
        return Promise.resolve(json(200, { events }));
      }),
    );

    const gemini = new ChatApiReplySource({
      origin: "https://openagents.test",
      token: "test-token",
      backend: findBackend("gemini-3.7-flash"),
    });

    expect(gemini.model).toBe("Gemini 3.7 Flash");
    expect(await collect(gemini, "hello")).toBe("hi");
    expect(seen[0]?.["model"]).toBe("gemini-3.7-flash");
  });

  it("cycles through every backend and wraps back to the first", () => {
    const cycling = source();
    const labels = CODER_BACKENDS.map(() => cycling.cycleBackend());

    // Every backend is reachable, and the last cycle returns to the start, so
    // a third entry needs no second key.
    expect(new Set(labels).size).toBe(CODER_BACKENDS.length);
    expect(cycling.model).toBe(defaultBackend().label);
  });
});
