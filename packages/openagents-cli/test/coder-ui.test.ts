import { EventEmitter } from "node:events";
import { describe, expect, it } from "vitest";

import { CODER_BACKENDS } from "../src/coder-backends.js";
import { CoderSession, type ReplyChunk, type ReplySource } from "../src/coder-session.js";
import { CoderTaskRegistry } from "../src/coder-tasks.js";
import { RELOAD_EXIT_CODE } from "../src/coder-reload.js";
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
      expect(session.snapshot().model).toBe(CODER_BACKENDS[0]?.label);
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

  it("keeps the stop key on a narrow row that has run out of room", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    stdout.columns = 80;
    const registry = new CoderTaskRegistry();
    const session = new CoderSession(
      source([
        {
          type: "text",
          value: Array.from({ length: 40 }, (_, index) => `line ${index}`).join("\n"),
        },
      ]),
      "repo",
      "main",
      {
        registry,
        fleet: {
          submit: async () => ({ status: "refused", code: "empty_prompt", reason: "not used" }),
        },
        label: "fake (fake/model)",
      },
    );
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    // A transcript long enough to scroll grows the counter on the right, which
    // is what used to push every hint off an eighty-column row.
    await session.submit("go");
    const task = registry.register(
      {
        id: "d1",
        description: "run the long build",
        prompt: "build",
        agent: "opencode",
        model: "fake/model",
        cwd: "/tmp",
        background: true,
      },
      0,
    );
    registry.start(task.id, new AbortController());
    await new Promise((resolve) => setTimeout(resolve, 20));
    const rows = screen(stdout.written);
    stdin.emit("data", "\x04");
    await running;
    session.close();

    expect(rows.at(-1) ?? "").toContain("ctrl+x to stop agents");
  });

  it("keeps a long typed line inside the row, showing its tail", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(source([]), "repo", "main");
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    // A row written past the last column is wrapped by the terminal, which
    // shifts every row below it and leaves text nothing will erase.
    stdin.emit("data", `${"a".repeat(120)}END`);
    const rows = screen(stdout.written);
    // Ctrl+D quits only from an empty composer, so clear it first.
    stdin.emit("data", "\x1b");
    await new Promise((resolve) => setTimeout(resolve, 60));
    stdin.emit("data", "\x04");
    await running;

    const composer = rows.find((row) => row.startsWith("  ›")) ?? "";
    expect([...composer].length).toBeLessThanOrEqual(stdout.columns);
    expect(composer.endsWith("END")).toBe(true);
    expect(composer).toContain("…");
  });
});

