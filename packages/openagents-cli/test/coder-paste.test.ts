import { EventEmitter } from "node:events";
import { describe, expect, it } from "vitest";
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

describe("pasting", () => {
  it("keeps a multi-line paste as one message", async () => {
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

  it("shows a paste as a blob rather than as its last line", async () => {
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

    // The reader needs to know how much is there and that it goes as one
    // message, not to read all of it in a one-row composer.
    const painted = stdout.written.split(new RegExp(`${ESC}\\[[0-9;]*m`)).join("");
    expect(painted).toContain("first line [+2 more lines]");
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
