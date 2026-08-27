// Deterministic unit tests for the computer-use tool surface.
//
// NO network, NO chromium, NO real PTY: a fake `ComputerUsePage` and fake `Pty`
// are injected against the seams. A retained caller must provide its own
// real-Chromium acceptance proof.

import { describe, expect, test } from "vite-plus/test";
import { Effect } from "effect";
import { makeFilesystemSurface, FilesystemScopeError } from "./filesystem";
import type { ComputerUsePage, WaitForCondition } from "./page";
import { makeTerminalSurface, type Pty, type PtySession } from "./terminal";
import { makeTimeline } from "./timeline";
import { makeComputerUseTools } from "./tools";
import { resetPermissionHandler, setPermissionHandler } from "../permission";

// ── Fakes ─────────────────────────────────────────────────────────────────

function fakePage(overrides: Partial<ComputerUsePage> = {}): ComputerUsePage {
  let currentUrl = "about:blank";
  return {
    navigate: async (u) => {
      currentUrl = u;
    },
    url: async () => currentUrl,
    click: async () => undefined,
    type: async () => undefined,
    readText: async () => "fake body text",
    readDom: async () => "<html></html>",
    waitFor: async (c: WaitForCondition) =>
      c.kind === "url-includes" && currentUrl.includes(c.value),
    screenshot: async () => undefined,
    ...overrides,
  };
}

function fakePty(scripted: Record<string, { code: number; output: string }>): Pty {
  return {
    spawn: (command): PtySession => {
      const result = scripted[command] ?? { code: 127, output: `not found: ${command}` };
      const listeners = new Set<(c: string) => void>();
      return {
        write: () => undefined,
        onData: (cb) => {
          listeners.add(cb);
          // stream the scripted output synchronously-ish
          queueMicrotask(() => cb(result.output));
          return () => listeners.delete(cb);
        },
        wait: async () => result,
        kill: () => undefined,
      };
    },
  };
}

// ── Timeline ────────────────────────────────────────────────────────────────

describe("timeline", () => {
  test("appends ordered named beats with an injected clock", () => {
    let t = 1000;
    const tl = makeTimeline({ now: () => (t += 5) });
    tl.beat({ surface: "browser", label: "navigate to /login", detail: { url: "/login" } });
    tl.beat({ surface: "terminal", label: "run echo", status: "ok" });
    const snap = tl.snapshot();
    expect(snap.beats).toHaveLength(2);
    expect(snap.beats[0]!.label).toBe("navigate to /login");
    expect(snap.beats[0]!.surface).toBe("browser");
    expect(snap.beats[1]!.at).toBeGreaterThanOrEqual(snap.beats[0]!.at);
  });
});

// ── Browser surface ──────────────────────────────────────────────────────────

// ── Terminal surface ─────────────────────────────────────────────────────────

describe("terminal surface", () => {
  test("runs a command, streams output, records exit code (not raw output)", async () => {
    const tl = makeTimeline();
    const term = makeTerminalSurface({
      pty: fakePty({ "echo hi": { code: 0, output: "hi\n" } }),
      timeline: tl,
    });
    const chunks: string[] = [];
    const result = await term.run("echo hi", undefined, { onChunk: (c) => chunks.push(c) });
    expect(result.code).toBe(0);
    expect(result.output).toBe("hi\n");
    expect(chunks.join("")).toBe("hi\n");
    const beat = tl.snapshot().beats[0]!;
    expect(beat.detail).toEqual({ command: "echo hi", exitCode: 0 });
    // raw output is NOT in the beat
    expect(JSON.stringify(beat)).not.toContain("hi\\n");
  });

  test("non-zero exit is recorded as an error beat", async () => {
    const tl = makeTimeline();
    const term = makeTerminalSurface({
      pty: fakePty({ false: { code: 1, output: "" } }),
      timeline: tl,
    });
    const result = await term.run("false");
    expect(result.code).toBe(1);
    expect(tl.snapshot().beats[0]!.status).toBe("error");
  });
});

