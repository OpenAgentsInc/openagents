/**
 * Foundation Models Service Module
 *
 * Provides Effect-native access to Apple Foundation Models for local LLM inference.
 *
 * @module
 */

// Schema exports - values
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
  defaultFMServiceConfig,
  isRetryableError,
  // Request isolation exports
  defaultFMSessionConfig,
  generateRequestId,
  generateSessionId,
  createRequestContext,
  createSessionConfig,
} from "./schema.js";

// Schema exports - types (interfaces must use export type)
export type {
  FMServiceConfig,
  FMRequestContext,
  FMSessionConfig,
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
  type FMSessionHandle,
  type FMSessionMetrics,
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

// Re-export underlying client types for advanced usage - values
export {
  FMClientTag,
  FMError,
  createFMClient,
  checkFMHealth,
  ensureServerRunning,
  findBridgePath,
  isMacOS,
  DEFAULT_FM_PORT,
  DEFAULT_FM_TIMEOUT_MS,
} from "../llm/foundation-models.js";

// Re-export underlying client types - interfaces
export type {
  FMClient,
  FMConfig,
  FMModelsResult as FMModelsResultClient,
} from "../llm/foundation-models.js";

// Code index exports for FM navigation
export {
  CodeIndexError,
  indexFile,
  createCodeIndex,
  buildIndexFromDirectory,
  findChunks,
  getChunkContent,
  getChunksWithinBudget,
  formatChunksForPrompt,
  selectChunksForTask,
  type CodeChunk,
  type ChunkType,
  type FileIndex,
  type CodeIndex,
} from "./code-index.js";

// Micro-task decomposition exports
export {
  FM_CONTEXT_BUDGET,
  MAX_SKILLS_CHARS,
  MAX_MEMORIES_CHARS,
  MAX_REFLECTIONS_CHARS,
  condenseSkill,
  condenseSkillsForPrompt,
  condenseMemoriesForPrompt,
  condenseReflectionsForPrompt,
  buildMicroTaskPrompt,
  getUserMessageBudget,
  truncateTaskDescription,
  decomposeTask,
  buildMicroStepPrompt,
} from "./micro-task.js";

// Verification loop exports
export {
  MAX_ERROR_CHARS,
  MAX_ERROR_CONTEXT_CHARS,
  MAX_VERIFY_ATTEMPTS,
  extractErrorCore,
  extractErrorLocation,
  runVerification,
  formatVerificationFeedback,
  runVerifyLoop,
  verifyFileExists,
  verifyFileContains,
  verifyCommand,
  type VerificationResult,
  type VerifyLoopOptions,
  type VerifyLoopResult,
} from "./verification-loop.js";
