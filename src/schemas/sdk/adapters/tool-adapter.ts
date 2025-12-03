/**
 * Tool adapter for converting Effect-based tools to MCP/SDK format.
 *
 * This module provides utilities to wrap Effect-based tools so they can
 * be used with Claude Agent SDK's MCP server infrastructure.
 *
 * @module
 */

import { Effect, Runtime, type Scope } from "effect";
import * as S from "effect/Schema";
import type { z } from "zod";
import { effectSchemaToZod } from "./effect-to-zod.js";
import type { ToolContent as ToolContentType } from "../tool-outputs.js";

// =============================================================================
// Types
// =============================================================================

/**
 * Content item in MCP tool results.
 */
export interface McpContentItem {
  type: "text" | "image" | "resource";
  text?: string;
  data?: string;
  mimeType?: string;
}

/**
 * Result from an MCP tool call.
 */
export interface McpCallToolResult {
  content: McpContentItem[];
  isError?: boolean;
}

/**
 * Definition of an MCP tool (matches SDK's tool() return type).
 */
export interface McpToolDefinition<Args = unknown> {
  name: string;
  description: string;
  inputSchema: z.ZodTypeAny;
  handler: (args: Args) => Promise<McpCallToolResult>;
}

/**
 * Effect-based tool interface (from src/tools/schema.ts).
 */
export interface EffectTool<Params, Details = unknown, R = never, E = never> {
  name: string;
  label: string;
  description: string;
  schema: S.Schema<Params>;
  execute: (
    params: Params,
    options?: { signal?: AbortSignal },
  ) => Effect.Effect<EffectToolResult<Details>, E | EffectToolExecutionError, R>;
}

/**
 * Result from an Effect tool.
 */
export interface EffectToolResult<Details = unknown> {
  content: ToolContentType[];
  details?: Details;
}

/**
 * Error from Effect tool execution.
 */
export interface EffectToolExecutionError {
  readonly _tag: "ToolExecutionError";
  readonly reason: string;
  readonly message: string;
}

// =============================================================================
// Conversion Utilities
// =============================================================================

/**
 * Convert Effect tool content to MCP content.
 */
export const toolContentToMcpContent = (content: ToolContentType[]): McpContentItem[] => {
  return content.map((item) => {
    if (item.type === "text") {
      return { type: "text" as const, text: item.text };
    }
    if (item.type === "image") {
      return {
        type: "image" as const,
        data: item.data,
        mimeType: item.mimeType,
      };
    }
    // Fallback for unknown types
    return { type: "text" as const, text: JSON.stringify(item) };
  });
};

/**
 * Convert Effect tool result to MCP tool result.
 */
export const effectResultToMcpResult = <Details>(
  result: EffectToolResult<Details>,
): McpCallToolResult => {
  return {
    content: toolContentToMcpContent(result.content),
    isError: false,
  };
};

/**
 * Convert Effect tool error to MCP error result.
 */
export const effectErrorToMcpResult = (
  error: EffectToolExecutionError | Error,
): McpCallToolResult => {
  const message = "message" in error ? error.message : String(error);
  const reason = "_tag" in error && error._tag === "ToolExecutionError"
    ? (error as EffectToolExecutionError).reason
    : "unknown";

  return {
    content: [{
      type: "text",
      text: `Error (${reason}): ${message}`,
    }],
    isError: true,
  };
};

// =============================================================================
// Tool Adapter
// =============================================================================

/**
 * Options for tool adaptation.
 */
export interface ToolAdapterOptions<R> {
  /**
   * Runtime to use for running Effect-based tools.
   * If not provided, tools will only work with Effect.runPromise (no deps).
   */
  runtime?: Runtime.Runtime<R>;

  /**
   * Optional scope for resource management.
   */
  scope?: Scope.Scope;

  /**
   * Whether to include details in the result.
   * Default: false (only include content)
   */
  includeDetails?: boolean;

  /**
   * Custom error handler.
   */
  onError?: (error: unknown) => McpCallToolResult;
}

/**
 * Adapt an Effect-based tool to MCP tool format.
 *
 * @example
 * ```typescript
 * import { readTool } from "../../../tools/read";
 * import { effectToolToMcp } from "./tool-adapter";
 * import { BunContext } from "@effect/platform-bun";
 *
 * const mcpReadTool = effectToolToMcp(readTool, {
 *   runtime: Runtime.defaultRuntime.pipe(
 *     Runtime.provideService(BunContext.layer)
 *   ),
 * });
 * ```
 */
