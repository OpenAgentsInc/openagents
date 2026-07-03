// Native-desktop execution backend — the REAL native-desktop driver.
//
// Issue #6199 (follow-up from #6186) asks for a real native-desktop driver
// (accessibility tree + screenshots) wired through the existing `backend.ts`
// `NativeDesktopDriver` seam, which #6186 left spec-only. This is that driver:
// it focuses a desktop app, reads its OS accessibility (AX) tree, synthesizes
// click/type, captures screenshots, and records a public-safe `QaRunResult`
// (`backend = "native-desktop"`) + a timeline + an AX-tree snapshot artifact,
// reusing the EXACT same `result.ts` schema + tripwire the browser/terminal
// runners use — so the brain/target/artifact contracts are unchanged.
//
// macOS is the implemented tier (osascript System Events AX + screencapture,
// optional cliclick). Windows is a typed spec only (`windowsNativeDesktopRuntime`).
//
// OWNER-GATED / ARMED-BY-ENV (default OFF), mirroring container-backend.ts:
//   The backend is INERT unless explicitly armed (`QA_NATIVE_DESKTOP=1`, or
//   `armed: true`). Driving a real desktop is a privileged action; it does not
//   turn itself on. When un-armed it throws `NativeDesktopNotArmedError`.
//
// HONEST ABOUT THE HELPER + PERMISSION:
//   When armed but the native runtime is not actually usable (helper binary
//   missing OR macOS Accessibility permission not granted), provisioning throws
//   `NativeDesktopUnavailableError`. It NEVER silently falls back and NEVER
//   fakes a green. The Accessibility permission is OWNER-GRANTABLE
//   (System Settings -> Privacy & Security -> Accessibility); see the doc.
//
// DETERMINISTIC IN CI:
//   The native runtime is injected (`NativeDesktopRuntime`). Unit tests pass a
//   FAKE runtime (scripted AX tree + screenshot, no real desktop, no permission)
//   proving the full focus -> read-AX -> assert -> screenshot -> teardown
//   lifecycle plus the armed/unarmed/unavailable branches.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import {
  assertPublicSafeResult,
  assertPublicSafeTextValues,
  type QaRunResult,
  type QaRunStep,
} from "./result";
import type { Target } from "./target";
import {
  macosNativeDesktopRuntime,
  type AxTreeSnapshot,
  type NativeAppTarget,
  type NativeDesktopOs,
  type NativeDesktopRuntime,
} from "./native-desktop-runtime";

const AX_TREE_FILE = "native-desktop-axtree.json";
const TIMELINE_FILE = "native-desktop-timeline.json";
const SCREENSHOT_PREFIX = "native-desktop";

/** One step in a native-desktop scenario. */
export type NativeDesktopStep =
  | { readonly kind: "focus"; readonly label?: string }
  | { readonly kind: "ax-snapshot"; readonly label?: string }
  | { readonly kind: "screenshot"; readonly label?: string }
  | { readonly kind: "click"; readonly selector: string; readonly label?: string }
  | { readonly kind: "type"; readonly text: string; readonly label?: string }
  | { readonly durationMs: number; readonly kind: "wait"; readonly label?: string }
  | {
      /** Assert the latest AX snapshot contains a node whose title/value matches `value`. */
      readonly kind: "assert-ax-contains";
      readonly value: string;
      readonly label?: string;
    }
  | {
      readonly kind: "assert-ax-not-contains";
      readonly value: string;
      readonly label?: string;
    };

export interface NativeDesktopScenario {
  /** Stable scenario name (lands in result.json). */
  readonly name: string;
  /** The desktop app to drive (e.g. "TextEdit"). */
  readonly app: string;
  /** Optional app process id when a smoke launched a specific app instance. */
  readonly appPid?: number;
  /** The ordered steps to replay. */
  readonly steps: ReadonlyArray<NativeDesktopStep>;
}

/** One frame of the timeline: a label + (optional) screenshot file at that step. */
export interface NativeDesktopFrame {
  readonly index: number;
  readonly label: string;
  /** Relative screenshot path when this frame captured one. */
  readonly screenshot?: string;
}

