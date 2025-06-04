import { Context, Effect, Layer } from "effect";
/**
 * @since 1.0.0
 */
export const AiService = Context.GenericTag("ai/AiService");
/**
 * @since 1.0.0
 */
export const AiServiceLive = Layer.succeed(AiService, {
    hello: (name) => Effect.succeed(`Hello ${name} from AI Service!`),
    complete: (prompt) => Effect.succeed({
        content: `Response to: ${prompt}`,
        model: "placeholder",
        usage: {
            promptTokens: 0,
            completionTokens: 0,
            totalTokens: 0
        }
    })
});
/**
 * @since 1.0.0
 */
export const hello = (name) => Effect.andThen(AiService, (service) => service.hello(name));
//# sourceMappingURL=AiService.js.map