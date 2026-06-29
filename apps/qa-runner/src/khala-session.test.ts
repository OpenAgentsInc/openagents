// Khala session runner tests (fakes, no network/chromium): a fake ChatClient
// feeds scripted JSON actions and a fake chromium provides page state, proving:
//   - Khala's actions are executed against the computer-use surface,
//   - observations are fed back (the client sees a growing conversation),
//   - BOTH a public-safe result.json and a deterministic session-trace.json are
//     produced,
//   - honest failure on unparseable model output and on a false assertion
//     (never a fabricated pass).

import { mkdtempSync, readFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { Effect } from "effect";

import { localBackend } from "./backend";
import { makeFakeChromium } from "./fake-chromium";
import type { ChatClient, ChatMessage } from "./khala-driver";
import { runKhalaSession } from "./khala-session";
import { decodeQaRunResult } from "./result";
import { decodeSessionTrace, verifyTraceDigest } from "./session-trace";
import { makeTarget } from "./target";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "khala-session-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

/** A ChatClient that replays a fixed list of model replies, in order. */
function scriptedChat(replies: ReadonlyArray<string>): ChatClient & { seen: ChatMessage[][] } {
  let i = 0;
  const seen: ChatMessage[][] = [];
  return {
    seen,
    complete: async (messages) => {
      seen.push([...messages]);
      return replies[i++] ?? '{"action":"fail","reason":"out of script"}';
    },
  };
}

const passingChromium = () =>
  makeFakeChromium({
    pages: { "/login": { text: "Log in to OpenAgents", html: "<form>Log in to OpenAgents</form>" } },
  });

const target = () => makeTarget({ name: "fake-target", baseUrl: "https://example.test" });

describe("runKhalaSession (fake model + fake chromium)", () => {
  test("Khala drives the loop, executes actions, and produces result + trace (PASS)", async () => {
    const chat = scriptedChat([
      '{"action":"navigate","url":"/login"}',
      '{"action":"waitFor","condition":{"kind":"text-visible","value":"Log in to OpenAgents"}}',
      '{"action":"screenshot","label":"login-page"}',
      '{"action":"assert","label":"stays at /login","check":{"kind":"url-includes","value":"/login"}}',
      '{"action":"assert","label":"body contains \\"Log in to OpenAgents\\"","check":{"kind":"text-contains","value":"Log in to OpenAgents"}}',
      '{"action":"done","verdict":"pass","summary":"login page works"}',
    ]);

    const outcome = await Effect.runPromise(
      runKhalaSession({
        target: target(),
        backend: localBackend({ chromium: passingChromium() }),
        chat,
        goal: "verify the login page works",
        artifactDir: dir,
        log: () => undefined,
        now: () => new Date("2026-06-24T00:00:00.000Z"),
      }),
    );

    expect(outcome.verdict).toBe("pass");
    expect(outcome.result.status).toBe("pass");
    expect(outcome.result.brain).toBe("khala");

    // Observations were fed back: the model saw a growing conversation.
    expect(chat.seen.length).toBe(6);
    expect(chat.seen[1]!.length).toBeGreaterThan(chat.seen[0]!.length);
    expect(chat.seen.at(-1)!.some((m) => m.role === "user" && m.content.includes("Observation:"))).toBe(true);

    // result.json round-trips + is public-safe (the runner asserted it).
    const result = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    expect(result.steps.every((s) => s.status === "ok")).toBe(true);

    // session-trace.json round-trips + is deterministic.
    const trace = decodeSessionTrace(JSON.parse(readFileSync(outcome.tracePath, "utf8")));
    expect(verifyTraceDigest(trace)).toBe(true);
    expect(trace.beats.some((b) => b.kind === "verdict" && b.verificationClass === "test_passed")).toBe(true);
    expect(existsSync(join(dir, "trace.zip"))).toBe(true);
  });

  test("FAILS honestly when Khala asserts something false (no fabricated pass)", async () => {
    const chat = scriptedChat([
      '{"action":"navigate","url":"/login"}',
      // false: the page does NOT redirect away from /login
      '{"action":"assert","label":"redirects away from /login","check":{"kind":"url-not-includes","value":"/login"}}',
      '{"action":"done","verdict":"pass","summary":"claiming pass anyway"}',
    ]);
    const outcome = await Effect.runPromise(
      runKhalaSession({
        target: target(),
        backend: localBackend({ chromium: passingChromium() }),
        chat,
        goal: "verify the login page redirects",
        artifactDir: dir,
        log: () => undefined,
      }),
    );
    // The model said done/pass, but the false assertion stands: honest FAIL.
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toBeDefined();
    expect(outcome.result.steps.some((s) => s.status === "failed")).toBe(true);
  });

  test("FAILS honestly when Khala emits an unparseable action", async () => {
    // Both the first reply AND the corrective re-prompt are invalid -> honest
    // failure (the bounded reparse retry does not paper over a broken model).
    const chat = scriptedChat([
      "I will navigate to the login page now.",
      "Still just prose, no JSON object here either.",
    ]);
    const outcome = await Effect.runPromise(
      runKhalaSession({
        target: target(),
        backend: localBackend({ chromium: passingChromium() }),
        chat,
        goal: "verify something",
        artifactDir: dir,
        log: () => undefined,
      }),
    );
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toContain("unparseable");
  });

  test("reports 'incomplete' when the model never reaches a verdict (step cap)", async () => {
    // Always returns a navigate; never a done -> the cap ends it.
    const chat: ChatClient = { complete: async () => '{"action":"navigate","url":"/login"}' };
    const outcome = await Effect.runPromise(
      runKhalaSession({
        target: target(),
        backend: localBackend({ chromium: passingChromium() }),
        chat,
        goal: "loop forever",
        artifactDir: dir,
        maxTurns: 3,
        log: () => undefined,
      }),
    );
    expect(outcome.verdict).toBe("incomplete");
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toContain("step cap");
  });

  test("the typed text is never echoed into the trace or result (public-safe)", async () => {
    const chat = scriptedChat([
      '{"action":"navigate","url":"/login"}',
      '{"action":"type","selector":"#password","text":"hunter2-TOP-SECRET"}',
      '{"action":"done","verdict":"pass"}',
    ]);
    const outcome = await Effect.runPromise(
      runKhalaSession({
        target: target(),
        backend: localBackend({ chromium: passingChromium() }),
        chat,
        goal: "type a secret",
        artifactDir: dir,
        log: () => undefined,
      }),
    );
    const resultRaw = readFileSync(outcome.resultPath, "utf8");
    const traceRaw = readFileSync(outcome.tracePath, "utf8");
    expect(resultRaw).not.toContain("hunter2-TOP-SECRET");
    expect(traceRaw).not.toContain("hunter2-TOP-SECRET");
  });
});
