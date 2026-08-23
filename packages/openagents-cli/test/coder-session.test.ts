import { describe, expect, it } from "vitest";

import { CoderSession, DummyReplySource, type ReplySource } from "../src/coder-session.js";

/** A source whose chunks are controlled by the test. */
const scripted = (chunks: ReadonlyArray<string>, delayMs = 0): ReplySource => ({
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

  it("refuses a second prompt while one is running rather than queueing it", async () => {
    const session = new CoderSession(scripted(["a", "b"], 15), "repo", "main");
    const first = session.submit("first");
    await session.submit("second");
    await first;

    const prompts = session
      .snapshot()
      .entries.filter((entry) => entry.role === "you")
      .map((entry) => entry.text);
    expect(prompts).toEqual(["first"]);
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
});
