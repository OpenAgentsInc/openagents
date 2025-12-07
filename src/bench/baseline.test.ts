/**
 * Baseline Comparison System Tests
 */

import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { rmSync, mkdirSync } from "fs";
import { join } from "path";
import {
  BaselineStore,
  compareWithBaseline,
  createBaseline,
  formatComparisonMarkdown,
  formatTrendMarkdown,
  compareOrCreateBaseline,
  updateBaselineIfImproved,
  type BaselineRecord,
  type BaselineComparison,
} from "./baseline.js";
import type { TerminalBenchResults } from "./terminal-bench.js";

const TEST_GYM_DIR = "/tmp/baseline-test-gym";

// Helper to create mock Terminal-Bench results
const createMockResults = (
  passRate: number,
  model = "test-model",
  taskStatuses: Record<string, "pass" | "fail"> = {},
): TerminalBenchResults => {
  const total = Object.keys(taskStatuses).length || 10;
  const passed = Math.round(total * passRate);
  const failed = total - passed;

  // Generate task results
  const results = Object.entries(taskStatuses).map(([task_id, status]) => ({
    task_id,
    status,
    duration_ms: 1000,
    turns: 5,
    tokens_used: 1000,
  }));

  // Fill remaining with pass/fail based on rate
  if (results.length === 0) {
    for (let i = 0; i < passed; i++) {
      results.push({
        task_id: `task-${i}`,
        status: "pass" as const,
        duration_ms: 1000,
        turns: 5,
        tokens_used: 1000,
      });
    }
    for (let i = 0; i < failed; i++) {
      results.push({
        task_id: `task-${passed + i}`,
        status: "fail" as const,
        duration_ms: 1000,
        turns: 5,
        tokens_used: 1000,
      });
    }
  }

  return {
    suite_name: "test-suite",
    suite_version: "1.0.0",
    model,
    timestamp: new Date().toISOString(),
    results,
  summary: {
    total,
    passed,
    failed,
    timeout: 0,
    error: 0,
    skipped: 0,
    pass_rate: passRate,
    avg_duration_ms: 1000,
    avg_turns: 5,
    total_tokens: 1000 * total,
  },
};
};

const withTimestamp = (baseline: BaselineRecord, timestamp: string): BaselineRecord => ({
  ...baseline,
  timestamp,
});

const withSuiteName = (baseline: BaselineRecord, suiteName: string): BaselineRecord => ({
  ...baseline,
  suiteName,
});

describe("BaselineStore", () => {
  beforeEach(() => {
    try {
      rmSync(TEST_GYM_DIR, { recursive: true, force: true });
    } catch {}
    mkdirSync(TEST_GYM_DIR, { recursive: true });
  });

  afterEach(() => {
    try {
      rmSync(TEST_GYM_DIR, { recursive: true, force: true });
    } catch {}
  });

  test("saves and loads baselines", async () => {
    const store = new BaselineStore(TEST_GYM_DIR);
    const results = createMockResults(0.8);
    const baseline = createBaseline(results);

    await store.save(baseline);
    const loaded = await store.loadAll();

    expect(loaded.length).toBe(1);
    expect(loaded[0].model).toBe("test-model");
    expect(loaded[0].passRate).toBe(0.8);
  });

  test("getBaseline returns most recent for model", async () => {
    const store = new BaselineStore(TEST_GYM_DIR);

    // Save older baseline
    const older = withTimestamp(
      createBaseline(createMockResults(0.7)),
      new Date(Date.now() - 10000).toISOString(),
    );
    await store.save(older);

    // Save newer baseline
    const newer = createBaseline(createMockResults(0.9));
    await store.save(newer);

    const baseline = await store.getBaseline("test-model");
    expect(baseline).not.toBeNull();
    expect(baseline!.passRate).toBe(0.9);
  });

  test("getBaseline filters by suite name", async () => {
    const store = new BaselineStore(TEST_GYM_DIR);

    const suiteA = withSuiteName(createBaseline(createMockResults(0.8)), "suite-a");
    await store.save(suiteA);

    const suiteB = withSuiteName(createBaseline(createMockResults(0.6)), "suite-b");
    await store.save(suiteB);

    const baseline = await store.getBaseline("test-model", "suite-a");
    expect(baseline).not.toBeNull();
    expect(baseline!.suiteName).toBe("suite-a");
    expect(baseline!.passRate).toBe(0.8);
  });

  test("getHistory returns baselines in chronological order", async () => {
    const store = new BaselineStore(TEST_GYM_DIR);

    for (let i = 0; i < 5; i++) {
      const baseline = withTimestamp(
        createBaseline(createMockResults(0.5 + i * 0.1)),
        new Date(Date.now() + i * 1000).toISOString(),
      );
      await store.save(baseline);
    }

    const history = await store.getHistory({ model: "test-model" });
    expect(history.length).toBe(5);
    // Should be oldest first
    expect(history[0].passRate).toBe(0.5);
    expect(history[4].passRate).toBe(0.9);
  });

  test("getHistory respects limit", async () => {
    const store = new BaselineStore(TEST_GYM_DIR);

    for (let i = 0; i < 10; i++) {
      const baseline = withTimestamp(
        createBaseline(createMockResults(0.5 + i * 0.05)),
        new Date(Date.now() + i * 1000).toISOString(),
      );
      await store.save(baseline);
    }

    const history = await store.getHistory({ model: "test-model", limit: 3 });
    expect(history.length).toBe(3);
    // Should be the last 3 (most recent)
    expect(history[0].passRate).toBeCloseTo(0.85);
    expect(history[2].passRate).toBeCloseTo(0.95);
  });
});

