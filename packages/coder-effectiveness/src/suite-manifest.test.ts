/**
 * The suite pin and the smoke rule.
 *
 * Two things are under test here and they are not the same thing. The digest
 * tests ask whether two runs that claim to be comparable really ran the same
 * work. The classification tests ask whether a run that did not cover its suite
 * can present itself as one that did — the question the store's refusal and the
 * gate's fourth criterion both hang off.
 */

import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vite-plus/test";

import {
  classifyRun,
  parseSuiteManifest,
  SUITE_MANIFEST_SCHEMA,
  type SuiteManifest,
  type SuiteTask,
  suiteDigestOf,
  taskDigestOf,
} from "./suite-manifest.ts";

const suitesDir = fileURLToPath(new URL("../../../bench/suites", import.meta.url));

const readSuite = (name: string): unknown =>
  JSON.parse(readFileSync(`${suitesDir}/${name}`, "utf8"));

const registryTask = (id: string, overrides: Partial<{ commit: string; path: string }> = {}) => ({
  id,
  pin: {
    kind: "harbor-registry" as const,
    dataset: "terminal-bench@2.0",
    gitUrl: "https://github.com/laude-institute/terminal-bench-2.git",
    commit: overrides.commit ?? "69671fbaac6d67a7ef0dfec016cc38a64ef7a77c",
    path: overrides.path ?? id,
  },
  environmentAvailable: true,
});

const suite = (overrides: Partial<SuiteManifest> = {}): SuiteManifest =>
  parseSuiteManifest({
    schema: SUITE_MANIFEST_SCHEMA,
    id: "test-suite",
    tier: "score",
    description: "two tasks",
    tasks: [registryTask("regex-log"), registryTask("fix-code-vulnerability")],
    ...overrides,
  });

describe("parseSuiteManifest", () => {
  test("reads every checked-in suite manifest", () => {
    const files = readdirSync(suitesDir).filter((name) => name.endsWith(".suite.json"));

    expect(files.length).toBeGreaterThan(0);
    for (const file of files) {
      const manifest = parseSuiteManifest(readSuite(file));
      expect(manifest.id).toBe(file.replace(".suite.json", ""));
      expect(manifest.tasks.length).toBeGreaterThan(0);
    }
  });

  test("the headline suite holds the 20-30 tasks issue #34 asks for", () => {
    const manifest = parseSuiteManifest(readSuite("coder-effectiveness-v1.suite.json"));

    expect(manifest.tier).toBe("score");
    expect(manifest.tasks.length).toBeGreaterThanOrEqual(20);
    expect(manifest.tasks.length).toBeLessThanOrEqual(30);
    // Both sources the issue names: the terminal-bench cross-section and the
    // bounded public SWE-bench subset.
    const datasets = new Set(
      manifest.tasks.map((task) => (task.pin.kind === "harbor-registry" ? task.pin.dataset : "")),
    );
    expect(datasets).toContain("terminal-bench@2.0");
    expect(datasets).toContain("swebench-verified@1.0");
  });

  test("the owned suite pins each task to the commit that closed its issue", () => {
    const manifest = parseSuiteManifest(readSuite("owned-closed-issues.suite.json"));

    for (const task of manifest.tasks) {
      expect(task.pin.kind).toBe("tracker-closed-issue");
      if (task.pin.kind !== "tracker-closed-issue") continue;
      expect(task.pin.repo).toBe("OpenAgentsInc/openagents");
      expect(task.pin.acceptedCommit).toMatch(/^[0-9a-f]{40}$/u);
    }
  });

  test("refuses a score suite holding a task nobody has ever run", () => {
    // The owned tasks are pinned and real and have no container yet. A score
    // over them would read their absence as the coder failing.
    expect(() =>
      suite({ tasks: [{ ...registryTask("regex-log"), environmentAvailable: false }] }),
    ).toThrow(/no environment that can grade/u);
  });

  test("allows an unproven task in a smoke suite", () => {
    const manifest = suite({
      tier: "smoke",
      tasks: [{ ...registryTask("regex-log"), environmentAvailable: false }],
    });

    expect(manifest.tier).toBe("smoke");
  });

  test("refuses a suite with no tasks", () => {
    expect(() => suite({ tasks: [] })).toThrow(/covers itself/u);
  });

  test("refuses a suite that names one task twice", () => {
    expect(() => suite({ tasks: [registryTask("regex-log"), registryTask("regex-log")] })).toThrow(
      /twice/u,
    );
  });
});

