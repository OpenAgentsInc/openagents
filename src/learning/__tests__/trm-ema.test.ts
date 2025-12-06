/**
 * TRM EMA Stability Tests
 *
 * Tests for Exponential Moving Average functions and stability tracking.
 */

import { describe, test, expect } from "bun:test";
import * as S from "effect/Schema";
import { Effect } from "effect";
import {
  DEFAULT_EMA_CONFIG,
  EMAValue,
  TaskTypeStats,
  createEMAValue,
  updateEMA,
  isReliable,
  getConfidenceInterval,
  createTaskTypeStats,
  updateTaskTypeStats,
  createSkillEMAStats,
  updateSkillEMAStats,
  TRMEMAService,
  TRMEMAServiceLive,
  makeTRMEMAServiceLayer,
  type EMAConfig,
} from "../trm-ema.js";
import { createMockEMAValue, createMockTaskTypeStats, createMockSkillEMAStats, runEffect } from "./test-helpers.js";

describe("EMA Configuration", () => {
  test("default config has expected values", () => {
    expect(DEFAULT_EMA_CONFIG.decay).toBe(0.999);
    expect(DEFAULT_EMA_CONFIG.minSamples).toBe(5);
    expect(DEFAULT_EMA_CONFIG.initialValue).toBe(0.5);
  });
});

describe("EMAValue Schema", () => {
  test("decodes valid EMA value", () => {
    const input = {
      value: 0.75,
      sampleCount: 10,
      updatedAt: new Date().toISOString(),
      variance: 0.01,
      recentValues: [0.7, 0.75, 0.8],
    };
    const decoded = S.decodeUnknownSync(EMAValue)(input);
    expect(decoded.value).toBe(0.75);
    expect(decoded.sampleCount).toBe(10);
  });
});

describe("TaskTypeStats Schema", () => {
  test("decodes valid task type stats", () => {
    const now = new Date().toISOString();
    const input = {
      taskType: "arc-agi",
      successRate: { value: 0.65, sampleCount: 100, updatedAt: now, variance: 0.02, recentValues: [] },
      optimalDepth: { value: 25, sampleCount: 50, updatedAt: now, variance: 10, recentValues: [] },
      tokensPerAttempt: { value: 1200, sampleCount: 100, updatedAt: now, variance: 500, recentValues: [] },
      timePerAttempt: { value: 35000, sampleCount: 100, updatedAt: now, variance: 5000, recentValues: [] },
      totalAttempts: 100,
      totalSuccesses: 65,
    };
    const decoded = S.decodeUnknownSync(TaskTypeStats)(input);
    expect(decoded.taskType).toBe("arc-agi");
    expect(decoded.totalAttempts).toBe(100);
  });
});

describe("createEMAValue", () => {
  test("creates with default initial value", () => {
    const ema = createEMAValue();
    expect(ema.value).toBe(0.5);
    expect(ema.sampleCount).toBe(0);
    expect(ema.variance).toBe(0);
    expect(ema.recentValues).toEqual([]);
  });

  test("creates with custom initial value", () => {
    const ema = createEMAValue(0.8);
    expect(ema.value).toBe(0.8);
  });

  test("has valid timestamp", () => {
    const ema = createEMAValue();
    expect(() => new Date(ema.updatedAt)).not.toThrow();
  });
});

