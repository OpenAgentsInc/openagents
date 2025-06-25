/**
 * @since 1.0.0
 */
import type { HttpClient } from "@effect/platform/HttpClient";
import * as Context from "effect/Context";
import * as Effect from "effect/Effect";
declare const OpenAiConfig_base: Context.TagClass<OpenAiConfig, "@effect/ai-openai/OpenAiConfig", OpenAiConfig.Service>;
/**
 * @since 1.0.0
 * @category Context
 */
export declare class OpenAiConfig extends OpenAiConfig_base {
    /**
     * @since 1.0.0
     */
    static readonly getOrUndefined: Effect.Effect<typeof OpenAiConfig.Service | undefined>;
}
/**
 * @since 1.0.0
 */
export declare namespace OpenAiConfig {
    /**
     * @since 1.0.
     * @category Models
     */
    interface Service {
        readonly transformClient?: (client: HttpClient) => HttpClient;
    }
}
/**
 * @since 1.0.0
 * @category Configuration
 */
export declare const withClientTransform: {
    (transform: (client: HttpClient) => HttpClient): <A, E, R>(self: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>;
    <A, E, R>(self: Effect.Effect<A, E, R>, transform: (client: HttpClient) => HttpClient): Effect.Effect<A, E, R>;
};
export {};
//# sourceMappingURL=OpenAiConfig.d.ts.map