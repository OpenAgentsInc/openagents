/**
 * Service for integrating message embeddings with Convex
 * @since 1.0.0
 */

import { ConvexClient, type Id } from "@openagentsinc/convex"
import * as Context from "effect/Context"
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"
import * as Schema from "effect/Schema"
import { AiError } from "../core/AiError.js"
import { type MessageContent, MessageEmbeddingService } from "./MessageEmbeddingService.js"

/**
 * Configuration for Convex embedding service
 * @since 1.0.0
 */
export interface ConvexEmbeddingConfig {
  /**
   * Whether to automatically store embeddings after generation
   */
  readonly autoStore?: boolean
  /**
   * Batch size for storing embeddings
   */
  readonly batchSize?: number
}

/**
 * Message with Convex ID
 * @since 1.0.0
 */
export interface ConvexMessage extends MessageContent {
  readonly _id: Id<"messages">
  readonly session_id: string
}

/**
 * Similarity search result
 * @since 1.0.0
 */
export class SimilaritySearchResult extends Schema.Class<SimilaritySearchResult>("SimilaritySearchResult")({
  message: Schema.Any, // ConvexMessage
  embedding: Schema.Any, // Embedding record
  similarity_score: Schema.Number
}) {}

/**
 * Service for managing embeddings in Convex
 * @since 1.0.0
 */
export class ConvexEmbeddingService extends Context.Tag("@openagentsinc/ai/ConvexEmbeddingService")<
  ConvexEmbeddingService,
  ConvexEmbeddingService.Service
>() {}

/**
 * @since 1.0.0
 */
export declare namespace ConvexEmbeddingService {
  /**
   * @since 1.0.0
   * @category Models
   */
  export interface Service {
    /**
     * Generate and store embedding for a message
     */
    readonly embedAndStore: (message: ConvexMessage) => Effect.Effect<Id<"message_embeddings">, AiError>

    /**
     * Generate and store embeddings for multiple messages
     */
    readonly embedAndStoreBatch: (
      messages: ReadonlyArray<ConvexMessage>
    ) => Effect.Effect<ReadonlyArray<{ message_id: Id<"messages">; embedding_id: Id<"message_embeddings"> }>, AiError>

    /**
     * Search for similar messages
     */
    readonly searchSimilar: (options: {
      query_embedding: Array<number>
      session_id?: string
      limit?: number
      model_filter?: string
    }) => Effect.Effect<ReadonlyArray<SimilaritySearchResult>, AiError>

    /**
     * Search for similar messages using text query
     */
    readonly searchSimilarByText: (options: {
      query_text: string
      session_id?: string
      limit?: number
      model_filter?: string
    }) => Effect.Effect<ReadonlyArray<SimilaritySearchResult>, AiError>

    /**
     * Get embedding statistics
     */
    readonly getStats: (session_id?: string) => Effect.Effect<{
      total: number
      by_model: Record<string, number>
      oldest: number | null
      newest: number | null
    }, AiError>
  }
}

/**
 * Create a Convex embedding service
 * @since 1.0.0
 * @category Constructors
 */