describe("updateEMA", () => {
  test("updates with single sample", () => {
    const ema = createEMAValue(0.5);
    const updated = updateEMA(ema, 1.0);

    // With decay=0.999: newValue = 0.999 * 0.5 + 0.001 * 1.0 = 0.5005
    expect(updated.value).toBeCloseTo(0.5005, 4);
    expect(updated.sampleCount).toBe(1);
  });

  test("updates variance with Welford's algorithm", () => {
    const ema = createEMAValue(0.5);
    const updated = updateEMA(ema, 1.0);

    // Variance should be non-zero after sample differs from mean
    expect(updated.variance).toBeGreaterThan(0);
  });

  test("accumulates samples in recentValues", () => {
    let ema = createEMAValue();
    for (let i = 0; i < 5; i++) {
      ema = updateEMA(ema, i * 0.2);
    }
    expect(ema.recentValues).toHaveLength(5);
  });

  test("keeps only last 10 recent values", () => {
    let ema = createEMAValue();
    for (let i = 0; i < 15; i++) {
      ema = updateEMA(ema, i * 0.1);
    }
    expect(ema.recentValues).toHaveLength(10);
    expect(ema.recentValues[0]).toBeCloseTo(0.5, 1);
  });

  test("respects custom decay", () => {
    const ema = createEMAValue(0);
    const config: EMAConfig = { ...DEFAULT_EMA_CONFIG, decay: 0.5 };
    const updated = updateEMA(ema, 1.0, config);

    // With decay=0.5: newValue = 0.5 * 0 + 0.5 * 1.0 = 0.5
    expect(updated.value).toBe(0.5);
  });

  test("handles decay=0 (no smoothing)", () => {
    const ema = createEMAValue(0);
    const config: EMAConfig = { ...DEFAULT_EMA_CONFIG, decay: 0 };
    const updated = updateEMA(ema, 1.0, config);

    // With decay=0: newValue = 0 * 0 + 1 * 1.0 = 1.0
    expect(updated.value).toBe(1.0);
  });

  test("handles decay=1 (complete smoothing)", () => {
    const ema = createEMAValue(0.5);
    const config: EMAConfig = { ...DEFAULT_EMA_CONFIG, decay: 1 };
    const updated = updateEMA(ema, 1.0, config);

    // With decay=1: newValue = 1 * 0.5 + 0 * 1.0 = 0.5
    expect(updated.value).toBe(0.5);
  });

  test("converges over many samples with low decay", () => {
    let ema = createEMAValue(0);
    const config: EMAConfig = { ...DEFAULT_EMA_CONFIG, decay: 0.9 };
    // With decay=0.9, converges: value = 1 - 0.9^n
    // After 100 samples: 1 - 0.9^100 ≈ 0.99997
    for (let i = 0; i < 100; i++) {
      ema = updateEMA(ema, 1.0, config);
    }
    expect(ema.value).toBeCloseTo(1.0, 1);
  });
});

describe("isReliable", () => {
  test("returns false when sampleCount below minSamples", () => {
    const ema = createMockEMAValue({ sampleCount: 3 });
    expect(isReliable(ema)).toBe(false);
  });

  test("returns true when sampleCount equals minSamples", () => {
    const ema = createMockEMAValue({ sampleCount: 5 });
    expect(isReliable(ema)).toBe(true);
  });

  test("returns true when sampleCount above minSamples", () => {
    const ema = createMockEMAValue({ sampleCount: 100 });
    expect(isReliable(ema)).toBe(true);
  });

  test("respects custom minSamples", () => {
    const ema = createMockEMAValue({ sampleCount: 5 });
    const config: EMAConfig = { ...DEFAULT_EMA_CONFIG, minSamples: 10 };
    expect(isReliable(ema, config)).toBe(false);
  });
});

describe("getConfidenceInterval", () => {
  test("returns [0, 1] when sampleCount < 2", () => {
    const ema = createMockEMAValue({ sampleCount: 1 });
    const [lower, upper] = getConfidenceInterval(ema);
    expect(lower).toBe(0);
    expect(upper).toBe(1);
  });

  test("returns bounded interval when sampleCount >= 2", () => {
    const ema = createMockEMAValue({ sampleCount: 10, value: 0.7, variance: 0.01 });
    const [lower, upper] = getConfidenceInterval(ema);

    expect(lower).toBeGreaterThanOrEqual(0);
    expect(upper).toBeLessThanOrEqual(1);
    expect(lower).toBeLessThan(ema.value);
    expect(upper).toBeGreaterThan(ema.value);
  });

  test("clamps bounds to [0, 1]", () => {
    // High variance should not push bounds outside [0, 1]
    const ema = createMockEMAValue({ sampleCount: 10, value: 0.9, variance: 1.0 });
    const [lower, upper] = getConfidenceInterval(ema);

    expect(lower).toBeGreaterThanOrEqual(0);
    expect(upper).toBeLessThanOrEqual(1);
  });

  test("handles zero variance", () => {
    const ema = createMockEMAValue({ sampleCount: 10, value: 0.5, variance: 0 });
    const [lower, upper] = getConfidenceInterval(ema);

    expect(lower).toBe(0.5);
    expect(upper).toBe(0.5);
  });
});