describe("the /skills screen", () => {
  const skill = (name: string, description: string) => ({
    name,
    description,
    body: "Body.",
    path: `/tmp/${name}/SKILL.md`,
  });

  /** A selection over two skills, recording what was switched. */
  const selection = () => {
    const off = new Set<string>();
    const all = [skill("alpha", "The first skill."), skill("beta", "The second skill.")];
    return {
      all,
      isOn: (name: string) => !off.has(name),
      toggle: (name: string) => {
        const on = off.has(name);
        if (on) off.delete(name);
        else off.add(name);
        return on;
      },
      active: () => all.filter((candidate) => !off.has(candidate.name)),
    };
  };

  const open = async (skills: ReturnType<typeof selection>) => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(source([]), "repo", "main");
    let declared = 0;
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
      skills,
      onSkillsChanged: () => {
        declared += 1;
      },
    });

    // Typed the way a reader types it, then entered.
    stdin.emit("data", "/skills");
    stdin.emit("data", "\r");

    return {
      stdin,
      stdout,
      session,
      running,
      declarations: () => declared,
      rows: () => screen(stdout.written),
      close: async () => {
        stdin.emit("data", "\x1b");
        stdin.emit("data", "\x04");
        await running;
      },
    };
  };

  it("lists every skill with its state, and describes the row in hand", async () => {
    const screenUnderTest = await open(selection());
    const rows = screenUnderTest.rows().join("\n");

    expect(rows).toContain("Skills");
    expect(rows).toContain("[on]  alpha");
    expect(rows).toContain("[on]  beta");
    // The description of the focused row only: eight at once is the wall of
    // text the catalog exists to avoid.
    expect(rows).toContain("The first skill.");
    expect(rows).not.toContain("The second skill.");

    await screenUnderTest.close();
  });

  it("moves the focus with the arrow keys", async () => {
    const screenUnderTest = await open(selection());

    screenUnderTest.stdin.emit("data", "\x1b[B");
    const rows = screenUnderTest.rows().join("\n");

    expect(rows).toContain("The second skill.");

    await screenUnderTest.close();
  });

  it("switches the focused skill with space and re-declares the tools", async () => {
    const skills = selection();
    const screenUnderTest = await open(skills);

    screenUnderTest.stdin.emit("data", " ");

    expect(skills.isOn("alpha")).toBe(false);
    expect(skills.active().map((candidate) => candidate.name)).toEqual(["beta"]);
    // Re-declared at the keystroke, so what the model is told matches the
    // screen the moment it is left.
    expect(screenUnderTest.declarations()).toBe(1);
    expect(screenUnderTest.rows().join("\n")).toContain("[off] alpha");

    await screenUnderTest.close();
  });

  it("sends nothing to the model, on the way in or out", async () => {
    const skills = selection();
    const screenUnderTest = await open(skills);

    screenUnderTest.stdin.emit("data", " ");
    screenUnderTest.stdin.emit("data", "\x1b");

    // `/skills` is a screen, not a turn: no prompt was sent and none was
    // recorded as one.
    expect(screenUnderTest.session.snapshot().turns).toBe(0);
    expect(
      screenUnderTest.session.snapshot().entries.filter((entry) => entry.role === "you"),
    ).toEqual([]);

    screenUnderTest.stdin.emit("data", "\x04");
    await screenUnderTest.running;
  });

  it("holds the keyboard, so typing does not reach the composer behind it", async () => {
    const screenUnderTest = await open(selection());

    screenUnderTest.stdin.emit("data", "hello");
    screenUnderTest.stdin.emit("data", "\x1b");
    const rows = screenUnderTest.rows().join("\n");

    // The letters went nowhere: a screen the reader cannot see must not be
    // collecting what they type.
    expect(rows).not.toContain("hello");

    screenUnderTest.stdin.emit("data", "\x04");
    await screenUnderTest.running;
  });


  it("returns on a lone escape, the way a terminal sends one", async () => {
    const screenUnderTest = await open(selection());
    expect(screenUnderTest.rows().join("\n")).toContain("Skills");

    // One escape byte and nothing after it. The interface holds a bare escape
    // for its window in case a sequence follows, so this only leaves the screen
    // once the window has passed -- which is the case a test that sends another
    // key immediately never exercises, and the one every terminal sends.
    screenUnderTest.stdin.emit("data", "\x1b");
    await new Promise((resolve) => setTimeout(resolve, 80));

    expect(screenUnderTest.rows().join("\n")).not.toContain("space toggles");

    screenUnderTest.stdin.emit("data", "\x04");
    await screenUnderTest.running;
  });

  it("returns on ctrl+c without ending the session", async () => {
    const screenUnderTest = await open(selection());

    screenUnderTest.stdin.emit("data", "\x03");

    expect(screenUnderTest.rows().join("\n")).not.toContain("space toggles");

    screenUnderTest.stdin.emit("data", "\x04");
    await screenUnderTest.running;
  });

  it("says so when the workspace has no skills", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(source([]), "repo", "main");
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    stdin.emit("data", "/skills");
    stdin.emit("data", "\r");

    expect(screen(stdout.written).join("\n")).toContain("No skills were found");

    stdin.emit("data", "\x1b");
    stdin.emit("data", "\x04");
    await running;
  });
});

describe("the /reload command", () => {
  it("asks the runner to restart, and sends nothing to the model", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const prompts: string[] = [];
    const session = new CoderSession(
      {
        model: "scripted",
        // eslint-disable-next-line require-yield -- a turn that must not happen
        async *reply(prompt: string) {
          prompts.push(prompt);
        },
      },
      "repo",
      "main",
    );
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    stdin.emit("data", "/reload");
    stdin.emit("data", "\r");

    // These tests run from a source checkout, so the command applies: the
    // interface exits with the code its runner rebuilds on.
    await expect(running).resolves.toBe(RELOAD_EXIT_CODE);
    expect(prompts).toEqual([]);
    expect(session.snapshot().turns).toBe(0);
  });
});

