// Native-desktop backend tests.
//
// All deterministic + NO real desktop / NO Accessibility permission required
// (a FAKE `NativeDesktopRuntime` is injected):
//   - armed + runtime-available: full focus -> read-AX -> screenshot -> assert
//     -> teardown; AX-tree + timeline + public-safe result.json all written.
//   - UN-armed (default): refuses with NativeDesktopNotArmedError; the runtime
//     is never even touched.
//   - armed but the runtime UNAVAILABLE (helper/permission absent): refuses
//     honestly with NativeDesktopUnavailableError; no fallback, no fake green.
//   - a wrong assertion FAILS honestly (real red), with teardown still run.
//   - never records raw typed text (credential) in the result.
//   - the real-driver adapter (`nativeDesktopDriverFromRuntime`) delegates.
//
// A REAL macOS proof (focus an app, read its AX tree, screenshot) runs ONLY
// when macOS Accessibility permission is already granted on this host; it
// skip-lives otherwise and says so (the permission is owner-grantable).

import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";

import { nativeDesktopDriverFromRuntime } from "./backend";
import {
  isNativeDesktopArmed,
  NativeDesktopNotArmedError,
  NativeDesktopUnavailableError,
  nativeDesktopExample,
  nativeDesktopExampleWrong,
  runNativeDesktopScenario,
  type NativeDesktopScenario,
} from "./native-desktop-backend";
import {
  macosNativeDesktopRuntime,
  parseAxDump,
  type AxTreeSnapshot,
  type NativeAppTarget,
  type NativeDesktopRuntime,
} from "./native-desktop-runtime";
import { decodeQaRunResult } from "./result";
import { makeTarget } from "./target";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-native-desktop-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const target = makeTarget({ name: "native-target", baseUrl: "https://example.test" });

// ── A deterministic fake native-desktop runtime (no desktop, no permission) ───
interface FakeLog {
  readonly events: string[];
  tornDown: boolean;
}

function makeFakeRuntime(opts: {
  readonly available?: boolean;
  /** AX tree the fake returns from accessibilityTree(). */
  readonly tree?: AxTreeSnapshot;
  /** When set, click() throws this for any selector (honest not-found). */
  readonly clickThrows?: string;
  readonly log?: FakeLog;
}): NativeDesktopRuntime {
  const log = opts.log ?? { events: [], tornDown: false };
  const tree: AxTreeSnapshot = opts.tree ?? {
    app: "FakeApp",
    nodes: [
      {
        role: "AXWindow",
        title: "Untitled",
        children: [
          { role: "AXButton", title: "OK" },
          { role: "AXStaticText", value: "hello from fake AX" },
        ],
      },
    ],
  };
  return {
    name: "fake-native",
    os: "macos",
    available: async () => opts.available ?? true,
    focus: async ({ app }) => {
      log.events.push(`focus:${app}`);
    },
    accessibilityTree: async ({ app }) => {
      log.events.push(`ax:${app}`);
      return tree;
    },
    click: async ({ app }, selector) => {
      log.events.push(`click:${app}:${selector}`);
      if (opts.clickThrows) throw new Error(opts.clickThrows);
    },
    type: async ({ app }, text) => {
      log.events.push(`type:${app}:${text.length}`);
    },
    screenshot: async (_t, path) => {
      log.events.push(`shot:${path}`);
      // materialize a tiny placeholder so the artifact path exists.
      const { writeFileSync } = require("node:fs");
      writeFileSync(path, "PNGPLACEHOLDER");
      return path;
    },
    teardown: async ({ app }) => {
      log.events.push(`teardown:${app}`);
      log.tornDown = true;
    },
  };
}

describe("isNativeDesktopArmed", () => {
  test("off by default; on only for explicit 1/true", () => {
    expect(isNativeDesktopArmed({})).toBe(false);
    expect(isNativeDesktopArmed({ QA_NATIVE_DESKTOP: "0" })).toBe(false);
    expect(isNativeDesktopArmed({ QA_NATIVE_DESKTOP: "1" })).toBe(true);
    expect(isNativeDesktopArmed({ QA_NATIVE_DESKTOP: "true" })).toBe(true);
  });
});

