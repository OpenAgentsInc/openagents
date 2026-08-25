import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { InMemoryGoalStore } from "../src/coder-goals.js";

import {
  CoderSession,
  DummyReplySource,
  type ReplyChunk,
  type ReplySource,
} from "../src/coder-session.js";

/** A source whose chunks are controlled by the test. */
const source = (chunks: ReadonlyArray<ReplyChunk>, delayMs = 0): ReplySource => ({
  model: "scripted",
  async *reply(_prompt, signal) {
    for (const chunk of chunks) {
      if (signal.aborted) return;
      if (delayMs > 0) await new Promise((resolve) => setTimeout(resolve, delayMs));
      if (signal.aborted) return;
      yield chunk;
    }
  },
});

/** The common case: a source that only produces assistant text. */
const scripted = (chunks: ReadonlyArray<string>, delayMs = 0): ReplySource =>
  source(
    chunks.map((value) => ({ type: "text", value }) as const),
    delayMs,
  );

describe("CoderSession", () => {
  it("appends the prompt and streams the reply into one entry", async () => {
    const session = new CoderSession(scripted(["one ", "two ", "three"]), "repo", "main");
    await session.submit("hello");

    const { entries, turns, running } = session.snapshot();
    expect(entries.map((entry) => entry.role)).toEqual(["you", "assistant"]);
    expect(entries[0]?.text).toBe("hello");
    expect(entries[1]?.text).toBe("one two three");
    expect(entries[1]?.settled).toBe(true);
    expect(turns).toBe(1);
    expect(running).toBe(false);
  });

  it("reports the reply as unsettled while it streams", async () => {
    const session = new CoderSession(scripted(["a", "b"], 5), "repo", "main");
    const settledDuring: boolean[] = [];
    session.onChange(() => {
      const last = session.snapshot().entries.at(-1);
      if (last?.role === "assistant") settledDuring.push(last.settled);
    });

    await session.submit("go");

    // At least one observation while streaming, and the final one settled.
    expect(settledDuring.some((settled) => settled === false)).toBe(true);
    expect(settledDuring.at(-1)).toBe(true);
  });

  it("keeps partial text when interrupted and marks it", async () => {
    const session = new CoderSession(scripted(["a", "b", "c", "d"], 20), "repo", "main");
    const pending = session.submit("go");
    await new Promise((resolve) => setTimeout(resolve, 30));

    expect(session.interrupt()).toBe(true);
    await pending;

    const reply = session.snapshot().entries.at(-1);
    expect(reply?.text).toContain("[interrupted]");
    expect(reply?.text.replace("\n\n[interrupted]", "").length).toBeGreaterThan(0);
    expect(session.snapshot().running).toBe(false);
  });

  it("withdraws the opening entry when the turn produces nothing", async () => {
    const session = new CoderSession(source([]), "repo", "main");
    await session.submit("go");

    // The empty assistant entry is the caret shown while the first chunk is in
    // flight. A turn that never produced one used to settle it empty, and the
    // interface drew a dot with nothing beside it.
    expect(session.snapshot().entries.map((entry) => entry.role)).toEqual(["you"]);
  });

  it("withdraws the opening entry when the turn is interrupted before its first chunk", async () => {
    const session = new CoderSession(scripted(["a"], 40), "repo", "main");
    const pending = session.submit("go");
    await new Promise((resolve) => setTimeout(resolve, 5));

    expect(session.interrupt()).toBe(true);
    await pending;

    expect(session.snapshot().entries.map((entry) => entry.role)).toEqual(["you"]);
  });

  it("keeps a withdrawn placeholder's cost on the entry that is left", async () => {
    const session = new CoderSession(
      source([
        { type: "usage", promptTokens: 12, completionTokens: 0, calls: 1 },
      ] as ReadonlyArray<ReplyChunk>),
      "repo",
      "main",
    );
    await session.submit("go");

    const entries = session.snapshot().entries;
    expect(entries.map((entry) => entry.role)).toEqual(["you"]);
    // Withdrawing the caret must not withdraw what the turn cost with it.
    expect(entries[0]?.metrics?.promptTokens).toBe(12);
  });

  it("queues a prompt typed during a turn, and sends it when the turn ends", async () => {
    const sent: string[] = [];
    const recording: ReplySource = {
      model: "scripted",
      async *reply(prompt) {
        sent.push(prompt);
        await new Promise((resolve) => setTimeout(resolve, 15));
        yield { type: "text", value: "ok" } as const;
      },
    };
    const session = new CoderSession(recording, "repo", "main");

    const first = session.submit("first");
    await session.submit("second");
    await first;
    // The queued turn starts as the first one ends.
    await new Promise((resolve) => setTimeout(resolve, 40));

    // Typing while the model works is how a reader steers. Dropping the key
    // was what made steering impossible.
    expect(sent).toEqual(["first", "second"]);

    const prompts = session
      .snapshot()
      .entries.filter((entry) => entry.role === "you")
      .map((entry) => entry.text);
    // Shown the moment it was typed, not when it was sent, and shown once.
    expect(prompts).toEqual(["first", "second"]);
  });

  it("says a prompt was queued rather than accepting it silently", async () => {
    const session = new CoderSession(scripted(["a"], 15), "repo", "main");
    const first = session.submit("first");
    await session.submit("second");
    await first;

    const notices = session
      .snapshot()
      .entries.filter((entry) => entry.role === "notice")
      .map((entry) => entry.text);
    expect(notices.some((text) => text.startsWith("Queued."))).toBe(true);
  });

  it("ignores an empty prompt", async () => {
    const session = new CoderSession(scripted(["a"]), "repo", "main");
    await session.submit("   ");
    expect(session.snapshot().entries).toHaveLength(0);
  });

  it("reports interrupt as a no-op when nothing is running", () => {
    const session = new CoderSession(scripted(["a"]), "repo", "main");
    expect(session.interrupt()).toBe(false);
  });

  it("makes a tool call its own entry with the name and arguments the source gave", async () => {
    const session = new CoderSession(
      source([
        { type: "tool_call", callId: "c1", name: "repo_grep", arguments: '{"pattern":"x"}' },
        { type: "tool_result", callId: "c1", output: '{"matches":[]}', error: undefined },
      ]),
      "repo",
      "main",
    );
    await session.submit("look");

    const tool = session.snapshot().entries.find((entry) => entry.role === "tool");
    expect(tool?.tool?.name).toBe("repo_grep");
    expect(tool?.tool?.arguments).toBe('{"pattern":"x"}');
    expect(tool?.tool?.output).toBe('{"matches":[]}');
    expect(tool?.tool?.status).toBe("succeeded");
    expect(tool?.settled).toBe(true);
  });

  it("records a failed tool call with the reason rather than an outcome", async () => {
    const session = new CoderSession(
      source([
        { type: "tool_call", callId: "c1", name: "conversation_search", arguments: "{}" },
        { type: "tool_result", callId: "c1", output: undefined, error: "not authorized" },
      ]),
      "repo",
      "main",
    );
    await session.submit("look");

    const tool = session.snapshot().entries.find((entry) => entry.role === "tool");
    expect(tool?.tool?.status).toBe("failed");
    expect(tool?.tool?.error).toBe("not authorized");
  });

  it("splits the text either side of a tool call rather than joining it", async () => {
    const session = new CoderSession(
      source([
        { type: "text", value: "Let me check what is connected:" },
        { type: "tool_call", callId: "c1", name: "repo_list", arguments: "{}" },
        { type: "tool_result", callId: "c1", output: "[]", error: undefined },
        { type: "text", value: "Here is the rundown" },
      ]),
      "repo",
      "main",
    );
    await session.submit("what can you do");

    const entries = session.snapshot().entries;
    expect(entries.map((entry) => entry.role)).toEqual(["you", "assistant", "tool", "assistant"]);
    expect(entries[1]?.text).toBe("Let me check what is connected:");
    expect(entries[3]?.text).toBe("Here is the rundown");
  });

  it("keeps reasoning, a tool call, and text as three entries in arrival order", async () => {
    const session = new CoderSession(
      source([
        { type: "reasoning", value: "I should " },
        { type: "reasoning", value: "check first." },
        { type: "tool_call", callId: "c1", name: "repo_grep", arguments: "{}" },
        { type: "tool_result", callId: "c1", output: "[]", error: undefined },
        { type: "text", value: "Done." },
      ]),
      "repo",
      "main",
    );
    await session.submit("go");

    const entries = session.snapshot().entries;
    expect(entries.map((entry) => entry.role)).toEqual(["you", "reasoning", "tool", "assistant"]);
    expect(entries[1]?.text).toBe("I should check first.");
    // The opening placeholder is withdrawn when the turn starts with a thought,
    // so an empty assistant entry never sits above the reasoning.
    expect(entries.filter((entry) => entry.text.length === 0)).toEqual([]);
  });

  it("marks a tool call the turn never resolved as failed", async () => {
    const session = new CoderSession(
      source([{ type: "tool_call", callId: "c1", name: "repo_grep", arguments: "{}" }]),
      "repo",
      "main",
    );
    await session.submit("go");

    const tool = session.snapshot().entries.find((entry) => entry.role === "tool");
    expect(tool?.tool?.status).toBe("failed");
    expect(tool?.settled).toBe(true);
  });

  it("does not let a renderer mutate the transcript through its snapshot", async () => {
    const session = new CoderSession(
      source([{ type: "tool_call", callId: "c1", name: "repo_grep", arguments: "{}" }]),
      "repo",
      "main",
    );
    await session.submit("go");

    const taken = session.snapshot().entries.find((entry) => entry.role === "tool");
    if (taken?.tool !== undefined) taken.tool.output = "changed";
    const again = session.snapshot().entries.find((entry) => entry.role === "tool");
    expect(again?.tool?.output).toBeUndefined();
  });

  it("counts the turn while it is streaming rather than only once it settles", async () => {
    const session = new CoderSession(scripted(["a", "b", "c"], 15), "repo", "main");
    const seen: number[] = [];
    session.onChange(() => {
      if (session.snapshot().running) seen.push(session.snapshot().turns);
    });

    await session.submit("go");

    // A reply the reader can watch arrive is a turn that has happened.
    expect(seen.length).toBeGreaterThan(0);
    expect(seen.every((turns) => turns === 1)).toBe(true);
    expect(session.snapshot().turns).toBe(1);
  });

  it("counts a queued prompt only when its turn starts", async () => {
    const session = new CoderSession(scripted(["a"], 15), "repo", "main");
    await session.submit("   ");
    expect(session.snapshot().turns).toBe(0);

    const first = session.submit("first");
    await session.submit("second while the first runs");
    // Waiting to be sent is not a turn in flight.
    expect(session.snapshot().turns).toBe(1);

    await first;
    await new Promise((resolve) => setTimeout(resolve, 40));
    expect(session.snapshot().turns).toBe(2);
  });

  it("carries workspace and model into the snapshot for the status line", () => {
    const session = new CoderSession(new DummyReplySource(), "openagents", "main");
    const snapshot = session.snapshot();
    expect(snapshot.repository).toBe("openagents");
    expect(snapshot.branch).toBe("main");
    expect(snapshot.model).toContain("dummy");
  });

  it("says plainly that the dummy reply is not an agent", async () => {
    const session = new CoderSession(new DummyReplySource(0), "repo", "main");
    await session.submit("what does this repository do");

    const reply = session.snapshot().entries.at(-1)?.text ?? "";
    expect(reply).toContain("dummy reply");
    expect(reply).toContain("what does this repository do");
  });

  it("exercises every entry kind offline, so --offline can prove the rendering", async () => {
    const session = new CoderSession(new DummyReplySource(0), "repo", "main");
    await session.submit("what can you do");

    const roles = new Set(session.snapshot().entries.map((entry) => entry.role));
    expect(roles).toEqual(new Set(["you", "reasoning", "tool", "assistant"]));
  });

  describe("switching backend", () => {
    const noop = () => {};

    /** A source that can switch, and records how often it was asked to. */
    const switchable = (hold: Promise<void>): ReplySource & { switches: number } => {
      const state = {
        switches: 0,
        model: "First",
        cycleBackend() {
          state.switches += 1;
          state.model = `Model ${state.switches}`;
          return state.model;
        },
        async *reply() {
          await hold;
          yield { type: "text", value: "done" } as const;
        },
      };
      return state;
    };

    it("a source with one backend offers no switch", () => {
      const session = new CoderSession(new DummyReplySource(0), "repo", "main");

      expect(session.canCycleBackend).toBe(false);
      expect(session.cycleBackend()).toEqual({ switched: false, label: undefined });
    });

    it("switches while idle and names the new model", () => {
      const session = new CoderSession(switchable(Promise.resolve()), "repo", "main");

      expect(session.canCycleBackend).toBe(true);
      expect(session.cycleBackend()).toEqual({ switched: true, label: "Model 1" });
      expect(session.snapshot().model).toBe("Model 1");
      expect(session.snapshot().entries.at(-1)?.text).toBe("Model switched to Model 1.");
    });

    it("refuses while a turn is running, and says why", async () => {
      let release: () => void = noop;
      const hold = new Promise<void>((resolve) => {
        release = resolve;
      });
      const source = switchable(hold);
      const session = new CoderSession(source, "repo", "main");

      const running = session.submit("go");
      expect(session.running).toBe(true);

      const result = session.cycleBackend();

      // The turn on screen was submitted against the model the status line
      // names, so the label must not move out from under it.
      expect(result.switched).toBe(false);
      expect(source.switches).toBe(0);
      expect(session.snapshot().model).toBe("First");
      expect(session.snapshot().entries.at(-1)?.text).toContain("on the next turn");

      release();
      await running;

      // And it works again the moment the turn is over.
      expect(session.cycleBackend().switched).toBe(true);
    });
  });
});

