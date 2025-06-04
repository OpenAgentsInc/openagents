import { Context, Effect, Layer } from "effect";
/**
 * AI completion response
 * @since 1.0.0
 */
export interface AiCompletionResponse {
    readonly content: string;
    readonly model: string;
    readonly usage: {
        readonly promptTokens: number;
        readonly completionTokens: number;
        readonly totalTokens: number;
    };
    readonly sessionId?: string;
}
/**
 * AI conversation options
 * @since 1.0.0
 */
export interface AiConversationOptions {
    readonly sessionId?: string;
    readonly systemPrompt?: string;
    readonly model?: string;
}
/**
 * @since 1.0.0
 */
export interface AiService {
    readonly hello: (name: string) => Effect.Effect<string>;
    readonly complete: (prompt: string) => Effect.Effect<AiCompletionResponse>;
    readonly conversation?: (prompt: string, options?: AiConversationOptions) => Effect.Effect<AiCompletionResponse>;
}
/**
 * @since 1.0.0
 */
export declare const AiService: Context.Tag<AiService, AiService>;
/**
 * @since 1.0.0
 */
export declare const AiServiceLive: Layer.Layer<AiService, never, never>;
/**
 * @since 1.0.0
 */
export declare const hello: (name: string) => Effect.Effect<string, never, AiService>;
//# sourceMappingURL=AiService.d.ts.map