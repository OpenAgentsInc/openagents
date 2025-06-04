import { Context, Effect, Layer } from "effect"

/**
 * @since 1.0.0
 */
export interface AiService {
  readonly hello: (name: string) => Effect.Effect<string>
  readonly complete: (prompt: string) => Effect.Effect<{
    content: string
    model: string
    usage: {
      promptTokens: number
      completionTokens: number
      totalTokens: number
    }
  }>
}

/**
 * @since 1.0.0
 */
export const AiService = Context.GenericTag<AiService>("ai/AiService")

/**
 * @since 1.0.0
 */
export const AiServiceLive = Layer.succeed(
  AiService,
  {
    hello: (name: string) => Effect.succeed(`Hello ${name} from AI Service!`),

    complete: (prompt: string) =>
      Effect.succeed({
        content: `Response to: ${prompt}`,
        model: "placeholder",
        usage: {
          promptTokens: 0,
          completionTokens: 0,
          totalTokens: 0
        }
      })
  }
)

/**
 * @since 1.0.0
 */
export const hello = (name: string) => Effect.andThen(AiService, (service) => service.hello(name))
