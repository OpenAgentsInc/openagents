/**
 * FM Service Layer Composition
 *
 * Provides production-ready layer compositions for FMService with:
 * - Health monitoring integration
 * - Auto-start bridge lifecycle management
 * - Service composition utilities
 *
 * @example
 * ```ts
 * // Basic usage
 * Effect.runPromise(program.pipe(Effect.provide(FMServiceLive)));
 *
 * // With health monitoring
 * Effect.runPromise(program.pipe(Effect.provide(FMServiceWithHealthMonitor)));
 *
 * // Custom configuration
 * const layer = makeFMLayerWithMonitor({ port: 12345, healthCheckIntervalMs: 60000 });
 * Effect.runPromise(program.pipe(Effect.provide(layer)));
 * ```
 */

import { Effect, Layer, Ref, Schedule, Duration, Fiber, Scope } from "effect";
import {
  FMService,
  FMServiceError,
  makeFMServiceLayer,
  type IFMService,
} from "./service.js";
import {
  type FMServiceConfig,
  type FMHealthStatus,
  defaultFMServiceConfig,
} from "./schema.js";
import { ensureServerRunning, checkFMHealth, isMacOS } from "../llm/foundation-models.js";

// --- Extended Configuration ---

/**
 * Extended configuration for layer composition.
 */
export interface FMLayerConfig extends FMServiceConfig {
  /** Health check interval in ms (default: 30000 = 30 seconds) */
  healthCheckIntervalMs: number;
  /** Whether to enable periodic health monitoring (default: false) */
  enableHealthMonitor: boolean;
  /** Callback for health status changes */
  onHealthChange?: (status: FMHealthStatus) => void;
  /** Maximum startup wait time in ms (default: 10000 = 10 seconds) */
  startupTimeoutMs: number;
}

/**
 * Default layer configuration.
 */
export const defaultFMLayerConfig: FMLayerConfig = {
  ...defaultFMServiceConfig,
  healthCheckIntervalMs: 30_000,
  enableHealthMonitor: false,
  startupTimeoutMs: 10_000,
};

// --- Health Monitor Service ---

/**
 * Health monitor state.
 */
interface HealthMonitorState {
  lastStatus: FMHealthStatus | null;
  checkCount: number;
  consecutiveFailures: number;
}

const initialHealthMonitorState: HealthMonitorState = {
  lastStatus: null,
  checkCount: 0,
  consecutiveFailures: 0,
};

/**
 * Create a health monitoring fiber that periodically checks FM health.
 */
const createHealthMonitor = (
  service: IFMService,
  config: FMLayerConfig,
  stateRef: Ref.Ref<HealthMonitorState>,
): Effect.Effect<Fiber.RuntimeFiber<void, never>, never, Scope.Scope> => {
  const healthCheck = Effect.gen(function* () {
    const status = yield* service.checkHealth().pipe(
      Effect.catchAll((error) =>
        Effect.succeed({
          available: false,
          serverRunning: false,
          modelAvailable: false,
          error: error.message,
          lastChecked: Date.now(),
        } as FMHealthStatus),
      ),
    );

    const previousState = yield* Ref.get(stateRef);
    const statusChanged =
      previousState.lastStatus === null ||
      previousState.lastStatus.available !== status.available ||
      previousState.lastStatus.serverRunning !== status.serverRunning;

    yield* Ref.update(stateRef, (state) => ({
      lastStatus: status,
      checkCount: state.checkCount + 1,
      consecutiveFailures: status.available ? 0 : state.consecutiveFailures + 1,
    }), );

    // Notify on status change
    if (statusChanged && config.onHealthChange) {
      config.onHealthChange(status);
    }

    // Auto-restart if configured and server went down
    if (config.autoStart && !status.serverRunning && previousState.lastStatus?.serverRunning) {
      yield* Effect.logWarning("[FM] Server went down, attempting restart...");
      yield* service.ensureRunning().pipe(Effect.catchAll(() => Effect.void));
    }
    
    // Always return void
    yield* Effect.void;
  });

  const schedule = Schedule.spaced(Duration.millis(config.healthCheckIntervalMs));

  return Effect.forkScoped(
    healthCheck.pipe(
      Effect.repeat(schedule),
      Effect.map(() => undefined), // Ensure the result is always void
      Effect.catchAll(() => Effect.void), // Never fail the monitor
    ),
  );
};

