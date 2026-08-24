import { describe, expect, it, vi } from "vitest";

import type { ReplyChunk } from "../src/coder-session.js";
import type { CoderTool } from "../src/coder-tools.js";
import {
  isOllamaModelFlag,
  OllamaReplySource,
  parseOllamaModelFlag,
} from "../src/coder-ollama.js";

const MODEL = "qwen3.8:27b-mtp-q8_0";

/** One streamed Ollama chunk, in the shape the client yields. */
const chunk = (message: Record<string, unknown>, done = false) => ({ message, done });

/**
 * Stand in for the Ollama client's `chat`, one scripted stream per round.
 *
 * The source drives a turn as a loop, so a tool test needs a second round to
 * answer with. Each call also records the request, which is how the assertions
 * about the declared tools and the transcript are made.
 */
const client = (rounds: ReadonlyArray<ReadonlyArray<ReturnType<typeof chunk>>>) => {
  const requests: Record<string, unknown>[] = [];
  let round = 0;
  const chat = vi.fn(async (request: Record<string, unknown>) => {
    requests.push(request);
    const scripted = rounds[round] ?? [];
    round += 1;
    return Object.assign(
      (async function* () {
        for (const piece of scripted) yield piece;
      })(),
      { abort: () => {} },
    );
  });
  return { chat, requests };
};

const sourceWith = (rounds: ReadonlyArray<ReadonlyArray<ReturnType<typeof chunk>>>) => {
  const stub = client(rounds);
  const source = new OllamaReplySource({ model: MODEL });
  // The client is constructed in the constructor, so the stub replaces it.
  (source as unknown as { client: unknown }).client = { chat: stub.chat };
  return { source, stub };
};

const collect = async (source: OllamaReplySource, prompt: string): Promise<ReplyChunk[]> => {
  const chunks: ReplyChunk[] = [];
  for await (const piece of source.reply(prompt, new AbortController().signal)) {
    chunks.push(piece);
  }
  return chunks;
};

describe("the ollama model flag", () => {
  it("recognizes an ollama model and takes its name", () => {
    expect(isOllamaModelFlag("ollama:qwen3")).toBe(true);
    expect(isOllamaModelFlag("ox-alpha")).toBe(false);
    expect(parseOllamaModelFlag("ollama:qwen3")).toBe("qwen3");
  });
});

describe("an ollama reply", () => {
  it("streams text and reasoning without declaring tools it does not have", async () => {
    const { source, stub } = sourceWith([
      [chunk({ thinking: "weighing it" }), chunk({ content: "Hello" }), chunk({}, true)],
    ]);

    const chunks = await collect(source, "hi");

    expect(chunks).toEqual([
      { type: "reasoning", value: "weighing it" },
      { type: "text", value: "Hello" },
    ]);
    expect(stub.requests[0]).not.toHaveProperty("tools");
  });
});

/** A `delegate` stand-in that records the arguments the model sent. */
const delegate = (calls: Record<string, unknown>[]): CoderTool => ({
  name: "delegate",
  description: "Run a prompt on child agents.",
  parameters: { type: "object", properties: { prompt: { type: "string" } } },
  run: async (args) => {
    calls.push(args);
    return "child 1 said PONG";
  },
});

describe("an ollama turn that calls a tool", () => {
  const CALLING = [
    chunk({
      content: "",
      tool_calls: [{ function: { name: "delegate", arguments: { prompt: "say PONG" } } }],
    }),
    chunk({}, true),
  ];

  it("declares the tools, runs the call, and answers from its result", async () => {
    const calls: Record<string, unknown>[] = [];
    const { source, stub } = sourceWith([CALLING, [chunk({ content: "They said PONG." }, true)]]);
    source.useTools([delegate(calls)]);

    const chunks = await collect(source, "delegate this");

    // The tool ran with the arguments the model sent, parsed.
    expect(calls).toEqual([{ prompt: "say PONG" }]);

    const call = chunks.find((piece) => piece.type === "tool_call");
    const result = chunks.find((piece) => piece.type === "tool_result");
    expect(call).toMatchObject({ name: "delegate" });
    expect(result).toMatchObject({ output: "child 1 said PONG", error: undefined });
    // The call and its result share an id, which is how a renderer pairs them.
    expect(result).toMatchObject({ callId: (call as { callId: string }).callId });
    // The turn continued rather than ending on the tool result.
    expect(chunks.at(-1)).toEqual({ type: "text", value: "They said PONG." });

    // The tool was declared, and the second round carried the exchange back.
    expect(stub.requests[0]).toHaveProperty("tools");
    const messages = stub.requests[1]?.["messages"] as ReadonlyArray<Record<string, unknown>>;
    expect(messages.map((message) => message["role"])).toEqual(["user", "assistant", "tool"]);
    expect(messages.at(-1)).toMatchObject({ content: "child 1 said PONG", tool_name: "delegate" });
  });

  it("reports a tool that throws instead of ending the turn", async () => {
    const { source } = sourceWith([CALLING, [chunk({ content: "It failed." }, true)]]);
    source.useTools([
      {
        name: "delegate",
        description: "Run a prompt on child agents.",
        parameters: { type: "object" },
        run: () => Promise.reject(new Error("the fleet is full")),
      },
    ]);

    const chunks = await collect(source, "delegate this");

    expect(chunks.find((piece) => piece.type === "tool_result")).toMatchObject({
      error: "the fleet is full",
      output: "the fleet is full",
    });
    expect(chunks.at(-1)).toEqual({ type: "text", value: "It failed." });
  });

  it("tells the model when it asks for a tool the session does not have", async () => {
    const { source } = sourceWith([
      [
        chunk({ content: "", tool_calls: [{ function: { name: "nope", arguments: {} } }] }),
        chunk({}, true),
      ],
      [chunk({ content: "Understood." }, true)],
    ]);
    source.useTools([]);

    const chunks = await collect(source, "call nope");

    expect(chunks.find((piece) => piece.type === "tool_result")).toMatchObject({
      error: "This session has no `nope` tool.",
    });
  });

  it("stops after the tool-call ceiling rather than looping forever", async () => {
    const rounds = Array.from({ length: 8 }, () => CALLING);
    const { source, stub } = sourceWith(rounds);
    source.useTools([delegate([])]);

    const chunks = await collect(source, "keep going");

    // Six rounds, then a sentence saying why it stopped.
    expect(stub.chat).toHaveBeenCalledTimes(6);
    expect(chunks.at(-1)).toMatchObject({ type: "text" });
    expect((chunks.at(-1) as { value: string }).value).toContain("Stopped after 6 rounds");
  });
});