describe("the /system command", () => {
  /** A source that records whether the model was reached. */
  const watched = (context?: string): ReplySource & { prompts: string[] } => {
    const prompts: string[] = [];
    return {
      model: "scripted",
      prompts,
      ...(context === undefined ? {} : { describeContext: () => context }),
      // eslint-disable-next-line require-yield -- a turn that must not happen
      async *reply(prompt) {
        prompts.push(prompt);
      },
    };
  };

  it("shows the source's context as a notice and never reaches the model", async () => {
    const reply = watched("System message sent with every turn:\n\nYou are `openagents coder`.");
    const session = new CoderSession(reply, "repo", "main");

    await session.submit("/system");

    const { entries, turns } = session.snapshot();
    // A notice, so the interface dims it: what the model was told is not
    // something the model said.
    expect(entries.map((entry) => entry.role)).toEqual(["you", "notice"]);
    expect(entries[1]?.text).toContain("You are `openagents coder`.");
    // Reading what the model was told must not change what the model was told.
    expect(reply.prompts).toEqual([]);
    expect(turns).toBe(0);
  });

  it("says so when the source composes no context of its own", async () => {
    const reply = watched();
    const session = new CoderSession(reply, "repo", "main");

    await session.submit("/system");

    expect(session.snapshot().entries[1]?.text).toContain("composes no context of its own");
    expect(reply.prompts).toEqual([]);
  });

  it("is the whole line, so a question about the system prompt is still a turn", async () => {
    const reply = watched("context");
    const session = new CoderSession(reply, "repo", "main");

    await session.submit("what is in your /system prompt");

    expect(reply.prompts).toEqual(["what is in your /system prompt"]);
  });
});

