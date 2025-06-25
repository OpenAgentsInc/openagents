/**
 * Service for generating embeddings for chat messages
 * @since 1.0.0
 */

import * as Context from "effect/Context"
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"
import * as Schema from "effect/Schema"
import { AiEmbeddingModel } from "../core/AiEmbeddingModel.js"
import type { AiError } from "../core/AiError.js"

/**
 * Configuration for the message embedding service
 * @since 1.0.0
 */
export interface MessageEmbeddingConfig {
  /**
   * The embedding model to use (e.g., "text-embedding-3-small")
   */
  readonly model: string
  /**
   * The dimensions of the embeddings (e.g., 1536 for OpenAI)
   */
  readonly dimensions: number
  /**
   * Maximum content length before truncation
   */
  readonly maxContentLength?: number
  /**
   * Whether to include thinking content in embeddings
   */
  readonly includeThinking?: boolean
  /**
   * Whether to include tool outputs in embeddings
   */
  readonly includeToolOutputs?: boolean
}

/**
 * Message content to be embedded
 * @since 1.0.0
 */
export class MessageContent extends Schema.Class<MessageContent>("MessageContent")({
  content: Schema.optional(Schema.String),
  thinking: Schema.optional(Schema.String),
  summary: Schema.optional(Schema.String),
  tool_output: Schema.optional(Schema.String),
  entry_type: Schema.String,
  role: Schema.optional(Schema.String)
}) {}

/**
 * Result of embedding generation
 * @since 1.0.0
 */
export class EmbeddingResult extends Schema.Class<EmbeddingResult>("EmbeddingResult")({
  embedding: Schema.Array(Schema.Number),
  model: Schema.String,
  dimensions: Schema.Number,
  input_text: Schema.String,
  truncated: Schema.Boolean
}) {}

/**
 * Service for generating embeddings for chat messages
 * @since 1.0.0
 */
export class MessageEmbeddingService extends Context.Tag("@openagentsinc/ai/MessageEmbeddingService")<
  MessageEmbeddingService,
  MessageEmbeddingService.Service
>() {}

/**
 * @since 1.0.0
 */
export declare namespace MessageEmbeddingService {
  /**
   * @since 1.0.0
   * @category Models
   */
  export interface Service {
    /**
     * Generate embedding for a single message
     */
    readonly embedMessage: (message: MessageContent) => Effect.Effect<EmbeddingResult, AiError>

    /**
     * Generate embeddings for multiple messages in batch
     */
    readonly embedMessages: (
      messages: ReadonlyArray<MessageContent>
    ) => Effect.Effect<ReadonlyArray<EmbeddingResult>, AiError>

    /**
     * Get the configuration
     */
    readonly config: MessageEmbeddingConfig
  }
}

/**
 * Prepare message content for embedding
 * @internal
 */
const prepareMessageText = (
  message: MessageContent,
  config: MessageEmbeddingConfig
): string => {
  const parts: Array<string> = []

  // Add role/type prefix for context
  if (message.role) {
    parts.push(`[${message.role}]`)
  } else if (message.entry_type) {
    parts.push(`[${message.entry_type}]`)
  }

  // Add main content
  if (message.content) {
    parts.push(message.content)
  }

  // Add summary if present
  if (message.summary) {
    parts.push(`Summary: ${message.summary}`)
  }

  // Add thinking content if configured
  if (config.includeThinking && message.thinking) {
    parts.push(`Thinking: ${message.thinking}`)
  }

  // Add tool output if configured
  if (config.includeToolOutputs && message.tool_output) {
    parts.push(`Tool Output: ${message.tool_output}`)
  }

  // Join and truncate if needed
  let text = parts.join(" ").trim()
  const truncated = config.maxContentLength && text.length > config.maxContentLength

  if (truncated && config.maxContentLength) {
    text = text.substring(0, config.maxContentLength) + "..."
  }

  return text
}

/**
 * Create a message embedding service
 * @since 1.0.0
 * @category Constructors
 */
export const make = (config: MessageEmbeddingConfig) =>
  Effect.gen(function*() {
    const embeddingModel = yield* AiEmbeddingModel

    const embedMessage = (message: MessageContent) =>
      Effect.gen(function*() {
        const inputText = prepareMessageText(message, config)

        // Skip empty messages
        if (!inputText) {
          return {
            embedding: new Array(config.dimensions).fill(0),
            model: config.model,
            dimensions: config.dimensions,
            input_text: "",
            truncated: false
          }
        }

        const embedding = yield* embeddingModel.embed(inputText)

        return {
          embedding,
          model: config.model,
          dimensions: config.dimensions,
          input_text: inputText,
          truncated: config.maxContentLength ? inputText.endsWith("...") : false
        }
      }).pipe(
        Effect.withSpan("MessageEmbeddingService.embedMessage", {
          attributes: {
            "message.type": message.entry_type,
            "message.role": message.role || "none",
            "embedding.model": config.model
          }
        })
      )

    const embedMessages = (messages: ReadonlyArray<MessageContent>) =>
      Effect.forEach(messages, embedMessage, {
        concurrency: 5, // Process up to 5 messages in parallel
        batching: true
      }).pipe(
        Effect.withSpan("MessageEmbeddingService.embedMessages", {
          attributes: {
            "messages.count": messages.length,
            "embedding.model": config.model
          }
        })
      )

    return MessageEmbeddingService.of({
      embedMessage,
      embedMessages,
      config
    })
  })

/**
 * Layer that provides MessageEmbeddingService with default configuration
 * @since 1.0.0
 * @category Layers
 */
export const layer = (config: MessageEmbeddingConfig) => Layer.effect(MessageEmbeddingService, make(config))

/**
 * Default configuration for OpenAI embeddings
 * @since 1.0.0
 * @category Configuration
 */
export const defaultOpenAIConfig: MessageEmbeddingConfig = {
  model: "text-embedding-3-small",
  dimensions: 1536,
  maxContentLength: 8000,
  includeThinking: false,
  includeToolOutputs: true
}