export class NativeDesktopNotArmedError extends Error {
  constructor() {
    super(
      "nativeDesktopBackend is not armed: the native-desktop driver " +
        "(real accessibility-tree + screenshot automation of a desktop app) is " +
        "owner-gated and OFF by default. Arm it explicitly with " +
        "QA_NATIVE_DESKTOP=1 (or { armed: true }).",
    );
    this.name = "NativeDesktopNotArmedError";
  }
}

export class NativeDesktopUnavailableError extends Error {
  constructor(engine: string) {
    super(
      `nativeDesktopBackend is armed but the native runtime "${engine}" is not ` +
        "usable on this host (helper binary missing, or — on macOS — " +
        "Accessibility permission not granted). It will NOT fall back or fake a " +
        "result. Grant Accessibility permission (System Settings -> Privacy & " +
        "Security -> Accessibility), or run un-armed.",
    );
    this.name = "NativeDesktopUnavailableError";
  }
}

export interface NativeDesktopBackendOptions {
  /**
   * Arm the backend. Defaults to reading `QA_NATIVE_DESKTOP` from `env`
   * ("1"/"true" => armed). Owner-gated: OFF unless explicitly set.
   */
  readonly armed?: boolean;
  /** Env source for the arming check (default `process.env`). */
  readonly env?: Readonly<Record<string, string | undefined>>;
  /** Injectable native runtime (tests inject a fake; default real macOS). */
  readonly runtime?: NativeDesktopRuntime;
  /** OS tier (informational; the implemented runtime is macOS). */
  readonly os?: NativeDesktopOs;
  /** Injectable clock for deterministic result timestamps. */
  readonly now?: () => number;
  /** Injectable wait hook for deterministic tests; production uses setTimeout. */
  readonly sleep?: (ms: number) => Promise<void>;
}

/** True when the env arms the native-desktop backend. */
export function isNativeDesktopArmed(
  env: Readonly<Record<string, string | undefined>> = process.env,
): boolean {
  const v = env.QA_NATIVE_DESKTOP;
  return v === "1" || v === "true";
}

export interface NativeDesktopOutcome {
  readonly result: QaRunResult;
  readonly resultPath: string;
  readonly axTreePath: string;
  readonly timelinePath: string;
}

/**
 * Flatten an AX tree's roles+titles+values into a searchable string for
 * assertions, so a scenario can assert on a role ("AXWindow"), a label
 * ("OK"), or a value alike.
 */
function flattenAxText(tree: AxTreeSnapshot): string {
  const out: string[] = [];
  const walk = (nodes: AxTreeSnapshot["nodes"]) => {
    for (const n of nodes) {
      out.push(n.role);
      if (n.title) out.push(n.title);
      if (n.value) out.push(n.value);
      if (n.children) walk(n.children);
    }
  };
  walk(tree.nodes);
  return out.join("\n");
}

const sleep = (ms: number): Promise<void> =>
  new Promise((resolveSleep) => setTimeout(resolveSleep, ms));

/**
 * Run a native-desktop scenario through the injected runtime and emit artifacts.
 * Owner-gated (armed) + honest about helper/permission availability. Resolves
 * with the public-safe result + artifact paths. A failed assertion is a real
 * red — status is "fail" and the failure is recorded.
 *
 * Teardown ALWAYS runs (even on a failed/early-exit path), so a focused app is
 * never left in an automation-locked state.
 */
