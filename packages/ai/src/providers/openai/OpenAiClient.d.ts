import * as HttpClient from "@effect/platform/HttpClient";
import type * as HttpClientError from "@effect/platform/HttpClientError";
import * as HttpClientRequest from "@effect/platform/HttpClientRequest";
import * as Config from "effect/Config";
import type { ConfigError } from "effect/ConfigError";
import * as Context from "effect/Context";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Redacted from "effect/Redacted";
import * as Stream from "effect/Stream";
import * as AiResponse from "../../core/AiResponse.js";
import * as Generated from "./Generated.js";
declare const OpenAiClient_base: Context.TagClass<OpenAiClient, "@effect/ai-openai/OpenAiClient", OpenAiClient.Service>;
/**
 * @since 1.0.0
 * @category Context
 */
export declare class OpenAiClient extends OpenAiClient_base {
}
/**
 * @since 1.0.0
 */
export declare namespace OpenAiClient {
    /**
     * @since 1.0.0
     * @category Models
     */
    interface Service {
        readonly client: Generated.Client;
        readonly streamRequest: <A>(request: HttpClientRequest.HttpClientRequest) => Stream.Stream<A, HttpClientError.HttpClientError>;
        readonly stream: (request: StreamCompletionRequest) => Stream.Stream<AiResponse.AiResponse, HttpClientError.HttpClientError>;
    }
}
/**
 * @since 1.0.0
 * @category Models
 */
export type StreamCompletionRequest = Omit<typeof Generated.CreateChatCompletionRequest.Encoded, "stream">;
/**
 * @since 1.0.0
 * @category Constructors
 */
export declare const make: (options: {
    /**
     * The API key to use to communicate with the OpenAi API.
     */
    readonly apiKey?: Redacted.Redacted | undefined;
    /**
     * The URL to use to communicate with the OpenAi API.
     */
    readonly apiUrl?: string | undefined;
    /**
     * The OpenAi organization identifier to use when communicating with the
     * OpenAi API.
     */
    readonly organizationId?: Redacted.Redacted | undefined;
    /**
     * The OpenAi project identifier to use when communicating with the OpenAi
     * API.
     */
    readonly projectId?: Redacted.Redacted | undefined;
    /**
     * A method which can be used to transform the underlying `HttpClient` which
     * will be used to communicate with the OpenAi API.
     */
    readonly transformClient?: ((client: HttpClient.HttpClient) => HttpClient.HttpClient) | undefined;
}) => Effect.Effect<OpenAiClient.Service, never, HttpClient.HttpClient>;
/**
 * @since 1.0.0
 * @category Layers
 */
export declare const layer: (options: {
    readonly apiKey?: Redacted.Redacted | undefined;
    readonly apiUrl?: string | undefined;
    readonly organizationId?: Redacted.Redacted | undefined;
    readonly projectId?: Redacted.Redacted | undefined;
    readonly transformClient?: (client: HttpClient.HttpClient) => HttpClient.HttpClient;
}) => Layer.Layer<OpenAiClient, never, HttpClient.HttpClient>;
/**
 * @since 1.0.0
 * @category Layers
 */
export declare const layerConfig: (options: Config.Config.Wrap<{
    readonly apiKey?: Redacted.Redacted | undefined;
    readonly apiUrl?: string | undefined;
    readonly organizationId?: Redacted.Redacted | undefined;
    readonly projectId?: Redacted.Redacted | undefined;
    readonly transformClient?: (client: HttpClient.HttpClient) => HttpClient.HttpClient;
}>) => Layer.Layer<OpenAiClient, ConfigError, HttpClient.HttpClient>;
export {};
//# sourceMappingURL=OpenAiClient.d.ts.map