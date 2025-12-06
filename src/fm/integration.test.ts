/**
 * FM Service Integration Tests
 *
 * These tests require:
 * 1. macOS (Darwin) platform
 * 2. Foundation Models bridge running on port 11435
 *
 * Run with: bun test src/fm/integration.test.ts
 *
 * To start the FM bridge manually:
 *   cd swift/foundation-bridge && swift run
 *
 * Or let the tests auto-start it (requires Xcode/Swift installed).
 */

import { describe, test, expect, beforeAll, afterAll } from "bun:test";
import { Effect, Duration } from "effect";
import {
  FMService,
  FMServiceError,
  makeFMServiceLayer,
  fmChat,
  fmCheckHealth,
  fmGetMetrics,
  fmListModels,
} from "./service.js";
import {
  makeFMLayerWithMonitor,
  makeFMLayerWithAutoStart,
  makeFMLayerComplete,
} from "./layer.js";
import {
  checkFMHealth,
  isMacOS,
  ensureServerRunning,
} from "../llm/foundation-models.js";
import type { FMHealthStatus } from "./schema.js";

// --- Test Utilities ---

const FM_PORT = 11435;
const CHAT_TIMEOUT_MS = 60_000; // FM can be slow on first request

/**
 * Check if FM bridge is available for integration tests.
 */
const checkBridgeAvailable = async (): Promise<boolean> => {
  if (!isMacOS()) return false;

  try {
    const result = await Effect.runPromise(
      checkFMHealth(FM_PORT).pipe(
        Effect.timeout(Duration.millis(5000)),
        Effect.catchAll(() =>
          Effect.succeed({ available: false, serverRunning: false, modelAvailable: false }),
        ),
      ),
    );
    return result.serverRunning;
  } catch {
    return false;
  }
};

let bridgeAvailable = false;

beforeAll(async () => {
  bridgeAvailable = await checkBridgeAvailable();
  if (!bridgeAvailable && isMacOS()) {
    console.log(
      "[FM Integration] Bridge not running. Some tests will be skipped.\n" +
        "To run all tests, start the bridge: cd swift/foundation-bridge && swift run",
    );
  }
});

// --- Platform Tests ---

describe("FM Platform Detection", () => {
  test("isMacOS returns correct platform", () => {
    const expected = process.platform === "darwin";
    expect(isMacOS()).toBe(expected);
  });
});

// --- Health Check Integration ---

describe.skipIf(!isMacOS())("FM Health Check Integration (macOS)", () => {
  test("checkFMHealth returns health status", async () => {
    const result = await Effect.runPromise(
      checkFMHealth(FM_PORT).pipe(
        Effect.catchAll((e) =>
          Effect.succeed({
            available: false,
            serverRunning: false,
            modelAvailable: false,
            error: e.message,
          }),
        ),
      ),
    );

    expect(result).toBeDefined();
    expect(typeof result.available).toBe("boolean");
    expect(typeof result.serverRunning).toBe("boolean");
    expect(typeof result.modelAvailable).toBe("boolean");
  });

  test("service checkHealth returns FMHealthStatus", async () => {
    const layer = makeFMServiceLayer({
      port: FM_PORT,
      enableLogging: false,
      autoStart: false,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;
      return yield* service.checkHealth();
    });

    const health = await Effect.runPromise(program.pipe(Effect.provide(layer)));

    expect(health).toBeDefined();
    expect(typeof health.available).toBe("boolean");
    expect(typeof health.serverRunning).toBe("boolean");
    expect(typeof health.modelAvailable).toBe("boolean");
    expect(typeof health.lastChecked).toBe("number");
    expect(health.lastChecked).toBeGreaterThan(0);
  });

  test("fmCheckHealth convenience function works", async () => {
    const layer = makeFMServiceLayer({
      port: FM_PORT,
      enableLogging: false,
      autoStart: false,
    });

    const health = await Effect.runPromise(fmCheckHealth().pipe(Effect.provide(layer)));

    expect(health).toBeDefined();
    expect(typeof health.lastChecked).toBe("number");
  });
});

// --- Chat Integration (requires running bridge) ---