describe("createTaskTypeStats", () => {
  test("creates with default values", () => {
    const stats = createTaskTypeStats("arc-agi");

    expect(stats.taskType).toBe("arc-agi");
    expect(stats.successRate.value).toBe(0.5);
    expect(stats.optimalDepth.value).toBe(20);
    expect(stats.totalAttempts).toBe(0);
    expect(stats.totalSuccesses).toBe(0);
  });
});

describe("updateTaskTypeStats", () => {
  test("updates on success", () => {
    const stats = createTaskTypeStats("test");
    const updated = updateTaskTypeStats(stats, {
      success: true,
      depth: 15,
      tokensUsed: 1000,
      timeMs: 5000,
    });

    expect(updated.totalAttempts).toBe(1);
    expect(updated.totalSuccesses).toBe(1);
    expect(updated.successRate.sampleCount).toBe(1);
    expect(updated.optimalDepth.sampleCount).toBe(1); // Updated on success
  });

  test("updates on failure", () => {
    const stats = createTaskTypeStats("test");
    const updated = updateTaskTypeStats(stats, {
      success: false,
      depth: 42,
      tokensUsed: 2000,
      timeMs: 10000,
    });

    expect(updated.totalAttempts).toBe(1);
    expect(updated.totalSuccesses).toBe(0);
    expect(updated.successRate.sampleCount).toBe(1);
    expect(updated.optimalDepth.sampleCount).toBe(0); // Not updated on failure
  });

  test("accumulates over multiple updates", () => {
    let stats = createTaskTypeStats("test");

    stats = updateTaskTypeStats(stats, { success: true, depth: 10, tokensUsed: 500, timeMs: 2000 });
    stats = updateTaskTypeStats(stats, { success: false, depth: 20, tokensUsed: 1000, timeMs: 4000 });
    stats = updateTaskTypeStats(stats, { success: true, depth: 15, tokensUsed: 750, timeMs: 3000 });

    expect(stats.totalAttempts).toBe(3);
    expect(stats.totalSuccesses).toBe(2);
    expect(stats.optimalDepth.sampleCount).toBe(2); // Only successful attempts
  });
});

describe("createSkillEMAStats", () => {
  test("creates with default values", () => {
    const stats = createSkillEMAStats("skill-001");

    expect(stats.skillId).toBe("skill-001");
    expect(stats.samplingSuccessRate.value).toBe(0.5);
    expect(stats.refinementSuccessRate.value).toBe(0.5);
    expect(stats.jointConfidence).toBe(0.5);
  });
});

describe("updateSkillEMAStats", () => {
  test("updates sampling success rate", () => {
    const stats = createSkillEMAStats("skill-001");
    const updated = updateSkillEMAStats(stats, "sampling", true);

    expect(updated.samplingSuccessRate.sampleCount).toBe(1);
    expect(updated.refinementSuccessRate.sampleCount).toBe(0);
  });

  test("updates refinement success rate", () => {
    const stats = createSkillEMAStats("skill-001");
    const updated = updateSkillEMAStats(stats, "refinement", false);

    expect(updated.samplingSuccessRate.sampleCount).toBe(0);
    expect(updated.refinementSuccessRate.sampleCount).toBe(1);
  });

  test("updates joint confidence as average", () => {
    let stats = createSkillEMAStats("skill-001");
    // After many successes, both rates should approach 1.0
    for (let i = 0; i < 100; i++) {
      stats = updateSkillEMAStats(stats, "sampling", true);
      stats = updateSkillEMAStats(stats, "refinement", true);
    }

    expect(stats.jointConfidence).toBeCloseTo(
      (stats.samplingSuccessRate.value + stats.refinementSuccessRate.value) / 2,
      2,
    );
  });

  test("respects custom config", () => {
    const stats = createSkillEMAStats("skill-001");
    const config: EMAConfig = { ...DEFAULT_EMA_CONFIG, decay: 0.5 };
    const updated = updateSkillEMAStats(stats, "sampling", true, config);

    // With decay=0.5 and success=1: newValue = 0.5 * 0.5 + 0.5 * 1 = 0.75
    expect(updated.samplingSuccessRate.value).toBe(0.75);
  });
});

