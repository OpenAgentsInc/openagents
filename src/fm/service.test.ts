/**
 * Foundation Models Service Tests
 */

import { describe, test, expect } from "bun:test";
import { Effect } from "effect";
import {
  FMService,
  FMServiceError,
  makeFMServiceLayer,
  fmCheckHealth,
  fmGetMetrics,
  fmListModels,
} from "./service.js";
import { isRetryableError, defaultFMServiceConfig } from "./schema.js";
import type { ChatResponse } from "../llm/openrouter-types.js";

describe("FM Service Schema", () => {
  test("isRetryableError returns true for retryable errors", () => {
    expect(isRetryableError("server_not_running")).toBe(true);
    expect(isRetryableError("timeout")).toBe(true);
    expect(isRetryableError("rate_limited")).toBe(true);
    expect(isRetryableError("request_failed")).toBe(true);
  });

  test("isRetryableError returns false for non-retryable errors", () => {
    expect(isRetryableError("not_macos")).toBe(false);
    expect(isRetryableError("bridge_not_found")).toBe(false);
    expect(isRetryableError("model_unavailable")).toBe(false);
    expect(isRetryableError("invalid_response")).toBe(false);
  });

  test("defaultFMServiceConfig has expected values", () => {
    expect(defaultFMServiceConfig.port).toBe(11435);
    expect(defaultFMServiceConfig.timeoutMs).toBe(300_000);
    expect(defaultFMServiceConfig.autoStart).toBe(true);
    expect(defaultFMServiceConfig.maxRetries).toBe(3);
    expect(defaultFMServiceConfig.retryDelayMs).toBe(1000);
    expect(defaultFMServiceConfig.enableMetrics).toBe(true);
    expect(defaultFMServiceConfig.enableLogging).toBe(true);
  });
});

describe("FMServiceError", () => {
  test("creates error with correct properties", () => {
    const error = new FMServiceError("test_reason", "Test message", true, 2);
    expect(error.reason).toBe("test_reason");
    expect(error.message).toBe("Test message");
    expect(error.retryable).toBe(true);
    expect(error.retryCount).toBe(2);
    expect(error._tag).toBe("FMServiceError");
  });
});

describe("FM Service Layer", () => {
  test("makeFMServiceLayer creates layer with custom config", async () => {
    const layer = makeFMServiceLayer({
      port: 12345,
      maxRetries: 5,
      enableLogging: false,
    });

    // The layer should be created without error
    expect(layer).toBeDefined();
  });

  test("service provides getMetrics with initial state", async () => {
    const layer = makeFMServiceLayer({
      enableLogging: false,
      autoStart: false,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;
      return yield* service.getMetrics();
    });

    const metrics = await Effect.runPromise(program.pipe(Effect.provide(layer)));

    expect(metrics.totalRequests).toBe(0);
    expect(metrics.successfulRequests).toBe(0);
    expect(metrics.failedRequests).toBe(0);
    expect(metrics.totalTokens).toBe(0);
    expect(metrics.averageLatencyMs).toBe(0);
    expect(metrics.successRate).toBe(0);
  });

  test("service provides resetMetrics", async () => {
    const layer = makeFMServiceLayer({
      enableLogging: false,
      autoStart: false,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;
      yield* service.resetMetrics();
      return yield* service.getMetrics();
    });

    const metrics = await Effect.runPromise(program.pipe(Effect.provide(layer)));
    expect(metrics.totalRequests).toBe(0);
  });

  test("service provides getClient", async () => {
    const layer = makeFMServiceLayer({
      enableLogging: false,
      autoStart: false,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;
      return service.getClient();
    });

    const client = await Effect.runPromise(program.pipe(Effect.provide(layer)));
    expect(client).toBeDefined();
    expect(client.chat).toBeDefined();
    expect(client.listModels).toBeDefined();
    expect(client.config).toBeDefined();
  });

  test("service provides listModels method", async () => {
    const layer = makeFMServiceLayer({
      enableLogging: false,
      autoStart: false,
    });

    const program = Effect.gen(function* () {
      const service = yield* FMService;
      return typeof service.listModels;
    });

    const listModelsType = await Effect.runPromise(program.pipe(Effect.provide(layer)));
    expect(listModelsType).toBe("function");
  });
});

describe("FM Service Convenience Functions", () => {
  test("fmGetMetrics works with service in context", async () => {
    const layer = makeFMServiceLayer({
      enableLogging: false,
      autoStart: false,
    });

    const metrics = await Effect.runPromise(fmGetMetrics().pipe(Effect.provide(layer)));
    expect(metrics.totalRequests).toBe(0);
  });

  test("fmListModels convenience function is defined", () => {
    expect(fmListModels).toBeDefined();
    expect(typeof fmListModels).toBe("function");
  });
});

// Integration tests that require macOS and the FM server running
describe.skipIf(process.platform !== "darwin")("FM Service Integration (macOS only)", () => {
  // These tests require the FM server to be running
  // They are skipped if not on macOS

  test("checkHealth returns status", async () => {
    const layer = makeFMServiceLayer({
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
    expect(typeof health.lastChecked).toBe("number");
  });
});