describe("the /export command", () => {
  it("writes the conversation and never reaches the model", async () => {
    const prompts: string[] = [];
    const reply: ReplySource = {
      model: "scripted",
      // eslint-disable-next-line require-yield -- a turn that must not happen
      async *reply(prompt) {
        prompts.push(prompt);
      },
    };
    // Its own directory, and no clipboard: a test that writes where a reader
    // exports, and takes their clipboard, changes the machine it is checking.
    const directory = mkdtempSync(join(tmpdir(), "coder-session-export-"));
    const session = new CoderSession(reply, "repo", "main", undefined, undefined, { directory });

    await session.submit("/export");

    const { entries, turns } = session.snapshot();
    expect(entries.map((entry) => entry.role)).toEqual(["you", "notice"]);
    expect(entries[1]?.text).toContain("as ATIF to");
    expect(prompts).toEqual([]);
    expect(turns).toBe(0);

    // Written into the test's own directory, so there is nothing to clean out
    // of the reader's.
    expect(entries[1]?.text).toContain(directory);
  });
});

describe("a turn's cost", () => {
  it("lands on the entry the turn ended on", async () => {
    const session = new CoderSession(
      source([
        { type: "text", value: "answer" },
        { type: "usage", promptTokens: 12, completionTokens: 34, calls: 2 },
      ]),
      "repo",
      "main",
    );

    await session.submit("go");

    const assistant = session.snapshot().entries.find((entry) => entry.role === "assistant");
    expect(assistant?.metrics).toEqual({ promptTokens: 12, completionTokens: 34, calls: 2 });
  });

  it("lands with a cache read split when the source reports one", async () => {
    const session = new CoderSession(
      source([
        { type: "text", value: "answer" },
        {
          type: "usage",
          promptTokens: 12,
          completionTokens: 34,
          calls: 2,
          cacheReadInputTokens: 5,
        },
      ]),
      "repo",
      "main",
    );

    await session.submit("go");

    const assistant = session.snapshot().entries.find((entry) => entry.role === "assistant");
    expect(assistant?.metrics).toEqual({
      promptTokens: 12,
      completionTokens: 34,
      calls: 2,
      cacheReadInputTokens: 5,
    });
  });
});

