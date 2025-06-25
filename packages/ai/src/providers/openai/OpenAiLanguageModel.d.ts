import * as Context from "effect/Context";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import type { Simplify } from "effect/Types";
import * as AiLanguageModel from "../../core/AiLanguageModel.js";
import * as AiModel from "../../core/AiModel.js";
import type * as Tokenizer from "../../core/Tokenizer.js";
import type * as Generated from "./Generated.js";
import { OpenAiClient } from "./OpenAiClient.js";
/**
 * @since 1.0.0
 * @category Models
 */
export type Model = typeof Generated.ModelIdsSharedEnum.Encoded;
declare const Config_base: Context.TagClass<Config, "@openagentsinc/ai/OpenAiLanguageModel/Config", {
    readonly model?: string;
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | null | undefined;
    readonly temperature?: number | null | undefined;
    readonly top_p?: number | null | undefined;
    readonly response_format?: {
        readonly type: "text";
    } | {
        readonly type: "json_object";
    } | {
        readonly type: "json_schema";
        readonly json_schema: {
            readonly name: string;
            readonly description?: string | null | undefined;
            readonly strict?: boolean | null | undefined;
            readonly schema?: {
                readonly [x: string]: unknown;
            } | null | undefined;
        };
    } | null | undefined;
    readonly reasoning_effort?: "low" | "medium" | "high" | null | undefined;
    readonly logprobs?: boolean | null | undefined;
    readonly function_call?: "none" | "auto" | {
        readonly name: string;
    } | null | undefined;
    readonly audio?: {
        readonly voice: string;
        readonly format: "mp3" | "opus" | "flac" | "wav" | "pcm16";
    } | null | undefined;
    readonly top_logprobs?: number | null | undefined;
    readonly stop?: string | readonly string[] | null | undefined;
    readonly service_tier?: "default" | "auto" | null | undefined;
    readonly user?: string | null | undefined;
    readonly modalities?: readonly ("text" | "audio")[] | null | undefined;
    readonly max_completion_tokens?: number | null | undefined;
    readonly frequency_penalty?: number | null | undefined;
    readonly presence_penalty?: number | null | undefined;
    readonly web_search_options?: {
        readonly user_location?: {
            readonly type: "approximate";
            readonly approximate: {
                readonly country?: string | null | undefined;
                readonly region?: string | null | undefined;
                readonly city?: string | null | undefined;
                readonly timezone?: string | null | undefined;
            };
        } | null | undefined;
        readonly search_context_size?: "low" | "medium" | "high" | null | undefined;
    } | null | undefined;
    readonly store?: boolean | null | undefined;
    readonly logit_bias?: {
        readonly [x: string]: unknown;
    } | null | undefined;
    readonly max_tokens?: number | null | undefined;
    readonly n?: number | null | undefined;
    readonly prediction?: {
        readonly content: string | readonly [{
            readonly type: "text";
            readonly text: string;
        }, ...{
            readonly type: "text";
            readonly text: string;
        }[]];
        readonly type: "content";
    } | null | undefined;
    readonly seed?: number | null | undefined;
    readonly parallel_tool_calls?: boolean | null | undefined;
}>;
/**
 * @since 1.0.0
 * @category Context
 */
export declare class Config extends Config_base {
    /**
     * @since 1.0.0
     */
    static readonly getOrUndefined: Effect.Effect<Config.Service | undefined>;
}
/**
 * @since 1.0.0
 */
export declare namespace Config {
    /**
     * @since 1.0.0
     * @category Models
     */
    type Service = Simplify<Partial<Omit<typeof Generated.CreateChatCompletionRequest.Encoded, "messages" | "tools" | "tool_choice" | "stream" | "stream_options" | "functions">>>;
}
declare const ProviderMetadata_base: Context.TagClass<ProviderMetadata, "@effect/ai-openai/OpenAiLanguageModel/ProviderMetadata", ProviderMetadata.Service>;
/**
 * @since 1.0.0
 * @category Context
 */
export declare class ProviderMetadata extends ProviderMetadata_base {
}
/**
 * @since 1.0.0
 */
export declare namespace ProviderMetadata {
    /**
     * @since 1.0.0
     * @category Provider Metadata
     */
    interface Service {
        /**
         * Specifies the latency tier that was used for processing the request.
         */
        readonly serviceTier?: string;
        /**
         * This fingerprint represents the backend configuration that the model
         * executes with.
         *
         * Can be used in conjunction with the seed request parameter to understand
         * when backend changes have been made that might impact determinism.
         */
        readonly systemFingerprint: string;
        /**
         * When using predicted outputs, the number of tokens in the prediction
         * that appeared in the completion.
         */
        readonly acceptedPredictionTokens: number;
        /**
         * When using predicted outputs, the number of tokens in the prediction
         * that did not appear in the completion. However, like reasoning tokens,
         * these tokens are still counted in the total completion tokens for
         * purposes of billing, output, and context window limits.
         */
        readonly rejectedPredictionTokens: number;
        /**
         * Audio tokens present in the prompt.
         */
        readonly inputAudioTokens: number;
        /**
         * Audio tokens generated by the model.
         */
        readonly outputAudioTokens: number;
    }
}
/**
 * @since 1.0.0
 * @category AiModel
 */
export declare const model: (model: (string & {}) | Model, config?: Omit<Config.Service, "model">) => AiModel.AiModel<AiLanguageModel.AiLanguageModel, OpenAiClient>;
/**
 * @since 1.0.0
 * @category AiModel
 */
export declare const modelWithTokenizer: (model: (string & {}) | Model, config?: Omit<Config.Service, "model">) => AiModel.AiModel<AiLanguageModel.AiLanguageModel | Tokenizer.Tokenizer, OpenAiClient>;
/**
 * @since 1.0.0
 * @category Constructors
 */
export declare const make: (options: {
    readonly model: (string & {}) | Model;
    readonly config?: Omit<Config.Service, "model">;
}) => Effect.Effect<AiLanguageModel.AiLanguageModel.Service<never>, never, OpenAiClient>;
/**
 * @since 1.0.0
 * @category Layers
 */
export declare const layer: (options: {
    readonly model: (string & {}) | Model;
    readonly config?: Omit<Config.Service, "model">;
}) => Layer.Layer<AiLanguageModel.AiLanguageModel, never, OpenAiClient>;
/**
 * @since 1.0.0
 * @category Layers
 */
export declare const layerWithTokenizer: (options: {
    readonly model: (string & {}) | Model;
    readonly config?: Omit<Config.Service, "model">;
}) => Layer.Layer<AiLanguageModel.AiLanguageModel | Tokenizer.Tokenizer, never, OpenAiClient>;
/**
 * @since 1.0.0
 * @category Configuration
 */
export declare const withConfigOverride: {
    (overrides: Config.Service): <A, E, R>(self: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>;
    <A, E, R>(self: Effect.Effect<A, E, R>, overrides: Config.Service): Effect.Effect<A, E, R>;
};
export {};
//# sourceMappingURL=OpenAiLanguageModel.d.ts.map