// ── Filesystem surface ───────────────────────────────────────────────────────

describe("filesystem surface", () => {
  const makeFakeIo = () => {
    const store = new Map<string, string>();
    return {
      store,
      io: {
        readFile: (p: string) => {
          const v = store.get(p);
          if (v === undefined) throw new Error(`ENOENT: ${p}`);
          return v;
        },
        writeFile: (p: string, c: string) => store.set(p, c),
      },
    };
  };

  test("reads/writes within the workspace and records beats with relative paths", () => {
    const { io, store } = makeFakeIo();
    const tl = makeTimeline();
    const fs = makeFilesystemSurface({ workspace: "/run/ws", timeline: tl, io });
    fs.write("notes/out.txt", "hello");
    expect(store.get("/run/ws/notes/out.txt")).toBe("hello");
    expect(fs.read("notes/out.txt")).toBe("hello");
    const beats = tl.snapshot().beats;
    expect(beats[0]!.detail).toEqual({ path: "notes/out.txt", bytes: 5 });
    // contents withheld from the beat
    expect(JSON.stringify(beats)).not.toContain("hello");
  });

  test("rejects path escapes (no .. breakout)", () => {
    const { io } = makeFakeIo();
    const fs = makeFilesystemSurface({ workspace: "/run/ws", timeline: makeTimeline(), io });
    expect(() => fs.read("../../etc/passwd")).toThrow(FilesystemScopeError);
    expect(() => fs.write("../escape.txt", "x")).toThrow(FilesystemScopeError);
  });
});

// ── Tool surface + permission gating ─────────────────────────────────────────

describe("computer-use tools", () => {
  test("exposes only the tools for wired surfaces", () => {
    const tl = makeTimeline();
    const onlyTerminal = makeComputerUseTools({
      terminal: makeTerminalSurface({ pty: fakePty({}), timeline: tl }),
    });
    expect(Object.keys(onlyTerminal)).toEqual(["terminal_run"]);
  });

  test("browser_navigate tool drives the page and returns the url", async () => {
    const tl = makeTimeline();
    const page = fakePage();
    const surface = {
      page,
      timeline: tl,
      navigate: async (u: string) => page.navigate(u),
      click: async () => undefined,
      type: async () => undefined,
      readText: async () => "x",
      readDom: async () => "<html></html>",
      waitFor: async () => true,
      screenshot: async () => "/tmp/x.png",
    };
    const tools = makeComputerUseTools({ browser: surface });
    const out = await Effect.runPromise(
      tools.browser_navigate!.execute!({ url: "/login" }, { id: "1", name: "browser_navigate" }),
    );
    expect(out).toEqual({ url: "/login" });
  });

  test("terminal_run is permission-gated: deny fails the tool without acting", async () => {
    let ran = false;
    const term = {
      run: async () => {
        ran = true;
        return { code: 0, output: "" };
      },
    };
    setPermissionHandler({ ask: () => Effect.succeed("deny") });
    try {
      const tools = makeComputerUseTools({ terminal: term });
      const exit = await Effect.runPromiseExit(
        tools.terminal_run!.execute!({ command: "rm -rf /" }, { id: "1", name: "terminal_run" }),
      );
      expect(exit._tag).toBe("Failure");
      expect(ran).toBe(false);
    } finally {
      resetPermissionHandler();
    }
  });

  test("terminal_run runs when permission allows", async () => {
    const tl = makeTimeline();
    const term = makeTerminalSurface({
      pty: fakePty({ "echo ok": { code: 0, output: "ok\n" } }),
      timeline: tl,
    });
    resetPermissionHandler(); // default allows
    const tools = makeComputerUseTools({ terminal: term });
    const out = (await Effect.runPromise(
      tools.terminal_run!.execute!({ command: "echo ok" }, { id: "1", name: "terminal_run" }),
    )) as { exitCode: number; output: string };
    expect(out.exitCode).toBe(0);
    expect(out.output).toBe("ok\n");
  });
});