describe("TRMEMAService", () => {
  test("createEMA returns initial value", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMEMAService;
        return yield* service.createEMA(0.7);
      }).pipe(Effect.provide(TRMEMAServiceLive)),
    );

    expect(result.value).toBe(0.7);
    expect(result.sampleCount).toBe(0);
  });

  test("updateEMA updates value", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMEMAService;
        const ema = yield* service.createEMA(0.5);
        return yield* service.updateEMA(ema, 1.0);
      }).pipe(Effect.provide(TRMEMAServiceLive)),
    );

    expect(result.sampleCount).toBe(1);
    expect(result.value).toBeGreaterThan(0.5);
  });

  test("isReliable checks sample count", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMEMAService;
        let ema = yield* service.createEMA();
        const beforeReliable = yield* service.isReliable(ema);

        for (let i = 0; i < 10; i++) {
          ema = yield* service.updateEMA(ema, 0.5);
        }
        const afterReliable = yield* service.isReliable(ema);

        return { beforeReliable, afterReliable };
      }).pipe(Effect.provide(TRMEMAServiceLive)),
    );

    expect(result.beforeReliable).toBe(false);
    expect(result.afterReliable).toBe(true);
  });

  test("getConfidenceInterval returns bounds", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMEMAService;
        const ema = createMockEMAValue({ sampleCount: 20, value: 0.6, variance: 0.01 });
        return yield* service.getConfidenceInterval(ema);
      }).pipe(Effect.provide(TRMEMAServiceLive)),
    );

    expect(result[0]).toBeLessThan(0.6);
    expect(result[1]).toBeGreaterThan(0.6);
  });

  test("createTaskStats returns initialized stats", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMEMAService;
        return yield* service.createTaskStats("arc-agi");
      }).pipe(Effect.provide(TRMEMAServiceLive)),
    );

    expect(result.taskType).toBe("arc-agi");
    expect(result.totalAttempts).toBe(0);
  });

  test("updateTaskStats updates correctly", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMEMAService;
        const stats = yield* service.createTaskStats("test");
        return yield* service.updateTaskStats(stats, {
          success: true,
          depth: 10,
          tokensUsed: 500,
          timeMs: 2000,
        });
      }).pipe(Effect.provide(TRMEMAServiceLive)),
    );

    expect(result.totalAttempts).toBe(1);
    expect(result.totalSuccesses).toBe(1);
  });

  test("createSkillStats returns initialized stats", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMEMAService;
        return yield* service.createSkillStats("skill-001");
      }).pipe(Effect.provide(TRMEMAServiceLive)),
    );

    expect(result.skillId).toBe("skill-001");
    expect(result.jointConfidence).toBe(0.5);
  });

  test("updateSkillStats updates correctly", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMEMAService;
        const stats = yield* service.createSkillStats("skill-001");
        return yield* service.updateSkillStats(stats, "sampling", true);
      }).pipe(Effect.provide(TRMEMAServiceLive)),
    );

    expect(result.samplingSuccessRate.sampleCount).toBe(1);
  });

  test("getConfig returns current config", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMEMAService;
        return yield* service.getConfig();
      }).pipe(Effect.provide(TRMEMAServiceLive)),
    );

    expect(result.decay).toBe(0.999);
  });

  test("updateConfig modifies config", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMEMAService;
        return yield* service.updateConfig({ decay: 0.9 });
      }).pipe(Effect.provide(TRMEMAServiceLive)),
    );

    expect(result.decay).toBe(0.9);
  });

  test("custom config layer", () => {
    const customLayer = makeTRMEMAServiceLayer({ decay: 0.5, minSamples: 10 });
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMEMAService;
        return yield* service.getConfig();
      }).pipe(Effect.provide(customLayer)),
    );

    expect(result.decay).toBe(0.5);
    expect(result.minSamples).toBe(10);
  });
});
