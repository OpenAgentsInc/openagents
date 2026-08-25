import { afterEach, describe, expect, it, vi } from "vitest";

import { ResponsesReplySource } from "../src/coder-responses.js";
import type { ReplyChunk } from "../src/coder-session.js";
import { ThreadTranscriptWriter } from "../src/coder-transcript.js";

const sse = (frames: ReadonlyArray<string>) =>
  new Response(
    new ReadableStream<Uint8Array>({
      start(controller) {
        const encoder = new TextEncoder();
        for (let i = 0; i < frames.length; i += 1) {
          const frame = frames[i];
          if (frame !== undefined) controller.enqueue(encoder.encode(frame));
        }
        controller.close();
      },
    }),
    { status: 200, headers: { "content-type": "text/event-stream" } },
  );

/** The stub's own sequence, as the server sends it. */
const STREAM = [
  'event: response.created\ndata: {"type":"response.created","sequence_number":0}\n\n',
  'event: response.output_text.delta\ndata: {"type":"response.output_text.delta","sequence_number":1,"delta":"Acknow"}\n\n',
  'event: response.output_text.delta\ndata: {"type":"response.output_text.delta","sequence_number":2,"delta":"ledged."}\n\n',
  'event: response.completed\ndata: {"type":"response.completed","sequence_number":3}\n\n',
];

afterEach(() => {
  vi.unstubAllGlobals();
});

const collect = async (source: ResponsesReplySource) => {
  const out: ReplyChunk[] = [];
  for await (const chunk of source.reply("hello", new AbortController().signal)) out.push(chunk);
  return out;
};

describe("ResponsesReplySource", () => {
  it("streams the request and concatenates the deltas it is sent", async () => {
    const calls: Array<{ url: string; body: Record<string, unknown>; accept: string }> = [];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (target: URL | string, init?: RequestInit) => {
        const headers = (init?.headers ?? {}) as Record<string, string>;
        calls.push({
          url: target.toString(),
          body: JSON.parse(typeof init?.body === "string" ? init.body : "{}") as Record<
            string,
            unknown
          >,
          accept: headers["accept"] ?? "",
        });
        return sse(STREAM);
      }),
    );

    const source = new ResponsesReplySource({ origin: "http://localhost:4000", token: "oa_pat_x" });
    const chunks = await collect(source);

    expect(calls[0]?.url).toBe("http://localhost:4000/api/v1/responses");
    // The conversation is client-held: the input is the item history.
    expect(calls[0]?.body).toEqual({
      input: [{ role: "user", content: "hello" }],
      stream: true,
    });
    // Both named: the pipeline negotiates on json, the answer is SSE.
    expect(calls[0]?.accept).toContain("application/json");
    const text = chunks.map((chunk) => (chunk.type === "text" ? chunk.value : "")).join("");
    expect(text).toBe("Acknowledged.");
    expect(chunks.at(-1)).toMatchObject({ type: "usage", calls: 1 });
  });

  it("labels itself as Coder Auto and records the product id", () => {
    const source = new ResponsesReplySource({ origin: "http://localhost:4000" });
    expect(source.model).toBe("Coder Auto");
    expect(source.modelId).toBe("openagents-coder");
  });

  it("reports a refusal with the origin and the status when retries exhaust", async () => {
    let callCount = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        callCount += 1;
        return new Response("no", { status: 503 });
      }),
    );
    const source = new ResponsesReplySource({
      origin: "http://localhost:4000",
      retryDelaysMs: [1, 1, 1],
    });

    await expect(collect(source)).rejects.toThrow("http://localhost:4000 answered HTTP 503");
    expect(callCount).toBe(4);
  });

  it("does not retry 4xx errors", async () => {
    let callCount = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        callCount += 1;
        return new Response("bad request", { status: 400 });
      }),
    );
    const source = new ResponsesReplySource({
      origin: "http://localhost:4000",
      retryDelaysMs: [1, 1, 1],
    });

    await expect(collect(source)).rejects.toThrow("http://localhost:4000 answered HTTP 400");
    expect(callCount).toBe(1);
  });

  it("retries transient 5xx server errors and succeeds when server recovers", async () => {
    let callCount = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        callCount += 1;
        if (callCount < 3) {
          return new Response("server down briefly", { status: 500 });
        }
        return sse(STREAM);
      }),
    );

    const source = new ResponsesReplySource({
      origin: "http://localhost:4000",
      retryDelaysMs: [1, 1, 1],
    });
    const chunks = await collect(source);
    expect(callCount).toBe(3);
    const text = chunks.map((chunk) => (chunk.type === "text" ? chunk.value : "")).join("");
    expect(text).toBe("Acknowledged.");
  });

  it("retries fetch network reachability failures and recovers", async () => {
    let callCount = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        callCount += 1;
        if (callCount === 1) {
          throw new TypeError("fetch failed: ECONNREFUSED");
        }
        return sse(STREAM);
      }),
    );

    const source = new ResponsesReplySource({
      origin: "http://localhost:4000",
      retryDelaysMs: [1, 1, 1],
    });
    const chunks = await collect(source);
    expect(callCount).toBe(2);
    const text = chunks.map((chunk) => (chunk.type === "text" ? chunk.value : "")).join("");
    expect(text).toBe("Acknowledged.");
  });
});