export const effectToolToMcp = <Params, Details, R, E>(
  tool: EffectTool<Params, Details, R, E>,
  options?: ToolAdapterOptions<R>,
): McpToolDefinition<Params> => {
  const zodSchema = effectSchemaToZod(tool.schema);

  return {
    name: tool.name,
    description: tool.description,
    inputSchema: zodSchema,
    handler: async (args: Params): Promise<McpCallToolResult> => {
      try {
        const effect = tool.execute(args);

        let result: EffectToolResult<Details>;

        if (options?.runtime) {
          result = await Runtime.runPromise(options.runtime)(effect as Effect.Effect<EffectToolResult<Details>, E, never>);
        } else {
          // Try to run without dependencies (will fail if R is not never)
          result = await Effect.runPromise(effect as Effect.Effect<EffectToolResult<Details>, E, never>);
        }

        const mcpResult = effectResultToMcpResult(result);

        // Optionally include details
        if (options?.includeDetails && result.details) {
          mcpResult.content.push({
            type: "text",
            text: `\n\nDetails: ${JSON.stringify(result.details, null, 2)}`,
          });
        }

        return mcpResult;
      } catch (error) {
        if (options?.onError) {
          return options.onError(error);
        }

        if (error && typeof error === "object" && "_tag" in error) {
          return effectErrorToMcpResult(error as EffectToolExecutionError);
        }

        return effectErrorToMcpResult(error instanceof Error ? error : new Error(String(error)));
      }
    },
  };
};

/**
 * Adapt multiple Effect-based tools to MCP format.
 */
export const effectToolsToMcp = <R>(
  tools: EffectTool<any, any, R, any>[],
  options?: ToolAdapterOptions<R>,
): McpToolDefinition<any>[] => {
  return tools.map((tool) => effectToolToMcp(tool, options));
};

// =============================================================================
// SDK Tool Builder (for compatibility with @anthropic-ai/claude-agent-sdk)
// =============================================================================

/**
 * Create a tool definition in SDK-compatible format.
 * This is a simplified version that doesn't require the full SDK.
 *
 * @example
 * ```typescript
 * const myTool = sdkTool(
 *   "my_tool",
 *   "Description of my tool",
 *   z.object({ param: z.string() }),
 *   async (args) => ({
 *     content: [{ type: "text", text: `Got: ${args.param}` }],
 *   })
 * );
 * ```
 */
export const sdkTool = <Args extends z.ZodTypeAny>(
  name: string,
  description: string,
  inputSchema: Args,
  handler: (args: z.infer<Args>) => Promise<McpCallToolResult>,
): McpToolDefinition<z.infer<Args>> => ({
  name,
  description,
  inputSchema,
  handler,
});

// =============================================================================
// Tool Registry
// =============================================================================

/**
 * Registry for managing adapted tools.
 */
export class ToolRegistry<R = never> {
  private tools: Map<string, McpToolDefinition<any>> = new Map();
  private options: ToolAdapterOptions<R>;

  constructor(options?: ToolAdapterOptions<R>) {
    this.options = options ?? {};
  }

  /**
   * Register an Effect-based tool.
   */
  register<Params, Details, E>(
    tool: EffectTool<Params, Details, R, E>,
  ): this {
    const mcpTool = effectToolToMcp(tool, this.options);
    this.tools.set(tool.name, mcpTool);
    return this;
  }

  /**
   * Register a pre-adapted MCP tool.
   */
  registerMcp<Args>(tool: McpToolDefinition<Args>): this {
    this.tools.set(tool.name, tool);
    return this;
  }

  /**
   * Get a tool by name.
   */
  get(name: string): McpToolDefinition<any> | undefined {
    return this.tools.get(name);
  }

  /**
   * Get all registered tools.
   */
  getAll(): McpToolDefinition<any>[] {
    return Array.from(this.tools.values());
  }

  /**
   * Get tool names.
   */
  getNames(): string[] {
    return Array.from(this.tools.keys());
  }

  /**
   * Check if a tool is registered.
   */
  has(name: string): boolean {
    return this.tools.has(name);
  }

  /**
   * Remove a tool.
   */
  remove(name: string): boolean {
    return this.tools.delete(name);
  }

  /**
   * Clear all tools.
   */
  clear(): void {
    this.tools.clear();
  }
}

// =============================================================================
// Exports
// =============================================================================

export type {
  ToolContentType,
};
