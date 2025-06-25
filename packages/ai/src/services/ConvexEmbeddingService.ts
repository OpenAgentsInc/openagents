/**
 * Service for integrating message embeddings with Convex
 * @since 1.0.0
 */

// TODO: Re-enable when convex dependency is added back
// import { ConvexHttpClient } from "convex/browser"
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
  readonly _id: string
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
    readonly embedAndStore: (message: ConvexMessage) => Effect.Effect<string, AiError>

    /**
     * Generate and store embeddings for multiple messages
     */
    readonly embedAndStoreBatch: (
      messages: ReadonlyArray<ConvexMessage>
    ) => Effect.Effect<ReadonlyArray<{ message_id: string; embedding_id: string }>, AiError>

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
    // TODO: Re-enable when convex dependency is added back
    // Get Convex client from environment
    // const url = typeof process !== "undefined" && process.env?.CONVEX_URL
    //   ? process.env.CONVEX_URL
    //   : "https://proficient-panther-764.convex.cloud"
    // const convexClient = new ConvexHttpClient(url)
    
    // Temporary placeholder until convex dependency is restored
    const convexClient = null as any

    const embeddingService = yield* MessageEmbeddingService

    const embedAndStore = (message: ConvexMessage) =>
      Effect.gen(function*() {
        // Generate embedding
        const result = yield* embeddingService.embedMessage(message)

        // Store in Convex
        // Note: This assumes the Convex function exists
        const embeddingId = yield* Effect.tryPromise({
          try: async () => {
            // Using string-based function name until API is generated
            const convexResult = await (convexClient as any).mutation("embeddings:storeEmbedding", {
              message_id: message._id,
              embedding: result.embedding,
              model: result.model,
              dimensions: result.dimensions
            })
            return convexResult as string
          },
          catch: (error) =>
            new AiError({
              module: "ConvexEmbeddingService",
              method: "embedAndStore",
              description: "Failed to store embedding in Convex",
              cause: error
            })
        })

        return embeddingId as string
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
        const results: Array<{ message_id: string; embedding_id: string }> = []

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
            try: async () => {
              const result = await (convexClient as any).mutation("embeddings:batchStoreEmbeddings", {
                embeddings: embeddingData
              })
              return result as Array<{ message_id: string; embedding_id: string }>
            },
            catch: (error) =>
              new AiError({
                module: "ConvexEmbeddingService",
                method: "embedAndStoreBatch",
                description: "Failed to batch store embeddings in Convex",
                cause: error
              })
          })

          for (const result of batchResults as Array<{ message_id: string; embedding_id: string }>) {
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
        try: async () => {
          const result = await (convexClient as any).action("embeddings:similarMessages", {
            query_embedding: options.query_embedding,
            session_id: options.session_id,
            limit: options.limit,
            model_filter: options.model_filter
          })
          return result as ReadonlyArray<SimilaritySearchResult>
        },
        catch: (error) =>
          new AiError({
            module: "ConvexEmbeddingService",
            method: "searchSimilar",
            description: "Failed to search similar messages",
            cause: error
          })
      }).pipe(
        Effect.map((results) => results as ReadonlyArray<SimilaritySearchResult>),
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
        const searchOptions: {
          query_embedding: Array<number>
          session_id?: string
          limit?: number
          model_filter?: string
        } = {
          query_embedding: [...queryEmbedding.embedding]
        }

        if (options.session_id !== undefined) {
          searchOptions.session_id = options.session_id
        }
        if (options.limit !== undefined) {
          searchOptions.limit = options.limit
        }
        if (options.model_filter !== undefined) {
          searchOptions.model_filter = options.model_filter
        }

        return yield* searchSimilar(searchOptions)
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
        try: async () => {
          const result = await (convexClient as any).query("embeddings:getEmbeddingStats", {
            session_id
          })
          return result as {
            total: number
            by_model: Record<string, number>
            oldest: number | null
            newest: number | null
          }
        },
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