describe("parseAxDump", () => {
  test("parses the tab/return-delimited dump into a bounded snapshot", () => {
    const dump = ["1\tAXWindow\tUntitled\t", "2\tAXButton\tOK\t", "2\tAXStaticText\t\thi"].join("\n");
    const snap = parseAxDump("TextEdit", dump);
    expect(snap.app).toBe("TextEdit");
    expect(snap.nodes.length).toBe(1);
    expect(snap.nodes[0]!.role).toBe("AXWindow");
    expect(snap.nodes[0]!.title).toBe("Untitled");
    expect(snap.nodes[0]!.children?.length).toBe(2);
    expect(snap.nodes[0]!.children?.[1]!.value).toBe("hi");
  });

  test("an ERR dump throws (honest, no empty fake tree)", () => {
    expect(() => parseAxDump("Nope", "ERR\tno-process")).toThrow(/no-process/);
  });
});

describe("runNativeDesktopScenario (fake runtime)", () => {
  test("armed: focus -> ax -> screenshot -> assert -> teardown; artifacts + public-safe result", async () => {
    const log: FakeLog = { events: [], tornDown: false };
    const runtime = makeFakeRuntime({ available: true, log });
    const outcome = await runNativeDesktopScenario(
      { target, scenario: nativeDesktopExample("FakeApp"), artifactDir: dir },
      { armed: true, runtime, now: () => 1_000_000 },
    );

    expect(outcome.result.status).toBe("pass");
    expect(outcome.result.backend).toBe("native-desktop");
    expect(outcome.result.brain).toBe("native-desktop-scenario");

    // lifecycle order: focus -> ax -> shot -> teardown
    expect(log.events[0]).toBe("focus:FakeApp");
    expect(log.events.some((e) => e.startsWith("ax:"))).toBe(true);
    expect(log.events.some((e) => e.startsWith("shot:"))).toBe(true);
    expect(log.tornDown).toBe(true);

    // result.json round-trips through the SHARED public-safe schema.
    const decoded = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    expect(decoded.status).toBe("pass");
    expect(decoded.backend).toBe("native-desktop");
    expect(decoded.artifacts.screenshots).toContain("native-desktop-axtree.json");
    expect(decoded.artifacts.screenshots).toContain("native-desktop-timeline.json");
    expect(decoded.artifacts.screenshots.some((s) => s.endsWith(".png"))).toBe(true);

    // AX-tree artifact exists with the snapshot.
    const ax = JSON.parse(readFileSync(outcome.axTreePath, "utf8"));
    expect(ax.app).toBe("FakeApp");
    expect(ax.tree.nodes[0].role).toBe("AXWindow");

    // timeline artifact exists with a screenshot frame.
    const tl = JSON.parse(readFileSync(outcome.timelinePath, "utf8"));
    expect(tl.frames.length).toBeGreaterThan(0);
    expect(existsSync(join(dir, tl.frames[0].screenshot))).toBe(true);
  });

  test("headed artifact metadata trips the public-safety text oracle", async () => {
    const runtime = makeFakeRuntime({
      available: true,
      tree: {
        app: "FakeApp",
        nodes: [
          {
            role: "AXWindow",
            title: "Unsafe /Users/operator/.codex/auth.json",
          },
        ],
      },
    });
    await expect(
      runNativeDesktopScenario(
        { target, scenario: nativeDesktopExample("FakeApp"), artifactDir: dir },
        { armed: true, runtime },
      ),
    ).rejects.toThrow("public_safety_violation");
  });

  test("a wrong assertion FAILS honestly (real red); teardown still runs", async () => {
    const log: FakeLog = { events: [], tornDown: false };
    const runtime = makeFakeRuntime({ available: true, log });
    const outcome = await runNativeDesktopScenario(
      { target, scenario: nativeDesktopExampleWrong("FakeApp"), artifactDir: dir },
      { armed: true, runtime },
    );
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toContain("THIS-NODE-WILL-NEVER-EXIST-12345");
    expect(outcome.result.steps.some((s) => s.status === "failed")).toBe(true);
    // teardown still ran despite the red.
    expect(log.tornDown).toBe(true);
  });

  test("assert-ax-contains matches a node value in the AX tree", async () => {
    const runtime = makeFakeRuntime({ available: true });
    const scenario: NativeDesktopScenario = {
      name: "value-assert",
      app: "FakeApp",
      steps: [
        { kind: "focus" },
        { kind: "ax-snapshot" },
        { kind: "assert-ax-contains", value: "hello from fake AX" },
      ],
    };
    const outcome = await runNativeDesktopScenario({ target, scenario, artifactDir: dir }, { armed: true, runtime });
    expect(outcome.result.status).toBe("pass");
  });

  test("never records the raw typed text (credential) in the result", async () => {
    const runtime = makeFakeRuntime({ available: true });
    const scenario: NativeDesktopScenario = {
      name: "typed-redaction",
      app: "FakeApp",
      steps: [
        { kind: "focus" },
        { kind: "type", text: "hunter2-super-secret", label: "type password" },
      ],
    };
    const outcome = await runNativeDesktopScenario({ target, scenario, artifactDir: dir }, { armed: true, runtime });
    const serialized = JSON.stringify(outcome.result);
    expect(serialized).not.toContain("hunter2-super-secret");
    expect(serialized).toContain('"length":20');
  });

  test("wait steps use the injected sleep hook and record bounded duration", async () => {
    const waits: number[] = [];
    const runtime = makeFakeRuntime({ available: true });
    const scenario: NativeDesktopScenario = {
      name: "wait-step",
      app: "FakeApp",
      steps: [
        { kind: "focus" },
        { durationMs: 42, kind: "wait", label: "settle app" },
        { durationMs: 60_000, kind: "wait", label: "clamped wait" },
      ],
    };
    const outcome = await runNativeDesktopScenario(
      { target, scenario, artifactDir: dir },
      { armed: true, runtime, sleep: async (ms) => { waits.push(ms); } },
    );
    expect(outcome.result.status).toBe("pass");
    expect(waits).toEqual([42, 30_000]);
    expect(outcome.result.steps.filter((step) => step.kind === "wait").map((step) => step.detail)).toEqual([
      { durationMs: 42 },
      { durationMs: 30_000 },
    ]);
  });

  test("a click that cannot be addressed FAILS honestly (no silent no-op)", async () => {
    const runtime = makeFakeRuntime({ available: true, clickThrows: "AX element not found" });
    const scenario: NativeDesktopScenario = {
      name: "click-missing",
      app: "FakeApp",
      steps: [{ kind: "focus" }, { kind: "click", selector: "AXButton:Nope" }],
    };
    const outcome = await runNativeDesktopScenario({ target, scenario, artifactDir: dir }, { armed: true, runtime });
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toContain("click failed");
  });
});

