// Hardening tests for the runner (#6193): per-step timeout, bounded retry (no
// flaky-pass), partial-failure continuation, and the critical guarantee —
// ARTIFACTS ARE FLUSHED ON CRASH / INTERRUPT (video/trace/result.json are
// written even when a run throws, times out, or is interrupted mid-step).
//
// Discipline: deterministic. A "hang" step never resolves; we drive the failure
// either with the runner's own per-step timeout (manual timer, no real wait) or
// by interrupting the Effect fiber mid-step. No `sleep`-to-cause-a-pass anywhere.

import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { Effect, Fiber } from "effect";

import type { AcquiredBrowser } from "@openagentsinc/probe-runtime/computer-use/browser";
import type { ComputerUsePage } from "@openagentsinc/probe-runtime/computer-use/page";
import type { PlaywrightArtifacts } from "@openagentsinc/probe-runtime/computer-use/playwright-page";
import type { Backend } from "./backend";
import type { BrainStep } from "./brain";
import { scriptedBrain } from "./brain";
import { decodeQaRunResult } from "./result";
import { runQaSession } from "./runner";
import { makeTarget } from "./target";
import type { TimerLike } from "./timeouts";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-runner-harden-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const target = () => makeTarget({ name: "fake", baseUrl: "https://example.test" });

// ── A controllable backend: each page action's behavior is supplied per call,
// and `flush()` writes a real trace.zip so we can PROVE artifacts flushed even
// when the run throws / is interrupted. ──────────────────────────────────────

function controllableBackend(makePage: (artifactDir: string) => ComputerUsePage): Backend {
  return {
    name: "controllable",
    provision: async ({ artifactDir }) => ({
      acquireBrowser: async (): Promise<AcquiredBrowser & { artifacts: () => PlaywrightArtifacts }> => {
        const tracePath = join(artifactDir, "trace.zip");
        const videoPath = join(artifactDir, "video.webm");
        let flushed = false;
        return {
          page: makePage(artifactDir),
          // RELEASE: writes the trace + video. withBrowserSurface guarantees
          // this runs even on interruption/throw — the artifact-flush contract.
          flush: async () => {
            if (flushed) return;
            flushed = true;
            writeFileSync(tracePath, Buffer.from("trace-bytes"));
            writeFileSync(videoPath, Buffer.from("video-bytes"));
          },
          artifacts: () => ({ tracePath, videoPath, videoFormat: "webm" as const }),
        };
      },
      teardown: async () => undefined,
    }),
  };
}

const stubPage = (overrides: Partial<ComputerUsePage> = {}): ComputerUsePage => ({
  navigate: async () => undefined,
  url: async () => "https://example.test/login",
  click: async () => undefined,
  type: async () => undefined,
  readText: async () => "",
  readDom: async () => "",
  waitFor: async () => true,
  screenshot: async (path) => writeFileSync(path, Buffer.from("png")),
  ...overrides,
});

const navSteps = (): ReadonlyArray<BrainStep> => [
  { kind: "navigate", url: "/login", label: "open /login" },
];

// A manual timer so the per-step timeout fires WITHOUT a real wait.
function makeManualTimer() {
  const pending: Array<{ resolve: () => void; cancelled: boolean }> = [];
  const timer: TimerLike = {
    delay: () => {
      const entry: { resolve: () => void; cancelled: boolean } = {
        resolve: () => undefined,
        cancelled: false,
      };
      const promise = new Promise<void>((resolve) => {
        entry.resolve = () => resolve();
      });
      pending.push(entry);
      return { promise, cancel: () => (entry.cancelled = true) };
    },
  };
  return { timer, fireAll: () => pending.forEach((p) => !p.cancelled && p.resolve()) };
}

describe("per-step timeout (#6193)", () => {
  test("a hanging step times out -> run FAILS honestly and artifacts (trace + result.json) are flushed", async () => {
    const { timer, fireAll } = makeManualTimer();
    const backend = controllableBackend(() =>
      stubPage({ navigate: () => new Promise<void>(() => {}) }), // never resolves
    );

    const fiber = Effect.runFork(
      runQaSession({
        target: target(),
        brain: scriptedBrain(navSteps()),
        backend,
        artifactDir: dir,
        stepPolicy: { timeoutMs: 50 },
        timer,
      }),
    );
    // let the step start, then fire the per-step deadline
    await new Promise((r) => setTimeout(r, 0));
    fireAll();

    const exit = await Effect.runPromise(Fiber.await(fiber));
    expect(exit._tag).toBe("Success");
    const outcome = (exit as Extract<typeof exit, { _tag: "Success" }>).value;

    // honest failure, not a fake pass
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toContain("timed out");
    // the failed step is recorded
    expect(outcome.result.steps.some((s) => s.status === "failed")).toBe(true);

    // ARTIFACTS FLUSHED despite the timeout
    expect(existsSync(join(dir, "trace.zip"))).toBe(true);
    expect(existsSync(join(dir, "result.json"))).toBe(true);
    const parsed = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    expect(parsed.status).toBe("fail");
  });
});

