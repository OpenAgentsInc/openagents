// Terminal backend tests.
//
// Two layers, both NO-NETWORK:
//   1) FAKE PTY (fully deterministic, no real process): a scripted `Pty` proves
//      spawn -> wait-for-text -> send-input -> assert-on-snapshot, the
//      text-snapshot timeline + asciicast artifacts, the public-safe result
//      shape, and an honest red (a wrong assertion fails).
//   2) REAL PTY smoke (local /bin/sh, still no network): the shipped
//      `echoPromptScenario` runs end-to-end and records its snapshot timeline.

import { mkdtempSync, readFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";

import type { Pty, PtySession } from "@openagentsinc/probe-runtime";
import { decodeQaRunResult } from "./result";
import { makeTarget } from "./target";
import { runTerminalScenario } from "./terminal-backend";
import { echoPromptScenario, echoPromptScenarioWrong, type TerminalScenario } from "./terminal-scenario";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-terminal-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const target = makeTarget({ name: "t", baseUrl: "https://example.test", capabilities: ["terminal"] });

// A scripted fake PTY: when it sees `input` written, it emits `output` so the
// next wait-for resolves. Deterministic, no real process.
function makeScriptedPty(opts: {
  readonly banner: string;
  readonly onInput: (data: string) => string;
}): Pty {
  return {
    spawn: (): PtySession => {
      const listeners = new Set<(c: string) => void>();
      let output = "";
      let resolveExit!: (r: { code: number; output: string }) => void;
      const exit = new Promise<{ code: number; output: string }>((r) => {
        resolveExit = r;
      });
      const emit = (chunk: string) => {
        output += chunk;
        for (const l of [...listeners]) l(chunk);
      };
      // banner is printed on the next microtask so a `wait-for` armed before it
      // streams resolves on data (not synchronously).
      queueMicrotask(() => emit(opts.banner));
      return {
        write: (data) => {
          const reply = opts.onInput(data);
          emit(reply);
          resolveExit({ code: 0, output });
        },
        onData: (cb) => {
          listeners.add(cb);
          return () => listeners.delete(cb);
        },
        wait: () => exit,
        kill: () => undefined,
      };
    },
  };
}

describe("runTerminalScenario (fake PTY)", () => {
  test("spawn -> wait -> send -> assert; records snapshot timeline + asciicast + public-safe result", async () => {
    const pty = makeScriptedPty({
      banner: "QA TERMINAL READY\nname> ",
      onInput: (data) => `hello, ${data.trim()}!\n`,
    });
    const outcome = await runTerminalScenario({
      target,
      scenario: echoPromptScenario(),
      artifactDir: dir,
      pty,
      now: (() => {
        let t = 1_000_000;
        return () => (t += 10);
      })(),
    });

    expect(outcome.result.status).toBe("pass");
    expect(outcome.result.backend).toBe("terminal");
    expect(outcome.result.brain).toBe("terminal-scenario");

    // result.json round-trips through the SHARED public-safe schema (contract unchanged).
    const decoded = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    expect(decoded.status).toBe("pass");
    expect(decoded.artifacts.screenshots).toContain("terminal-snapshots.json");
    expect(decoded.artifacts.screenshots).toContain("terminal.cast");

    // snapshot timeline artifact exists with frames showing the on-screen text.
    const snaps = JSON.parse(readFileSync(outcome.snapshotTimelinePath, "utf8"));
    expect(snaps.scenario).toBe("echo-prompt");
    expect(snaps.frames.length).toBeGreaterThan(0);
    const joined = snaps.frames.map((f: { snapshot: string }) => f.snapshot).join("\n");
    expect(joined).toContain("QA TERMINAL READY");
    expect(joined).toContain("hello, khala!");

    // asciicast artifact exists, asciinema v2 (header line + output events).
    expect(existsSync(outcome.asciicastPath)).toBe(true);
    const castLines = readFileSync(outcome.asciicastPath, "utf8").trim().split("\n");
    expect(JSON.parse(castLines[0]!).version).toBe(2);
    const ev = JSON.parse(castLines[1]!);
    expect(ev[1]).toBe("o"); // an output event
  });

  test("never records the raw sent input (credential) in the result", async () => {
    const pty = makeScriptedPty({
      banner: "name> ",
      onInput: () => "hello!\n",
    });
    const scenario: TerminalScenario = {
      name: "secret-input",
      command: "noop",
      steps: [
        { kind: "wait-for", condition: { kind: "text-visible", value: "name>" } },
        { kind: "send", input: "hunter2-super-secret\n", label: "type password" },
        { kind: "wait-exit" },
      ],
    };
    const outcome = await runTerminalScenario({ target, scenario, artifactDir: dir, pty });
    const serialized = JSON.stringify(outcome.result);
    expect(serialized).not.toContain("hunter2-super-secret");
    // length is recorded instead
    expect(serialized).toContain('"length":21');
  });

  test("a wrong assertion FAILS honestly (real red) and persists the failure snapshot", async () => {
    const pty = makeScriptedPty({
      banner: "QA TERMINAL READY\nname> ",
      onInput: (data) => `hello, ${data.trim()}!\n`,
    });
    const outcome = await runTerminalScenario({
      target,
      scenario: echoPromptScenarioWrong(),
      artifactDir: dir,
      pty,
    });
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toContain("goodbye, khala!");
    const failed = outcome.result.steps.find((s) => s.status === "failed");
    expect(failed).toBeDefined();
  });

  test("an unmet wait-for fails honestly without hanging (short timeout)", async () => {
    // PTY that never prints the awaited text.
    const pty: Pty = {
      spawn: (): PtySession => {
        const listeners = new Set<(c: string) => void>();
        queueMicrotask(() => {
          for (const l of listeners) l("nothing useful\n");
        });
        return {
          write: () => undefined,
          onData: (cb) => {
            listeners.add(cb);
            return () => listeners.delete(cb);
          },
          wait: () => new Promise(() => undefined), // never exits on its own
          kill: () => undefined,
        };
      },
    };
    const scenario: TerminalScenario = {
      name: "never",
      command: "noop",
      steps: [{ kind: "wait-for", condition: { kind: "text-visible", value: "WILL-NEVER-APPEAR" }, timeoutMs: 20 }],
    };
    const outcome = await runTerminalScenario({ target, scenario, artifactDir: dir, pty, defaultTimeoutMs: 20 });
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toContain("wait-for did not complete");
  });
});

describe("runTerminalScenario (real PTY, local shell, no network)", () => {
  test("the shipped echo scenario runs end-to-end and records its snapshot timeline", async () => {
    const outcome = await runTerminalScenario({
      target,
      scenario: echoPromptScenario(),
      artifactDir: dir,
      // default pty = real subprocess via /bin/sh; no network.
    });
    expect(outcome.result.status).toBe("pass");
    const snaps = JSON.parse(readFileSync(outcome.snapshotTimelinePath, "utf8"));
    const joined = snaps.frames.map((f: { snapshot: string }) => f.snapshot).join("\n");
    expect(joined).toContain("QA TERMINAL READY");
    expect(joined).toContain("hello, khala!");
  });
});
