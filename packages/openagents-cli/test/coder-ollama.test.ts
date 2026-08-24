import { afterEach, describe, expect, it, vi } from "vitest";

import type { ReplyChunk } from "../src/coder-session.js";
import type { CoderTool } from "../src/coder-tools.js";
import {
  discoverOllamaModel,
  resolveOllamaModel,
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

    // Every turn closes with what it cost, so the text chunks are read apart
    // from the usage chunk that trails them.
    expect(chunks.filter((piece) => piece.type !== "usage")).toEqual([
      { type: "reasoning", value: "weighing it" },
      { type: "text", value: "Hello" },
    ]);
    expect(chunks.at(-1)).toMatchObject({ type: "usage" });
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
    const spoken = chunks.filter((piece) => piece.type !== "usage");
    expect(spoken.at(-1)).toEqual({ type: "text", value: "They said PONG." });

    // The tool was declared, and the second round carried the exchange back.
    expect(stub.requests[0]).toHaveProperty("tools");
    const messages = stub.requests[1]?.["messages"] as ReadonlyArray<Record<string, unknown>>;
    expect(messages.map((message) => message["role"])).toEqual([
      "system",
      "user",
      "assistant",
      "tool",
    ]);
    expect(messages.at(-1)).toMatchObject({ content: "child 1 said PONG", tool_name: "delegate" });
  });


  it("reports the turn's cost, summed over the rounds it took", async () => {
    const calls: Record<string, unknown>[] = [];
    const { source } = sourceWith([
      [
        chunk({
          content: "",
          tool_calls: [{ function: { name: "delegate", arguments: { prompt: "go" } } }],
        }),
        { message: {}, done: true, prompt_eval_count: 100, eval_count: 10 },
      ],
      [{ message: { content: "done" }, done: true, prompt_eval_count: 200, eval_count: 20 }],
    ] as never);
    source.useTools([delegate(calls)]);

    const chunks = await collect(source, "delegate this");

    // Two LLM calls in one turn: the counts are their total, not the last
    // round's presented as the turn's.
    expect(chunks.at(-1)).toEqual({
      type: "usage",
      promptTokens: 300,
      completionTokens: 30,
      calls: 2,
    });
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
    expect(chunks.filter((piece) => piece.type !== "usage").at(-1)).toEqual({
      type: "text",
      value: "It failed.",
    });
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

  it("answers on the last round instead of stopping mid-work", async () => {
    // Always asking for a tool, so the ceiling is what ends the turn. The
    // ceiling is a backstop against looping forever, not a budget: reaching it
    // used to end with "Stopped after 6 rounds" and throw away every read.
    const rounds = Array.from({ length: 200 }, () => CALLING);
    const { source, stub } = sourceWith(rounds);
    source.useTools([delegate([])]);

    await collect(source, "keep going");

    expect(stub.chat).toHaveBeenCalledTimes(100);

    // The last round is asked without tools, so the model has one thing left it
    // can do: report what it found.
    const last = stub.requests.at(-1);
    expect(last).not.toHaveProperty("tools");
    const messages = last?.["messages"] as ReadonlyArray<Record<string, unknown>>;
    expect(messages.at(-1)).toMatchObject({
      role: "user",
      content: expect.stringContaining("Do not call another tool"),
    });
  });
});

