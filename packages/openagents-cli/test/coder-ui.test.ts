import { EventEmitter } from "node:events";
import { describe, expect, it } from "vitest";

import { CODER_BACKENDS, defaultBackend } from "../src/coder-backends.js";
import { CoderSession, type ReplyChunk, type ReplySource } from "../src/coder-session.js";
import { runCoderUi } from "../src/coder-ui.js";

/** A writable that records what the interface painted. */
class FakeOut extends EventEmitter {
  columns = 100;
  rows = 24;
  written = "";
  write(text: string): boolean {
    this.written += text;
    return true;
  }
}

/** A readable TTY the test can push keystrokes into. */
class FakeIn extends EventEmitter {
  isTTY = true;
  setRawMode(): this {
    return this;
  }
  resume(): this {
    return this;
  }
  pause(): this {
    return this;
  }
  setEncoding(): this {
    return this;
  }
}

const source = (chunks: ReadonlyArray<ReplyChunk>): ReplySource => ({
  model: "scripted",
  async *reply() {
    for (const chunk of chunks) yield chunk;
  },
});

/** A source with backends to cycle, which is what makes Tab mean anything. */
const switchable = (chunks: ReadonlyArray<ReplyChunk>): ReplySource => {
  let index = 0;
  return {
    get model() {
      return CODER_BACKENDS[index]?.label ?? "none";
    },
    cycleBackend() {
      index = (index + 1) % CODER_BACKENDS.length;
      return CODER_BACKENDS[index]?.label ?? "none";
    },
    async *reply() {
      for (const chunk of chunks) yield chunk;
    },
  };
};

/**
 * Replay the painted rows.
 *
 * Every row the interface writes is positioned absolutely and erased to the
 * end of the line first, so the last write to a row is what that row shows.
 */
