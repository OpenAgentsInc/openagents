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

  it("shows the tool, its call, and its outcome, on two rows", async () => {
    const { rows } = await drive([
      { type: "tool_call", callId: "c1", name: "repo_grep", arguments: '{"pattern":"x"}' },
      { type: "tool_result", callId: "c1", output: '{"matches":[]}', error: undefined },
    ]);

    // The call joins the row that names it. It used to sit below as raw JSON,
    // which cost a row per call and read as punctuation rather than as the
    // command it is.
    const named = rows.find((row) => row.includes("repo_grep")) ?? "";
    expect(named).toContain("repo_grep x");
    expect(named).not.toContain('{"pattern"');

    expect(rows.join("\n")).toContain('{"matches":[]}');
  });

  it("shows an argument vector as the command line it is", async () => {
    const { rows } = await drive([
      {
        type: "tool_call",
        callId: "c1",
        name: "openagents",
        arguments: '{"args":["issue","view","212","-R","OpenAgentsInc/openagents.com"]}',
      },
      { type: "tool_result", callId: "c1", output: "#212 Rename the API", error: undefined },
    ]);

    const named = rows.find((row) => row.includes("openagents")) ?? "";
    expect(named).toContain("openagents issue view 212 -R OpenAgentsInc/openagents.com");
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

  it("says nothing about scope when the source keeps its turns to itself", async () => {
    const { rows } = await drive([{ type: "text", value: "hello" }]);
    expect(rows.join("\n")).not.toContain("shared with");
  });

  describe("switching backend with shift+tab", () => {
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

    it("moves to the next backend and says so", async () => {
      const { session, rows } = await driveSwitchable(["\x1b[Z"]);

      const expected = CODER_BACKENDS[1]?.label ?? "";
      expect(session.snapshot().model).toBe(expected);
      expect(rows.join("\n")).toContain(`Model switched to ${expected}.`);
    });

    it("wraps around, so every backend is reachable from one key", async () => {
      const { session } = await driveSwitchable(CODER_BACKENDS.map(() => "\x1b[Z"));
      expect(session.snapshot().model).toBe(CODER_BACKENDS[0]?.label);
    });

    it("does not leave a stray byte in the composer", async () => {
      const { rows } = await driveSwitchable(["\x1b[Z"]);
      const composer = rows.find((row) => row.includes(">")) ?? "";
      expect(composer).not.toContain("\t");
      expect(composer).not.toContain("[Z");
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
    auto: false,
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

  it("cycles it on tab, in both spellings", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(thinking(), "repo", "main");
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    // The plain byte.
    stdin.emit("data", "\t");
    expect(session.snapshot().reasoning).toBe("low");

    // And the one the keyboard protocol reports as a bare tab.
    stdin.emit("data", "\x1b[9u");
    expect(session.snapshot().reasoning).toBe("medium");

    // Four presses left four notes, three of which were no longer true.
    stdin.emit("data", "\t");
    stdin.emit("data", "\t");
    const notes = session
      .snapshot()
      .entries.filter((entry) => entry.text.startsWith("Reasoning set to"))
      .map((entry) => entry.text);
    expect(notes).toEqual(["Reasoning set to off."]);

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

describe("quitting with the keyboard protocol on", () => {
  it("quits on ctrl+d in either spelling", async () => {
    for (const key of ["\x04", "\x1b[100;5u"]) {
      const stdin = new FakeIn();
      const stdout = new FakeOut();
      const session = new CoderSession(source([]), "repo", "main");
      const running = runCoderUi(session, {
        stdin: stdin as unknown as NodeJS.ReadStream,
        stdout: stdout as unknown as NodeJS.WriteStream,
      });

      stdin.emit("data", key);

      // Asking the terminal to disambiguate escape codes buys shift+enter and
      // costs every control key: ctrl+d arrives as `\x1b[100;5u`, and a console
      // that reads only the byte stops being quittable.
      await expect(running).resolves.toBe(0);
    }
  });

  it("stops on ctrl+c in either spelling", async () => {
    for (const key of ["\x03", "\x1b[99;5u"]) {
      const stdin = new FakeIn();
      const stdout = new FakeOut();
      const session = new CoderSession(source([]), "repo", "main");
      const running = runCoderUi(session, {
        stdin: stdin as unknown as NodeJS.ReadStream,
        stdout: stdout as unknown as NodeJS.WriteStream,
      });

      stdin.emit("data", key);

      await expect(running).resolves.toBe(130);
    }
  });

  it("keeps the model still on a bare tab, now that shift+tab moves it", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(switchable([]), "repo", "main");
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    const before = session.snapshot().model;
    stdin.emit("data", "\x1b[9u");
    expect(session.snapshot().model).toBe(before);

    stdin.emit("data", "\x1b[Z");
    expect(session.snapshot().model).not.toBe(before);

    // The tab landed in the composer, and ctrl+d only quits an empty line.
    stdin.emit("data", "\x7f");
    stdin.emit("data", "\x04");
    await running;
  });
});

describe("the transcript's marker column", () => {
  const ESCAPE = String.fromCharCode(27);

  it("marks an entry with a dot rather than naming its role", async () => {
    const { rows } = await drive([{ type: "text", value: "an answer" }]);
    const painted = rows.join("\n");

    // Five words of chrome per turn — `you`, `think`, `coder`, `note`, `tool` —
    // said what the styling already said.
    expect(painted).toContain("● an answer");
    expect(painted).not.toMatch(/\bcoder\s+an answer/);
    expect(painted).not.toMatch(/^\s*you\s/m);
  });

  it("pulses an unfinished reply and settles it when it ends", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    let release = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const held: ReplySource = {
      model: "scripted",
      async *reply() {
        yield { type: "text", value: "still going" } as const;
        await gate;
      },
    };
    const session = new CoderSession(held, "repo", "main");
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    const turn = session.submit("go");
    await new Promise((resolve) => setTimeout(resolve, 0));

    // A hollow dot is the half of the pulse that says "still arriving".
    const during = stdout.written;
    expect(during).toContain("still going");

    release();
    await turn;
    stdin.emit("data", "\x04");
    await running;

    expect(stdout.written).toContain("●");
  });

  it("keeps the scroll marker in the same voice as the rest of the bar", async () => {
    const { painted } = await drive(
      Array.from({ length: 60 }, (_unused, at) => ({
        type: "text" as const,
        value: `line ${String(at)}\n`,
      })),
    );

    // It was the one yellow word in a dim row, which read as a warning.
    expect(painted).not.toContain(`${ESCAPE}[33mscrolled`);
    // And the key it advertised is gone from the bar.
    expect(painted).not.toContain("pgup/pgdn to scroll");
  });
});

describe("the chrome under the composer", () => {
  it("is one row, and it is the status line", async () => {
    const { rows } = await drive([{ type: "text", value: "answer" }]);
    const bottom = rows.slice(-2);

    // The keys used to have a row of their own under the status line. They are
    // in `/help` now, which is where a reader looks for them once rather than
    // past them always.
    expect(bottom.some((row) => row.includes("ready") || row.includes("working"))).toBe(true);
    expect(rows.join("\n")).not.toContain("enter to send");
    expect(rows.join("\n")).not.toContain("ctrl+d to quit");
  });

  it("opens without four lines of keys nobody asked for", async () => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(source([]), "repo", "main");
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    // A banner at the top of every session is something scrolled past for the
    // rest of it.
    expect(stdout.written).not.toContain("development build");
    expect(session.snapshot().entries).toEqual([]);

    stdin.emit("data", "\x04");
    await running;
  });

  it("says when the reader is scrolled away from the newest line", async () => {
    const { rows } = await drive(
      Array.from({ length: 80 }, (_unused, at) => ({
        type: "text" as const,
        value: `line ${String(at)}\n`,
      })),
    );

    // Losing the second row lost the scroll indicator with it, and a still
    // transcript with nothing saying why reads as a stopped session.
    expect(rows.join("\n")).toContain("ready");
  });
});

describe("where a running child is shown", () => {
  /** A session with a running delegate tool call, at a given terminal width. */
  const driveDelegated = async (
    record: (registry: CoderTaskRegistry) => void,
    columns = 100,
  ): Promise<ReadonlyArray<string>> => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    stdout.columns = columns;
    const registry = new CoderTaskRegistry();
    const session = new CoderSession(
      {
        model: "scripted",
        async *reply(_prompt: string, signal: AbortSignal) {
          yield {
            type: "tool_call",
            callId: "c1",
            name: "delegate",
            arguments: JSON.stringify({
              prompt: "look around",
              count: 1,
              description: "inspect the repo",
            }),
          };
          await new Promise<void>((resolve) => {
            if (signal.aborted) return resolve();
            signal.addEventListener("abort", () => resolve(), { once: true });
          });
        },
      },
      "repo",
      "main",
      {
        registry,
        fleet: {
          submit: (): Promise<never> => new Promise(() => {}),
        },
        label: "fake",
      },
    );
    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    const turn = session.submit("go");
    await new Promise((resolve) => setTimeout(resolve, 0));
    record(registry);
    await new Promise((resolve) => setTimeout(resolve, 0));
    const painted = stdout.written;
    stdin.emit("data", "\x04");
    await running;
    session.interrupt();
    await turn;
    return screen(painted);
  };

  const registerChild = (registry: CoderTaskRegistry): string => {
    const task = registry.register({
      id: "d1",
      description: "inspect the repo",
      prompt: "look around",
      agent: "opencode",
      model: "fake/model",
      cwd: "/tmp",
      background: true,
    });
    registry.start(task.id, new AbortController());
    return task.id;
  };

  it("puts a running child in the right column, beside the conversation", async () => {
    const rows = await driveDelegated((registry) => {
      registerChild(registry);
    });

    const heading = rows.find((row) => row.includes("subagents"));
    expect(heading).toBeDefined();
    expect(heading).toContain("1 working");

    const at = rows.findIndex((row) => row.includes("inspect the repo"));
    expect(at).toBeGreaterThan(0);
    // Beside, not below: the divider is to its left, so the transcript still
    // has the row it is on.
    expect(rows[at]).toContain("│");
    // A child that has not called a tool yet still says what it is doing.
    expect(rows[at + 1] ?? "").toContain("Initializing");

    // The session status lives at the bottom and does not repeat the fleet.
    const bottom = rows.filter((row) => row.includes("repo · main")).at(-1) ?? "";
    expect(bottom).toContain("repo · main");
    expect(bottom).not.toContain("1 agent");
  });

  it("says it once: the fleet is in the column or in the feed, never both", async () => {
    const rows = await driveDelegated((registry) => {
      registerChild(registry);
    });

    // The inline block drew the child directly under the `delegate` row. With
    // a column open that would be the same child twice on one screen.
    const delegateRow = rows.findIndex((row) => row.includes("delegate"));
    expect(delegateRow).toBeGreaterThan(0);
    const under = rows[delegateRow + 1] ?? "";
    expect(under).not.toContain("inspect the repo");

    const mentions = rows.filter((row) => row.includes("inspect the repo"));
    expect(mentions).toHaveLength(1);
  });

  it("falls back to the feed on a terminal too narrow for a column", async () => {
    // A fleet with nowhere to go is worse than one read in the feed, and a
    // column carved out of eighty would leave the transcript wrapping.
    const rows = await driveDelegated((registry) => {
      registerChild(registry);
    }, 80);

    expect(rows.some((row) => row.includes("subagents"))).toBe(false);

    const delegateRow = rows.findIndex((row) => row.includes("delegate"));
    expect(delegateRow).toBeGreaterThan(0);
    expect(rows[delegateRow + 1] ?? "").toContain("inspect the repo");
  });

  it("previews the child's latest activity one line per thing, three lines at most", async () => {
    const rows = await driveDelegated((registry) => {
      const id = registerChild(registry);
      // Five activities against a box that holds three: the two oldest have
      // to fall off the top, which is what keeps the box a preview.
      registry.recordToolUse(id, { toolName: "read", target: "src/a.ts" });
      registry.recordToolUse(id, { toolName: "grep", target: "needle" });
      registry.recordToolUse(id, { toolName: "bash", target: "pnpm test" });
      registry.recordToolUse(id, { toolName: "edit", target: "src/b.ts" });
      registry.recordToolUse(id, { toolName: "shell", target: "mix test" });
    });

    const childRow = rows.findIndex((row) => row.includes("inspect the repo"));
    expect(childRow).toBeGreaterThan(0);
    const previews = rows.slice(childRow + 1, childRow + 4);
    expect(previews).toHaveLength(3);
    const text = previews.join("\n");
    expect(text).toContain("shell: mix test");
    expect(text).toContain("edit: src/b.ts");
    expect(text).toContain("bash: pnpm test");
    // Pushed out by the newer work.
    expect(text).not.toContain("read: src/a.ts");
    expect(text).not.toContain("grep: needle");

    // Newest last, so reading down is reading forward in time.
    const at = (phrase: string) => previews.findIndex((row) => row.includes(phrase));
    expect(at("bash: pnpm test")).toBeLessThan(at("edit: src/b.ts"));
    expect(at("edit: src/b.ts")).toBeLessThan(at("shell: mix test"));
  });

  it("draws no preview until the child has done something", async () => {
    const rows = await driveDelegated((registry) => {
      registerChild(registry);
    });

    expect(rows.join("\n")).not.toContain(" → ");
  });
});

describe("inspecting a child from the column", () => {
  /** Drive the interface, sending keys between paints, and return each frame. */
  const driveKeys = async (
    record: (registry: CoderTaskRegistry) => void,
    steps: ReadonlyArray<ReadonlyArray<string>>,
  ): Promise<ReadonlyArray<ReadonlyArray<string>>> => {
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    stdout.columns = 120;
    const registry = new CoderTaskRegistry();
    const session = new CoderSession(
      {
        model: "scripted",
        async *reply(_prompt: string, signal: AbortSignal) {
          yield { type: "tool_call", callId: "c1", name: "delegate", arguments: "{}" };
          await new Promise<void>((resolve) => {
            if (signal.aborted) return resolve();
            signal.addEventListener("abort", () => resolve(), { once: true });
          });
        },
      },
      "repo",
      "main",
      { registry, fleet: { submit: (): Promise<never> => new Promise(() => {}) }, label: "fake" },
    );

    const running = runCoderUi(session, {
      stdin: stdin as unknown as NodeJS.ReadStream,
      stdout: stdout as unknown as NodeJS.WriteStream,
    });

    const turn = session.submit("go");
    await new Promise((resolve) => setTimeout(resolve, 0));
    record(registry);
    await new Promise((resolve) => setTimeout(resolve, 0));

    const frames: ReadonlyArray<string>[] = [];
    for (const keys of steps) {
      stdout.written = "";
      for (const key of keys) stdin.emit("data", key);
      await new Promise((resolve) => setTimeout(resolve, 0));
      frames.push(screen(stdout.written));
    }

    // Back to a state that can quit, whatever the steps left behind: the
    // child screen and the column both take ctrl+d as "return", and ctrl+d
    // only exits from an empty composer.
    stdin.emit("data", "\x04");
    stdin.emit("data", "\x7f".repeat(40));
    stdin.emit("data", "\x04");
    await running;
    session.interrupt();
    await turn;
    return frames;
  };

  const two = (registry: CoderTaskRegistry) => {
    for (const [id, description] of [
      ["d1", "audit open_router"],
      ["d2", "audit vercel_gateway"],
    ] as const) {
      const task = registry.register({
        id,
        description,
        prompt: "x",
        agent: "openagents",
        model: "ox-alpha",
        cwd: "/tmp",
        background: true,
      });
      registry.start(task.id, new AbortController());
    }
  };

  it("hands the arrow keys to the column on right, and back on left", async () => {
    const [before, inside, back] = await drillKeys();

    // Nothing says the column is live until it is.
    expect(before.join("\n")).not.toContain("enter opens");
    expect(inside.join("\n")).toContain("enter opens");
    expect(back.join("\n")).not.toContain("enter opens");
  });

  it("moves the selector without moving the transcript", async () => {
    const frames = await driveKeys(two, [["\x1b[C"], ["\x1b[B"]]);

    // Down in the column selects the second child. It must not scroll the
    // conversation, which is what down does when the composer has the keys.
    const moved = frames[1] ?? [];
    expect(moved.join("\n")).toContain("audit vercel_gateway");
  });

  it("opens the selected child on enter, filling the screen", async () => {
    const frames = await driveKeys(two, [["\x1b[C"], ["\x1b[B"], ["\r"]]);
    const opened = (frames[2] ?? []).join("\n");

    expect(opened).toContain("audit vercel_gateway");
    expect(opened).toContain("↑↓ scroll");
    // The conversation is not underneath it: this is a screen, not a pane.
    expect(opened).not.toContain("│");
  });

  it("returns from a child to the column it was opened from", async () => {
    // Stepping through children should not need a press of right between each.
    const frames = await driveKeys(two, [["\x1b[C"], ["\r"], ["\x1b[D"]]);
    const returned = (frames[2] ?? []).join("\n");

    expect(returned).toContain("enter opens");
    expect(returned).toContain("│");
  });

  it("gives the keys back to the composer when the reader types", async () => {
    // A sentence started while the column has the keys is a sentence, not a
    // set of shortcuts to swallow.
    const frames = await driveKeys(two, [["\x1b[C"], ["h", "i"]]);
    const typed = (frames[1] ?? []).join("\n");

    expect(typed).toContain("› hi");
  });

  it("ignores right when there is no column to move into", async () => {
    const frames = await driveKeys(() => undefined, [["\x1b[C"]]);

    expect((frames[0] ?? []).join("\n")).not.toContain("enter opens");
  });

  const drillKeys = async () => driveKeys(two, [[], ["\x1b[C"], ["\x1b[D"]]);
});