describe("notices that replace one another", () => {
  it("keeps the last of a run about the same setting, and only the last", async () => {
    const session = new CoderSession(scripted(["a"]), "repo", "main");

    session.notice("Reasoning set to low.", "reasoning");
    session.notice("Reasoning set to high.", "reasoning");
    session.notice("Reasoning set to off.", "reasoning");

    expect(session.snapshot().entries.map((entry) => entry.text)).toEqual([
      "Reasoning set to off.",
    ]);
  });

  it("does not fold two notices that are about different things", async () => {
    const session = new CoderSession(scripted(["a"]), "repo", "main");

    session.notice("Reasoning set to low.", "reasoning");
    session.notice("Model switched to Ox Alpha.", "model");
    session.notice("Reasoning set to high.", "reasoning");

    // The model notice separates them, so the second reasoning notice is a new
    // one rather than a replacement: what it replaced is no longer the last
    // thing said.
    expect(session.snapshot().entries).toHaveLength(3);
  });

  it("leaves an ordinary notice alone", async () => {
    const session = new CoderSession(scripted(["a"]), "repo", "main");

    session.notice("Interrupted.");
    session.notice("Interrupted.");

    // Two interruptions are two events, not one restated.
    expect(session.snapshot().entries).toHaveLength(2);
  });
});

