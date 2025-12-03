/**
 * Result Adapter for converting between OpenAgents and SDK result formats.
 *
 * This module provides bidirectional conversion between:
 * - Internal ToolResult ↔ McpCallToolResult (MCP format)
 * - Internal ToolResult ↔ SDK BaseToolResult (Effect Schema)
 * - SubagentResult conversions for orchestrator integration
 *
 * @module
 */

import type { ToolContent, ToolResult } from "../../../tools/schema.js";
import type { SubagentResult } from "../../../agent/orchestrator/types.js";
import type { McpContentItem, McpCallToolResult } from "./tool-adapter.js";
import { toolContentToMcpContent, effectResultToMcpResult, effectErrorToMcpResult } from "./tool-adapter.js";

// =============================================================================
// Re-exports from tool-adapter for convenience
// =============================================================================

export { toolContentToMcpContent, effectResultToMcpResult, effectErrorToMcpResult };

// =============================================================================
// Types
// =============================================================================

/**
 * SDK-compatible subagent result format.
 * Matches Claude Agent SDK's expected result structure.
 */
export interface SDKSubagentResult {
  /** Whether the subagent completed successfully */
  success: boolean;
  /** ID of the subtask that was executed */
  subtask_id: string;
  /** List of files that were modified */
  files_modified: string[];
  /** Error message if failed */
  error?: string;
  /** Number of conversation turns used */
  turns: number;
  /** Token usage statistics */
  usage?: {
    input_tokens: number;
    output_tokens: number;
  };
  /** Optional summary of work done */
  summary?: string;
}

/**
 * Options for result conversion.
 */
export interface ResultAdapterOptions {
  /** Include details field in conversion */
  includeDetails?: boolean;
  /** Custom error handler */
  onError?: (error: unknown) => McpCallToolResult;
}

// =============================================================================
// MCP Content → Internal Content Conversion
// =============================================================================

/**
 * Convert single MCP content item to internal ToolContent.
 */
export const mcpContentToToolContent = (item: McpContentItem): ToolContent => {
  if (item.type === "text" && item.text) {
    return { type: "text", text: item.text };
  }
  if (item.type === "image" && item.data && item.mimeType) {
    return { type: "image", data: item.data, mimeType: item.mimeType };
  }
  // Fallback for resource or unknown types
  return { type: "text", text: item.text || JSON.stringify(item) };
};

/**
 * Convert MCP content array to internal ToolContent array.
 */
export const mcpContentsToToolContent = (items: McpContentItem[]): ToolContent[] =>
  items.map(mcpContentToToolContent);

// =============================================================================
// MCP Result → Internal Result Conversion
// =============================================================================

/**
 * Convert McpCallToolResult to internal ToolResult.
 */
export const mcpResultToToolResult = <Details = unknown>(
  result: McpCallToolResult,
  details?: Details
): ToolResult<Details> => ({
  content: mcpContentsToToolContent(result.content),
  ...(details !== undefined ? { details } : {}),
});

/**
 * Check if an MCP result represents an error.
 */
export const isMcpErrorResult = (result: McpCallToolResult): boolean =>
  result.isError === true;

/**
 * Extract error message from MCP error result.
 */
export const getMcpErrorMessage = (result: McpCallToolResult): string | undefined => {
  if (!result.isError) return undefined;
  const textItem = result.content.find(c => c.type === "text");
  return textItem?.text;
};

// =============================================================================
// Internal Result → MCP Result Conversion
// =============================================================================

/**
 * Convert internal ToolResult to McpCallToolResult.
 */
export const toolResultToMcpResult = <Details>(
  result: ToolResult<Details>,
  options?: ResultAdapterOptions
): McpCallToolResult => {
  const mcpResult = effectResultToMcpResult(result);

  // Optionally include details as additional text content
  if (options?.includeDetails && result.details !== undefined) {
    mcpResult.content.push({
      type: "text",
      text: `\n[Details]\n${JSON.stringify(result.details, null, 2)}`,
    });
  }

  return mcpResult;
};

/**
 * Create an MCP error result from an error message.
 */
export const createMcpErrorResult = (
  message: string,
  reason?: string
): McpCallToolResult => ({
  content: [{
    type: "text",
    text: reason ? `[${reason}] ${message}` : message,
  }],
  isError: true,
});

// =============================================================================
// SubagentResult Conversion
// =============================================================================

/**
 * Convert internal SubagentResult to SDK format.
 */
export const subagentResultToSdk = (
  result: SubagentResult,
  summary?: string
): SDKSubagentResult => ({
  success: result.success,
  subtask_id: result.subtaskId,
  files_modified: result.filesModified,
  ...(result.error ? { error: result.error } : {}),
  turns: result.turns,
  ...(result.tokenUsage ? {
    usage: {
      input_tokens: result.tokenUsage.input,
      output_tokens: result.tokenUsage.output,
    },
  } : {}),
  ...(summary ? { summary } : {}),
});

/**
 * Convert SDK SubagentResult to internal format.
 */
export const sdkSubagentResultToInternal = (
  result: SDKSubagentResult
): SubagentResult => ({
  success: result.success,
  subtaskId: result.subtask_id,
  filesModified: result.files_modified,
  ...(result.error ? { error: result.error } : {}),
  turns: result.turns,
  ...(result.usage ? {
    tokenUsage: {
      input: result.usage.input_tokens,
      output: result.usage.output_tokens,
    },
  } : {}),
});

/**
 * Convert SubagentResult to MCP tool result format.
 * Useful for returning subagent results through MCP tools.
 */
export const subagentResultToMcp = (
  result: SubagentResult
): McpCallToolResult => ({
  content: [
    {
      type: "text",
      text: result.success
        ? `Subtask completed successfully.\nFiles modified: ${result.filesModified.join(", ") || "none"}\nTurns: ${result.turns}`
        : `Subtask failed: ${result.error || "Unknown error"}\nTurns: ${result.turns}`,
    },
  ],
  isError: !result.success,
});

// =============================================================================
// Utility Functions
// =============================================================================

/**
 * Create a successful tool result with text content.
 */
export const createTextResult = <Details = unknown>(
  text: string,
  details?: Details
): ToolResult<Details> => ({
  content: [{ type: "text", text }],
  ...(details !== undefined ? { details } : {}),
});

/**
 * Create a successful tool result with multiple content items.
 */
export const createResult = <Details = unknown>(
  content: ToolContent[],
  details?: Details
): ToolResult<Details> => ({
  content,
  ...(details !== undefined ? { details } : {}),
});

/**
 * Merge multiple tool results into one.
 */
export const mergeResults = <Details = unknown>(
  results: ToolResult<unknown>[],
  mergedDetails?: Details
): ToolResult<Details> => ({
  content: results.flatMap(r => r.content),
  ...(mergedDetails !== undefined ? { details: mergedDetails } : {}),
});

/**
 * Extract text content from a tool result.
 */
export const getResultText = (result: ToolResult<unknown>): string =>
  result.content
    .filter((c): c is { type: "text"; text: string } => c.type === "text")
    .map(c => c.text)
    .join("\n");

/**
 * Check if a tool result contains any images.
 */
export const resultHasImages = (result: ToolResult<unknown>): boolean =>
  result.content.some(c => c.type === "image");
