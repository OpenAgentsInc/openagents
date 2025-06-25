/**
 * Service for generating embeddings for chat messages
 * @since 1.0.0
 */
import * as Context from "effect/Context";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Schema from "effect/Schema";
import { AiEmbeddingModel } from "../core/AiEmbeddingModel.js";
import { AiError } from "../core/AiError.js";
/**
 * Configuration for the message embedding service
 * @since 1.0.0
 */
export interface MessageEmbeddingConfig {
    /**
     * The embedding model to use (e.g., "text-embedding-3-small")
     */
    readonly model: string;
    /**
     * The dimensions of the embeddings (e.g., 1536 for OpenAI)
     */
    readonly dimensions: number;
    /**
     * Maximum content length before truncation
     */
    readonly maxContentLength?: number;
    /**
     * Whether to include thinking content in embeddings
     */
    readonly includeThinking?: boolean;
    /**
     * Whether to include tool outputs in embeddings
     */
    readonly includeToolOutputs?: boolean;
}
declare const MessageContent_base: Schema.Class<MessageContent, {
    content: Schema.optional<typeof Schema.String>;
    thinking: Schema.optional<typeof Schema.String>;
    summary: Schema.optional<typeof Schema.String>;
    tool_output: Schema.optional<typeof Schema.String>;
    entry_type: typeof Schema.String;
    role: Schema.optional<typeof Schema.String>;
}, Schema.Struct.Encoded<{
    content: Schema.optional<typeof Schema.String>;
    thinking: Schema.optional<typeof Schema.String>;
    summary: Schema.optional<typeof Schema.String>;
    tool_output: Schema.optional<typeof Schema.String>;
    entry_type: typeof Schema.String;
    role: Schema.optional<typeof Schema.String>;
}>, never, {
    readonly entry_type: string;
} & {
    readonly content?: string | undefined;
} & {
    readonly thinking?: string | undefined;
} & {
    readonly summary?: string | undefined;
} & {
    readonly tool_output?: string | undefined;
} & {
    readonly role?: string | undefined;
}, {}, {}>;
/**
 * Message content to be embedded
 * @since 1.0.0
 */
export declare class MessageContent extends MessageContent_base {
}
declare const EmbeddingResult_base: Schema.Class<EmbeddingResult, {
    embedding: Schema.Array$<typeof Schema.Number>;
    model: typeof Schema.String;
    dimensions: typeof Schema.Number;
    input_text: typeof Schema.String;
    truncated: typeof Schema.Boolean;
}, Schema.Struct.Encoded<{
    embedding: Schema.Array$<typeof Schema.Number>;
    model: typeof Schema.String;
    dimensions: typeof Schema.Number;
    input_text: typeof Schema.String;
    truncated: typeof Schema.Boolean;
}>, never, {
    readonly model: string;
} & {
    readonly dimensions: number;
} & {
    readonly input_text: string;
} & {
    readonly truncated: boolean;
} & {
    readonly embedding: readonly number[];
}, {}, {}>;
/**
 * Result of embedding generation
 * @since 1.0.0
 */
export declare class EmbeddingResult extends EmbeddingResult_base {
}
declare const MessageEmbeddingService_base: Context.TagClass<MessageEmbeddingService, "@openagentsinc/ai/MessageEmbeddingService", MessageEmbeddingService.Service>;
/**
 * Service for generating embeddings for chat messages
 * @since 1.0.0
 */
export declare class MessageEmbeddingService extends MessageEmbeddingService_base {
}
/**
 * @since 1.0.0
 */
export declare namespace MessageEmbeddingService {
    /**
     * @since 1.0.0
     * @category Models
     */
    interface Service {
        /**
         * Generate embedding for a single message
         */
        readonly embedMessage: (message: MessageContent) => Effect.Effect<EmbeddingResult, AiError>;
        /**
         * Generate embeddings for multiple messages in batch
         */
        readonly embedMessages: (messages: ReadonlyArray<MessageContent>) => Effect.Effect<ReadonlyArray<EmbeddingResult>, AiError>;
        /**
         * Get the configuration
         */
        readonly config: MessageEmbeddingConfig;
    }
}
/**
 * Create a message embedding service
 * @since 1.0.0
 * @category Constructors
 */
export declare const make: (config: MessageEmbeddingConfig) => Effect.Effect<MessageEmbeddingService.Service, never, AiEmbeddingModel>;
/**
 * Layer that provides MessageEmbeddingService with default configuration
 * @since 1.0.0
 * @category Layers
 */
export declare const layer: (config: MessageEmbeddingConfig) => Layer.Layer<MessageEmbeddingService, never, AiEmbeddingModel>;
/**
 * Default configuration for OpenAI embeddings
 * @since 1.0.0
 * @category Configuration
 */
export declare const defaultOpenAIConfig: MessageEmbeddingConfig;
export {};
//# sourceMappingURL=MessageEmbeddingService.d.ts.map