export async function runNativeDesktopScenario(
  input: {
    readonly target: Target;
    readonly scenario: NativeDesktopScenario;
    /** Directory artifacts (axtree, timeline, screenshots, result.json) go in. */
    readonly artifactDir: string;
  },
  options: NativeDesktopBackendOptions = {},
): Promise<NativeDesktopOutcome> {
  const env = options.env ?? process.env;
  const armed = options.armed ?? isNativeDesktopArmed(env);
  if (!armed) throw new NativeDesktopNotArmedError();

  const runtime = options.runtime ?? macosNativeDesktopRuntime();
  if (!(await runtime.available())) {
    throw new NativeDesktopUnavailableError(runtime.name);
  }

  const now = options.now ?? (() => Date.now());
  const sleepImpl = options.sleep ?? sleep;
  const startedAt = new Date(now());
  mkdirSync(input.artifactDir, { recursive: true });

  const appTarget: NativeAppTarget = {
    app: input.scenario.app,
    ...(input.scenario.appPid === undefined ? {} : { pid: input.scenario.appPid }),
  };
  const steps: QaRunStep[] = [];
  const frames: NativeDesktopFrame[] = [];
  let lastTree: AxTreeSnapshot | undefined;
  let failure: string | undefined;
  let screenshotCount = 0;

  const record = (
    index: number,
    kind: string,
    status: "ok" | "failed",
    label: string,
    detail?: Record<string, string | number | boolean>,
  ) => steps.push({ index, kind, label, status, ...(detail ? { detail } : {}) });

  try {
    for (let index = 0; index < input.scenario.steps.length; index++) {
      const step = input.scenario.steps[index]!;
      try {
        switch (step.kind) {
          case "focus":
            await runtime.focus(appTarget);
            record(index, "focus", "ok", step.label ?? `focus ${input.scenario.app}`);
            break;
          case "ax-snapshot": {
            lastTree = await runtime.accessibilityTree(appTarget);
            record(index, "ax-snapshot", "ok", step.label ?? "ax-snapshot", {
              windows: lastTree.nodes.length,
            });
            break;
          }
          case "screenshot": {
            const file = `${SCREENSHOT_PREFIX}-${screenshotCount++}.png`;
            await runtime.screenshot(appTarget, join(input.artifactDir, file));
            frames.push({ index, label: step.label ?? "screenshot", screenshot: file });
            record(index, "screenshot", "ok", step.label ?? "screenshot", { file });
            break;
          }
          case "click":
            await runtime.click(appTarget, step.selector);
            record(index, "click", "ok", step.label ?? `click ${step.selector}`, {
              selector: step.selector,
            });
            break;
          case "type":
            await runtime.type(appTarget, step.text);
            // NEVER record the raw typed text (may be a credential); length only.
            record(index, "type", "ok", step.label ?? "type", { length: step.text.length });
            break;
          case "wait": {
            const durationMs = Math.max(0, Math.min(30_000, step.durationMs));
            await sleepImpl(durationMs);
            record(index, "wait", "ok", step.label ?? `wait ${durationMs}ms`, { durationMs });
            break;
          }
          case "assert-ax-contains": {
            const hay = lastTree ? flattenAxText(lastTree) : "";
            if (!lastTree) {
              record(index, "assert-ax-contains", "failed", step.label ?? `ax contains "${step.value}"`, {
                reason: "no ax-snapshot taken before assertion",
              });
              failure = "assert-ax-contains before any ax-snapshot";
            } else if (hay.includes(step.value)) {
              record(index, "assert-ax-contains", "ok", step.label ?? `ax contains "${step.value}"`);
            } else {
              record(index, "assert-ax-contains", "failed", step.label ?? `ax contains "${step.value}"`, {
                reason: `expected AX tree to contain "${step.value}"`,
              });
              failure = `assert-ax-contains failed: "${step.value}" not in AX tree`;
            }
            break;
          }
          case "assert-ax-not-contains": {
            const hay = lastTree ? flattenAxText(lastTree) : "";
            if (!hay.includes(step.value)) {
              record(index, "assert-ax-not-contains", "ok", step.label ?? `ax not-contains "${step.value}"`);
            } else {
              record(index, "assert-ax-not-contains", "failed", step.label ?? `ax not-contains "${step.value}"`, {
                reason: `expected AX tree NOT to contain "${step.value}"`,
              });
              failure = `assert-ax-not-contains failed: "${step.value}" present in AX tree`;
            }
            break;
          }
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        record(index, step.kind, "failed", step.label ?? step.kind, { error: message });
        failure = `${step.kind} failed: ${message}`;
      }
      // capture-on-failure: stop on the first honest red.
      if (failure) break;
    }
  } finally {
    // Never leave the app in an automation-locked state.
    await runtime.teardown(appTarget).catch(() => undefined);
  }

  const endedAt = new Date(now());

  // ── Artifact 1: AX-tree snapshot ──────────────────────────────────────────
  const axTreePath = join(input.artifactDir, AX_TREE_FILE);
  const axTreeArtifact = {
    schemaVersion: "openagents.qa_runner.native_desktop_axtree.v1",
    scenario: input.scenario.name,
    app: input.scenario.app,
    tree: lastTree ?? { app: input.scenario.app, nodes: [] },
  };
  assertPublicSafeResult(axTreeArtifact);
  assertPublicSafeTextValues(axTreeArtifact);
  writeFileSync(axTreePath, `${JSON.stringify(axTreeArtifact, null, 2)}\n`);

  // ── Artifact 2: timeline (per-step labels + screenshot files) ─────────────
  const timelinePath = join(input.artifactDir, TIMELINE_FILE);
  const timelineArtifact = {
    schemaVersion: "openagents.qa_runner.native_desktop_timeline.v1",
    scenario: input.scenario.name,
    frames,
  };
  assertPublicSafeResult(timelineArtifact);
  assertPublicSafeTextValues(timelineArtifact);
  writeFileSync(timelinePath, `${JSON.stringify(timelineArtifact, null, 2)}\n`);

  // ── Artifact 3: result.json (same public-safe schema as browser/terminal) ──
  const status: "pass" | "fail" = failure === undefined ? "pass" : "fail";
  const screenshots = [
    AX_TREE_FILE,
    TIMELINE_FILE,
    ...frames.filter((f) => f.screenshot).map((f) => f.screenshot!),
  ];
  const result: QaRunResult = {
    schemaVersion: "openagents.qa_runner.result.v1",
    status,
    target: { name: input.target.name, baseUrl: input.target.baseUrl },
    brain: "native-desktop-scenario",
    backend: "native-desktop",
    startedAt: startedAt.toISOString(),
    endedAt: endedAt.toISOString(),
    durationMs: endedAt.getTime() - startedAt.getTime(),
    steps,
    artifacts: { screenshots },
    ...(failure ? { failure } : {}),
  };
  assertPublicSafeResult(result);
  assertPublicSafeTextValues(result);
  const resultPath = join(input.artifactDir, "result.json");
  writeFileSync(resultPath, `${JSON.stringify(result, null, 2)}\n`);

  return { result, resultPath, axTreePath, timelinePath };
}

/**
 * The shipped deterministic example scenario: focus an app, read its AX tree,
 * screenshot it, and assert the AX tree carries the app's own window. Pointing
 * the assertion at text the app never shows (see `nativeDesktopExampleWrong`)
 * FAILS honestly. The app name is a parameter so the real proof can target a
 * lightweight always-present app (e.g. "Finder").
 */
export function nativeDesktopExample(app: string): NativeDesktopScenario {
  return {
    name: "native-desktop-ax-probe",
    app,
    steps: [
      { kind: "focus", label: `focus ${app}` },
      { kind: "ax-snapshot", label: "read AX tree" },
      { kind: "screenshot", label: "screenshot the desktop" },
      { kind: "assert-ax-contains", value: "AXWindow", label: "AX tree has a window" },
    ],
  };
}

/**
 * A deliberately-wrong variant: asserts the AX tree contains text the app never
 * exposes. Proves a red is a real red (the failed assertion is recorded).
 */
export function nativeDesktopExampleWrong(app: string): NativeDesktopScenario {
  return {
    name: "native-desktop-ax-probe-wrong",
    app,
    steps: [
      { kind: "focus", label: `focus ${app}` },
      { kind: "ax-snapshot", label: "read AX tree" },
      {
        kind: "assert-ax-contains",
        value: "THIS-NODE-WILL-NEVER-EXIST-12345",
        label: "ax contains impossible node (intentionally wrong)",
      },
    ],
  };
}
