/**
 * Service for integrating message embeddings with Convex
 * @since 1.0.0
 */
import * as Context from "effect/Context";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Schema from "effect/Schema";
import { type Id } from "@openagentsinc/convex";
import { type MessageContent } from "./MessageEmbeddingService.js";
import { AiError } from "../core/AiError.js";
/**
 * Configuration for Convex embedding service
 * @since 1.0.0
 */
export interface ConvexEmbeddingConfig {
    /**
     * Whether to automatically store embeddings after generation
     */
    readonly autoStore?: boolean;
    /**
     * Batch size for storing embeddings
     */
    readonly batchSize?: number;
}
/**
 * Message with Convex ID
 * @since 1.0.0
 */
export interface ConvexMessage extends MessageContent {
    readonly _id: Id<"messages">;
    readonly session_id: string;
}
declare const SimilaritySearchResult_base: Schema.Class<SimilaritySearchResult, {
    message: typeof Schema.Any;
    embedding: typeof Schema.Any;
    similarity_score: typeof Schema.Number;
}, Schema.Struct.Encoded<{
    message: typeof Schema.Any;
    embedding: typeof Schema.Any;
    similarity_score: typeof Schema.Number;
}>, never, {
    readonly embedding: any;
} & {
    readonly message: any;
} & {
    readonly similarity_score: number;
}, {}, {}>;
/**
 * Similarity search result
 * @since 1.0.0
 */
export declare class SimilaritySearchResult extends SimilaritySearchResult_base {
}
declare const ConvexEmbeddingService_base: Context.TagClass<ConvexEmbeddingService, "@openagentsinc/ai/ConvexEmbeddingService", ConvexEmbeddingService.Service>;
/**
 * Service for managing embeddings in Convex
 * @since 1.0.0
 */
export declare class ConvexEmbeddingService extends ConvexEmbeddingService_base {
}
/**
 * @since 1.0.0
 */
export declare namespace ConvexEmbeddingService {
    /**
     * @since 1.0.0
     * @category Models
     */
    interface Service {
        /**
         * Generate and store embedding for a message
         */
        readonly embedAndStore: (message: ConvexMessage) => Effect.Effect<Id<"message_embeddings">, AiError>;
        /**
         * Generate and store embeddings for multiple messages
         */
        readonly embedAndStoreBatch: (messages: ReadonlyArray<ConvexMessage>) => Effect.Effect<ReadonlyArray<{
            message_id: Id<"messages">;
            embedding_id: Id<"message_embeddings">;
        }>, AiError>;
        /**
         * Search for similar messages
         */
        readonly searchSimilar: (options: {
            query_embedding: number[];
            session_id?: string;
            limit?: number;
            model_filter?: string;
        }) => Effect.Effect<ReadonlyArray<SimilaritySearchResult>, AiError>;
        /**
         * Search for similar messages using text query
         */
        readonly searchSimilarByText: (options: {
            query_text: string;
            session_id?: string;
            limit?: number;
            model_filter?: string;
        }) => Effect.Effect<ReadonlyArray<SimilaritySearchResult>, AiError>;
        /**
         * Get embedding statistics
         */
        readonly getStats: (session_id?: string) => Effect.Effect<{
            total: number;
            by_model: Record<string, number>;
            oldest: number | null;
            newest: number | null;
        }, AiError>;
    }
}
/**
 * Create a Convex embedding service
 * @since 1.0.0
 * @category Constructors
 */
export declare const make: (config?: ConvexEmbeddingConfig) => Effect.Effect<ConvexEmbeddingService.Service, unknown, unknown>;
/**
 * Layer that provides ConvexEmbeddingService
 * @since 1.0.0
 * @category Layers
 */
export declare const layer: (config?: ConvexEmbeddingConfig) => Layer.Layer<ConvexEmbeddingService, unknown, unknown>;
export {};
//# sourceMappingURL=ConvexEmbeddingService.d.ts.map