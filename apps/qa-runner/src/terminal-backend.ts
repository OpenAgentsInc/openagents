// Terminal/TUI execution backend for the qa-runner.
//
// The browser runner (runner.ts) drives a `BrowserSurface` and films a video.
// This is its terminal-surface sibling: it spawns a command/TUI inside a PTY,
// replays a `TerminalScenario` (wait-for-text -> send-input -> assert-on-
// snapshot) using the deterministic computer-use `TerminalView` (no sleeps),
// and records:
//   - a TEXT-SNAPSHOT TIMELINE: one entry per step with the on-screen snapshot
//     at that moment — the terminal analogue of per-step screenshots;
//   - a best-effort ASCIICAST (asciinema v2 JSONL) of the raw output stream — a
//     replayable "video" artifact (best-effort: it is text, always writable);
//   - a `result.json` that reuses the EXACT same public-safe `QaRunResult`
//     schema + tripwire the browser runner uses (brain="terminal-scenario",
//     backend="terminal"), so the brain/target/artifact contracts are unchanged.
//
// Determinism + fakes-in-CI: the PTY is injectable. Unit tests inject a fake
// `Pty` (scripted output, no real process, no network); the real path uses
// `makeNodePty` from probe-runtime. A failed assertion is a real red — the
// snapshot at the moment of failure is persisted and `status` is "fail".

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  makeNodePty,
  makeTerminalView,
  type Pty,
  type TerminalView,
} from "@openagentsinc/probe-runtime";
import { assertPublicSafeResult, type QaRunResult, type QaRunStep } from "./result";
import type { Target } from "./target";
import type { TerminalScenario, TerminalStep } from "./terminal-scenario";

/** One frame of the text-snapshot timeline: what the screen showed at a step. */
export interface TerminalSnapshotFrame {
  readonly index: number;
  readonly label: string;
  /** The normalized (ANSI-stripped) on-screen text at this step. */
  readonly snapshot: string;
}

export interface RunTerminalScenarioInput {
  readonly target: Target;
  readonly scenario: TerminalScenario;
  /** Directory artifacts (snapshot-timeline, asciicast, result.json) go in. */
  readonly artifactDir: string;
  /** Injectable PTY (tests inject a fake; defaults to a real subprocess PTY). */
  readonly pty?: Pty;
  /** Working directory for the spawned command (scope to a run workspace). */
  readonly cwd?: string;
  /** Per-wait failure timeout default (ms). Keeps an unmet wait from hanging. */
  readonly defaultTimeoutMs?: number;
  /** Injectable clock for deterministic result timestamps + asciicast times. */
  readonly now?: () => number;
}

export interface RunTerminalScenarioOutcome {
  readonly result: QaRunResult;
  readonly resultPath: string;
  readonly snapshotTimelinePath: string;
  readonly asciicastPath: string;
}

const SNAPSHOT_TIMELINE_FILE = "terminal-snapshots.json";
const ASCIICAST_FILE = "terminal.cast";

