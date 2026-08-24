import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

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
});
