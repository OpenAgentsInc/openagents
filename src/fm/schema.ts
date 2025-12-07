/**
 * Foundation Models Service Schema
 *
 * Effect Schema definitions for FM service types, metrics, and errors.
 */

import * as S from "effect/Schema";

// --- Request Types ---

export const FMChatMessage = S.Struct({
  role: S.Literal("system", "user", "assistant"),
  content: S.String,
});
export type FMChatMessage = S.Schema.Type<typeof FMChatMessage>;

export const FMChatRequest = S.Struct({
  messages: S.Array(FMChatMessage),
  model: S.optional(S.String),
  temperature: S.optional(S.Number),
  maxTokens: S.optional(S.Number),
});
export type FMChatRequest = S.Schema.Type<typeof FMChatRequest>;

// --- Response Types ---

export const FMToolCall = S.Struct({
  id: S.String,
  name: S.String,
  arguments: S.String,
});
export type FMToolCall = S.Schema.Type<typeof FMToolCall>;

export const FMUsage = S.Struct({
  promptTokens: S.Number,
  completionTokens: S.Number,
  totalTokens: S.Number,
});
export type FMUsage = S.Schema.Type<typeof FMUsage>;

export const FMChatResponse = S.Struct({
  id: S.String,
  content: S.NullOr(S.String),
  toolCalls: S.optional(S.Array(FMToolCall)),
  finishReason: S.String,
  usage: S.optional(FMUsage),
});
export type FMChatResponse = S.Schema.Type<typeof FMChatResponse>;

// --- Metrics Types ---

export const FMRequestMetrics = S.Struct({
  requestId: S.String,
  startTime: S.Number,
  endTime: S.Number,
  latencyMs: S.Number,
  promptTokens: S.Number,
  completionTokens: S.Number,
  totalTokens: S.Number,
  success: S.Boolean,
  retryCount: S.Number,
  errorType: S.optional(S.String),
});
export type FMRequestMetrics = S.Schema.Type<typeof FMRequestMetrics>;

export const FMAggregateMetrics = S.Struct({
  totalRequests: S.Number,
  successfulRequests: S.Number,
  failedRequests: S.Number,
  totalTokens: S.Number,
  totalPromptTokens: S.Number,
  totalCompletionTokens: S.Number,
  averageLatencyMs: S.Number,
  successRate: S.Number,
  totalRetries: S.Number,
});
export type FMAggregateMetrics = S.Schema.Type<typeof FMAggregateMetrics>;

// --- Error Types ---

export const FMErrorReason = S.Literal(
  "not_macos",
  "bridge_not_found",
  "server_not_running",
  "model_unavailable",
  "request_failed",
  "invalid_response",
  "timeout",
  "rate_limited",
);
export type FMErrorReason = S.Schema.Type<typeof FMErrorReason>;

/**
 * Determine if an error is retryable.
 */
export const isRetryableError = (reason: FMErrorReason): boolean => {
  switch (reason) {
    case "server_not_running":
    case "timeout":
    case "rate_limited":
    case "request_failed":
      return true;
    case "not_macos":
    case "bridge_not_found":
    case "model_unavailable":
    case "invalid_response":
      return false;
  }
};

// --- Model Types ---

export const FMModel = S.Struct({
  id: S.String,
  object: S.Literal("model"),
  created: S.Number,
  owned_by: S.String,
});
export type FMModel = S.Schema.Type<typeof FMModel>;

export const FMModelsResult = S.Struct({
  object: S.Literal("list"),
  data: S.Array(FMModel),
});
export type FMModelsResult = S.Schema.Type<typeof FMModelsResult>;

// --- Health Types ---

export const FMHealthStatus = S.Struct({
  available: S.Boolean,
  serverRunning: S.Boolean,
  modelAvailable: S.Boolean,
  version: S.optional(S.String),
  error: S.optional(S.String),
  lastChecked: S.Number,
});
export type FMHealthStatus = S.Schema.Type<typeof FMHealthStatus>;

// --- Config Types ---

/**
 * FM Service configuration interface.
 * Use defaultFMServiceConfig for defaults and merge with overrides.
 */
export interface FMServiceConfig {
  /** Server port (default: 11435) */
  port: number;
  /** Path to foundation-bridge binary (auto-detected if not specified) */
  bridgePath?: string;
  /** Request timeout in ms (default: 300000 = 5 minutes) */
  timeoutMs: number;
  /** Auto-start server if not running (default: true) */
  autoStart: boolean;
  /** Maximum retry attempts (default: 3) */
  maxRetries: number;
  /** Initial retry delay in ms (default: 1000) */
  retryDelayMs: number;
  /** Enable metrics collection (default: true) */
  enableMetrics: boolean;
  /** Enable logging (default: true) */
  enableLogging: boolean;
}

// Default configuration
export const defaultFMServiceConfig: FMServiceConfig = {
  port: 11435,
  timeoutMs: 300_000,
  autoStart: true,
  maxRetries: 3,
  retryDelayMs: 1000,
  enableMetrics: true,
  enableLogging: true,
};

// --- Request Isolation Types ---

/**
 * Request context for isolation and tracing.
 * Each request carries this context for proper routing and logging.
 */
export interface FMRequestContext {
  /** Unique request ID for correlation */
  requestId: string;
  /** Session ID for grouping related requests */
  sessionId: string;
  /** Parent request ID for nested calls */
  parentRequestId?: string;
  /** Task ID if part of a training task */
  taskId?: string;
  /** Timestamp when request was created */
  createdAt: number;
}

/**
 * Session configuration for request isolation.
 */
export interface FMSessionConfig {
  /** Unique session identifier */
  sessionId: string;
  /** Maximum concurrent requests per session (default: 1 for serial execution) */
  maxConcurrentRequests: number;
  /** Request timeout override for this session (uses service default if not set) */
  timeoutMs?: number;
  /** Priority level for queue ordering (higher = more priority) */
  priority: number;
}

/**
 * Default session configuration.
 */
export const defaultFMSessionConfig: Omit<FMSessionConfig, "sessionId"> = {
  maxConcurrentRequests: 1,
  priority: 0,
};

/**
 * Generate a unique request ID.
 */
export const generateRequestId = (): string =>
  `fm-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;

/**
 * Generate a unique session ID.
 */
export const generateSessionId = (): string =>
  `session-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;

/**
 * Create a request context.
 */
export const createRequestContext = (
  sessionId: string,
  options?: {
    parentRequestId?: string;
    taskId?: string;
  },
): FMRequestContext => {
  const ctx: FMRequestContext = {
    requestId: generateRequestId(),
    sessionId,
    createdAt: Date.now(),
  };
  if (options?.parentRequestId) {
    ctx.parentRequestId = options.parentRequestId;
  }
  if (options?.taskId) {
    ctx.taskId = options.taskId;
  }
  return ctx;
};

/**
 * Create a session configuration.
 */
export const createSessionConfig = (
  options?: Partial<FMSessionConfig>,
): FMSessionConfig => ({
  sessionId: options?.sessionId ?? generateSessionId(),
  maxConcurrentRequests: options?.maxConcurrentRequests ?? defaultFMSessionConfig.maxConcurrentRequests,
  priority: options?.priority ?? defaultFMSessionConfig.priority,
  ...(options?.timeoutMs !== undefined ? { timeoutMs: options.timeoutMs } : {}),
});