describe("typing while a turn is running", () => {
  const held = (): { source: ReplySource; release: () => void; sent: string[] } => {
    const sent: string[] = [];
    let release = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    return {
      sent,
      release: () => release(),
      source: {
        model: "scripted",
        async *reply(prompt: string) {
          sent.push(prompt);
          yield { type: "text", value: "working" } as const;
          await gate;
        },
      },
    };
  };

  it("runs an interface command at once rather than dropping the key", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const { source: paused, release } = held();
    const session = new CoderSession(paused, "repo", "main");
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    const turn = session.submit("go");
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(session.snapshot().running).toBe(true);

    // `/export` was impossible mid-turn: enter was ignored while running.
    stdin.emit("data", "/system");
    stdin.emit("data", "\r");

    const notices = session
      .snapshot()
      .entries.filter((entry) => entry.role === "notice")
      .map((entry) => entry.text);
    expect(notices.some((text) => text.includes("standing context") || text.length > 0)).toBe(true);

    release();
    await turn;
    stdin.emit("data", "\x04");
    await running;
  });


  it("steers on enter and queues on shift+enter", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const modes: string[] = [];
    const { source: paused, release } = held();
    const session = new CoderSession(paused, "repo", "main");
    const realSubmit = session.submit.bind(session);
    session.submit = (prompt: string, mode?: "steer" | "queue") => {
      modes.push(mode ?? "steer");
      return realSubmit(prompt, mode);
    };
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    const turn = session.submit("go");
    await new Promise((resolve) => setTimeout(resolve, 0));

    stdin.emit("data", "one");
    stdin.emit("data", "\r");
    // Shift+enter, as the keyboard protocol reports it: key 13, modifier 2.
    stdin.emit("data", "two");
    stdin.emit("data", "\x1b[13;2u");
    // And as the terminals that do not speak the protocol send it.
    stdin.emit("data", "three");
    stdin.emit("data", "\x1b\r");

    expect(modes).toEqual(["steer", "steer", "queue", "queue"]);

    release();
    await turn;
    stdin.emit("data", "\x04");
    await running;
  });

  it("asks the terminal to tell enter and shift+enter apart, and stops asking on the way out", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(source([]), "repo", "main");
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    // Without this they arrive as the same carriage return and cannot be told
    // apart at all.
    expect(stdout.written).toContain("\x1b[>1u");

    stdin.emit("data", "\x04");
    await running;

    // Left as it was found: a terminal still reporting this after the session
    // has gone is a terminal the next program has to cope with.
    expect(stdout.written).toContain("\x1b[<u");
  });

  it("queues ordinary text and sends it when the turn ends", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const { source: paused, release, sent } = held();
    const session = new CoderSession(paused, "repo", "main");
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    const turn = session.submit("go");
    await new Promise((resolve) => setTimeout(resolve, 0));

    stdin.emit("data", "steer me");
    stdin.emit("data", "\r");

    // Shown at once, sent later: a reader sees what they said when they said it.
    expect(
      session
        .snapshot()
        .entries.filter((entry) => entry.role === "you")
        .map((entry) => entry.text),
    ).toEqual(["go", "steer me"]);
    expect(sent).toEqual(["go"]);

    release();
    await turn;
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(sent).toEqual(["go", "steer me"]);

    stdin.emit("data", "\x04");
    await running;
  });
});

describe("reasoning in the transcript", () => {
  it("renders the Markdown a model writes in it", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(
      source([{ type: "reasoning", value: "1. **#160** is the bug" }]),
      "repo",
      "main",
    );
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    await session.submit("go");
    const rows = screen(stdout.written).join("\n");

    // Models write `**bold**` and numbered lists in their reasoning, and
    // unrendered markup is harder to read than rendered markup in any style.
    expect(rows).toContain("#160 is the bug");
    expect(rows).not.toContain("**#160**");

    stdin.emit("data", "\x04");
    await running;
  });
});

describe("changing how hard the model thinks", () => {
  const thinking = () => {
    let at = 0;
    const levels = ["off", "low", "medium", "high"] as const;
    return {
      model: "scripted",
      get reasoning() {
        return { level: levels[at] ?? "medium", levels };
      },
      cycleReasoning() {
        at = (at + 1) % levels.length;
        return levels[at] ?? "medium";
      },
      async *reply() {
        yield { type: "text", value: "ok" } as const;
      },
    };
  };

  it("shows the level in the status line", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(thinking(), "repo", "main");
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    expect(screen(stdout.written).join("\n")).toContain("thinking off");

    stdin.emit("data", "\x04");
    await running;
  });

  it("cycles it on shift+tab, in both spellings", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(thinking(), "repo", "main");
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    // The classic back-tab.
    stdin.emit("data", "\x1b[Z");
    expect(session.snapshot().reasoning).toBe("low");

    // And the one the keyboard protocol reports.
    stdin.emit("data", "\x1b[9;2u");
    expect(session.snapshot().reasoning).toBe("medium");

    stdin.emit("data", "\x04");
    await running;
  });

  it("offers no key when a source has one level and no other", async () => {
    const session = new CoderSession(source([]), "repo", "main");

    // Showing a key that does nothing is worse than showing none.
    expect(session.canCycleReasoning).toBe(false);
    expect(session.snapshot().reasoning).toBeUndefined();
  });
});
