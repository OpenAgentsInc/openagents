/**
 * Foundation Models Service Module
 *
 * Provides Effect-native access to Apple Foundation Models for local LLM inference.
 *
 * @module
 */

// Schema exports
export {
  FMChatMessage,
  FMChatRequest,
  FMChatResponse,
  FMToolCall,
  FMUsage,
  FMRequestMetrics,
  FMAggregateMetrics,
  FMErrorReason,
  FMHealthStatus,
  FMServiceConfig,
  defaultFMServiceConfig,
  isRetryableError,
} from "./schema.js";

// Service exports
export {
  FMService,
  FMServiceError,
  FMServiceLive,
  makeFMServiceLayer,
  fmChat,
  fmCheckHealth,
  fmGetMetrics,
  type IFMService,
} from "./service.js";

// Re-export underlying client types for advanced usage
export {
  FMClientTag,
  FMClient,
  FMConfig,
  FMError,
  createFMClient,
  checkFMHealth,
  ensureServerRunning,
  findBridgePath,
  isMacOS,
  DEFAULT_FM_PORT,
  DEFAULT_FM_TIMEOUT_MS,
} from "../llm/foundation-models.js";
