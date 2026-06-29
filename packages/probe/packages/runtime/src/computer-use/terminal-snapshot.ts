// Deterministic terminal snapshot + wait-for-condition primitives.
//
// The existing `TerminalSurface` (terminal.ts) is fire-and-forget: it runs a
// command to completion and records the exit code. Driving an interactive
// command or TUI needs more: a live, accumulating *view* of what the terminal
// currently shows, plus the ability to WAIT (without sleeps) until that view
// satisfies a condition before sending the next input.
//
// `TerminalView` wraps a `PtySession` (the same seam terminal.ts uses, so the
// REAL adapter and the test fake both work) and:
//   - buffers streamed output into a normalized text snapshot (ANSI stripped,
//     CRLF folded), so `snapshot()` reads like what a human sees on screen;
//   - resolves `waitFor(condition)` the instant the buffered snapshot satisfies
//     the condition — driven by the PTY's `onData`/exit events, NEVER a timer
//     poll. The only timer is the optional *failure* timeout, so a never-met
//     condition fails honestly instead of hanging forever.
//
// This is the determinism contract the qa-runner terminal backend relies on:
// given the same scripted PTY output, the same waits resolve in the same order.

import type { PtyExitResult, PtySession } from "./terminal";

/** A condition evaluated against the current terminal snapshot. */
export type TerminalCondition =
  | { readonly kind: "text-visible"; readonly value: string }
  | { readonly kind: "text-absent"; readonly value: string }
  | { readonly kind: "matches"; readonly pattern: string; readonly flags?: string }
  | { readonly kind: "exited" };

export interface TerminalWaitResult {
  /** Whether the condition was met before the timeout. */
  readonly met: boolean;
  /** The snapshot text at the moment the wait resolved (met or timed out). */
  readonly snapshot: string;
  /** Set when the wait resolved because the process exited. */
  readonly exit?: PtyExitResult;
}

export interface TerminalViewOptions {
  /**
   * Strip ANSI/VT control sequences from the buffered snapshot. Default true:
   * snapshots become plain on-screen text, which is what assertions match
   * against and what is public-safe to persist. Set false to keep raw bytes.
   */
  readonly stripAnsi?: boolean;
  /**
   * Default failure timeout (ms) for `waitFor` when a per-call timeout is not
   * given. This is ONLY a safety net so an unmet condition fails honestly; the
   * happy path resolves on data, not on the clock. Default 5000.
   */
  readonly defaultTimeoutMs?: number;
  /**
   * Injectable timer (for deterministic tests). Defaults to global
   * setTimeout/clearTimeout. A fake can make the timeout deterministic without
   * wall-clock waiting.
   */
  readonly timers?: {
    readonly setTimeout: (cb: () => void, ms: number) => unknown;
    readonly clearTimeout: (handle: unknown) => void;
  };
}

export interface TerminalView {
  /** The live normalized snapshot of everything the terminal has shown so far. */
  readonly snapshot: () => string;
  /** Send input to the underlying PTY (e.g. answer a prompt / drive a TUI). */
  readonly send: (data: string) => void;
  /**
   * Resolve the instant the snapshot satisfies `condition`. If it is already
   * satisfied, resolves immediately. If never satisfied, resolves with
   * `met: false` after the (per-call or default) failure timeout — no hang.
   * Driven by PTY data/exit events; the timeout is the only timer.
   */
  readonly waitFor: (
    condition: TerminalCondition,
    options?: { readonly timeoutMs?: number },
  ) => Promise<TerminalWaitResult>;
  /** Resolve when the process exits, with the final snapshot + exit result. */
  readonly waitForExit: () => Promise<TerminalWaitResult>;
  /** Kill the underlying process. */
  readonly kill: () => void;
}

