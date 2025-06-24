/**
 * Standardized tool types supporting both Vercel AI SDK and Effect patterns
 * @since 1.0.0
 */
import * as Effect from "effect/Effect"
import * as Schema from "effect/Schema"
import type { ZodSchema } from "zod"

// =============================================================================
// Core Tool Definition (Vercel AI SDK Compatible)
// =============================================================================

/**
 * Tool execution result
 * @since 1.0.0
 * @category Models
 */
export type ToolResult<T = unknown> =
  | { success: true; value: T }
  | { success: false; error: string }

/**
 * Core tool definition interface (Vercel AI SDK compatible)
 * @since 1.0.0
 * @category Models
 */
export interface CoreToolDefinition<TParameters = any, TResult = any> {
  /**
   * Optional description of what the tool does
   */
  description: string | undefined

  /**
   * Schema for validating tool parameters
   * Supports both Effect Schema and Zod
   */
  parameters: Schema.Schema<TParameters, any, any> | ZodSchema<TParameters>

  /**
   * Function to execute the tool
   * Can return either an Effect or a Promise
   */
  execute: (args: TParameters) => Effect.Effect<TResult, any, any> | Promise<TResult>
}

// =============================================================================
// Effect-Enhanced Tool
// =============================================================================

/**
 * Tool error for Effect-based execution
 * @since 1.0.0
 * @category Models
 */
export class ToolError extends Schema.TaggedError<ToolError>("ToolError")("ToolError", {
  toolName: Schema.String,
  message: Schema.String,
  cause: Schema.optional(Schema.Unknown)
}) {}

/**
 * Effect-enhanced tool with full type safety
 * @since 1.0.0
 * @category Models
 */
export interface Tool<
  Name extends string = string,
  Parameters = any,
  Result = any,
  Error = ToolError,
  Requirements = never
> {
  readonly name: Name
  readonly description: string | undefined
  readonly parametersSchema: Schema.Schema<Parameters, any, any>
  readonly resultSchema: Schema.Schema<Result, any, any> | undefined
  readonly errorSchema: Schema.Schema<Error, any, any> | undefined
  readonly requirements: ReadonlyArray<string> | undefined
  readonly execute: (args: Parameters) => Effect.Effect<Result, Error, Requirements>
}

/**
 * Create a tool from a Vercel AI SDK compatible definition
 * @since 1.0.0
 * @category Constructors
 */
export const fromCore = <N extends string, P, R>(
  name: N,
  definition: CoreToolDefinition<P, R>
): Tool<N, P, R, ToolError, never> => {
  // Convert Zod schema to Effect Schema if needed
  const parametersSchema = isZodSchema(definition.parameters)
    ? Schema.Any // TODO: Implement Zod to Effect Schema conversion
    : definition.parameters as Schema.Schema<P, any, any>

  // Create execute function that handles both Promise and Effect returns
  const execute = (args: P): Effect.Effect<R, ToolError, never> => {
    const result = definition.execute(args)

    if (isPromise(result)) {
      return Effect.tryPromise({
        try: () => result,
        catch: (error) =>
          new ToolError({
            toolName: name,
            message: error instanceof Error ? error.message : String(error),
            cause: error
          })
      })
    }

    return result as Effect.Effect<R, ToolError, never>
  }

  return {
    name,
    description: definition.description,
    parametersSchema,
    resultSchema: Schema.Any,
    errorSchema: ToolError,
    requirements: undefined,
    execute
  }
}

/**
 * Convert tool to Vercel AI SDK compatible format
 * @since 1.0.0
 * @category Conversions
 */
export const toCore = <N extends string, P, R, E, Req>(
  tool: Tool<N, P, R, E, Req>
): CoreToolDefinition<P, R> => ({
  description: tool.description,
  parameters: tool.parametersSchema,
  execute: (args) => Effect.runPromise(tool.execute(args) as Effect.Effect<R, E, never>)
})

// =============================================================================
// Tool Registry
// =============================================================================

/**
 * Collection of tools
 * @since 1.0.0
 * @category Models
 */
export interface ToolRegistry {
  readonly tools: Record<string, Tool<any, any, any, any, any>>
}

/**
 * Create a tool registry
 * @since 1.0.0
 * @category Constructors
 */
export const createToolRegistry = (tools: Record<string, Tool<any, any, any, any, any>> = {}): ToolRegistry => ({
  tools
})

/**
 * Get a tool by name
 * @since 1.0.0
 * @category Accessors
 */
export const getTool = <N extends string>(
  registry: ToolRegistry,
  name: N
): Tool<any, any, any, any, any> | undefined => registry.tools[name]

/**
 * Add a tool to the registry
 * @since 1.0.0
 * @category Combinators
 */
export const addTool = <N extends string, P, R, E, Req>(
  registry: ToolRegistry,
  tool: Tool<N, P, R, E, Req>
): ToolRegistry => ({
  tools: {
    ...registry.tools,
    [tool.name]: tool
  }
})

/**
 * Create registry from an array of tools
 * @since 1.0.0
 * @category Constructors
 */
export const fromToolArray = (tools: ReadonlyArray<Tool<any, any, any, any, any>>): ToolRegistry => {
  const toolMap: Record<string, Tool<any, any, any, any, any>> = {}
  for (const tool of tools) {
    toolMap[tool.name] = tool
  }
  return createToolRegistry(toolMap)
}

/**
 * Convert registry to Vercel AI SDK compatible format
 * @since 1.0.0
 * @category Conversions
 */
export const registryToCore = (registry: ToolRegistry): Record<string, CoreToolDefinition> => {
  const result: Record<string, CoreToolDefinition> = {}
  for (const [name, tool] of Object.entries(registry.tools)) {
    result[name] = toCore(tool)
  }
  return result
}

// =============================================================================
// Tool Choice
// =============================================================================

/**
 * Tool choice specification
 * @since 1.0.0
 * @category Models
 */
export const ToolChoice = Schema.Union(
  Schema.Literal("auto"),
  Schema.Literal("none"),
  Schema.Literal("required"),
  Schema.Struct({
    type: Schema.Literal("tool"),
    toolName: Schema.String
  })
)

/**
 * @since 1.0.0
 * @category Models
 */
export type ToolChoice = typeof ToolChoice.Type

// =============================================================================
// Utility Functions
// =============================================================================

/**
 * Type guard for Zod schemas
 */
function isZodSchema(schema: any): schema is ZodSchema {
  return schema && typeof schema.parse === "function" && typeof schema.safeParse === "function"
}

/**
 * Type guard for Promises
 */
function isPromise<T>(value: any): value is Promise<T> {
  return value && typeof value.then === "function"
}

// =============================================================================
// Tool Builder API
// =============================================================================

/**
 * Create a new tool
 * @since 1.0.0
 * @category Constructors
 */
export const createTool = <Name extends string, Parameters, Result, Error = ToolError, Requirements = never>(
  config: {
    name: Name
    description: string | undefined
    parametersSchema: Schema.Schema<Parameters, any, any>
    resultSchema: Schema.Schema<Result, any, any> | undefined
    errorSchema: Schema.Schema<Error, any, any> | undefined
    execute: (args: Parameters) => Effect.Effect<Result, Error, Requirements>
  }
): Tool<Name, Parameters, Result, Error, Requirements> => ({
  name: config.name,
  description: config.description,
  parametersSchema: config.parametersSchema,
  resultSchema: config.resultSchema,
  errorSchema: config.errorSchema,
  requirements: undefined,
  execute: config.execute
})
