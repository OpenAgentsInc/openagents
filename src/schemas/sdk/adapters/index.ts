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