// ANSI/VT escape-sequence stripper. Covers CSI (ESC [ ... final), OSC
// (ESC ] ... BEL/ST), and lone single-char escapes, then folds CR and drops
// remaining non-printable control bytes (keeping \n and \t). All control bytes
// are referenced via \u escapes so the source stays printable/portable.
/* eslint-disable no-control-regex */
const ESC = "\\u001b";
const OSC = new RegExp(`${ESC}\\][\\s\\S]*?(?:\\u0007|${ESC}\\\\)`, "g");
const CSI = new RegExp(`${ESC}\\[[0-?]*[ -/]*[@-~]`, "g");
const SINGLE_ESC = new RegExp(`${ESC}[@-Z\\\\-_]`, "g");
// Remaining C0 control bytes except \t (\u0009) and \n (\u000a).
const OTHER_CONTROL = new RegExp("[\\u0000-\\u0008\\u000b\\u000c\\u000e-\\u001f]", "g");
/* eslint-enable no-control-regex */

export function stripAnsi(text: string): string {
  return text
    .replace(OSC, "")
    .replace(CSI, "")
    .replace(SINGLE_ESC, "")
    .replace(/\r\n/g, "\n")
    .replace(/\r/g, "\n")
    .replace(OTHER_CONTROL, "");
}

function conditionMet(
  condition: TerminalCondition,
  snapshot: string,
  exited: boolean,
): boolean {
  switch (condition.kind) {
    case "text-visible":
      return snapshot.includes(condition.value);
    case "text-absent":
      return !snapshot.includes(condition.value);
    case "matches":
      return new RegExp(condition.pattern, condition.flags).test(snapshot);
    case "exited":
      return exited;
  }
}

export function makeTerminalView(
  session: PtySession,
  options: TerminalViewOptions = {},
): TerminalView {
  const doStrip = options.stripAnsi ?? true;
  const defaultTimeoutMs = options.defaultTimeoutMs ?? 5000;
  const setT = options.timers?.setTimeout ?? ((cb: () => void, ms: number) => setTimeout(cb, ms));
  const clearT =
    options.timers?.clearTimeout ?? ((h: unknown) => clearTimeout(h as ReturnType<typeof setTimeout>));

  let raw = "";
  let exitResult: PtyExitResult | undefined;
  // Waiters re-evaluated on every data chunk and on exit.
  const waiters = new Set<() => void>();

  const currentSnapshot = () => (doStrip ? stripAnsi(raw) : raw);

  session.onData((chunk) => {
    raw += chunk;
    for (const w of [...waiters]) w();
  });

  const exitPromise = session.wait().then((result) => {
    exitResult = result;
    // The final captured output is authoritative; reconcile the buffer with it
    // so a snapshot taken after exit reflects everything the process printed.
    if (result.output.length > raw.length) raw = result.output;
    for (const w of [...waiters]) w();
    return result;
  });

  const waitFor: TerminalView["waitFor"] = (condition, opts) =>
    new Promise<TerminalWaitResult>((resolve) => {
      const timeoutMs = opts?.timeoutMs ?? defaultTimeoutMs;
      let settled = false;
      let timer: unknown;

      const finish = (met: boolean) => {
        if (settled) return;
        settled = true;
        waiters.delete(check);
        if (timer !== undefined) clearT(timer);
        const snap = currentSnapshot();
        resolve({
          met,
          snapshot: snap,
          ...(exitResult !== undefined ? { exit: exitResult } : {}),
        });
      };

      const check = () => {
        if (conditionMet(condition, currentSnapshot(), exitResult !== undefined)) finish(true);
      };

      waiters.add(check);
      // Evaluate against current state first (may already be satisfied).
      check();
      if (!settled) {
        timer = setT(() => finish(false), timeoutMs);
      }
    });

  const waitForExit: TerminalView["waitForExit"] = () =>
    exitPromise.then((exit) => ({ met: true, snapshot: currentSnapshot(), exit }));

  return {
    snapshot: currentSnapshot,
    send: (data) => session.write(data),
    waitFor,
    waitForExit,
    kill: () => session.kill(),
  };
}