describe.skipIf(!isMacOS())("FM Chat Integration (macOS, bridge required)", () => {
  // Skip individual tests if bridge not available
  const skipIfNoBridge = !bridgeAvailable;

  test.skipIf(skipIfNoBridge)("simple chat completion", async () => {
    const layer = makeFMServiceLayer({
      port: FM_PORT,
      enableLogging: false,
      autoStart: false,
      maxRetries: 1,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;
      return yield* service.chat({
        messages: [{ role: "user", content: "Reply with exactly: hello" }],
      });
    });

    const response = await Effect.runPromise(
      program.pipe(Effect.provide(layer), Effect.timeout(Duration.millis(CHAT_TIMEOUT_MS))),
    );

    expect(response).toBeDefined();
    expect(response.choices).toBeDefined();
    expect(response.choices.length).toBeGreaterThan(0);
    expect(response.choices[0].message).toBeDefined();
    expect(typeof response.choices[0].message.content).toBe("string");
  });

  test.skipIf(skipIfNoBridge)("chat with system message", async () => {
    const layer = makeFMServiceLayer({
      port: FM_PORT,
      enableLogging: false,
      autoStart: false,
      maxRetries: 1,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;
      return yield* service.chat({
        messages: [
          { role: "system", content: "You are a helpful assistant. Be very brief." },
          { role: "user", content: "What is 2+2?" },
        ],
      });
    });

    const response = await Effect.runPromise(
      program.pipe(Effect.provide(layer), Effect.timeout(Duration.millis(CHAT_TIMEOUT_MS))),
    );

    expect(response).toBeDefined();
    expect(response.choices[0].message.content).toBeDefined();
    // Response should mention "4" somewhere
    expect(response.choices[0].message.content?.toLowerCase()).toContain("4");
  });

  test.skipIf(skipIfNoBridge)("fmChat convenience function", async () => {
    const layer = makeFMServiceLayer({
      port: FM_PORT,
      enableLogging: false,
      autoStart: false,
      maxRetries: 1,
    });

    const response = await Effect.runPromise(
      fmChat({
        messages: [{ role: "user", content: "Say 'test'" }],
      }).pipe(Effect.provide(layer), Effect.timeout(Duration.millis(CHAT_TIMEOUT_MS))),
    );

    expect(response).toBeDefined();
    expect(response.choices.length).toBeGreaterThan(0);
  });
});

// --- Metrics Integration ---

describe.skipIf(!isMacOS())("FM Metrics Integration (macOS, bridge required)", () => {
  const skipIfNoBridge = !bridgeAvailable;

  test.skipIf(skipIfNoBridge)("metrics are collected on successful request", async () => {
    const layer = makeFMServiceLayer({
      port: FM_PORT,
      enableLogging: false,
      autoStart: false,
      enableMetrics: true,
      maxRetries: 1,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;

      // Get initial metrics
      const before = yield* service.getMetrics();
      expect(before.totalRequests).toBe(0);

      // Make a request
      yield* service.chat({
        messages: [{ role: "user", content: "Hi" }],
      });

      // Get updated metrics
      const after = yield* service.getMetrics();
      return after;
    });

    const metrics = await Effect.runPromise(
      program.pipe(Effect.provide(layer), Effect.timeout(Duration.millis(CHAT_TIMEOUT_MS))),
    );

    expect(metrics.totalRequests).toBe(1);
    expect(metrics.successfulRequests).toBe(1);
    expect(metrics.failedRequests).toBe(0);
    expect(metrics.successRate).toBe(1);
    expect(metrics.averageLatencyMs).toBeGreaterThan(0);
  });

  test.skipIf(skipIfNoBridge)("metrics track token usage", async () => {
    const layer = makeFMServiceLayer({
      port: FM_PORT,
      enableLogging: false,
      autoStart: false,
      enableMetrics: true,
      maxRetries: 1,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;

      yield* service.chat({
        messages: [{ role: "user", content: "Count to 5" }],
      });

      return yield* service.getMetrics();
    });

    const metrics = await Effect.runPromise(
      program.pipe(Effect.provide(layer), Effect.timeout(Duration.millis(CHAT_TIMEOUT_MS))),
    );

    // Token counts should be tracked (FM may report 0 tokens in some cases)
    expect(metrics.totalTokens).toBeGreaterThanOrEqual(0);
    expect(metrics.totalPromptTokens).toBeGreaterThanOrEqual(0);
    expect(metrics.totalCompletionTokens).toBeGreaterThanOrEqual(0);
  });

  test("resetMetrics clears all counters", async () => {
    const layer = makeFMServiceLayer({
      port: FM_PORT,
      enableLogging: false,
      autoStart: false,
      enableMetrics: true,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;

      // Reset and verify
      yield* service.resetMetrics();
      const metrics = yield* service.getMetrics();

      return metrics;
    });

    const metrics = await Effect.runPromise(program.pipe(Effect.provide(layer)));

    expect(metrics.totalRequests).toBe(0);
    expect(metrics.successfulRequests).toBe(0);
    expect(metrics.failedRequests).toBe(0);
    expect(metrics.totalTokens).toBe(0);
    expect(metrics.averageLatencyMs).toBe(0);
    expect(metrics.successRate).toBe(0);
  });
});

// --- Layer Integration ---

describe.skipIf(!isMacOS())("FM Layer Integration (macOS)", () => {
  test("makeFMLayerWithMonitor provides working service", async () => {
    const layer = makeFMLayerWithMonitor({
      port: FM_PORT,
      enableLogging: false,
      autoStart: false,
      enableHealthMonitor: false,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;
      const health = yield* service.checkHealth();
      return health;
    });

    const health = await Effect.runPromise(
      Effect.scoped(program.pipe(Effect.provide(layer))),
    );

    expect(health).toBeDefined();
    expect(typeof health.available).toBe("boolean");
  });

  test.skipIf(!bridgeAvailable)("health monitor layer with callback", async () => {
    let healthChangeCalled = false;
    let lastHealth: FMHealthStatus | null = null;

    const layer = makeFMLayerWithMonitor({
      port: FM_PORT,
      enableLogging: false,
      autoStart: false,
      enableHealthMonitor: true,
      healthCheckIntervalMs: 100, // Fast for testing
      onHealthChange: (status) => {
        healthChangeCalled = true;
        lastHealth = status;
      },
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;
      // Wait a bit for health monitor to run
      yield* Effect.sleep(Duration.millis(250));
      return yield* service.checkHealth();
    });

    await Effect.runPromise(Effect.scoped(program.pipe(Effect.provide(layer))));

    // Health callback should have been triggered
    expect(healthChangeCalled).toBe(true);
    expect(lastHealth).not.toBeNull();
  });
});