describe("taskDigestOf", () => {
  test("changes when the task's content moves under the same name", () => {
    const before = taskDigestOf(registryTask("regex-log") as SuiteTask);
    const after = taskDigestOf(registryTask("regex-log", { commit: "0".repeat(40) }) as SuiteTask);

    // This is the whole reason a suite pins more than a name: `regex-log` at
    // one dataset commit and `regex-log` at another are different work.
    expect(after).not.toBe(before);
  });

  test("ignores the rationale, which is prose about the suite and not the task", () => {
    const bare = taskDigestOf(registryTask("regex-log") as SuiteTask);
    const annotated = taskDigestOf({
      ...(registryTask("regex-log") as SuiteTask),
      rationale: "the suite's fast lane",
    });

    expect(annotated).toBe(bare);
  });
});

describe("suiteDigestOf", () => {
  test("is stable across task order", () => {
    const forward = suite();
    const reversed = suite({ tasks: [...forward.tasks].toReversed() });

    expect(suiteDigestOf(reversed)).toBe(suiteDigestOf(forward));
  });

  test("changes when a task is added, removed, or repinned", () => {
    const base = suiteDigestOf(suite());

    expect(suiteDigestOf(suite({ tasks: [registryTask("regex-log")] }))).not.toBe(base);
    expect(
      suiteDigestOf(
        suite({
          tasks: [
            registryTask("regex-log", { commit: "1".repeat(40) }),
            registryTask("fix-code-vulnerability"),
          ],
        }),
      ),
    ).not.toBe(base);
  });

  test("changes when the tier flips, because the claim changed", () => {
    // Same tasks, but one manifest's results may be published and the other's
    // may not. A digest that ignored the tier would let a stored row's pin
    // match a manifest that now says something else about it.
    expect(suiteDigestOf(suite({ tier: "smoke" }))).not.toBe(suiteDigestOf(suite()));
  });
});

describe("classifyRun", () => {
  test("a full run of a score suite is a score", () => {
    const result = classifyRun(suite(), ["regex-log", "fix-code-vulnerability"]);

    expect(result.tier).toBe("score");
    expect(result.smokeReasons).toEqual([]);
    expect(result.missing).toEqual([]);
  });

  test("a partial run is a smoke run, whatever it was called", () => {
    const result = classifyRun(suite(), ["regex-log"]);

    expect(result.tier).toBe("smoke");
    expect(result.missing).toEqual(["fix-code-vulnerability"]);
    expect(result.smokeReasons.map((reason) => reason.kind)).toEqual(["incomplete_coverage"]);
  });

  test("an extra task is its own finding, not the same as a missing one", () => {
    // Trimming a suite and substituting a task are different mistakes and the
    // messages send an operator to different places.
    const result = classifyRun(suite(), ["regex-log", "fix-code-vulnerability", "fix-git"]);

    expect(result.tier).toBe("smoke");
    expect(result.unexpected).toEqual(["fix-git"]);
    expect(result.smokeReasons.map((reason) => reason.kind)).toEqual(["unexpected_tasks"]);
  });

  test("a declared smoke suite stays smoke even when it ran completely", () => {
    const result = classifyRun(suite({ tier: "smoke" }), ["regex-log", "fix-code-vulnerability"]);

    expect(result.tier).toBe("smoke");
    expect(result.missing).toEqual([]);
    expect(result.smokeReasons.map((reason) => reason.kind)).toEqual(["declared_smoke"]);
  });

  test("counts a task once however many trials it produced", () => {
    const result = classifyRun(suite(), ["regex-log", "regex-log", "fix-code-vulnerability"]);

    expect(result.tier).toBe("score");
    expect(result.ran).toEqual(["fix-code-vulnerability", "regex-log"]);
  });

  test("the checked-in smoke suite classifies as smoke on a complete run", () => {
    const manifest = parseSuiteManifest(readSuite("smoke.suite.json"));
    const result = classifyRun(
      manifest,
      manifest.tasks.map((task) => task.id),
    );

    expect(result.tier).toBe("smoke");
  });
});
