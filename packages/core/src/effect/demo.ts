import { AnthropicClient, AnthropicCompletions } from "@effect/ai-anthropic"
import { Completions } from "@effect/ai"
import { Config, Effect } from "effect"

// Using `Completions` will add it to your program's requirements
//
//          ┌─── Effect<AiResponse, AiError, Completions>
//          ▼
const generateDadJoke = Effect.gen(function* () {
  // Extract the `Completions` service from the Effect environment
  const completions = yield* Completions.Completions
  // Use the `Completions` service to generate some text
  const response = yield* completions.create("Generate a dad joke")
  // Log the generated text to the console
  console.log(response.text)
  // Return the response
  return response
})

// Create an `AiModel` which provides a concrete implementation of
// `Completions` and requires an `AnthropicClient`
//
//      ┌─── AiModel<Completions, AnthropicClient>
//      ▼
const Sonnet35 = AnthropicCompletions.model("claude-3-5-sonnet-20241022")

const Anthropic = AnthropicClient.layerConfig({
  apiKey: Config.redacted("ANTHROPIC_API_KEY")
})

// Building an `AiModel` adds its requirements to the program
//
//     ┌─── Effect<void, AiError, AnthropicClient>
//     ▼
const main = Effect.gen(function* () {
  // Build the `AiModel` into a `Provider`
  const sonnet35 = yield* Sonnet35
  // Provide the implementation of `Completions` to `generateDadJoke`
  const response = yield* sonnet35.provide(generateDadJoke)
})