describe("plugin occurrences", () => {
  const digest = `sha256:${"ab".repeat(32)}`;

  const loaded = {
    message: "Loaded plugin `word_stats` v0.1.0.",
    event: "plugin_loaded" as const,
    plugin: {
      name: "word_stats",
      version: "0.1.0",
      artifactDigest: digest,
      bytes: 1024,
      abi: { entry: "handle_packet", alloc: "packet_alloc" },
      timeoutMs: 2000,
      capabilities: { mounts: [], hosts: [] },
      manifestPath: "/work/demo/plugin.json",
      toolName: "word_stats",
    },
  };

  it("records loads and refusals on the snapshot, not the transcript", () => {
    const session = new CoderSession(scripted(["a"]), "repo", "main");

    session.recordPluginEvent({
      message: "Plugin not loaded (digest_mismatch): wrong artifact.",
      event: "plugin_load_refused",
      code: "digest_mismatch",
      plugin: { manifestPath: "/work/demo/plugin.json" },
    });
    session.recordPluginEvent(loaded);

    const { entries, pluginEvents } = session.snapshot();
    // A renderer draws entries; the occurrences are the export's to read.
    expect(entries).toHaveLength(0);
    expect(pluginEvents?.map((event) => event.event)).toEqual([
      "plugin_load_refused",
      "plugin_loaded",
    ]);
    expect(pluginEvents?.[1]?.plugin.artifactDigest).toBe(digest);
    expect(pluginEvents?.[0]?.at).toBeGreaterThan(0);
  });

  it("stamps a call of the loaded tool with the plugin's identity", async () => {
    const session = new CoderSession(
      source([
        { type: "tool_call", callId: "c1", name: "word_stats", arguments: "{}" },
        { type: "tool_result", callId: "c1", output: "{}", error: undefined },
        { type: "tool_call", callId: "c2", name: "shell", arguments: "{}" },
        { type: "tool_result", callId: "c2", output: "", error: undefined },
      ]),
      "repo",
      "main",
    );
    session.recordPluginEvent(loaded);

    await session.submit("count");

    const tools = session.snapshot().entries.filter((entry) => entry.role === "tool");
    expect(tools[0]?.tool?.plugin).toEqual({
      name: "word_stats",
      version: "0.1.0",
      artifactDigest: digest,
    });
    // A tool no plugin backs carries no provenance to mistake for some.
    expect(tools[1]?.tool?.plugin).toBeUndefined();
  });

  it("does not register a tool from a refused load", async () => {
    const session = new CoderSession(
      source([
        { type: "tool_call", callId: "c1", name: "word_stats", arguments: "{}" },
        { type: "tool_result", callId: "c1", output: "{}", error: undefined },
      ]),
      "repo",
      "main",
    );
    session.recordPluginEvent({
      message: "Plugin not loaded (manifest_unreadable): no such file.",
      event: "plugin_load_refused",
      code: "manifest_unreadable",
      plugin: { manifestPath: "/work/demo/plugin.json" },
    });

    await session.submit("count");

    const tool = session.snapshot().entries.find((entry) => entry.role === "tool");
    expect(tool?.tool?.plugin).toBeUndefined();
  });
});

describe("the /goal command in CoderSession", () => {
  it("handles /goal lifecycle via session prompts", async () => {
    const store = new InMemoryGoalStore();
    const session = new CoderSession(
      scripted(["hello"]),
      "repo",
      "main",
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      store,
    );

    // Initial status notice
    await session.submit("/goal");
    const snap1 = session.snapshot();
    const notice1 = snap1.entries.find((e) => e.role === "notice");
    expect(notice1?.text).toContain("No active task goal.");

    // Set goal
    await session.submit("/goal Build persistent task goals");
    expect(store.getGoal()?.objective).toBe("Build persistent task goals");
    expect(store.getGoal()?.status).toBe("active");
    expect(session.snapshot().goal?.objective).toBe("Build persistent task goals");

    // Pause goal
    await session.submit("/goal pause");
    expect(store.getGoal()?.status).toBe("paused");

    // Resume goal
    await session.submit("/goal resume");
    expect(store.getGoal()?.status).toBe("active");

    // Clear goal
    await session.submit("/goal clear");
    expect(store.getGoal()).toBeUndefined();
    expect(session.snapshot().goal).toBeUndefined();
  });
});