describe("what a local session tells the model about itself", () => {
  const systemOf = (stub: { requests: Record<string, unknown>[] }, round = 0) => {
    const messages = stub.requests[round]?.["messages"] as ReadonlyArray<Record<string, unknown>>;
    return messages[0] as { role: string; content: string };
  };

  it("opens with a system message naming every declared tool and no others", async () => {
    const { source, stub } = sourceWith([[chunk({ content: "ok" }, true)]]);
    source.useTools([delegate([])]);

    await collect(source, "hi");

    const system = systemOf(stub);
    expect(system.role).toBe("system");
    expect(system.content).toContain("1 tool");
    expect(system.content).toContain("`delegate`");
    // The failure this exists to stop: the model answering with the tools a
    // coding agent usually has rather than the ones it was given.
    // Stated as a closed list, not by naming what is absent: this once said
    // there was no shell, and then a shell tool was added.
    expect(system.content).toContain("a capability not on it is one you do not have");
    expect(system.content).not.toMatch(/no file, shell/);
    // A tool description says what a child can do. That is not the model's.
    expect(system.content).toContain("that is the child's capability and not yours");
  });

  it("says it has none when the session declares no tools", async () => {
    const { source, stub } = sourceWith([[chunk({ content: "ok" }, true)]]);

    await collect(source, "hi");

    expect(systemOf(stub).content).toContain("You have no tools in this session");
  });

  it("is derived from the tools, so it cannot name one the session does not pass", async () => {
    const { source, stub } = sourceWith([[chunk({ content: "ok" }, true)]]);
    source.useTools([
      { name: "alpha", description: "a", parameters: {}, run: () => Promise.resolve("") },
      { name: "beta", description: "b", parameters: {}, run: () => Promise.resolve("") },
    ]);

    await collect(source, "hi");

    const system = systemOf(stub);
    expect(system.content).toContain("2 tools");
    expect(system.content).toContain("`alpha`");
    expect(system.content).toContain("`beta`");
    expect(system.content).not.toContain("`delegate`");
  });

  it("says it once, not on every turn", async () => {
    const { source, stub } = sourceWith([
      [chunk({ content: "one" }, true)],
      [chunk({ content: "two" }, true)],
    ]);
    source.useTools([delegate([])]);

    await collect(source, "first");
    await collect(source, "second");

    const messages = stub.requests[1]?.["messages"] as ReadonlyArray<Record<string, unknown>>;
    expect(messages.filter((message) => message["role"] === "system")).toHaveLength(1);
    expect(messages.map((message) => message["role"])).toEqual([
      "system",
      "user",
      "assistant",
      "user",
    ]);
  });
});

describe("finding a local model to default to", () => {
  const serve = (body: unknown, status = 200) =>
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } }),
    );

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("takes the most recently modified model", async () => {
    serve({
      models: [
        { name: "older", modified_at: "2026-01-01T00:00:00Z" },
        { name: "newest", modified_at: "2026-08-01T00:00:00Z" },
        { name: "middle", modified_at: "2026-04-01T00:00:00Z" },
      ],
    });

    // With one model there is no choice; with several the one most recently
    // pulled is the one the reader was last working with.
    await expect(discoverOllamaModel()).resolves.toBe("newest");
  });

  it("finds nothing when the library is empty", async () => {
    serve({ models: [] });

    await expect(discoverOllamaModel()).resolves.toBeUndefined();
  });

  it("finds nothing when the server refuses", async () => {
    serve({}, 500);

    await expect(discoverOllamaModel()).resolves.toBeUndefined();
  });

  it("finds nothing when there is no server, rather than failing", async () => {
    vi.spyOn(globalThis, "fetch").mockRejectedValue(new Error("ECONNREFUSED"));

    // A machine with no Ollama on it is the common case, and it must cost the
    // session nothing but the deadline.
    await expect(discoverOllamaModel()).resolves.toBeUndefined();
  });
});