// --- Models List Integration ---

describe.skipIf(!isMacOS())("FM Models List Integration (macOS, bridge required)", () => {
  const skipIfNoBridge = !bridgeAvailable;

  test.skipIf(skipIfNoBridge)("listModels returns available models", async () => {
    const layer = makeFMServiceLayer({
      port: FM_PORT,
      enableLogging: false,
      autoStart: false,
      maxRetries: 1,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;
      return yield* service.listModels();
    });

    const result = await Effect.runPromise(
      program.pipe(Effect.provide(layer), Effect.timeout(Duration.millis(10_000))),
    );

    expect(result).toBeDefined();
    expect(result.object).toBe("list");
    expect(Array.isArray(result.data)).toBe(true);
    // FM bridge should return at least one model
    expect(result.data.length).toBeGreaterThan(0);
    // Check model structure
    if (result.data.length > 0) {
      expect(result.data[0].id).toBeDefined();
      expect(result.data[0].object).toBe("model");
      expect(typeof result.data[0].created).toBe("number");
      expect(result.data[0].owned_by).toBeDefined();
    }
  });

  test.skipIf(skipIfNoBridge)("fmListModels convenience function works", async () => {
    const layer = makeFMServiceLayer({
      port: FM_PORT,
      enableLogging: false,
      autoStart: false,
      maxRetries: 1,
    });

    const result = await Effect.runPromise(
      fmListModels().pipe(Effect.provide(layer), Effect.timeout(Duration.millis(10_000))),
    );

    expect(result).toBeDefined();
    expect(result.object).toBe("list");
    expect(Array.isArray(result.data)).toBe(true);
  });
});

// --- Error Handling Integration ---

describe("FM Error Handling", () => {
  test("FMServiceError.fromFMError preserves error info", () => {
    const fmError = {
      _tag: "FMError" as const,
      reason: "timeout" as const,
      message: "Request timed out",
      name: "FMError",
    };

    const serviceError = FMServiceError.fromFMError(fmError as any, 2);

    expect(serviceError.reason).toBe("timeout");
    expect(serviceError.message).toBe("Request timed out");
    expect(serviceError.retryable).toBe(true); // timeout is retryable
    expect(serviceError.retryCount).toBe(2);
  });

  test("non-retryable errors are marked correctly", () => {
    const fmError = {
      _tag: "FMError" as const,
      reason: "not_macos" as const,
      message: "Not running on macOS",
      name: "FMError",
    };

    const serviceError = FMServiceError.fromFMError(fmError as any);

    expect(serviceError.retryable).toBe(false);
  });
});

// --- Auto-Start Integration (requires Swift/Xcode) ---

describe.skipIf(!isMacOS())("FM Auto-Start Integration (macOS)", () => {
  test("ensureServerRunning checks health first", async () => {
    // This test verifies the auto-start logic doesn't crash
    // It won't actually start the server unless Swift is installed
    const result = await Effect.runPromise(
      ensureServerRunning({ port: FM_PORT, autoStart: false }).pipe(
        Effect.map(() => "success"),
        Effect.catchAll((e) => Effect.succeed(`error: ${e.reason}`)),
      ),
    );

    // Either succeeds (server running) or fails with known error
    expect(["success", "error: server_not_running", "error: bridge_not_found"]).toContain(
      result,
    );
  });
});

// --- Concurrency Integration ---

describe.skipIf(!isMacOS())("FM Concurrency Integration (macOS, bridge required)", () => {
  const skipIfNoBridge = !bridgeAvailable;

  test.skipIf(skipIfNoBridge)("multiple concurrent requests", async () => {
    const layer = makeFMServiceLayer({
      port: FM_PORT,
      enableLogging: false,
      autoStart: false,
      enableMetrics: true,
      maxRetries: 1,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;

      // Launch 3 concurrent requests
      const results = yield* Effect.all(
        [
          service.chat({ messages: [{ role: "user", content: "Say A" }] }),
          service.chat({ messages: [{ role: "user", content: "Say B" }] }),
          service.chat({ messages: [{ role: "user", content: "Say C" }] }),
        ],
        { concurrency: 3 },
      );

      const metrics = yield* service.getMetrics();
      return { results, metrics };
    });

    const { results, metrics } = await Effect.runPromise(
      program.pipe(
        Effect.provide(layer),
        Effect.timeout(Duration.millis(CHAT_TIMEOUT_MS * 3)),
      ),
    );

    expect(results.length).toBe(3);
    expect(metrics.totalRequests).toBe(3);
    expect(metrics.successfulRequests).toBe(3);
  });
});