function screen(written: string): ReadonlyArray<string> {
  const rows: string[] = [];
  const positions = /\x1b\[(\d+);(\d+)H(\x1b\[K)?/g;
  let row: number | undefined;
  let from = 0;
  const flush = (end: number) => {
    if (row === undefined) return;
    rows[row - 1] = written
      .slice(from, end)
      .replace(/\x1b\[[0-9;]*m/g, "")
      .trimEnd();
  };
  for (let match = positions.exec(written); match !== null; match = positions.exec(written)) {
    flush(match.index);
    row = match[3] === undefined ? undefined : Number(match[1]);
    from = positions.lastIndex;
  }
  flush(written.length);
  return rows;
}

const drive = async (
  chunks: ReadonlyArray<ReplyChunk>,
  prompt = "go",
  from: ReplySource = source(chunks),
) => {
  const stdin = new FakeIn();
  const stdout = new FakeOut();
  const session = new CoderSession(from, "repo", "main");
  const running = runCoderUi(session, {
    stdin: stdin as unknown as NodeJS.ReadStream,
    stdout: stdout as unknown as NodeJS.WriteStream,
  });

  await session.submit(prompt);
  const painted = stdout.written;
  stdin.emit("data", "\x04");
  await running;
  return { painted, rows: screen(painted) };
};

describe("runCoderUi", () => {
  it("never clears the screen and never writes a newline", async () => {
    const { painted } = await drive([{ type: "text", value: "hello" }]);
    // Both are how a frame reaches the terminal's own scrollback.
    expect(painted).not.toContain("\x1b[2J");
    expect(painted).not.toContain("\n");
    expect(painted).toContain("\x1b[?1049h");
  });

  it("erases each row it repaints rather than clearing the screen", async () => {
    const { painted } = await drive([{ type: "text", value: "hello" }]);
    expect(/\x1b\[\d+;1H\x1b\[K/.test(painted)).toBe(true);
  });

  it("puts a blank row on both sides of a tool call", async () => {
    const { rows } = await drive([
      { type: "text", value: "Let me check what is connected:" },
      { type: "tool_call", callId: "c1", name: "repo_grep", arguments: '{"pattern":"x"}' },
      { type: "tool_result", callId: "c1", output: "{}", error: undefined },
      { type: "text", value: "Here is the rundown" },
    ]);

    const tool = rows.findIndex((row) => row.includes("repo_grep"));
    expect(tool).toBeGreaterThan(0);
    expect(rows[tool - 1]).toBe("");
    const after = rows.findIndex((row) => row.includes("Here is the rundown"));
    expect(rows[after - 1]).toBe("");
    // The sentences either side of the call are on different rows, which is
    // the defect: they used to be appended to one another.
    expect(rows.join("\n")).not.toContain("connected:Here");
  });

  it("shows the tool name, its arguments, and its outcome", async () => {
    const { rows } = await drive([
      { type: "tool_call", callId: "c1", name: "repo_grep", arguments: '{"pattern":"x"}' },
      { type: "tool_result", callId: "c1", output: '{"matches":[]}', error: undefined },
    ]);
    const text = rows.join("\n");
    expect(text).toContain("repo_grep");
    expect(text).toContain('{"pattern":"x"}');
    expect(text).toContain('{"matches":[]}');
  });

  it("renders assistant Markdown rather than its source", async () => {
    const { painted, rows } = await drive([
      { type: "text", value: "hello **ox-alpha** and `code`" },
    ]);
    expect(rows.join("\n")).toContain("hello ox-alpha and code");
    expect(painted).toContain("\x1b[1mox-alpha\x1b[0m");
  });

  it("streams reasoning dim and italic, above the text of the same turn", async () => {
    const { painted, rows } = await drive([
      { type: "reasoning", value: "I should check first." },
      { type: "text", value: "Done." },
    ]);

    const thought = rows.findIndex((row) => row.includes("I should check first."));
    const answer = rows.findIndex((row) => row.includes("Done."));
    expect(thought).toBeGreaterThanOrEqual(0);
    expect(answer).toBeGreaterThan(thought);
    expect(rows[answer - 1]).toBe("");
    expect(painted).toContain("\x1b[2m\x1b[3mI should check first.\x1b[0m");
  });

  it("counts the streaming turn, so the bar never reads zero under a live reply", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    let release = () => {};
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    const paused: ReplySource = {
      model: "scripted",
      async *reply() {
        yield { type: "text", value: "still arriving" } as const;
        await held;
      },
    };

    const session = new CoderSession(paused, "repo", "main");
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });
    const turn = session.submit("go");
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(session.snapshot().running).toBe(true);
    const bar = screen(stdout.written).at(-1) ?? "";
    expect(bar).toContain("1 reply this run");
    expect(bar).not.toContain("0 replies");

    release();
    await turn;
    stdin.emit("data", "\x04");
    await running;
  });

  it("labels the count as this run's, because the conversation is not", async () => {
    const { rows } = await drive([{ type: "text", value: "hello" }]);
    expect(rows.at(-1)).toContain("1 reply this run");
  });

  it("says nothing about scope when the source keeps its turns to itself", async () => {
    const { rows } = await drive([{ type: "text", value: "hello" }]);
    expect(rows.join("\n")).not.toContain("shared with");
  });

  it("offers no key in the bottom bar that does nothing in that state", async () => {
    const { rows } = await drive([{ type: "text", value: "hello" }]);
    const bar = rows.at(-1) ?? "";
    expect(bar).toContain("enter to send");
    expect(bar).toContain("ctrl+d to quit");
    // There is nothing to interrupt while the session is idle.
    expect(bar).not.toContain("interrupt");
    // And nothing to switch to, because this source has one backend.
    expect(bar).not.toContain("tab to switch");
  });

  describe("switching backend with tab", () => {
    const driveSwitchable = async (keys: ReadonlyArray<string>) => {
      const stdin = new FakeIn();
      const stdout = new FakeOut();
      const session = new CoderSession(
        switchable([{ type: "text", value: "hello" }]),
        "repo",
        "main",
      );
      const running = runCoderUi(session, {
        stdin: stdin as unknown as NodeJS.ReadStream,
        stdout: stdout as unknown as NodeJS.WriteStream,
      });

      await session.submit("go");
      for (const key of keys) stdin.emit("data", key);
      const painted = stdout.written;
      stdin.emit("data", "\x04");
      await running;
      return { painted, rows: screen(painted), session };
    };

    it("names the key only where there is another backend to reach", async () => {
      const { rows } = await driveSwitchable([]);
      expect(rows.at(-1) ?? "").toContain("tab to switch model");
    });

    it("moves to the next backend and says so", async () => {
      const { session, rows } = await driveSwitchable(["\t"]);

      const expected = CODER_BACKENDS[1]?.label ?? "";
      expect(session.snapshot().model).toBe(expected);
      expect(rows.join("\n")).toContain(`Model switched to ${expected}.`);
    });

    it("wraps around, so every backend is reachable from one key", async () => {
      const { session } = await driveSwitchable(CODER_BACKENDS.map(() => "\t"));
      expect(session.snapshot().model).toBe(defaultBackend().label);
    });

    it("does not leave a tab in the composer", async () => {
      const { rows } = await driveSwitchable(["\t"]);
      const composer = rows.find((row) => row.includes(">")) ?? "";
      expect(composer).not.toContain("\t");
    });
  });

  it("puts the thread's remaining budget in the status row beside the model", async () => {
    const metered: ReplySource = { ...source([{ type: "text", value: "hi" }]) };
    Object.defineProperty(metered, "budget", { get: () => "255 calls · 1.0M tok · $2.00" });

    const { rows } = await drive([{ type: "text", value: "hi" }], "go", metered);
    const status = rows.find((row) => row.includes("repo · main"));

    expect(status).toContain("scripted · 255 calls · 1.0M tok · $2.00");
  });

  it("says only how long a turn has run, never that it is streaming", async () => {
    // The inference proxy builds the whole body and sends it once, so a status
    // line that claimed streaming would be describing something the reader can
    // see is not happening.
    const { rows } = await drive([{ type: "text", value: "hi" }]);
    expect(rows.some((row) => row.includes("streaming"))).toBe(false);
  });

  it("drops the repository before the budget when the row will not fit", async () => {
    const metered: ReplySource = { ...source([{ type: "text", value: "hi" }]) };
    Object.defineProperty(metered, "budget", { get: () => "255 calls · 1.0M tok · $2.00" });

    const stdin = new FakeIn();
    const stdout = new FakeOut();
    stdout.columns = 56;
    const session = new CoderSession(metered, "a-long-repository-name", "main");
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });
    await session.submit("go");
    const rows = screen(stdout.written);
    stdin.emit("data", "\x04");
    await running;

    const status = rows.find((row) => row.includes("calls"));
    expect(status).not.toContain("a-long-repository-name");
    expect(status).toContain("$2.00");
  });
});