// The agentic loop over the surface: the model asks for a tool, the client
// runs it, replays the output, and the next call answers in text.
describe("ResponsesReplySource tools", () => {
  const callStream = [
    'event: response.output_item.added\ndata: {"type":"response.output_item.added","sequence_number":0}\n\n',
    'event: response.output_item.done\ndata: {"type":"response.output_item.done","sequence_number":1,"output_index":1,"item":{"type":"function_call","call_id":"call_1","name":"read_conversation","arguments":"{\\"max_turns\\":4}","status":"completed"}}\n\n',
    'event: response.completed\ndata: {"type":"response.completed","sequence_number":2}\n\n',
  ];
  const answerStream = [
    'event: response.output_text.delta\ndata: {"type":"response.output_text.delta","sequence_number":0,"delta":"Four turns, read."}\n\n',
    'event: response.completed\ndata: {"type":"response.completed","sequence_number":1}\n\n',
  ];

  it("runs the requested tool and continues to the text answer", async () => {
    const bodies: Array<Record<string, unknown>> = [];
    const streams = [callStream, answerStream];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_target: URL | string, init?: RequestInit) => {
        bodies.push(
          JSON.parse(typeof init?.body === "string" ? init.body : "{}") as Record<string, unknown>,
        );
        return sse(streams.shift() ?? []);
      }),
    );

    const ran: Array<Record<string, unknown>> = [];
    const source = new ResponsesReplySource({ origin: "http://localhost:4000" });
    source.useTools([
      {
        name: "read_conversation",
        description: "Read a conversation back.",
        parameters: { type: "object" },
        run: (args) => {
          ran.push(args);
          return Promise.resolve("four turns of text");
        },
      },
    ]);

    const chunks = await collect(source);

    // The tool ran with the model's arguments.
    expect(ran).toEqual([{ max_turns: 4 }]);
    // The call and its result both rendered, then the answer.
    expect(chunks.map((chunk) => chunk.type)).toEqual([
      "tool_call",
      "tool_result",
      "text",
      "usage",
    ]);
    // The second request replayed the call and its output as items.
    const replayed = bodies[1]?.["input"] as Array<Record<string, unknown>>;
    expect(replayed.at(-2)).toMatchObject({ type: "function_call", call_id: "call_1" });
    expect(replayed.at(-1)).toMatchObject({
      type: "function_call_output",
      call_id: "call_1",
      output: "four turns of text",
    });
    // And declared the tools both times.
    expect(bodies[0]?.["tools"]).toBeDefined();
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

describe("the dev lane's durable transcript", () => {
  const callStream = [
    'event: response.reasoning_summary_text.delta\ndata: {"type":"response.reasoning_summary_text.delta","sequence_number":0,"delta":"Listing "}\n\n',
    'event: response.reasoning_summary_text.delta\ndata: {"type":"response.reasoning_summary_text.delta","sequence_number":1,"delta":"first."}\n\n',
    'event: response.output_item.done\ndata: {"type":"response.output_item.done","sequence_number":2,"output_index":0,"item":{"type":"function_call","call_id":"call_1","name":"shell","arguments":"{\\"command\\":\\"ls\\"}","status":"completed"}}\n\n',
    'event: response.completed\ndata: {"type":"response.completed","sequence_number":3}\n\n',
  ];
  const answerStream = [
    'event: response.output_text.delta\ndata: {"type":"response.output_text.delta","sequence_number":0,"delta":"Two files."}\n\n',
    'event: response.completed\ndata: {"type":"response.completed","sequence_number":1}\n\n',
  ];

  it("records the turn in order: what was asked, the reasoning, each tool run, the answer", async () => {
    const streams = [callStream, answerStream];
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => sse(streams.shift() ?? [])),
    );

    const source = new ResponsesReplySource({ origin: "http://localhost:4000", token: "oa_pat_x" });
    const sink = recorder();
    source.useTranscript(sink);
    source.useTools([
      {
        name: "shell",
        description: "run a command",
        parameters: { type: "object" },
        run: () => Promise.resolve("README.md\nsrc"),
      },
    ]);

    await collect(source);

    expect(sink.events.map((event) => event.eventType)).toEqual([
      "turn.user",
      "turn.reasoning",
      "tool.ran",
      "turn.assistant",
    ]);
    expect(sink.events[0]?.payload).toEqual({ text: "hello" });
    // One event per block, whole, never the deltas it arrived in.
    expect(sink.events[1]?.payload).toEqual({ text: "Listing first." });
    expect(sink.events[2]?.payload).toEqual({
      call_id: "call_1",
      tool: "shell",
      arguments: `{"command":"ls"}`,
      status: "succeeded",
      output: "README.md\nsrc",
    });
    // This surface reports no token counts, so the usage carries only the
    // call count rather than zeros a reader would take for measurements.
    expect(sink.events[3]?.payload).toEqual({
      text: "Two files.",
      usage: { calls: 2 },
      tool_calls: 1,
    });
  });

  it("records a failed tool run as failed, not as a crash", async () => {
    const streams = [callStream, answerStream];
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => sse(streams.shift() ?? [])),
    );

    const source = new ResponsesReplySource({ origin: "http://localhost:4000" });
    const sink = recorder();
    source.useTranscript(sink);
    source.useTools([
      {
        name: "shell",
        description: "run a command",
        parameters: { type: "object" },
        run: () => Promise.reject(new Error("no such directory")),
      },
    ]);

    await collect(source);

    const ran = sink.events.find((event) => event.eventType === "tool.ran");
    expect(ran?.payload).toEqual({
      call_id: "call_1",
      tool: "shell",
      arguments: `{"command":"ls"}`,
      status: "failed",
      error: "no such directory",
    });
  });

  it("posts the recorded events to the thread's transcript on the server", async () => {
    const streams = [callStream, answerStream];
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => sse(streams.shift() ?? [])),
    );

    // The writer's own transport is injected, so this test sees exactly what
    // a server at the other end would: the events route, in order.
    const posted: Array<{ url: string; body: Record<string, unknown> }> = [];
    const writer = new ThreadTranscriptWriter({
      origin: "http://localhost:4000",
      threadId: "9bb19447-ecf4-4f1b-b44e-6b128664da9c",
      token: "oa_pat_x",
      fetch: async (url, init) => {
        posted.push({
          url: url.toString(),
          body: JSON.parse(String(init?.body)) as Record<string, unknown>,
        });
        return new Response("{}", { status: 201 });
      },
    });

    const source = new ResponsesReplySource({ origin: "http://localhost:4000", token: "oa_pat_x" });
    source.useTranscript(writer);
    source.useTools([
      {
        name: "shell",
        description: "run a command",
        parameters: { type: "object" },
        run: () => Promise.resolve("README.md\nsrc"),
      },
    ]);

    await collect(source);
    await writer.close();

    expect(posted[0]?.url).toBe(
      "http://localhost:4000/api/v1/threads/9bb19447-ecf4-4f1b-b44e-6b128664da9c/events",
    );
    expect(posted.map((post) => post.body["event_type"])).toEqual([
      "turn.user",
      "turn.reasoning",
      "tool.ran",
      "turn.assistant",
    ]);
  });

  it("runs exactly as before when no transcript is attached, recording nothing", async () => {
    // The unauthenticated case: `--dev` without a credential opens no thread
    // and attaches no writer, and the turn must not know the difference.
    const streams = [callStream, answerStream];
    const fetchMock = vi.fn(async () => sse(streams.shift() ?? []));
    vi.stubGlobal("fetch", fetchMock);

    const source = new ResponsesReplySource({ origin: "http://localhost:4000" });
    source.useTools([
      {
        name: "shell",
        description: "run a command",
        parameters: { type: "object" },
        run: () => Promise.resolve("README.md\nsrc"),
      },
    ]);

    const chunks = await collect(source);

    // The turn ran whole — call, result, answer — and the only requests made
    // were the two responses calls; nothing went to a threads route.
    expect(chunks.at(-1)).toMatchObject({ type: "usage", calls: 2 });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });
});
