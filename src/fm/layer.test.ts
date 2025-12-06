/**
 * FM Layer Composition Tests
 */

import { describe, test, expect } from "bun:test";
import { Effect, Layer } from "effect";
import {
  makeFMLayerWithMonitor,
  makeFMLayerWithAutoStart,
  makeFMLayerComplete,
  FMServiceWithHealthMonitor,
  FMServiceWithAutoStart,
  FMServiceComplete,
  defaultFMLayerConfig,
  provideFM,
  withFM,
} from "./layer.js";
import { FMService, makeFMServiceLayer } from "./service.js";

describe("FM Layer Configuration", () => {
  test("defaultFMLayerConfig has expected values", () => {
    expect(defaultFMLayerConfig.port).toBe(11435);
    expect(defaultFMLayerConfig.healthCheckIntervalMs).toBe(30_000);
    expect(defaultFMLayerConfig.enableHealthMonitor).toBe(false);
    expect(defaultFMLayerConfig.startupTimeoutMs).toBe(10_000);
    expect(defaultFMLayerConfig.autoStart).toBe(true);
    expect(defaultFMLayerConfig.maxRetries).toBe(3);
  });
});

describe("FM Layer Factories", () => {
  test("makeFMLayerWithMonitor creates layer", () => {
    const layer = makeFMLayerWithMonitor({
      enableHealthMonitor: false,
      enableLogging: false,
      autoStart: false,
    });
    expect(layer).toBeDefined();
  });

  test("makeFMLayerWithMonitor with custom config", () => {
    const layer = makeFMLayerWithMonitor({
      port: 12345,
      healthCheckIntervalMs: 60_000,
      enableHealthMonitor: true,
      enableLogging: false,
      autoStart: false,
    });
    expect(layer).toBeDefined();
  });

  test("makeFMLayerWithAutoStart creates layer", () => {
    const layer = makeFMLayerWithAutoStart({
      enableLogging: false,
      autoStart: false,
    });
    expect(layer).toBeDefined();
  });

  test("makeFMLayerComplete creates layer", () => {
    const layer = makeFMLayerComplete({
      enableLogging: false,
      autoStart: false,
      enableHealthMonitor: false,
    });
    expect(layer).toBeDefined();
  });
});

describe("FM Pre-configured Layers", () => {
  test("FMServiceWithHealthMonitor is defined", () => {
    expect(FMServiceWithHealthMonitor).toBeDefined();
  });

  test("FMServiceWithAutoStart is defined", () => {
    expect(FMServiceWithAutoStart).toBeDefined();
  });

  test("FMServiceComplete is defined", () => {
    expect(FMServiceComplete).toBeDefined();
  });
});

describe("FM Layer - getMetrics works with monitor layer", () => {
  test("service provides getMetrics with initial state via monitor layer", async () => {
    const layer = makeFMLayerWithMonitor({
      enableLogging: false,
      autoStart: false,
      enableHealthMonitor: false,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;
      return yield* service.getMetrics();
    });

    const metrics = await Effect.runPromise(
      Effect.scoped(program.pipe(Effect.provide(layer))),
    );

    expect(metrics.totalRequests).toBe(0);
    expect(metrics.successfulRequests).toBe(0);
    expect(metrics.failedRequests).toBe(0);
  });
});

describe("FM Layer Composition Utilities", () => {
  test("provideFM composes layers", () => {
    // Create a simple layer that requires FMService
    const testLayer = Layer.effect(
      FMService,
      Effect.gen(function* () {
        const fm = yield* FMService;
        return fm;
      }),
    );

    // provideFM should compose without error
    const composed = provideFM(testLayer as any, {
      enableLogging: false,
      autoStart: false,
    });
    expect(composed).toBeDefined();
  });

  test("withFM merges layers", () => {
    const baseLayer = makeFMServiceLayer({
      enableLogging: false,
      autoStart: false,
    });

    const merged = withFM(baseLayer, {
      enableLogging: false,
      autoStart: false,
    });
    expect(merged).toBeDefined();
  });
});

describe("FM Layer - Health callback", () => {
  test("health change callback is configurable", () => {
    let callbackCalled = false;

    const layer = makeFMLayerWithMonitor({
      enableLogging: false,
      autoStart: false,
      enableHealthMonitor: false,
      onHealthChange: () => {
        callbackCalled = true;
      },
    });

    expect(layer).toBeDefined();
    // Callback won't be called since health monitor is disabled
    expect(callbackCalled).toBe(false);
  });
});

// Integration tests that require macOS
describe.skipIf(process.platform !== "darwin")(
  "FM Layer Integration (macOS only)",
  () => {
    test("checkHealth returns status via monitor layer", async () => {
      const layer = makeFMLayerWithMonitor({
        enableLogging: false,
        autoStart: false,
        enableHealthMonitor: false,
      });

      const program = Effect.gen(function* () {
        const service = yield* FMService;
        return yield* service.checkHealth();
      });

      const health = await Effect.runPromise(
        Effect.scoped(program.pipe(Effect.provide(layer))),
      );

      expect(health).toBeDefined();
      expect(typeof health.available).toBe("boolean");
      expect(typeof health.serverRunning).toBe("boolean");
      expect(typeof health.lastChecked).toBe("number");
    });
  },
);