describe("owner-gating + availability honesty", () => {
  test("UN-armed (default): refuses, runtime never touched", async () => {
    const log: FakeLog = { events: [], tornDown: false };
    const runtime = makeFakeRuntime({ available: true, log });
    await expect(
      runNativeDesktopScenario(
        { target, scenario: nativeDesktopExample("FakeApp"), artifactDir: dir },
        { armed: false, runtime },
      ),
    ).rejects.toBeInstanceOf(NativeDesktopNotArmedError);
    expect(log.events.length).toBe(0);
    expect(existsSync(join(dir, "result.json"))).toBe(false);
  });

  test("UN-armed via env (no QA_NATIVE_DESKTOP) refuses", async () => {
    const runtime = makeFakeRuntime({ available: true });
    await expect(
      runNativeDesktopScenario(
        { target, scenario: nativeDesktopExample("FakeApp"), artifactDir: dir },
        { runtime, env: {} },
      ),
    ).rejects.toBeInstanceOf(NativeDesktopNotArmedError);
  });

  test("armed via env QA_NATIVE_DESKTOP=1 + fake runtime: runs", async () => {
    const runtime = makeFakeRuntime({ available: true });
    const outcome = await runNativeDesktopScenario(
      { target, scenario: nativeDesktopExample("FakeApp"), artifactDir: dir },
      { runtime, env: { QA_NATIVE_DESKTOP: "1" } },
    );
    expect(existsSync(join(outcome.resultPath))).toBe(true);
  });

  test("armed but runtime UNAVAILABLE (helper/permission absent): refuses honestly, no fake green", async () => {
    const log: FakeLog = { events: [], tornDown: false };
    const runtime = makeFakeRuntime({ available: false, log });
    await expect(
      runNativeDesktopScenario(
        { target, scenario: nativeDesktopExample("FakeApp"), artifactDir: dir },
        { armed: true, runtime },
      ),
    ).rejects.toBeInstanceOf(NativeDesktopUnavailableError);
    // never focused an app, no artifacts fabricated.
    expect(log.events.length).toBe(0);
    expect(existsSync(join(dir, "result.json"))).toBe(false);
  });
});

