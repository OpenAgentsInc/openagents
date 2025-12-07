/**
 * Foundation Models Effect Service
 *
 * Production-ready Effect service for Apple Foundation Models with:
 * - Automatic retry with exponential backoff
 * - Metrics collection (tokens, latency, success rate)
 * - Structured logging for ATIF capture
 * - Health monitoring
 *
 * @example
 * ```ts
 * const program = Effect.gen(function* () {
 *   const fm = yield* FMService;
 *   const response = yield* fm.chat({
 *     messages: [{ role: "user", content: "Hello!" }],
 *   });
 *   return response;
 * });
 *
 * Effect.runPromise(program.pipe(Effect.provide(FMServiceLive)));
 * ```
 */

import { Effect, Context, Layer, Schedule, Ref, Duration } from "effect";
import {
  createFMClient,
  checkFMHealth,
  ensureServerRunning,
  FMError,
  type FMClient,
  type FMHealthResult,
  type FMModelsResult,
} from "../llm/foundation-models.js";
import type { ChatRequest, ChatResponse } from "../llm/openrouter-types.js";
import {
  type FMServiceConfig,
  type FMRequestMetrics,
  type FMAggregateMetrics,
  type FMHealthStatus,
  type FMRequestContext,
  type FMSessionConfig,
  defaultFMServiceConfig,
  isRetryableError,
  generateRequestId,
  createRequestContext,
  createSessionConfig,
} from "./schema.js";

// --- Service Interface ---

/**
 * Foundation Models Service interface.
 */
export interface IFMService {
  /**
   * Send a chat completion request with automatic retry and metrics.
   */
  readonly chat: (request: ChatRequest) => Effect.Effect<ChatResponse, FMServiceError>;

  /**
   * Send a chat request with explicit context for isolation and tracing.
   * The context includes request ID, session ID, and optional parent/task correlation.
   */
  readonly chatWithContext: (
    request: ChatRequest,
    context: FMRequestContext,
  ) => Effect.Effect<ChatResponse, FMServiceError>;

  /**
   * Create an isolated session for a sequence of related requests.
   * Returns a session-scoped service that ensures request isolation.
   */
  readonly createSession: (
    config?: Partial<FMSessionConfig>,
  ) => Effect.Effect<FMSessionHandle, never>;

  /**
   * Check server health and model availability.
   */
  readonly checkHealth: () => Effect.Effect<FMHealthStatus, FMServiceError>;

  /**
   * Ensure the server is running (auto-start if needed).
   */
  readonly ensureRunning: () => Effect.Effect<void, FMServiceError>;

  /**
   * Get current aggregate metrics.
   */
  readonly getMetrics: () => Effect.Effect<FMAggregateMetrics, never>;

  /**
   * Reset metrics counters.
   */
  readonly resetMetrics: () => Effect.Effect<void, never>;

  /**
   * Get the underlying client for advanced usage.
   */
  readonly getClient: () => FMClient;

  /**
   * List available models from the FM bridge.
   */
  readonly listModels: () => Effect.Effect<FMModelsResult, FMServiceError>;
}

/**
 * Handle for an isolated FM session.
 * Provides session-scoped chat with automatic context propagation.
 */
export interface FMSessionHandle {
  /** Session configuration */
  readonly config: FMSessionConfig;
  /** Send a chat request within this session */
  readonly chat: (request: ChatRequest, taskId?: string) => Effect.Effect<ChatResponse, FMServiceError>;
  /** Get metrics for this session only */
  readonly getSessionMetrics: () => Effect.Effect<FMSessionMetrics, never>;
}

/**
 * Metrics for a specific session.
 */
export interface FMSessionMetrics {
  sessionId: string;
  requestCount: number;
  successfulRequests: number;
  failedRequests: number;
  totalTokens: number;
  totalLatencyMs: number;
}

// --- Service Tag ---

export class FMService extends Context.Tag("FMService")<FMService, IFMService>() {}

// --- Error Types ---

export class FMServiceError extends Error {
  readonly _tag = "FMServiceError";
  constructor(
    readonly reason: string,
    message: string,
    readonly retryable: boolean = false,
    readonly retryCount: number = 0,
    readonly cause?: Error,
  ) {
    super(message);
    this.name = "FMServiceError";
  }

  static fromFMError(error: FMError, retryCount = 0): FMServiceError {
    return new FMServiceError(
      error.reason,
      error.message,
      isRetryableError(error.reason),
      retryCount,
      error,
    );
  }
}