describe("compareWithBaseline", () => {
  test("detects improvement", () => {
    const baseline = createBaseline(
      createMockResults(0.6, "test-model", {
        "task-1": "fail",
        "task-2": "pass",
        "task-3": "pass",
        "task-4": "fail",
        "task-5": "fail",
      }),
    );

    const current = createMockResults(0.8, "test-model", {
      "task-1": "pass", // improved
      "task-2": "pass", // unchanged
      "task-3": "pass", // unchanged
      "task-4": "pass", // improved
      "task-5": "fail", // unchanged
    });

    const comparison = compareWithBaseline(current, baseline);

    expect(comparison.verdict).toBe("improved");
    expect(comparison.improved).toBe(true);
    expect(comparison.regressed).toBe(false);
    expect(comparison.improvedTasks).toContain("task-1");
    expect(comparison.improvedTasks).toContain("task-4");
    expect(comparison.passRateDelta).toBeCloseTo(0.2);
  });

  test("detects regression", () => {
    const baseline = createBaseline(
      createMockResults(0.6, "test-model", {
        "task-1": "pass",
        "task-2": "pass",
        "task-3": "pass",
        "task-4": "fail",
        "task-5": "fail",
      }),
    );

    // Small regression: 60% -> 40% (2 tasks regressed, -20% delta)
    // This triggers "warning" severity (not "critical" which needs >= 3 regressions OR >= -10% delta)
    const current = createMockResults(0.4, "test-model", {
      "task-1": "fail", // regressed
      "task-2": "fail", // regressed
      "task-3": "pass", // unchanged
      "task-4": "fail", // unchanged
      "task-5": "fail", // unchanged
    });

    const comparison = compareWithBaseline(current, baseline);

    expect(comparison.verdict).toBe("regressed");
    expect(comparison.regressed).toBe(true);
    expect(comparison.improved).toBe(false);
    expect(comparison.regressedTasks).toContain("task-1");
    expect(comparison.regressedTasks).toContain("task-2");
    expect(comparison.regressionAlert).toBeDefined();
    // -20% delta triggers critical (>= -10% threshold)
    expect(comparison.regressionAlert!.severity).toBe("critical");
  });

  test("detects critical regression", () => {
    const baseline = createBaseline(
      createMockResults(0.9, "test-model", {
        "task-1": "pass",
        "task-2": "pass",
        "task-3": "pass",
        "task-4": "pass",
        "task-5": "pass",
        "task-6": "pass",
        "task-7": "pass",
        "task-8": "pass",
        "task-9": "fail",
        "task-10": "fail",
      }),
    );

    const current = createMockResults(0.5, "test-model", {
      "task-1": "fail", // regressed
      "task-2": "fail", // regressed
      "task-3": "fail", // regressed
      "task-4": "fail", // regressed
      "task-5": "pass",
      "task-6": "pass",
      "task-7": "pass",
      "task-8": "pass",
      "task-9": "fail",
      "task-10": "pass", // improved
    });

    const comparison = compareWithBaseline(current, baseline);

    expect(comparison.verdict).toBe("mixed");
    expect(comparison.regressionAlert).toBeDefined();
    expect(comparison.regressionAlert!.severity).toBe("critical");
    expect(comparison.regressedTasks.length).toBe(4);
  });

  test("detects unchanged", () => {
    const baseline = createBaseline(
      createMockResults(0.6, "test-model", {
        "task-1": "pass",
        "task-2": "pass",
        "task-3": "pass",
        "task-4": "fail",
        "task-5": "fail",
      }),
    );

    const current = createMockResults(0.6, "test-model", {
      "task-1": "pass",
      "task-2": "pass",
      "task-3": "pass",
      "task-4": "fail",
      "task-5": "fail",
    });

    const comparison = compareWithBaseline(current, baseline);

    expect(comparison.verdict).toBe("unchanged");
    expect(comparison.improved).toBe(false);
    expect(comparison.regressed).toBe(false);
    expect(comparison.improvedTasks.length).toBe(0);
    expect(comparison.regressedTasks.length).toBe(0);
  });

  test("detects mixed results", () => {
    const baseline = createBaseline(
      createMockResults(0.6, "test-model", {
        "task-1": "pass",
        "task-2": "fail",
        "task-3": "pass",
        "task-4": "fail",
        "task-5": "fail",
      }),
    );

    const current = createMockResults(0.6, "test-model", {
      "task-1": "fail", // regressed
      "task-2": "pass", // improved
      "task-3": "pass",
      "task-4": "fail",
      "task-5": "fail",
    });

    const comparison = compareWithBaseline(current, baseline);

    expect(comparison.verdict).toBe("mixed");
    expect(comparison.improved).toBe(true);
    expect(comparison.regressed).toBe(true);
    expect(comparison.improvedTasks).toContain("task-2");
    expect(comparison.regressedTasks).toContain("task-1");
  });
});

