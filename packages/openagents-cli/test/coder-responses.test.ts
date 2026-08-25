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
    expect(calls[0]?.body).toEqual({ input: "hello", stream: true });
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
