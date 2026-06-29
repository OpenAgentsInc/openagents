// Deterministic unit tests for the terminal snapshot + wait-for primitives.
//
// NO real PTY, NO sleeps, NO network: a fake `PtySession` with a manual data
// pump and a manual exit drives the `TerminalView`. The failure timeout is
// exercised via an injected fake timer, so an unmet condition fails honestly
// without any wall-clock waiting.

import { describe, expect, test } from "bun:test";

import type { PtyExitResult, PtySession } from "./terminal";
import { makeTerminalView, stripAnsi } from "./terminal-snapshot";

// A controllable fake PTY session: tests push chunks and resolve exit by hand.
function makeFakeSession() {
  const listeners = new Set<(c: string) => void>();
  let resolveExit!: (r: PtyExitResult) => void;
  let captured = "";
  const exit = new Promise<PtyExitResult>((r) => {
    resolveExit = r;
  });
  const written: string[] = [];
  let killed = false;
  const session: PtySession = {
    write: (d) => written.push(d),
    onData: (cb) => {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    wait: () => exit,
    kill: () => {
      killed = true;
    },
  };
  return {
    session,
    written,
    isKilled: () => killed,
    emit: (chunk: string) => {
      captured += chunk;
      for (const l of [...listeners]) l(chunk);
    },
    finish: (code: number) => resolveExit({ code, output: captured }),
  };
}

// A deterministic fake timer: tests fire the registered timeout by hand.
function makeFakeTimers() {
  const pending = new Map<number, () => void>();
  let id = 0;
  return {
    timers: {
      setTimeout: (cb: () => void) => {
        const handle = ++id;
        pending.set(handle, cb);
        return handle;
      },
      clearTimeout: (h: unknown) => {
        pending.delete(h as number);
      },
    },
    fireAll: () => {
      for (const cb of [...pending.values()]) cb();
      pending.clear();
    },
    pendingCount: () => pending.size,
  };
}

describe("stripAnsi", () => {
  test("removes CSI color codes, OSC titles, and folds CRLF", () => {
    const esc = String.fromCharCode(27);
    const input = `${esc}[2J${esc}[1;31mhello${esc}[0m world\r\n${esc}]0;the-title${esc}\\done`;
    expect(stripAnsi(input)).toBe("hello world\ndone");
  });

  test("keeps tabs and newlines but drops bare control bytes", () => {
    const bel = String.fromCharCode(7);
    expect(stripAnsi(`a\tb\n${bel}c`)).toBe("a\tb\nc");
  });
});

describe("makeTerminalView", () => {
  test("snapshot accumulates streamed output (ANSI stripped)", () => {
    const fake = makeFakeSession();
    const view = makeTerminalView(fake.session);
    const esc = String.fromCharCode(27);
    fake.emit(`${esc}[32mready${esc}[0m`);
    fake.emit(" set");
    expect(view.snapshot()).toBe("ready set");
  });

  test("waitFor resolves the instant the condition is met by streamed data", async () => {
    const fake = makeFakeSession();
    const timers = makeFakeTimers();
    const view = makeTerminalView(fake.session, { timers: timers.timers });
    const pending = view.waitFor({ kind: "text-visible", value: "PASSWORD:" });
    // not yet satisfied -> a failure timer is armed
    expect(timers.pendingCount()).toBe(1);
    fake.emit("Enter PASSWORD: ");
    const result = await pending;
    expect(result.met).toBe(true);
    expect(result.snapshot).toContain("PASSWORD:");
    // the failure timer was cleared on success
    expect(timers.pendingCount()).toBe(0);
  });

  test("waitFor resolves immediately when the condition is already satisfied", async () => {
    const fake = makeFakeSession();
    const timers = makeFakeTimers();
    const view = makeTerminalView(fake.session, { timers: timers.timers });
    fake.emit("already here");
    const result = await view.waitFor({ kind: "text-visible", value: "already" });
    expect(result.met).toBe(true);
    expect(timers.pendingCount()).toBe(0); // never armed a timer
  });

  test("waitFor fails honestly (met:false) when the condition is never met", async () => {
    const fake = makeFakeSession();
    const timers = makeFakeTimers();
    const view = makeTerminalView(fake.session, { timers: timers.timers });
    const pending = view.waitFor({ kind: "text-visible", value: "NEVER" }, { timeoutMs: 10 });
    fake.emit("something else");
    timers.fireAll(); // simulate the timeout firing — no wall clock
    const result = await pending;
    expect(result.met).toBe(false);
    expect(result.snapshot).toBe("something else");
  });

  test("send writes input to the underlying PTY (drives a prompt/TUI)", () => {
    const fake = makeFakeSession();
    const view = makeTerminalView(fake.session);
    view.send("hunter2\n");
    expect(fake.written).toEqual(["hunter2\n"]);
  });

  test("waitFor exited + waitForExit reconcile the final captured output", async () => {
    const fake = makeFakeSession();
    const view = makeTerminalView(fake.session);
    fake.emit("working...\n");
    const exitedWait = view.waitFor({ kind: "exited" });
    fake.finish(0);
    const exited = await exitedWait;
    expect(exited.met).toBe(true);
    expect(exited.exit?.code).toBe(0);
    const final = await view.waitForExit();
    expect(final.snapshot).toContain("working...");
  });

  test("kill propagates to the session", () => {
    const fake = makeFakeSession();
    const view = makeTerminalView(fake.session);
    view.kill();
    expect(fake.isKilled()).toBe(true);
  });
});