export const make = (config: ConvexEmbeddingConfig = {}) =>
  Effect.gen(function*() {
    const convexClient = yield* ConvexClient
    const embeddingService = yield* MessageEmbeddingService

    const embedAndStore = (message: ConvexMessage) =>
      Effect.gen(function*() {
        // Generate embedding
        const result = yield* embeddingService.embedMessage(message)

        // Store in Convex
        const embeddingId = yield* Effect.tryPromise({
          try: () =>
            convexClient.mutation("embeddings:storeEmbedding", {
              message_id: message._id,
              embedding: result.embedding,
              model: result.model,
              dimensions: result.dimensions
            }),
          catch: (error) =>
            new AiError({
              module: "ConvexEmbeddingService",
              method: "embedAndStore",
              description: "Failed to store embedding in Convex",
              cause: error
            })
        })

        return embeddingId as Id<"message_embeddings">
      }).pipe(
        Effect.withSpan("ConvexEmbeddingService.embedAndStore", {
          attributes: {
            "message.id": message._id,
            "message.type": message.entry_type,
            "session.id": message.session_id
          }
        })
      )

    const embedAndStoreBatch = (messages: ReadonlyArray<ConvexMessage>) =>
      Effect.gen(function*() {
        const batchSize = config.batchSize ?? 10
        const results: Array<{ message_id: Id<"messages">; embedding_id: Id<"message_embeddings"> }> = []

        // Process in batches
        for (let i = 0; i < messages.length; i += batchSize) {
          const batch = messages.slice(i, i + batchSize)

          // Generate embeddings for batch
          const embeddings = yield* embeddingService.embedMessages(batch)

          // Prepare batch storage data
          const embeddingData = batch.map((message, index) => ({
            message_id: message._id,
            embedding: embeddings[index].embedding,
            model: embeddings[index].model,
            dimensions: embeddings[index].dimensions
          }))

          // Store batch in Convex
          const batchResults = yield* Effect.tryPromise({
            try: () =>
              convexClient.mutation("embeddings:batchStoreEmbeddings", {
                embeddings: embeddingData
              }),
            catch: (error) =>
              new AiError({
                module: "ConvexEmbeddingService",
                method: "embedAndStoreBatch",
                description: "Failed to batch store embeddings in Convex",
                cause: error
              })
          })

          for (const result of batchResults) {
            results.push(result)
          }
        }

        return results
      }).pipe(
        Effect.withSpan("ConvexEmbeddingService.embedAndStoreBatch", {
          attributes: {
            "messages.count": messages.length,
            "batch.size": config.batchSize ?? 10
          }
        })
      )

    const searchSimilar = (options: {
      query_embedding: Array<number>
      session_id?: string
      limit?: number
      model_filter?: string
    }) =>
      Effect.tryPromise({
        try: () =>
          convexClient.action("embeddings:similarMessages", {
            query_embedding: options.query_embedding,
            session_id: options.session_id,
            limit: options.limit,
            model_filter: options.model_filter
          }),
        catch: (error) =>
          new AiError({
            module: "ConvexEmbeddingService",
            method: "searchSimilar",
            description: "Failed to search similar messages",
            cause: error
          })
      }).pipe(
        Effect.withSpan("ConvexEmbeddingService.searchSimilar", {
          attributes: {
            "search.limit": options.limit ?? 10,
            "search.has_session_filter": !!options.session_id,
            "search.has_model_filter": !!options.model_filter
          }
        })
      )

    const searchSimilarByText = (options: {
      query_text: string
      session_id?: string
      limit?: number
      model_filter?: string
    }) =>
      Effect.gen(function*() {
        // Generate embedding for query text
        const queryEmbedding = yield* embeddingService.embedMessage({
          content: options.query_text,
          entry_type: "query",
          role: undefined,
          thinking: undefined,
          summary: undefined,
          tool_output: undefined
        })

        // Search with the generated embedding
        return yield* searchSimilar({
          query_embedding: queryEmbedding.embedding,
          session_id: options.session_id,
          limit: options.limit,
          model_filter: options.model_filter
        })
      }).pipe(
        Effect.withSpan("ConvexEmbeddingService.searchSimilarByText", {
          attributes: {
            "search.query_length": options.query_text.length,
            "search.limit": options.limit ?? 10
          }
        })
      )

    const getStats = (session_id?: string) =>
      Effect.tryPromise({
        try: () =>
          convexClient.query("embeddings:getEmbeddingStats", {
            session_id
          }),
        catch: (error) =>
          new AiError({
            module: "ConvexEmbeddingService",
            method: "getStats",
            description: "Failed to get embedding statistics",
            cause: error
          })
      }).pipe(
        Effect.withSpan("ConvexEmbeddingService.getStats", {
          attributes: {
            "stats.has_session_filter": !!session_id
          }
        })
      )

    return ConvexEmbeddingService.of({
      embedAndStore,
      embedAndStoreBatch,
      searchSimilar,
      searchSimilarByText,
      getStats
    })
  })

/**
 * Layer that provides ConvexEmbeddingService
 * @since 1.0.0
 * @category Layers
 */
export const layer = (config: ConvexEmbeddingConfig = {}) => Layer.effect(ConvexEmbeddingService, make(config))