// --- Layer Factories ---

/**
 * Create FMService layer with health monitoring.
 *
 * The health monitor runs in the background and:
 * - Periodically checks server health
 * - Notifies on status changes via onHealthChange callback
 * - Auto-restarts the server if it goes down (when autoStart is true)
 */
export const makeFMLayerWithMonitor = (
  config: Partial<FMLayerConfig> = {},
): Layer.Layer<FMService, never, never> => {
  const fullConfig: FMLayerConfig = {
    ...defaultFMLayerConfig,
    ...config,
  };

  return Layer.scoped(
    FMService,
    Effect.gen(function* () {
      // Create base service
      const baseLayer = makeFMServiceLayer(fullConfig);
      const service = yield* Effect.provide(FMService, baseLayer);

      // Start health monitor if enabled
      if (fullConfig.enableHealthMonitor) {
        const stateRef = yield* Ref.make(initialHealthMonitorState);
        yield* createHealthMonitor(service, fullConfig, stateRef);
        yield* Effect.logDebug(
          `[FM] Health monitor started (interval: ${fullConfig.healthCheckIntervalMs}ms)`,
        );
      }

      return service;
    }),
  );
};

/**
 * Create FMService layer that ensures bridge is running before service is ready.
 *
 * This layer will:
 * 1. Check if running on macOS
 * 2. Start the foundation-bridge if not running
 * 3. Wait for server to be healthy
 * 4. Then provide the FMService
 */
export const makeFMLayerWithAutoStart = (
  config: Partial<FMLayerConfig> = {},
): Layer.Layer<FMService, FMServiceError, never> => {
  const fullConfig: FMLayerConfig = {
    ...defaultFMLayerConfig,
    autoStart: true,
    ...config,
  };

  return Layer.effect(
    FMService,
    Effect.gen(function* () {
      // Check macOS requirement
      if (!isMacOS()) {
        return yield* Effect.fail(
          new FMServiceError("not_macos", "Foundation Models require macOS", false),
        );
      }

      yield* Effect.logInfo("[FM] Ensuring foundation-bridge is running...");

      // Ensure server is running with timeout
      yield* ensureServerRunning(fullConfig).pipe(
        Effect.timeout(Duration.millis(fullConfig.startupTimeoutMs)),
        Effect.catchAll((error) => {
          if (error._tag === "TimeoutException") {
            return Effect.fail(
              new FMServiceError(
                "timeout",
                `Bridge startup timed out after ${fullConfig.startupTimeoutMs}ms`,
                true,
              ),
            );
          }
          return Effect.fail(FMServiceError.fromFMError(error as any));
        }),
      );

      // Verify health
      const health = yield* checkFMHealth(fullConfig.port).pipe(
        Effect.mapError((e) => FMServiceError.fromFMError(e)),
      );

      if (!health.available) {
        return yield* Effect.fail(
          new FMServiceError(
            "server_not_running",
            `Bridge started but not healthy: ${health.error ?? "unknown error"}`,
            true,
          ),
        );
      }

      yield* Effect.logInfo(
        `[FM] Bridge ready (version: ${health.version ?? "unknown"}, model: ${health.modelAvailable ? "available" : "unavailable"})`,
      );

      // Create and return service
      const baseLayer = makeFMServiceLayer(fullConfig);
      return yield* Effect.provide(FMService, baseLayer);
    }),
  );
};

/**
 * Create a complete FMService layer with both auto-start and health monitoring.
 */
