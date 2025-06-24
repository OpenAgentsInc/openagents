/**
 * Standardized AI types aligned with Vercel AI SDK v5
 * @since 1.0.0
 */

// Message types
export {
  AssistantMessage,
  ContentPart,
  FilePart,
  FileUrlPart,
  FinishReason,
  fromPlainMessage,
  ImagePart,
  ImageUrlPart,
  Message,
  MessageMetadata,
  // Compatibility types
  type PlainMessage,
  ProviderMetadata,
  ReasoningPart,
  ResponseMetadata,
  SystemMessage,
  // Content parts
  TextPart,
  // Base types
  TokenUsage,
  ToolCallId,
  ToolCallPart,
  ToolMessage,
  ToolResultPart,
  toPlainMessage,
  // Message types
  UserMessage
} from "./messages.js"

// Tool types
export {
  addTool,
  type CoreToolDefinition,
  createTool,
  createToolRegistry,
  // Construction helpers
  fromCore,
  fromToolArray,
  getTool,
  registryToCore,
  toCore,
  type Tool,
  ToolChoice,
  ToolError,
  type ToolRegistry,
  // Core types
  type ToolResult
} from "./tools.js"

// Provider types
export {
  // Errors
  AIError,
  APICallError,
  // Options
  type BaseGenerateOptions,
  // Utilities
  createModel,
  type GenerateObjectOptions,
  type GenerateObjectResult,
  type GenerateTextOptions,
  // Results
  type GenerateTextResult,
  InvalidPromptError,
  // Services
  type LanguageModelService,
  type Provider,
  type ProviderConfig,
  ProviderNotFoundError,
  ProviderRegistry,
  type StreamChunk,
  wrapVercelModel
} from "./providers.js"

// Re-export schema utilities for convenience
export type { Schema } from "effect/Schema"
