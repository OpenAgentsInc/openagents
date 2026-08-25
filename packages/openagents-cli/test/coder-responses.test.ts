import { afterEach, describe, expect, it, vi } from "vitest";

import { ResponsesReplySource } from "../src/coder-responses.js";
import type { ReplyChunk } from "../src/coder-session.js";

const sse = (frames: ReadonlyArray<string>) =>
  new Response(
    new ReadableStream<Uint8Array>({
      start(controller) {
        const encoder = new TextEncoder();
        for (const frame of frames) controller.enqueue(encoder.encode(frame));
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
    const text = chunks
      .map((chunk) => (chunk.type === "text" ? chunk.value : ""))
      .join("");
    expect(text).toBe("Acknowledged.");
    expect(chunks.at(-1)).toMatchObject({ type: "usage", calls: 1 });
  });

  it("labels itself as Coder Auto and records the product id", () => {
    const source = new ResponsesReplySource({ origin: "http://localhost:4000" });
    expect(source.model).toBe("Coder Auto");
    expect(source.modelId).toBe("openagents-coder");
  });

  it("reports a refusal with the origin and the status", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response("no", { status: 503 })));
    const source = new ResponsesReplySource({ origin: "http://localhost:4000" });

    await expect(collect(source)).rejects.toThrow("http://localhost:4000 answered HTTP 503");
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