async function driveScenario(
  view: TerminalView,
  scenario: TerminalScenario,
  defaultTimeoutMs: number,
): Promise<{ steps: QaRunStep[]; frames: TerminalSnapshotFrame[]; failure?: string }> {
  const steps: QaRunStep[] = [];
  const frames: TerminalSnapshotFrame[] = [];
  let failure: string | undefined;

  const record = (
    index: number,
    kind: string,
    status: "ok" | "failed",
    label: string,
    detail?: Record<string, string | number | boolean>,
  ) => steps.push({ index, kind, label, status, ...(detail ? { detail } : {}) });

  const frame = (index: number, label: string) =>
    frames.push({ index, label, snapshot: view.snapshot() });

  for (let index = 0; index < scenario.steps.length; index++) {
    const step: TerminalStep = scenario.steps[index]!;
    try {
      switch (step.kind) {
        case "wait-for": {
          const r = await view.waitFor(
            step.condition,
            step.timeoutMs !== undefined ? { timeoutMs: step.timeoutMs } : { timeoutMs: defaultTimeoutMs },
          );
          frame(index, step.label ?? `wait ${step.condition.kind}`);
          if (r.met) record(index, "wait-for", "ok", step.label ?? `wait ${step.condition.kind}`);
          else {
            record(index, "wait-for", "failed", step.label ?? `wait ${step.condition.kind}`, {
              reason: `condition not met: ${JSON.stringify(step.condition)}`,
            });
            failure = `wait-for did not complete: ${JSON.stringify(step.condition)}`;
          }
          break;
        }
        case "send":
          view.send(step.input);
          // NEVER record the raw input (may be a credential); only its length.
          record(index, "send", "ok", step.label ?? "send input", { length: step.input.length });
          break;
        case "snapshot":
          frame(index, step.label ?? "snapshot");
          record(index, "snapshot", "ok", step.label ?? "snapshot");
          break;
        case "wait-exit": {
          const r = await view.waitForExit();
          frame(index, step.label ?? "wait-exit");
          record(index, "wait-exit", "ok", step.label ?? "wait-exit", {
            exitCode: r.exit?.code ?? -1,
          });
          break;
        }
        case "assert-contains": {
          const snap = view.snapshot();
          frame(index, step.label ?? `contains "${step.value}"`);
          if (snap.includes(step.value)) record(index, "assert-contains", "ok", step.label ?? `contains "${step.value}"`);
          else {
            record(index, "assert-contains", "failed", step.label ?? `contains "${step.value}"`, {
              reason: `expected snapshot to contain "${step.value}"`,
            });
            failure = `assert-contains failed: "${step.value}" not in snapshot`;
          }
          break;
        }
        case "assert-not-contains": {
          const snap = view.snapshot();
          frame(index, step.label ?? `not-contains "${step.value}"`);
          if (!snap.includes(step.value)) record(index, "assert-not-contains", "ok", step.label ?? `not-contains "${step.value}"`);
          else {
            record(index, "assert-not-contains", "failed", step.label ?? `not-contains "${step.value}"`, {
              reason: `expected snapshot NOT to contain "${step.value}"`,
            });
            failure = `assert-not-contains failed: "${step.value}" present in snapshot`;
          }
          break;
        }
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      record(index, step.kind, "failed", step.label ?? step.kind, { error: message });
      failure = `${step.kind} failed: ${message}`;
    }
    // capture-on-failure: stop on the first honest red so the final snapshot
    // reflects the broken state.
    if (failure) break;
  }

  return failure !== undefined ? { steps, frames, failure } : { steps, frames };
}

/**
 * Run a terminal scenario and emit artifacts. Resolves with the public-safe
 * result + artifact paths. Honest: a failed assertion or unmet wait yields a
 * non-passing result; no fabricated success.
 */
export async function runTerminalScenario(
  input: RunTerminalScenarioInput,
): Promise<RunTerminalScenarioOutcome> {
  const now = input.now ?? (() => Date.now());
  const defaultTimeoutMs = input.defaultTimeoutMs ?? 5000;
  mkdirSync(input.artifactDir, { recursive: true });

  const pty = input.pty ?? makeNodePty(input.cwd !== undefined ? { cwd: input.cwd } : {});
  const startedAtMs = now();
  const startedAt = new Date(startedAtMs);

  const session = pty.spawn(input.scenario.command, input.scenario.args);

  // Best-effort asciicast: record raw output events with relative timestamps.
  // This is the replayable "video" artifact. It is plain text, so it is always
  // writable (best-effort wrt a true video transcode, which a terminal lacks).
  const castEvents: Array<[number, "o", string]> = [];
  const unsubscribe = session.onData((chunk) => {
    castEvents.push([(now() - startedAtMs) / 1000, "o", chunk]);
  });

  const view = makeTerminalView(session, { defaultTimeoutMs });

  let drive: Awaited<ReturnType<typeof driveScenario>>;
  try {
    drive = await driveScenario(view, input.scenario, defaultTimeoutMs);
  } finally {
    unsubscribe();
    // Ensure the process is not left running on an early/failed exit path.
    view.kill();
  }

  const endedAt = new Date(now());

  // ── Artifact 1: text-snapshot timeline ────────────────────────────────────
  const snapshotTimelinePath = join(input.artifactDir, SNAPSHOT_TIMELINE_FILE);
  const snapshotTimeline = {
    schemaVersion: "openagents.qa_runner.terminal_snapshots.v1",
    scenario: input.scenario.name,
    frames: drive.frames,
  };
  assertPublicSafeResult(snapshotTimeline);
  writeFileSync(snapshotTimelinePath, `${JSON.stringify(snapshotTimeline, null, 2)}\n`);

  // ── Artifact 2: asciicast (asciinema v2 JSONL: header + output events) ─────
  const asciicastPath = join(input.artifactDir, ASCIICAST_FILE);
  const header = {
    version: 2,
    width: 80,
    height: 24,
    timestamp: Math.floor(startedAtMs / 1000),
    title: `qa-runner: ${input.scenario.name}`,
  };
  const castLines = [JSON.stringify(header), ...castEvents.map((e) => JSON.stringify(e))];
  writeFileSync(asciicastPath, `${castLines.join("\n")}\n`);

  // ── Artifact 3: result.json (same public-safe schema as the browser runner) ─
  const status: "pass" | "fail" = drive.failure === undefined ? "pass" : "fail";
  const result: QaRunResult = {
    schemaVersion: "openagents.qa_runner.result.v1",
    status,
    target: { name: input.target.name, baseUrl: input.target.baseUrl },
    brain: "terminal-scenario",
    backend: "terminal",
    startedAt: startedAt.toISOString(),
    endedAt: endedAt.toISOString(),
    durationMs: endedAt.getTime() - startedAt.getTime(),
    steps: drive.steps,
    artifacts: {
      // The asciicast is the replayable timeline; declared under `video` so the
      // shared QaRunArtifacts contract is unchanged, with a webm/mp4-free format
      // marker omitted (an .cast is its own format, not webm/mp4).
      screenshots: [SNAPSHOT_TIMELINE_FILE, ASCIICAST_FILE],
    },
    ...(drive.failure ? { failure: drive.failure } : {}),
  };
  assertPublicSafeResult(result);
  const resultPath = join(input.artifactDir, "result.json");
  writeFileSync(resultPath, `${JSON.stringify(result, null, 2)}\n`);

  return { result, resultPath, snapshotTimelinePath, asciicastPath };
}
