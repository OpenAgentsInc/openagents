/**
 * Standardized provider types for language models
 * @since 1.0.0
 */
import * as Context from "effect/Context"
import * as Effect from "effect/Effect"
import * as Schema from "effect/Schema"
import * as Stream from "effect/Stream"
import type { FinishReason, Message, ResponseMetadata, TokenUsage } from "./messages.js"
import type { CoreToolDefinition, ToolChoice } from "./tools.js"

// =============================================================================
// Core Options
// =============================================================================

/**
 * Base options for all generation methods
 * @since 1.0.0
 * @category Models
 */
export interface BaseGenerateOptions {
  /**
   * The model identifier
   */
  model?: string

  /**
   * Maximum number of tokens to generate
   */
  maxTokens?: number

  /**
   * Temperature for randomness (0-2)
   */
  temperature?: number

  /**
   * Top-p nucleus sampling
   */
  topP?: number

  /**
   * Top-k sampling
   */
  topK?: number

  /**
   * Frequency penalty (-2 to 2)
   */
  frequencyPenalty?: number

  /**
   * Presence penalty (-2 to 2)
   */
  presencePenalty?: number

  /**
   * Stop sequences
   */
  stopSequences?: Array<string>

  /**
   * Random seed for deterministic generation
   */
  seed?: number

  /**
   * Maximum number of retries
   */
  maxRetries?: number

  /**
   * Abort signal for cancellation
   */
  abortSignal?: AbortSignal

  /**
   * Custom headers to send with the request
   */
  headers?: Record<string, string>
}

/**
 * Options for text generation
 * @since 1.0.0
 * @category Models
 */
export interface GenerateTextOptions extends BaseGenerateOptions {
  /**
   * The messages to send
   */
  messages: ReadonlyArray<Message>

  /**
   * System prompt (if not using system message)
   */
  system?: string

  /**
   * Tools available for the model to use
   */
  tools?: Record<string, CoreToolDefinition>

  /**
   * How the model should use tools
   */
  toolChoice?: ToolChoice

  /**
   * Response format hint
   */
  responseFormat?: "text" | "json"
}

/**
 * Options for object generation
 * @since 1.0.0
 * @category Models
 */
export interface GenerateObjectOptions<T> extends BaseGenerateOptions {
  /**
   * The messages to send
   */
  messages: ReadonlyArray<Message>

  /**
   * System prompt (if not using system message)
   */
  system?: string

  /**
   * Schema for the output object
   */
  schema: Schema.Schema<T, any, any>

  /**
   * Schema name and description for the model
   */
  schemaName?: string
  schemaDescription?: string
}

// =============================================================================
// Generation Results
// =============================================================================

/**
 * Result from text generation
 * @since 1.0.0
 * @category Models
 */
export interface GenerateTextResult {
  /**
   * The generated text
   */
  text: string

  /**
   * Tool calls made by the model
   */
  toolCalls?: Array<{
    toolCallId: string
    toolName: string
    args: unknown
  }>

  /**
   * Reasoning text (if supported by model)
   */
  reasoning?: string

  /**
   * Response metadata
   */
  metadata: ResponseMetadata
}

/**
 * Result from object generation
 * @since 1.0.0
 * @category Models
 */
export interface GenerateObjectResult<T> {
  /**
   * The generated object
   */
  object: T

  /**
   * Response metadata
   */
  metadata: ResponseMetadata
}

/**
 * Chunk in a streaming response
 * @since 1.0.0
 * @category Models
 */
export type StreamChunk =
  | { type: "text"; text: string }
  | { type: "tool-call"; toolCallId: string; toolName: string; args: unknown }
  | { type: "reasoning"; reasoning: string }
  | { type: "metadata"; metadata: Partial<ResponseMetadata> }
  | { type: "finish"; finishReason: FinishReason; usage?: TokenUsage }

// =============================================================================
// Language Model Service (Vercel AI SDK Compatible)
// =============================================================================

/**
 * Core language model service interface
 * @since 1.0.0
 * @category Services
 */
export interface LanguageModelService {
  /**
   * Model identifier
   */
  readonly modelId: string

  /**
   * Provider identifier
   */
  readonly provider: string

  /**
   * Generate text (Promise-based for Vercel AI SDK compatibility)
   */
  doGenerate(options: GenerateTextOptions): Promise<GenerateTextResult>

  /**
   * Stream text (Promise-based for Vercel AI SDK compatibility)
   */
  doStream(options: GenerateTextOptions): Promise<ReadableStream<StreamChunk>>

  /**
   * Generate structured object (Promise-based)
   */
  doGenerateObject?<T>(options: GenerateObjectOptions<T>): Promise<GenerateObjectResult<T>>

  /**
   * Generate text (Effect-based)
   */
  generateText(options: GenerateTextOptions): Effect.Effect<GenerateTextResult, AIError | APICallError, any>

  /**
   * Stream text (Effect-based)
   */
  streamText(options: GenerateTextOptions): Stream.Stream<StreamChunk, AIError | APICallError, any>

  /**
   * Generate structured object (Effect-based)
   */
  generateObject<T>(
    options: GenerateObjectOptions<T>
  ): Effect.Effect<GenerateObjectResult<T>, AIError | APICallError, any>
}

// =============================================================================
// Errors
// =============================================================================

/**
 * Base AI error
 * @since 1.0.0
 * @category Errors
 */
export class AIError extends Schema.TaggedError<AIError>("AIError")("AIError", {
  message: Schema.String,
  cause: Schema.optional(Schema.Unknown),
  isRetryable: Schema.optional(Schema.Boolean)
}) {}