export const makeFMLayerComplete = (
  config: Partial<FMLayerConfig> = {},
): Layer.Layer<FMService, FMServiceError, never> => {
  const fullConfig: FMLayerConfig = {
    ...defaultFMLayerConfig,
    autoStart: true,
    enableHealthMonitor: true,
    ...config,
  };

  return Layer.scoped(
    FMService,
    Effect.gen(function* () {
      // Check macOS requirement
      if (!isMacOS()) {
        return yield* Effect.fail(
          new FMServiceError("not_macos", "Foundation Models require macOS", false),
        );
      }

      yield* Effect.logInfo("[FM] Initializing complete FM service layer...");

      // Ensure server is running
      yield* ensureServerRunning(fullConfig).pipe(
        Effect.timeout(Duration.millis(fullConfig.startupTimeoutMs)),
        Effect.catchAll((error) => {
          if (error._tag === "TimeoutException") {
            return Effect.fail(
              new FMServiceError(
                "timeout",
                `Bridge startup timed out after ${fullConfig.startupTimeoutMs}ms`,
                true,
              ),
            );
          }
          return Effect.fail(FMServiceError.fromFMError(error as any));
        }),
      );

      // Verify initial health
      const health = yield* checkFMHealth(fullConfig.port).pipe(
        Effect.mapError((e) => FMServiceError.fromFMError(e)),
      );

      if (!health.available) {
        return yield* Effect.fail(
          new FMServiceError(
            "server_not_running",
            `Bridge started but not healthy: ${health.error ?? "unknown error"}`,
            true,
          ),
        );
      }

      yield* Effect.logInfo(
        `[FM] Bridge ready (version: ${health.version ?? "unknown"})`,
      );

      // Create service
      const baseLayer = makeFMServiceLayer(fullConfig);
      const service = yield* Effect.provide(FMService, baseLayer);

      // Start health monitor
      const stateRef = yield* Ref.make(initialHealthMonitorState);
      yield* createHealthMonitor(service, fullConfig, stateRef);
      yield* Effect.logDebug(
        `[FM] Health monitor started (interval: ${fullConfig.healthCheckIntervalMs}ms)`,
      );

      return service;
    }),
  );
};

// --- Pre-configured Layers ---

/**
 * FMService with health monitoring enabled (30 second interval).
 * Does not auto-start the bridge - use FMServiceWithAutoStart for that.
 */
export const FMServiceWithHealthMonitor: Layer.Layer<FMService, never, never> =
  makeFMLayerWithMonitor({
    enableHealthMonitor: true,
    healthCheckIntervalMs: 30_000,
  });

/**
 * FMService with auto-start enabled.
 * Ensures bridge is running before service is ready.
 */
export const FMServiceWithAutoStart: Layer.Layer<FMService, FMServiceError, never> =
  makeFMLayerWithAutoStart({
    autoStart: true,
    startupTimeoutMs: 10_000,
  });

/**
 * Complete FMService with both auto-start and health monitoring.
 * This is the recommended layer for production use.
 */
export const FMServiceComplete: Layer.Layer<FMService, FMServiceError, never> =
  makeFMLayerComplete({
    autoStart: true,
    enableHealthMonitor: true,
    healthCheckIntervalMs: 30_000,
    startupTimeoutMs: 10_000,
  });

// --- Composition Utilities ---

/**
 * Provide FMService to a layer that requires it.
 */
export const provideFM = <A, E, R>(
  layer: Layer.Layer<A, E, R | FMService>,
  fmConfig: Partial<FMLayerConfig> = {},
): Layer.Layer<A, E | FMServiceError, Exclude<R, FMService>> => {
  const fmLayer = makeFMLayerWithAutoStart(fmConfig);
  return Layer.provide(layer, fmLayer) as Layer.Layer<A, E | FMServiceError, Exclude<R, FMService>>;
};

/**
 * Merge FMService with another layer.
 */
export const withFM = <A, E, R>(
  layer: Layer.Layer<A, E, R>,
  fmConfig: Partial<FMLayerConfig> = {},
): Layer.Layer<A | FMService, E | FMServiceError, R> => {
  const fmLayer = makeFMLayerWithAutoStart(fmConfig);
  return Layer.merge(layer, fmLayer);
};

// --- Re-exports for convenience ---

export { FMService, FMServiceError, makeFMServiceLayer, FMServiceLive } from "./service.js";
export type { IFMService } from "./service.js";
export type { FMServiceConfig, FMHealthStatus, FMAggregateMetrics } from "./schema.js";
