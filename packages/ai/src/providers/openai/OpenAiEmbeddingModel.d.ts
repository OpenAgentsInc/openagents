/**
 * @since 1.0.0
 */
import * as Context from "effect/Context";
import type * as Duration from "effect/Duration";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import type { Simplify } from "effect/Types";
import * as AiEmbeddingModel from "../../core/AiEmbeddingModel.js";
import * as AiModel from "../../core/AiModel.js";
import type * as Generated from "./Generated.js";
import { OpenAiClient } from "./OpenAiClient.js";
/**
 * @since 1.0.0
 * @category Models
 */
export type Model = typeof Generated.CreateEmbeddingRequestModelEnum.Encoded;
declare const Config_base: Context.TagClass<Config, "@effect/ai-openai/OpenAiEmbeddingModel/Config", {
    readonly model?: string;
    readonly dimensions?: number | null | undefined;
    readonly user?: string | null | undefined;
    readonly encoding_format?: "float" | "base64" | null | undefined;
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
     * @since 1.0.
     * @category Configuration
     */
    type Service = Simplify<Partial<Omit<typeof Generated.CreateEmbeddingRequest.Encoded, "input">>>;
    /**
     * @since 1.0.
     * @category Configuration
     */
    interface Batched extends Omit<Config.Service, "model"> {
        readonly maxBatchSize?: number;
        readonly cache?: {
            readonly capacity: number;
            readonly timeToLive: Duration.DurationInput;
        };
    }
    /**
     * @since 1.0.
     * @category Configuration
     */
    interface DataLoader extends Omit<Config.Service, "model"> {
        readonly window: Duration.DurationInput;
        readonly maxBatchSize?: number;
    }
}
/**
 * @since 1.0.0
 * @category Models
 */
export declare const model: (model: (string & {}) | Model, config: Simplify<(({
    readonly mode: "batched";
} & Config.Batched) | ({
    readonly mode: "data-loader";
} & Config.DataLoader))>) => AiModel.AiModel<AiEmbeddingModel.AiEmbeddingModel, OpenAiClient>;
/**
 * @since 1.0.0
 * @category Constructors
 */
export declare const makeDataLoader: (options: {
    readonly model: (string & {}) | Model;
    readonly config: Config.DataLoader;
}) => Effect.Effect<AiEmbeddingModel.AiEmbeddingModel.Service, never, OpenAiClient | import("effect/Scope").Scope>;
/**
 * @since 1.0.0
 * @category Layers
 */
export declare const layerBatched: (options: {
    readonly model: (string & {}) | Model;
    readonly config?: Config.Batched;
}) => Layer.Layer<AiEmbeddingModel.AiEmbeddingModel, never, OpenAiClient>;
/**
 * @since 1.0.0
 * @category Layers
 */
export declare const layerDataLoader: (options: {
    readonly model: (string & {}) | Model;
    readonly config: Config.DataLoader;
}) => Layer.Layer<AiEmbeddingModel.AiEmbeddingModel, never, OpenAiClient>;
/**
 * @since 1.0.0
 * @category Configuration
 */
export declare const withConfigOverride: {
    (config: Config.Service): <A, E, R>(self: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>;
    <A, E, R>(self: Effect.Effect<A, E, R>, config: Config.Service): Effect.Effect<A, E, R>;
};
export {};
//# sourceMappingURL=OpenAiEmbeddingModel.d.ts.map