/**
 * API call error
 * @since 1.0.0
 * @category Errors
 */
export class APICallError extends Schema.TaggedError<APICallError>("APICallError")("APICallError", {
  message: Schema.String,
  statusCode: Schema.optional(Schema.Number),
  responseBody: Schema.optional(Schema.String),
  cause: Schema.optional(Schema.Unknown),
  isRetryable: Schema.optional(Schema.Boolean)
}) {}

/**
 * Invalid prompt error
 * @since 1.0.0
 * @category Errors
 */
export class InvalidPromptError
  extends Schema.TaggedError<InvalidPromptError>("InvalidPromptError")("InvalidPromptError", {
    message: Schema.String,
    cause: Schema.optional(Schema.Unknown)
  })
{}

// =============================================================================
// Provider Registry
// =============================================================================

/**
 * Provider configuration
 * @since 1.0.0
 * @category Models
 */
export interface ProviderConfig {
  /**
   * API key for authentication
   */
  apiKey?: string

  /**
   * Base URL for API calls
   */
  baseURL?: string

  /**
   * Default headers
   */
  headers?: Record<string, string>

  /**
   * Fetch implementation
   */
  fetch?: typeof fetch
}

/**
 * Provider interface
 * @since 1.0.0
 * @category Services
 */
export interface Provider {
  /**
   * Provider identifier
   */
  readonly id: string

  /**
   * Create a language model
   */
  languageModel(modelId: string): LanguageModelService

  /**
   * Create a text embedding model
   */
  textEmbeddingModel?(modelId: string): any // TODO: Define embedding model interface
}

/**
 * Provider registry service
 * @since 1.0.0
 * @category Services
 */
export class ProviderRegistry extends Context.Tag("ProviderRegistry")<
  ProviderRegistry,
  {
    /**
     * Get a provider by ID
     */
    readonly getProvider: (id: string) => Effect.Effect<Provider, ProviderNotFoundError>

    /**
     * Register a provider
     */
    readonly registerProvider: (provider: Provider) => Effect.Effect<void>

    /**
     * List all registered providers
     */
    readonly listProviders: () => Effect.Effect<ReadonlyArray<string>>
  }
>() {}

/**
 * Provider not found error
 * @since 1.0.0
 * @category Errors
 */
export class ProviderNotFoundError extends Schema.TaggedError<ProviderNotFoundError>(
  "ProviderNotFoundError"
)("ProviderNotFoundError", {
  providerId: Schema.String,
  availableProviders: Schema.Array(Schema.String)
}) {}

// =============================================================================
// Model Creation Helpers
// =============================================================================

/**
 * Create a language model from a provider string
 * @since 1.0.0
 * @category Constructors
 */
export const createModel = (
  modelString: string // format: "provider:model-id" or just "model-id" for default provider
): Effect.Effect<LanguageModelService, AIError | ProviderNotFoundError, ProviderRegistry> =>
  Effect.gen(function*() {
    const registry = yield* ProviderRegistry

    const [providerId, modelId] = modelString.includes(":")
      ? modelString.split(":", 2)
      : ["openai", modelString] // default to OpenAI

    const provider = yield* registry.getProvider(providerId)
    return provider.languageModel(modelId)
  })

// =============================================================================
// Adapter Utilities
// =============================================================================

/**
 * Wrap a Vercel AI SDK compatible model as an Effect service
 * @since 1.0.0
 * @category Adapters
 */
export const wrapVercelModel = (
  model: {
    modelId: string
    provider: string
    doGenerate: (options: any) => Promise<any>
    doStream: (options: any) => Promise<ReadableStream<any>>
    doGenerateObject?: (options: any) => Promise<any>
  }
): LanguageModelService => ({
  modelId: model.modelId,
  provider: model.provider,

  // Promise-based methods (pass through)
  doGenerate: model.doGenerate.bind(model),
  doStream: model.doStream.bind(model),
  ...(model.doGenerateObject && { doGenerateObject: model.doGenerateObject.bind(model) }),

  // Effect-based methods
  generateText: (options) =>
    Effect.tryPromise({
      try: () => model.doGenerate(options),
      catch: (error) =>
        new APICallError({
          message: error instanceof Error ? error.message : String(error),
          cause: error,
          isRetryable: true
        })
    }),

  streamText: (options) =>
    Stream.fromEffect(
      Effect.tryPromise({
        try: async () => {
          const stream = await model.doStream(options)
          const reader = stream.getReader()

          return {
            async *[Symbol.asyncIterator]() {
              try {
                while (true) {
                  const { done, value } = await reader.read()
                  if (done) break
                  yield value
                }
              } finally {
                reader.releaseLock()
              }
            }
          }
        },
        catch: (error) =>
          new APICallError({
            message: error instanceof Error ? error.message : String(error),
            cause: error,
            isRetryable: true
          })
      })
    ).pipe(
      Stream.flatMap((asyncIterable) =>
        Stream.fromAsyncIterable(asyncIterable, (error) =>
          new APICallError({
            message: error instanceof Error ? error.message : String(error),
            cause: error,
            isRetryable: false
          }))
      )
    ),

  generateObject: model.doGenerateObject
    ? (options) =>
      Effect.tryPromise({
        try: () => model.doGenerateObject!(options),
        catch: (error) =>
          new APICallError({
            message: error instanceof Error ? error.message : String(error),
            cause: error,
            isRetryable: true
          })
      })
    : (_options) =>
      Effect.fail(
        new AIError({
          message: "Object generation not supported by this model",
          isRetryable: false
        })
      )
})
