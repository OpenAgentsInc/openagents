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
  FMModel,
  FMModelsResult,
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
  fmListModels,
  type IFMService,
} from "./service.js";

// Layer composition exports
export {
  // Layer factories
  makeFMLayerWithMonitor,
  makeFMLayerWithAutoStart,
  makeFMLayerComplete,
  // Pre-configured layers
  FMServiceWithHealthMonitor,
  FMServiceWithAutoStart,
  FMServiceComplete,
  // Composition utilities
  provideFM,
  withFM,
  // Types
  type FMLayerConfig,
  defaultFMLayerConfig,
} from "./layer.js";

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
  type FMModelsResult as FMModelsResultClient,
} from "../llm/foundation-models.js";