describe("nativeDesktopDriverFromRuntime (real-driver adapter)", () => {
  test("delegates AX/screenshot/click/type/teardown to the runtime", async () => {
    const log: FakeLog = { events: [], tornDown: false };
    const runtime = makeFakeRuntime({ available: true, log });
    const appTarget: NativeAppTarget = { app: "FakeApp" };
    const driver = nativeDesktopDriverFromRuntime(runtime, appTarget);
    expect(driver.os).toBe("macos");

    const tree = (await driver.accessibilityTree()) as AxTreeSnapshot;
    expect(tree.nodes[0]!.role).toBe("AXWindow");
    await driver.screenshot(join(dir, "drv.png"));
    await driver.click("AXButton:OK");
    await driver.type("hi");
    await driver.teardown();

    expect(log.events).toEqual([
      "ax:FakeApp",
      `shot:${join(dir, "drv.png")}`,
      "click:FakeApp:AXButton:OK",
      "type:FakeApp:2",
      "teardown:FakeApp",
    ]);
    expect(log.tornDown).toBe(true);
  });
});

// ── REAL macOS proof — runs ONLY if Accessibility permission is granted ───────
describe("runNativeDesktopScenario (real macOS, one proof)", () => {
  test("focuses Finder, reads its AX tree, screenshots, asserts a window", async () => {
    // OPT-IN only: this drives a live Finder window via the macOS Accessibility
    // API, which needs a real GUI session + granted Accessibility permission.
    // It is NOT reliable in headless / CI / subagent contexts, so it is skipped
    // unless explicitly enabled — otherwise it is a false red on every qa-runner
    // `bun run test`. Run it with QA_NATIVE_DESKTOP_LIVE=1 on a permissioned Mac.
    if (process.env.QA_NATIVE_DESKTOP_LIVE !== "1") {
      console.log("[skip-live] set QA_NATIVE_DESKTOP_LIVE=1 (permissioned macOS GUI) to run the real native-desktop AX proof");
      return;
    }
    if (process.platform !== "darwin") {
      console.log("[skip-live] not macOS; the real native-desktop proof is macOS-only");
      return;
    }
    const runtime = macosNativeDesktopRuntime();
    const available = await runtime.available();
    if (!available) {
      // Honest skip: macOS Accessibility permission not granted to the test
      // runner. The fake-runtime tests above prove the full lifecycle; this
      // would prove it against the real OS. The permission is OWNER-GRANTABLE:
      // System Settings -> Privacy & Security -> Accessibility.
      console.log(
        "[skip-live] macOS Accessibility permission not granted; skipping real native-desktop proof " +
          "(owner-grantable: System Settings -> Privacy & Security -> Accessibility)",
      );
      return;
    }
    // Finder is always running and lightweight to focus + read.
    const outcome = await runNativeDesktopScenario(
      { target, scenario: nativeDesktopExample("Finder"), artifactDir: dir },
      { armed: true, runtime },
    );
    expect(outcome.result.status).toBe("pass");
    expect(outcome.result.backend).toBe("native-desktop");
    const decoded = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    expect(decoded.status).toBe("pass");
    expect(existsSync(outcome.axTreePath)).toBe(true);
    // a real PNG screenshot was written.
    const shot = decoded.artifacts.screenshots.find((s) => s.endsWith(".png"));
    expect(shot).toBeDefined();
    expect(existsSync(join(dir, shot!))).toBe(true);
  }, 60_000);
});