describe("createBaseline", () => {
  test("creates baseline with correct task results", () => {
    const results = createMockResults(0.7, "test-model", {
      "task-1": "pass",
      "task-2": "fail",
      "task-3": "pass",
    });

    const baseline = createBaseline(results);

    expect(baseline.model).toBe("test-model");
    expect(baseline.suiteName).toBe("test-suite");
    expect(baseline.passRate).toBe(0.7);
    expect(baseline.taskResults["task-1"]).toBe("pass");
    expect(baseline.taskResults["task-2"]).toBe("fail");
    expect(baseline.taskResults["task-3"]).toBe("pass");
  });

  test("includes git metadata", () => {
    const results = createMockResults(0.8);
    const baseline = createBaseline(results, {
      gitCommit: "abc123",
      gitBranch: "main",
      notes: "Test baseline",
    });

    expect(baseline.gitCommit).toBe("abc123");
    expect(baseline.gitBranch).toBe("main");
    expect(baseline.notes).toBe("Test baseline");
  });
});

describe("formatComparisonMarkdown", () => {
  test("formats improved comparison", () => {
    const comparison: BaselineComparison = {
      comparedAt: new Date().toISOString(),
      baseline: { id: "baseline-1", timestamp: "2024-01-01T00:00:00Z", passRate: 0.6 },
      current: { passRate: 0.8, passed: 8, total: 10 },
      passRateDelta: 0.2,
      passRateDeltaPercent: 33.3,
      improved: true,
      regressed: false,
      verdict: "improved",
      taskDeltas: [
        { taskId: "task-1", baseline: "fail", current: "pass", changed: true, improved: true, regressed: false },
      ],
      improvedTasks: ["task-1"],
      regressedTasks: [],
    };

    const markdown = formatComparisonMarkdown(comparison);

    expect(markdown).toContain("IMPROVED");
    expect(markdown).toContain("✅");
    expect(markdown).toContain("task-1");
    expect(markdown).toContain("60.0%");
    expect(markdown).toContain("80.0%");
  });

  test("formats regression with alert", () => {
    const comparison: BaselineComparison = {
      comparedAt: new Date().toISOString(),
      baseline: { id: "baseline-1", timestamp: "2024-01-01T00:00:00Z", passRate: 0.9 },
      current: { passRate: 0.5, passed: 5, total: 10 },
      passRateDelta: -0.4,
      passRateDeltaPercent: -44.4,
      improved: false,
      regressed: true,
      verdict: "regressed",
      taskDeltas: [
        { taskId: "task-1", baseline: "pass", current: "fail", changed: true, improved: false, regressed: true },
      ],
      improvedTasks: [],
      regressedTasks: ["task-1"],
      regressionAlert: {
        severity: "critical",
        message: "Critical regression detected",
        affectedTasks: ["task-1"],
      },
    };

    const markdown = formatComparisonMarkdown(comparison);

    expect(markdown).toContain("REGRESSED");
    expect(markdown).toContain("❌");
    expect(markdown).toContain("CRITICAL");
    expect(markdown).toContain("🚨");
  });
});

describe("formatTrendMarkdown", () => {
  test("formats trend with improvement", () => {
    const baselines: BaselineRecord[] = [
      {
        id: "b1",
        model: "test",
        suiteName: "suite",
        suiteVersion: "1.0",
        timestamp: "2024-01-01T00:00:00Z",
        passRate: 0.5,
        passed: 5,
        total: 10,
        taskResults: {},
      },
      {
        id: "b2",
        model: "test",
        suiteName: "suite",
        suiteVersion: "1.0",
        timestamp: "2024-01-02T00:00:00Z",
        passRate: 0.7,
        passed: 7,
        total: 10,
        taskResults: {},
      },
      {
        id: "b3",
        model: "test",
        suiteName: "suite",
        suiteVersion: "1.0",
        timestamp: "2024-01-03T00:00:00Z",
        passRate: 0.9,
        passed: 9,
        total: 10,
        taskResults: {},
      },
    ];

    const markdown = formatTrendMarkdown(baselines);

    expect(markdown).toContain("Pass Rate Trend");
    expect(markdown).toContain("📈"); // Upward trend
    expect(markdown).toContain("+40.0%"); // Overall improvement
    expect(markdown).toContain("50.0%");
    expect(markdown).toContain("90.0%");
  });

  test("handles empty baselines", () => {
    const markdown = formatTrendMarkdown([]);
    expect(markdown).toContain("No baseline data available");
  });
});