// --- Metrics Collector ---

interface MetricsState {
  requests: FMRequestMetrics[];
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  totalTokens: number;
  totalPromptTokens: number;
  totalCompletionTokens: number;
  totalLatencyMs: number;
  totalRetries: number;
}

const initialMetricsState: MetricsState = {
  requests: [],
  totalRequests: 0,
  successfulRequests: 0,
  failedRequests: 0,
  totalTokens: 0,
  totalPromptTokens: 0,
  totalCompletionTokens: 0,
  totalLatencyMs: 0,
  totalRetries: 0,
};

// --- Service Implementation ---

const makeService = (
  config: FMServiceConfig,
  client: FMClient,
  metricsRef: Ref.Ref<MetricsState>,
): IFMService => {
  const recordMetrics = (metrics: FMRequestMetrics) =>
    Ref.update(metricsRef, (state) => ({
      requests: [...state.requests.slice(-99), metrics], // Keep last 100 requests
      totalRequests: state.totalRequests + 1,
      successfulRequests: state.successfulRequests + (metrics.success ? 1 : 0),
      failedRequests: state.failedRequests + (metrics.success ? 0 : 1),
      totalTokens: state.totalTokens + metrics.totalTokens,
      totalPromptTokens: state.totalPromptTokens + metrics.promptTokens,
      totalCompletionTokens: state.totalCompletionTokens + metrics.completionTokens,
      totalLatencyMs: state.totalLatencyMs + metrics.latencyMs,
      totalRetries: state.totalRetries + metrics.retryCount,
    }));

  const chat = (request: ChatRequest): Effect.Effect<ChatResponse, FMServiceError> =>
    Effect.gen(function* () {
      const requestId = generateRequestId();
      const startTime = Date.now();
      let retryCount = 0;

      // Build retry schedule with exponential backoff
      const retrySchedule = Schedule.exponential(Duration.millis(config.retryDelayMs)).pipe(
        Schedule.jittered,
        Schedule.whileInput((error: FMError) => isRetryableError(error.reason)),
        Schedule.recurs(config.maxRetries),
        Schedule.tapOutput(() =>
          Effect.sync(() => {
            retryCount++;
            if (config.enableLogging) {
              console.log(`[FM] Retry attempt ${retryCount} for request ${requestId}`);
            }
          }),
        ),
      );

      // Log request start
      if (config.enableLogging) {
        yield* Effect.log(`[FM] Starting chat request ${requestId}`);
      }

      // Execute with retry
      const result = yield* client.chat(request).pipe(
        Effect.retry(retrySchedule),
        Effect.mapError((error) => FMServiceError.fromFMError(error, retryCount)),
      );

      const endTime = Date.now();
      const latencyMs = endTime - startTime;

      // Record metrics
      if (config.enableMetrics) {
        yield* recordMetrics({
          requestId,
          startTime,
          endTime,
          latencyMs,
          promptTokens: result.usage?.prompt_tokens ?? 0,
          completionTokens: result.usage?.completion_tokens ?? 0,
          totalTokens: result.usage?.total_tokens ?? 0,
          success: true,
          retryCount,
        });
      }

      // Log success
      if (config.enableLogging) {
        yield* Effect.log(
          `[FM] Request ${requestId} completed in ${latencyMs}ms (${retryCount} retries, ${result.usage?.total_tokens ?? 0} tokens)`,
        );
      }

      return result;
    }).pipe(
      Effect.catchAll((error) =>
        Effect.gen(function* () {
          // Record failed metrics
          if (config.enableMetrics && error instanceof FMServiceError) {
            yield* recordMetrics({
              requestId: generateRequestId(),
              startTime: Date.now(),
              endTime: Date.now(),
              latencyMs: 0,
              promptTokens: 0,
              completionTokens: 0,
              totalTokens: 0,
              success: false,
              retryCount: error.retryCount,
              errorType: error.reason,
            });
          }

          // Log error
          if (config.enableLogging) {
            yield* Effect.logError(`[FM] Request failed: ${error.message}`);
          }

          return yield* Effect.fail(error);
        }),
      ),
    );

  const checkHealthImpl = (): Effect.Effect<FMHealthStatus, FMServiceError> =>
    Effect.gen(function* () {
      const result = yield* checkFMHealth(config.port).pipe(
        Effect.mapError((e) => FMServiceError.fromFMError(e)),
        Effect.catchAll((e) =>
          Effect.succeed({
            available: false,
            serverRunning: false,
            modelAvailable: false,
            error: e.message,
          } as FMHealthResult),
        ),
      );

      return {
        available: result.available,
        serverRunning: result.serverRunning,
        modelAvailable: result.modelAvailable,
        version: result.version,
        error: result.error,
        lastChecked: Date.now(),
      };
    });

  const ensureRunningImpl = (): Effect.Effect<void, FMServiceError> =>
    ensureServerRunning(config).pipe(Effect.mapError((e) => FMServiceError.fromFMError(e)));

  const getMetricsImpl = (): Effect.Effect<FMAggregateMetrics, never> =>
    Ref.get(metricsRef).pipe(
      Effect.map((state) => ({
        totalRequests: state.totalRequests,
        successfulRequests: state.successfulRequests,
        failedRequests: state.failedRequests,
        totalTokens: state.totalTokens,
        totalPromptTokens: state.totalPromptTokens,
        totalCompletionTokens: state.totalCompletionTokens,
        averageLatencyMs:
          state.totalRequests > 0 ? state.totalLatencyMs / state.totalRequests : 0,
        successRate:
          state.totalRequests > 0 ? state.successfulRequests / state.totalRequests : 0,
        totalRetries: state.totalRetries,
      })),
    );

  const resetMetricsImpl = (): Effect.Effect<void, never> =>
    Ref.set(metricsRef, initialMetricsState);

  const listModelsImpl = (): Effect.Effect<FMModelsResult, FMServiceError> =>
    client.listModels().pipe(Effect.mapError((e) => FMServiceError.fromFMError(e)));

  // --- Request Isolation Implementation ---

  const chatWithContext = (
    request: ChatRequest,
    context: FMRequestContext,
  ): Effect.Effect<ChatResponse, FMServiceError> =>
    Effect.gen(function* () {
      const startTime = Date.now();
      let retryCount = 0;

      // Build retry schedule with exponential backoff
      const retrySchedule = Schedule.exponential(Duration.millis(config.retryDelayMs)).pipe(
        Schedule.jittered,
        Schedule.whileInput((error: FMError) => isRetryableError(error.reason)),
        Schedule.recurs(config.maxRetries),
        Schedule.tapOutput(() =>
          Effect.sync(() => {
            retryCount++;
            if (config.enableLogging) {
              console.log(
                `[FM] Retry attempt ${retryCount} for request ${context.requestId} (session: ${context.sessionId})`,
              );
            }
          }),
        ),
      );

      // Log request start with context
      if (config.enableLogging) {
        const parentInfo = context.parentRequestId ? ` parent:${context.parentRequestId}` : "";
        const taskInfo = context.taskId ? ` task:${context.taskId}` : "";
        yield* Effect.log(
          `[FM] Starting chat request ${context.requestId} (session: ${context.sessionId}${parentInfo}${taskInfo})`,
        );
      }

      // Execute with retry
      const result = yield* client.chat(request).pipe(
        Effect.retry(retrySchedule),
        Effect.mapError((error) => FMServiceError.fromFMError(error, retryCount)),
      );

      const endTime = Date.now();
      const latencyMs = endTime - startTime;

      // Record metrics with context
      if (config.enableMetrics) {
        yield* recordMetrics({
          requestId: context.requestId,
          startTime,
          endTime,
          latencyMs,
          promptTokens: result.usage?.prompt_tokens ?? 0,
          completionTokens: result.usage?.completion_tokens ?? 0,
          totalTokens: result.usage?.total_tokens ?? 0,
          success: true,
          retryCount,
        });
      }

      // Log success with context
      if (config.enableLogging) {
        yield* Effect.log(
          `[FM] Request ${context.requestId} (session: ${context.sessionId}) completed in ${latencyMs}ms`,
        );
      }

      return result;
    }).pipe(
      Effect.catchAll((error) =>
        Effect.gen(function* () {
          if (config.enableMetrics && error instanceof FMServiceError) {
            yield* recordMetrics({
              requestId: context.requestId,
              startTime: Date.now(),
              endTime: Date.now(),
              latencyMs: 0,
              promptTokens: 0,
              completionTokens: 0,
              totalTokens: 0,
              success: false,
              retryCount: error.retryCount,
              errorType: error.reason,
            });
          }

          if (config.enableLogging) {
            yield* Effect.logError(
              `[FM] Request ${context.requestId} (session: ${context.sessionId}) failed: ${error.message}`,
            );
          }

          return yield* Effect.fail(error);
        }),
      ),
    );

  const createSessionImpl = (
    sessionConfig?: Partial<FMSessionConfig>,
  ): Effect.Effect<FMSessionHandle, never> =>
    Effect.gen(function* () {
      const cfg = createSessionConfig(sessionConfig);
      const sessionMetricsRef = yield* Ref.make<FMSessionMetrics>({
        sessionId: cfg.sessionId,
        requestCount: 0,
        successfulRequests: 0,
        failedRequests: 0,
        totalTokens: 0,
        totalLatencyMs: 0,
      });

      const sessionChat = (
        request: ChatRequest,
        taskId?: string,
      ): Effect.Effect<ChatResponse, FMServiceError> =>
        Effect.gen(function* () {
          const context = createRequestContext(cfg.sessionId, taskId ? { taskId } : undefined);
          const startTime = Date.now();

          const result = yield* chatWithContext(request, context);

          // Update session-specific metrics
          yield* Ref.update(sessionMetricsRef, (m) => ({
            ...m,
            requestCount: m.requestCount + 1,
            successfulRequests: m.successfulRequests + 1,
            totalTokens: m.totalTokens + (result.usage?.total_tokens ?? 0),
            totalLatencyMs: m.totalLatencyMs + (Date.now() - startTime),
          }));

          return result;
        }).pipe(
          Effect.catchAll((error) =>
            Effect.gen(function* () {
              yield* Ref.update(sessionMetricsRef, (m) => ({
                ...m,
                requestCount: m.requestCount + 1,
                failedRequests: m.failedRequests + 1,
              }));
              return yield* Effect.fail(error);
            }),
          ),
        );

      const getSessionMetricsImpl = (): Effect.Effect<FMSessionMetrics, never> =>
        Ref.get(sessionMetricsRef);

      return {
        config: cfg,
        chat: sessionChat,
        getSessionMetrics: getSessionMetricsImpl,
      };
    });

  return {
    chat,
    chatWithContext,
    createSession: createSessionImpl,
    checkHealth: checkHealthImpl,
    ensureRunning: ensureRunningImpl,
    getMetrics: getMetricsImpl,
    resetMetrics: resetMetricsImpl,
    getClient: () => client,
    listModels: listModelsImpl,
  };
};