describe("bounded retry — no flaky-pass (#6193)", () => {
  test("a step flaky-then-good is retried and the flake is VISIBLE (attempts recorded), not silent", async () => {
    let calls = 0;
    const backend = controllableBackend(() =>
      stubPage({
        navigate: async () => {
          calls++;
          if (calls < 3) throw new Error("transient nav error");
        },
      }),
    );
    const outcome = await Effect.runPromise(
      runQaSession({
        target: target(),
        brain: scriptedBrain(navSteps()),
        backend,
        artifactDir: dir,
        stepPolicy: { retry: { maxAttempts: 3 } },
      }),
    );
    expect(outcome.result.status).toBe("pass");
    const navStep = outcome.result.steps.find((s) => s.kind === "navigate")!;
    expect(navStep.status).toBe("ok");
    // the flake is SURFACED — a reviewer sees it was retried (not a silent pass)
    expect(navStep.detail?.attempts).toBe(3);
    expect(calls).toBe(3);
  });

  test("a step that fails EVERY attempt fails the run honestly (no fake pass) and flushes artifacts", async () => {
    let calls = 0;
    const backend = controllableBackend(() =>
      stubPage({
        navigate: async () => {
          calls++;
          throw new Error("always broken");
        },
      }),
    );
    const outcome = await Effect.runPromise(
      runQaSession({
        target: target(),
        brain: scriptedBrain(navSteps()),
        backend,
        artifactDir: dir,
        stepPolicy: { retry: { maxAttempts: 3 } },
      }),
    );
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toContain("always broken");
    expect(calls).toBe(3); // exactly the bound
    expect(existsSync(join(dir, "trace.zip"))).toBe(true);
    expect(existsSync(join(dir, "result.json"))).toBe(true);
  });
});

describe("partial-failure continuation (#6193)", () => {
  const twoAsserts = (): ReadonlyArray<BrainStep> => [
    { kind: "navigate", url: "/login", label: "open /login" },
    // both intentionally wrong against a page at /login
    { kind: "assert", label: "wrong A", check: { kind: "url-not-includes", value: "/login" } },
    { kind: "assert", label: "wrong B", check: { kind: "url-includes", value: "/nope" } },
  ];

  test("default (stop-on-first-failure): only the FIRST failed assert is recorded", async () => {
    const backend = controllableBackend(() => stubPage());
    const outcome = await Effect.runPromise(
      runQaSession({ target: target(), brain: scriptedBrain(twoAsserts()), backend, artifactDir: dir }),
    );
    expect(outcome.result.status).toBe("fail");
    const failed = outcome.result.steps.filter((s) => s.status === "failed");
    expect(failed.length).toBe(1); // stopped at the first failure
  });

  test("continueOnFailure: BOTH failed asserts are recorded; status is still fail (never a fake pass)", async () => {
    const backend = controllableBackend(() => stubPage());
    const outcome = await Effect.runPromise(
      runQaSession({
        target: target(),
        brain: scriptedBrain(twoAsserts()),
        backend,
        artifactDir: dir,
        continueOnFailure: true,
      }),
    );
    expect(outcome.result.status).toBe("fail"); // still honest red
    const failed = outcome.result.steps.filter((s) => s.status === "failed");
    expect(failed.length).toBe(2); // both surfaced, not masked by the first
  });
});

describe("artifact flush on INTERRUPT mid-step (#6193, the critical one)", () => {
  test("interrupting the run while a step hangs STILL writes trace + result.json", async () => {
    let navigateStarted = false;
    const backend = controllableBackend(() =>
      stubPage({
        navigate: () =>
          new Promise<void>(() => {
            navigateStarted = true; // hang forever
          }),
      }),
    );

    const fiber = Effect.runFork(
      runQaSession({
        target: target(),
        brain: scriptedBrain(navSteps()),
        backend,
        artifactDir: dir,
        // no timeout — we interrupt EXTERNALLY (simulates SIGINT / kill)
      }),
    );

    // wait until the step is actually in flight, then interrupt
    await new Promise<void>((resolve) => {
      const check = () => (navigateStarted ? resolve() : setTimeout(check, 1));
      check();
    });
    await Effect.runPromise(Fiber.interrupt(fiber));

    // The ensuring finalizer must have flushed artifacts despite the interrupt.
    expect(existsSync(join(dir, "trace.zip"))).toBe(true);
    expect(existsSync(join(dir, "video.webm"))).toBe(true);
    expect(existsSync(join(dir, "result.json"))).toBe(true);

    // result.json is honest: a partial/interrupted run is status fail, never pass
    const parsed = decodeQaRunResult(JSON.parse(readFileSync(join(dir, "result.json"), "utf8")));
    expect(parsed.status).toBe("fail");
    expect(parsed.failure).toContain("interrupted");
  });
});
