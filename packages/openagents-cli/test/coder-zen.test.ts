import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ReplyChunk } from "../src/coder-session.js";
import { ZenReplySource, zenCredential, zenModelId } from "../src/coder-zen.js";

const authFile = (contents: unknown) => {
  const path = join(mkdtempSync(join(tmpdir(), "oa-zen-")), "auth.json");
  writeFileSync(path, JSON.stringify(contents));
  return path;
};

const sse = (lines: ReadonlyArray<string>) =>
  new Response(
    new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(new TextEncoder().encode(lines.join("\n\n") + "\n\n"));
        controller.close();
      },
    }),
    { status: 200, headers: { "content-type": "text/event-stream" } },
  );

interface Sent {
  readonly url: string;
  readonly authorization: string;
  readonly body: Record<string, unknown>;
}

const stub = (responses: ReadonlyArray<Response>) => {
  const sent: Sent[] = [];
  const queue = [...responses];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (target: URL | string, init?: RequestInit) => {
      const headers = (init?.headers ?? {}) as Record<string, string>;
      sent.push({
        url: String(target),
        authorization: headers["authorization"] ?? "",
        body: JSON.parse(typeof init?.body === "string" ? init.body : "{}") as Record<
          string,
          unknown
        >,
      });
      return queue.shift() ?? sse([`data: [DONE]`]);
    }),
  );
  return sent;
};

const chunks = async (source: ZenReplySource, prompt: string) => {
  const out: ReplyChunk[] = [];
  for await (const chunk of source.reply(prompt, new AbortController().signal)) out.push(chunk);
  return out;
};

const textOf = (out: ReadonlyArray<ReplyChunk>) =>
  out.map((chunk) => (chunk.type === "text" ? chunk.value : "")).join("");

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("naming the model", () => {
  it("takes the name Ox Alpha is known by and sends the slug it answers to", () => {
    expect(zenModelId("ox-alpha")).toBe("x-preview-f-free");
  });

  it("passes an unrecognised name through for the API to refuse by name", () => {
    expect(zenModelId("some-other-model")).toBe("some-other-model");
  });

  it("reports the readable name back, whichever was asked for", () => {
    expect(new ZenReplySource({ model: "x-preview-f-free", key: "k" }).model).toBe("ox-alpha");
    expect(new ZenReplySource({ model: "ox-alpha", key: "k" }).model).toBe("ox-alpha");
  });
});

describe("finding the credential", () => {
  it("prefers the environment", () => {
    expect(zenCredential({ OPENCODE_API_KEY: "from-env" }, authFile({}))).toBe("from-env");
  });

  it("reads opencode's own store when the environment names none", () => {
    const path = authFile({ opencode: { type: "api", key: "from-store" } });
    expect(zenCredential({}, path)).toBe("from-store");
  });

  it("takes only opencode's entry, not another provider's key", () => {
    const path = authFile({ google: { type: "api", key: "not-this-one" } });
    expect(zenCredential({}, path)).toBe(undefined);
  });

  it("reports none rather than calling without one", () => {
    expect(zenCredential({}, join(tmpdir(), "oa-zen-absent", "auth.json"))).toBe(undefined);
    expect(zenCredential({}, authFile("not an object"))).toBe(undefined);
  });
});

describe("answering a turn", () => {
  it("calls Zen with the slug and the borrowed key", async () => {
    const sent = stub([sse([`data: {"choices":[{"delta":{"content":"Hi"}}]}`, `data: [DONE]`])]);
    const source = new ZenReplySource({ model: "ox-alpha", key: "secret-key" });

    expect(textOf(await chunks(source, "hello"))).toBe("Hi");
    expect(sent[0]?.url).toBe("https://opencode.ai/zen/v1/chat/completions");
    expect(sent[0]?.authorization).toBe("Bearer secret-key");
    expect(sent[0]?.body["model"]).toBe("x-preview-f-free");
  });

  it("opens with the session's anchor, once", async () => {
    const sent = stub([
      sse([`data: {"choices":[{"delta":{"content":"a"}}]}`, `data: [DONE]`]),
      sse([`data: {"choices":[{"delta":{"content":"b"}}]}`, `data: [DONE]`]),
    ]);
    const source = new ZenReplySource({ model: "ox-alpha", key: "k" });
    source.useContext("Workspace facts.");

    await chunks(source, "first");
    await chunks(source, "second");

    const messages = sent[1]?.body["messages"] as Array<Record<string, unknown>>;
    const anchors = messages.filter((message) => message["role"] === "system");
    expect(anchors).toHaveLength(1);
    expect(String(anchors[0]?.["content"])).toContain("Ox Alpha through OpenCode Zen");
    expect(String(anchors[0]?.["content"])).toContain("Workspace facts.");
  });

  it("runs a declared tool and answers the model with its result", async () => {
    const ran: string[] = [];
    const sent = stub([
      sse([
        `data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"shell","arguments":"{\\"command\\":\\"ls\\"}"}}]}}]}`,
        `data: [DONE]`,
      ]),
      sse([`data: {"choices":[{"delta":{"content":"two files"}}]}`, `data: [DONE]`]),
    ]);
    const source = new ZenReplySource({ model: "ox-alpha", key: "k" });
    source.useTools([
      {
        name: "shell",
        description: "run a command",
        parameters: { type: "object" },
        run: async (args) => {
          ran.push(String((args as Record<string, unknown>)["command"]));
          return "a.txt b.txt";
        },
      },
    ]);

    expect(textOf(await chunks(source, "list them"))).toBe("two files");
    expect(ran).toEqual(["ls"]);

    // The result is answered back on the transcript, keyed to the call. A call
    // whose result never follows is a transcript the provider refuses.
    const second = sent[1]?.body["messages"] as Array<Record<string, unknown>>;
    const result = second.find((message) => message["role"] === "tool");
    expect(result).toMatchObject({ tool_call_id: "c1", content: "a.txt b.txt" });
  });

  it("declares the session's tools to the model", async () => {
    const sent = stub([sse([`data: [DONE]`])]);
    const source = new ZenReplySource({ model: "ox-alpha", key: "k" });
    source.useTools([
      { name: "shell", description: "run", parameters: { type: "object" }, run: async () => "" },
    ]);

    await chunks(source, "hello");

    const tools = sent[0]?.body["tools"] as Array<Record<string, unknown>>;
    expect(tools).toHaveLength(1);
    expect((tools[0]?.["function"] as Record<string, unknown>)["name"]).toBe("shell");
  });

  it("says which call was refused rather than falling silent", async () => {
    stub([new Response("no such model", { status: 404 })]);
    const source = new ZenReplySource({ model: "nonsense", key: "k" });

    await expect(chunks(source, "hello")).rejects.toThrow(/404.*no such model/s);
  });
});