// --- Layer Factory ---

/**
 * Create FMService layer with custom configuration.
 */
export const makeFMServiceLayer = (
  config: Partial<FMServiceConfig> = {},
): Layer.Layer<FMService, never, never> =>
  Layer.effect(
    FMService,
    Effect.gen(function* () {
      const fullConfig: FMServiceConfig = {
        ...defaultFMServiceConfig,
        ...config,
      };
      const clientConfig: Parameters<typeof createFMClient>[0] = {
        port: fullConfig.port,
        timeoutMs: fullConfig.timeoutMs,
        autoStart: fullConfig.autoStart,
      };
      if (fullConfig.bridgePath) {
        clientConfig.bridgePath = fullConfig.bridgePath;
      }
      const client = createFMClient(clientConfig);
      const metricsRef = yield* Ref.make(initialMetricsState);
      return makeService(fullConfig, client, metricsRef);
    }),
  );

/**
 * Default FMService layer with standard configuration.
 */
export const FMServiceLive: Layer.Layer<FMService, never, never> = makeFMServiceLayer();

// --- Convenience Functions ---

/**
 * Run a chat request using FMService from context.
 */
export const fmChat = (
  request: ChatRequest,
): Effect.Effect<ChatResponse, FMServiceError, FMService> =>
  Effect.gen(function* () {
    const service = yield* FMService;
    return yield* service.chat(request);
  });

/**
 * Check FM health using FMService from context.
 */
export const fmCheckHealth = (): Effect.Effect<FMHealthStatus, FMServiceError, FMService> =>
  Effect.gen(function* () {
    const service = yield* FMService;
    return yield* service.checkHealth();
  });

/**
 * Get FM metrics using FMService from context.
 */
export const fmGetMetrics = (): Effect.Effect<FMAggregateMetrics, never, FMService> =>
  Effect.gen(function* () {
    const service = yield* FMService;
    return yield* service.getMetrics();
  });

/**
 * List available FM models using FMService from context.
 */
export const fmListModels = (): Effect.Effect<FMModelsResult, FMServiceError, FMService> =>
  Effect.gen(function* () {
    const service = yield* FMService;
    return yield* service.listModels();
  });