describe("compareOrCreateBaseline", () => {
  beforeEach(() => {
    try {
      rmSync(TEST_GYM_DIR, { recursive: true, force: true });
    } catch {}
    mkdirSync(TEST_GYM_DIR, { recursive: true });
  });

  afterEach(() => {
    try {
      rmSync(TEST_GYM_DIR, { recursive: true, force: true });
    } catch {}
  });

  test("creates new baseline when none exists", async () => {
    const store = new BaselineStore(TEST_GYM_DIR);
    const results = createMockResults(0.8);

    const { comparison, baseline, isNewBaseline } = await compareOrCreateBaseline(
      store,
      results,
    );

    expect(isNewBaseline).toBe(true);
    expect(comparison).toBeNull();
    expect(baseline.passRate).toBe(0.8);

    // Verify it was saved
    const saved = await store.getBaseline("test-model");
    expect(saved).not.toBeNull();
  });

  test("compares against existing baseline", async () => {
    const store = new BaselineStore(TEST_GYM_DIR);

    // Create initial baseline
    const initial = createMockResults(0.6, "test-model", {
      "task-1": "pass",
      "task-2": "fail",
    });
    await store.save(createBaseline(initial));

    // Compare new results
    const newResults = createMockResults(0.8, "test-model", {
      "task-1": "pass",
      "task-2": "pass", // improved
    });

    const { comparison, isNewBaseline } = await compareOrCreateBaseline(
      store,
      newResults,
    );

    expect(isNewBaseline).toBe(false);
    expect(comparison).not.toBeNull();
    expect(comparison!.verdict).toBe("improved");
  });
});

describe("updateBaselineIfImproved", () => {
  beforeEach(() => {
    try {
      rmSync(TEST_GYM_DIR, { recursive: true, force: true });
    } catch {}
    mkdirSync(TEST_GYM_DIR, { recursive: true });
  });

  afterEach(() => {
    try {
      rmSync(TEST_GYM_DIR, { recursive: true, force: true });
    } catch {}
  });

  test("updates baseline when improved", async () => {
    const store = new BaselineStore(TEST_GYM_DIR);

    // Create initial baseline with older timestamp
    const initial = createMockResults(0.6);
    const initialBaseline = withTimestamp(
      createBaseline(initial),
      new Date(Date.now() - 10000).toISOString(),
    );
    await store.save(initialBaseline);

    // Try to update with better results
    const better = createMockResults(0.8);
    const { updated, comparison, newBaseline } = await updateBaselineIfImproved(
      store,
      better,
    );

    expect(updated).toBe(true);
    expect(comparison).not.toBeNull();
    expect(newBaseline).not.toBeNull();
    expect(newBaseline!.passRate).toBe(0.8);

    // Verify there are now 2 baselines stored
    const all = await store.loadAll();
    expect(all.length).toBe(2);

    // Verify new baseline is the most recent (has later timestamp)
    const current = await store.getBaseline("test-model");
    expect(current!.passRate).toBe(0.8);
  });

  test("does not update when not improved enough", async () => {
    const store = new BaselineStore(TEST_GYM_DIR);

    // Create initial baseline
    const initial = createMockResults(0.8);
    await store.save(createBaseline(initial));

    // Try to update with slightly worse results
    const worse = createMockResults(0.75);
    const { updated, newBaseline } = await updateBaselineIfImproved(store, worse);

    expect(updated).toBe(false);
    expect(newBaseline).toBeNull();
  });

  test("respects minImprovement threshold", async () => {
    const store = new BaselineStore(TEST_GYM_DIR);

    // Create initial baseline
    const initial = createMockResults(0.8);
    await store.save(createBaseline(initial));

    // Slightly better but below threshold
    const slightlyBetter = createMockResults(0.81);
    const { updated: notUpdated } = await updateBaselineIfImproved(
      store,
      slightlyBetter,
      { minImprovement: 0.05 },
    );
    expect(notUpdated).toBe(false);

    // Significantly better
    const muchBetter = createMockResults(0.9);
    const { updated } = await updateBaselineIfImproved(
      store,
      muchBetter,
      { minImprovement: 0.05 },
    );
    expect(updated).toBe(true);
  });
});