describe("naming a model by the part a reader remembers", () => {
  const serve = (names: ReadonlyArray<string>) =>
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(
        JSON.stringify({
          models: names.map((name, at) => ({
            name,
            modified_at: `2026-0${String(at + 1)}-01T00:00:00Z`,
          })),
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      ),
    );

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("resolves a family name to the installed tag", async () => {
    serve(["qwen3.8:27b-mtp-q8_0"]);

    // An Ollama name carries its size and quantisation after a colon, and a
    // reader names the model they pulled.
    await expect(resolveOllamaModel("qwen3.8")).resolves.toMatchObject({
      model: "qwen3.8:27b-mtp-q8_0",
    });
  });

  it("takes a full name exactly, without reinterpreting it", async () => {
    serve(["qwen3.8:27b-mtp-q8_0", "qwen3.8:7b"]);

    await expect(resolveOllamaModel("qwen3.8:7b")).resolves.toMatchObject({ model: "qwen3.8:7b" });
  });

  it("does not let one family stand for another", async () => {
    serve(["qwen3.85:7b"]);

    // `qwen3.8` means `qwen3.8:…`, and must not mean `qwen3.85:…`.
    const found = await resolveOllamaModel("qwen3.8");
    expect(found.model).not.toBe("qwen3.85:7b");
  });

  it("reports what is installed when nothing matches", async () => {
    serve(["llama3:8b"]);

    await expect(resolveOllamaModel("qwen3.8")).resolves.toEqual({ installed: ["llama3:8b"] });
  });
});

describe("how hard the model is asked to think", () => {
  const seen: unknown[] = [];
  const sourceWithThink = () => {
    const source = new OllamaReplySource({ model: "m" });
    (source as unknown as { client: unknown }).client = {
      chat: async (request: Record<string, unknown>) => {
        seen.push(request["think"]);
        return Object.assign(
          (async function* () {
            yield { message: { content: "x" }, done: true };
          })(),
          { abort: () => {} },
        );
      },
    };
    return source;
  };

  it("sends the level on every request", async () => {
    seen.length = 0;
    const source = sourceWithThink();

    for await (const _ of source.reply("x", new AbortController().signal)) void _;

    expect(seen.at(-1)).toBe("medium");
  });

  it("cycles through the levels Ollama has, and off is not a level", async () => {
    seen.length = 0;
    const source = sourceWithThink();

    expect(source.reasoning.levels).toEqual(["off", "low", "medium", "high"]);

    const sent: unknown[] = [];
    for (let round = 0; round < 4; round += 1) {
      for await (const _ of source.reply("x", new AbortController().signal)) void _;
      sent.push(seen.at(-1));
      source.cycleReasoning();
    }

    // A model asked to think at no level still thinks; the way to stop it is to
    // say not to, which is the boolean.
    expect(sent).toEqual(["medium", "high", false, "low"]);
  });

  it("maps the flag's ladder onto the four rungs Ollama has", () => {
    // The flag is the thread lane's, and has five names.
    expect(new OllamaReplySource({ model: "m", reasoning: "minimal" }).reasoning.level).toBe("off");
    expect(new OllamaReplySource({ model: "m", reasoning: "max" }).reasoning.level).toBe("high");
    expect(new OllamaReplySource({ model: "m", reasoning: "low" }).reasoning.level).toBe("low");
  });
});

describe("what goes back to the model each round", () => {
  it("keeps a long tool result at both ends rather than whole", async () => {
    const long = "A".repeat(3_000) + "MIDDLE" + "B".repeat(3_000);
    const { source, stub } = sourceWith([
      [
        chunk({ content: "", tool_calls: [{ function: { name: "t", arguments: {} } }] }),
        chunk({}, true),
      ],
      [chunk({ content: "done" }, true)],
    ]);
    source.useTools([
      { name: "t", description: "d", parameters: {}, run: () => Promise.resolve(long) },
    ]);

    await collect(source, "go");

    const messages = stub.requests[1]?.["messages"] as ReadonlyArray<Record<string, unknown>>;
    const result = messages.find((message) => message["role"] === "tool");
    const content = String(result?.["content"]);

    // The reader saw all of it; this is what is re-sent on every round after,
    // and a session that re-sends what it has already read spends its wall
    // clock reading it again.
    expect(content.length).toBeLessThan(long.length);
    expect(content).toContain("characters omitted from the middle");
    // Both ends survive, which is what a long output is read for.
    expect(content.startsWith("A")).toBe(true);
    expect(content.endsWith("B")).toBe(true);
  });

  it("leaves an ordinary result alone", async () => {
    const { source, stub } = sourceWith([
      [
        chunk({ content: "", tool_calls: [{ function: { name: "t", arguments: {} } }] }),
        chunk({}, true),
      ],
      [chunk({ content: "done" }, true)],
    ]);
    source.useTools([
      { name: "t", description: "d", parameters: {}, run: () => Promise.resolve("short output") },
    ]);

    await collect(source, "go");

    const messages = stub.requests[1]?.["messages"] as ReadonlyArray<Record<string, unknown>>;
    expect(messages.find((message) => message["role"] === "tool")?.["content"]).toBe(
      "short output",
    );
  });

  it("reports what a turn spent even when it is cut short", async () => {
    const { source } = sourceWith([
      [{ message: {}, done: true, prompt_eval_count: 500, eval_count: 50 } as never],
    ]);
    const controller = new AbortController();
    const chunks: unknown[] = [];

    for await (const piece of source.reply("go", controller.signal)) {
      chunks.push(piece);
      controller.abort();
    }

    // The expensive turns are the ones that get interrupted, and those were
    // recording nothing at all.
    expect(chunks.at(-1)).toMatchObject({ type: "usage" });
  });
});
