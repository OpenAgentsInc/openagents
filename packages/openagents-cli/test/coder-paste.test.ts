import { EventEmitter } from "node:events";
import { describe, expect, it } from "vitest";
import {
  backspaceComposer,
  expandPastedTextRefs,
  formatPastedTextRef,
  getPastedTextRefNumLines,
  PASTE_MAX_LINES,
  PASTE_TEXT_THRESHOLD,
  shouldCollapsePaste,
} from "../src/coder-paste.js";
import { CoderSession, type ReplySource } from "../src/coder-session.js";
import { runCoderUi } from "../src/coder-ui.js";

class FakeOut extends EventEmitter {
  columns = 100;
  rows = 24;
  written = "";
  write(t: string) {
    this.written += t;
    return true;
  }
}
class FakeIn extends EventEmitter {
  isTTY = true;
  setRawMode() {
    return this;
  }
  resume() {
    return this;
  }
  pause() {
    return this;
  }
  setEncoding() {
    return this;
  }
}

describe("coder-paste utilities", () => {
  it("counts additional lines accurately", () => {
    expect(getPastedTextRefNumLines("one line")).toBe(0);
    expect(getPastedTextRefNumLines("line 1\nline 2")).toBe(1);
    expect(getPastedTextRefNumLines("line 1\r\nline 2\r\nline 3")).toBe(2);
  });

  it("formats reference placeholders", () => {
    expect(formatPastedTextRef(1, 0)).toBe("[Pasted text #1]");
    expect(formatPastedTextRef(2, 1)).toBe("[Pasted text #2 +1 line]");
    expect(formatPastedTextRef(3, 15)).toBe("[Pasted text #3 +15 lines]");
  });

  it("identifies when paste should collapse", () => {
    expect(shouldCollapsePaste("short text")).toBe(false);
    expect(shouldCollapsePaste("line 1\nline 2\nline 3")).toBe(true);
    expect(shouldCollapsePaste("a".repeat(PASTE_TEXT_THRESHOLD + 1))).toBe(true);
  });

  it("expands single and multiple pasted text references", () => {
    const pasted = new Map([
      [1, { id: 1, content: "full contents of first paste\nwith lines" }],
      [2, { id: 2, content: "second paste content" }],
    ]);

    const input = "Please inspect [Pasted text #1 +1 line] and also [Pasted text #2] carefully.";
    const expanded = expandPastedTextRefs(input, pasted);

    expect(expanded).toBe(
      "Please inspect full contents of first paste\nwith lines and also second paste content carefully.",
    );
  });

  it("deletes placeholder token atomically on backspace", () => {
    const composerWithToken = "prefix [Pasted text #1 +5 lines]";
    expect(backspaceComposer(composerWithToken)).toBe("prefix ");

    const composerWithSingleLineToken = "prefix [Pasted text #2]";
    expect(backspaceComposer(composerWithSingleLineToken)).toBe("prefix ");

    const normalComposer = "normal text";
    expect(backspaceComposer(normalComposer)).toBe("normal tex");
  });
});

describe("pasting in coder UI", () => {
  it("keeps a multi-line paste as a compact placeholder in UI and expands on submit", async () => {
    const sent: string[] = [];
    const src: ReplySource = {
      model: "m",
      async *reply(p: string) {
        sent.push(p);
        yield { type: "text", value: "ok" } as const;
      },
    };
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(src, "repo", "main");
    const running = runCoderUi(session, { stdin: stdin as never, stdout: stdout as never });

    const ESC = String.fromCharCode(27);
    stdin.emit("data", `${ESC}[200~line one\nline two\nline three${ESC}[201~`);
    expect(sent).toEqual([]);
    stdin.emit("data", "\r");
    await new Promise((r) => setTimeout(r, 20));

    expect(sent).toEqual(["line one\nline two\nline three"]);
    stdin.emit("data", "\x04");
    await running;
  });

  it("holds a paste whose end has not arrived", async () => {
    const sent: string[] = [];
    const src: ReplySource = {
      model: "m",
      async *reply(p: string) {
        sent.push(p);
        yield { type: "text", value: "ok" } as const;
      },
    };
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(src, "repo", "main");
    const running = runCoderUi(session, { stdin: stdin as never, stdout: stdout as never });

    const ESC = String.fromCharCode(27);
    stdin.emit("data", `${ESC}[200~first\nsec`);
    stdin.emit("data", `ond${ESC}[201~`);
    stdin.emit("data", "\r");
    await new Promise((r) => setTimeout(r, 20));

    expect(sent).toEqual(["first\nsecond"]);
    stdin.emit("data", "\x04");
    await running;
  });

  it("shows large paste as a placeholder token in the composer", async () => {
    const src: ReplySource = {
      model: "m",
      async *reply() {
        yield { type: "text", value: "ok" } as const;
      },
    };
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(src, "repo", "main");
    const running = runCoderUi(session, { stdin: stdin as never, stdout: stdout as never });

    const ESC = String.fromCharCode(27);
    stdin.emit("data", `${ESC}[200~first line\nsecond\nthird${ESC}[201~`);

    // The reader sees a compact placeholder token
    const painted = stdout.written.split(new RegExp(`${ESC}\\[[0-9;]*m`)).join("");
    expect(painted).toContain("[Pasted text #1 +2 lines]");
    expect(painted).not.toContain("second");

    // Ctrl+D quits only an empty composer, which is why escape comes first.
    stdin.emit("data", ESC);
    await new Promise((r) => setTimeout(r, 60));
    stdin.emit("data", "\x04");
    await running;
  });

  it("asks the terminal to bracket pastes, and stops asking on the way out", async () => {
    const src: ReplySource = {
      model: "m",
      async *reply() {
        yield { type: "text", value: "ok" } as const;
      },
    };
    const stdin = new FakeIn();
    const stdout = new FakeOut();
    const session = new CoderSession(src, "repo", "main");
    const running = runCoderUi(session, { stdin: stdin as never, stdout: stdout as never });

    const ESC = String.fromCharCode(27);
    // Without this a paste is indistinguishable from typing, and every newline
    // in it is an enter.
    expect(stdout.written).toContain(`${ESC}[?2004h`);

    stdin.emit("data", "\x04");
    await running;

    expect(stdout.written).toContain(`${ESC}[?2004l`);
  });
});
