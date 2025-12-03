/**
 * SDK Schema Adapters
 *
 * This module provides adapters for converting between Effect-based types
 * and SDK/MCP-compatible formats.
 *
 * @module
 */

// Effect Schema to Zod conversion
export {
  effectSchemaToZod,
  effectSchemaToJsonSchema,
  SchemaConversionError,
} from "./effect-to-zod.js";

// Tool adaptation
export {
  effectToolToMcp,
  effectToolsToMcp,
  sdkTool,
  ToolRegistry,
  toolContentToMcpContent,
  effectResultToMcpResult,
  effectErrorToMcpResult,
} from "./tool-adapter.js";

export type {
  McpContentItem,
  McpCallToolResult,
  McpToolDefinition,
  EffectTool,
  EffectToolResult,
  EffectToolExecutionError,
  ToolAdapterOptions,
} from "./tool-adapter.js";

// Message adaptation
export {
  // Content block conversion
  internalContentToSdk,
  internalContentsToSdk,
  internalContentToToolContent,
  internalContentsToToolContent,
  sdkContentToInternal,
  sdkContentsToInternal,
  // Tool call conversion
  toolCallToToolUseBlock,
  toolUseBlockToToolCall,
  extractToolCalls,
  // Message conversion (internal → SDK)
  internalMessageToSdkChat,
  internalToSdkUserMessage,
  internalToSdkAssistantMessage,
  // Message conversion (SDK → internal)
  sdkChatToInternalMessage,
  sdkUserToInternalMessage,
  sdkAssistantToInternalMessage,
  // Tool result helpers
  createToolResultBlock,
  createInternalToolResult,
  // Batch conversion
  internalMessagesToSdkChat,
  sdkChatToInternalMessages,
} from "./message-adapter.js";

export type {
  MessageConversionResult,
  MessageAdapterOptions,
} from "./message-adapter.js";

// Result adaptation (note: toolContentToMcpContent, effectResultToMcpResult, effectErrorToMcpResult already exported from tool-adapter)
export {
  // MCP → Internal conversion
  mcpContentToToolContent,
  mcpContentsToToolContent,
  mcpResultToToolResult,
  isMcpErrorResult,
  getMcpErrorMessage,
  // Internal → MCP conversion
  toolResultToMcpResult,
  createMcpErrorResult,
  // SubagentResult conversion
  subagentResultToSdk,
  sdkSubagentResultToInternal,
  subagentResultToMcp,
  // Utility functions
  createTextResult,
  createResult,
  mergeResults,
  getResultText,
  resultHasImages,
} from "./result-adapter.js";

export type {
  SDKSubagentResult,
  ResultAdapterOptions,
} from "./result-adapter.js";
