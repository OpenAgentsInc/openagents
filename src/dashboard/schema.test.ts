/**
 * Dashboard Schema Tests
 */

import { describe, test, expect } from "bun:test";
import {
  formatDuration,
  formatPercent,
  formatNumber,
  buildProgressBar,
  calculateHealth,
  createDashboardState,
  DEFAULT_PROGRESS_CONFIG,
  type LearningStats,
} from "./schema.js";

describe("formatDuration", () => {
  test("formats milliseconds", () => {
    expect(formatDuration(500)).toBe("500ms");
    expect(formatDuration(999)).toBe("999ms");
  });

  test("formats seconds", () => {
    expect(formatDuration(1000)).toBe("1.0s");
    expect(formatDuration(5500)).toBe("5.5s");
    expect(formatDuration(59000)).toBe("59.0s");
  });

  test("formats minutes", () => {
    expect(formatDuration(60000)).toBe("1m 0s");
    expect(formatDuration(90000)).toBe("1m 30s");
    expect(formatDuration(3599000)).toBe("59m 59s");
  });

  test("formats hours", () => {
    expect(formatDuration(3600000)).toBe("1h 0m");
    expect(formatDuration(5400000)).toBe("1h 30m");
    expect(formatDuration(7200000)).toBe("2h 0m");
  });
});

describe("formatPercent", () => {
  test("formats decimal to percentage", () => {
    expect(formatPercent(0)).toBe("0.0%");
    expect(formatPercent(0.5)).toBe("50.0%");
    expect(formatPercent(1)).toBe("100.0%");
    expect(formatPercent(0.333)).toBe("33.3%");
  });
});

describe("formatNumber", () => {
  test("formats small numbers directly", () => {
    expect(formatNumber(0)).toBe("0");
    expect(formatNumber(999)).toBe("999");
  });

  test("formats thousands with K", () => {
    expect(formatNumber(1000)).toBe("1.0K");
    expect(formatNumber(1500)).toBe("1.5K");
    expect(formatNumber(999999)).toBe("1000.0K");
  });

  test("formats millions with M", () => {
    expect(formatNumber(1000000)).toBe("1.0M");
    expect(formatNumber(2500000)).toBe("2.5M");
  });
});

describe("buildProgressBar", () => {
  test("builds empty progress bar", () => {
    const bar = buildProgressBar({ total: 100, completed: 0, width: 10, showPercent: false });
    expect(bar).toBe("[░░░░░░░░░░]");
  });

  test("builds full progress bar", () => {
    const bar = buildProgressBar({ total: 100, completed: 100, width: 10, showPercent: false });
    expect(bar).toBe("[██████████]");
  });

  test("builds partial progress bar", () => {
    const bar = buildProgressBar({ total: 100, completed: 50, width: 10, showPercent: false });
    expect(bar).toBe("[█████░░░░░]");
  });

  test("includes percentage when enabled", () => {
    const bar = buildProgressBar({ total: 100, completed: 50, width: 10, showPercent: true });
    expect(bar).toContain("50.0%");
  });

  test("uses default config", () => {
    const bar = buildProgressBar();
    expect(bar).toContain("[");
    expect(bar).toContain("]");
    expect(bar).toContain("0.0%");
  });
});

describe("calculateHealth", () => {
  const makeStats = (overrides?: Partial<LearningStats>): LearningStats => ({
    skills: { total: 50, bootstrapped: 40, learned: 10, byCategory: {} },
    memories: { total: 100, episodic: 50, semantic: 30, procedural: 20 },
    reflexion: { totalFailures: 5, totalReflections: 5, successfulReflections: 3, skillsLearned: 2 },
    archivist: { totalTrajectories: 10, unarchivedTrajectories: 2, successfulTrajectories: 8, patternsExtracted: 5, skillsCreated: 3 },
    training: { totalRuns: 5, totalTasksCompleted: 50, overallSuccessRate: 0.8, currentTier: "TB_10" },
    loop: null,
    ...overrides,
  });

  test("returns healthy for good stats", () => {
    const health = calculateHealth(makeStats());
    expect(health.status).toBe("healthy");
    expect(health.issues).toEqual([]);
  });

  test("returns degraded for low skill count", () => {
    const health = calculateHealth(makeStats({
      skills: { total: 5, bootstrapped: 5, learned: 0, byCategory: {} },
    }));
    expect(health.status).toBe("degraded");
    expect(health.issues.length).toBeGreaterThan(0);
  });

  test("warns about missing archivist trajectories", () => {
    const health = calculateHealth(makeStats({
      archivist: { totalTrajectories: 0, unarchivedTrajectories: 0, successfulTrajectories: 0, patternsExtracted: 0, skillsCreated: 0 },
    }));
    expect(health.issues.some(i => i.includes("Archivist"))).toBe(true);
  });
});

describe("createDashboardState", () => {
  test("creates default state", () => {
    const state = createDashboardState();
    expect(state.mode).toBe("compact");
    expect(state.interval).toBe("normal");
    expect(state.currentView).toBe("overview");
    expect(state.liveUpdates).toBe(false);
  });

  test("accepts overrides", () => {
    const state = createDashboardState({
      mode: "detailed",
      liveUpdates: true,
    });
    expect(state.mode).toBe("detailed");
    expect(state.liveUpdates).toBe(true);
  });
});

describe("DEFAULT_PROGRESS_CONFIG", () => {
  test("has expected defaults", () => {
    expect(DEFAULT_PROGRESS_CONFIG.width).toBe(40);
    expect(DEFAULT_PROGRESS_CONFIG.fillChar).toBe("█");
    expect(DEFAULT_PROGRESS_CONFIG.emptyChar).toBe("░");
    expect(DEFAULT_PROGRESS_CONFIG.showPercent).toBe(true);
  });
});
