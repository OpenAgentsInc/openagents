import { AiToolkit, Completions } from "@effect/ai"
import { AnthropicClient, AnthropicCompletions } from "@effect/ai-anthropic"
import { HttpClient, HttpClientResponse } from "@effect/platform"
import { NodeHttpClient } from "@effect/platform-node"
import { Array, Chunk, Config, Console, Effect, Layer, Option, Schema, Stream } from "effect"
import { startServer } from "./Server.js"

class DadJoke extends Schema.Class<DadJoke>("DadJoke")({
  id: Schema.String,
  joke: Schema.String
}) { }

class SearchResponse extends Schema.Class<SearchResponse>("SearchResponse")({
  results: Schema.Array(DadJoke)
}) { }

class ICanHazDadJoke extends Effect.Service<ICanHazDadJoke>()("ICanHazDadJoke", {
  dependencies: [NodeHttpClient.layerUndici],
  effect: Effect.gen(function* () {
    const httpClient = yield* HttpClient.HttpClient
    const httpClientOk = httpClient.pipe(
      HttpClient.filterStatusOk,
      HttpClient.mapRequest((request) => ({ ...request, url: `https://icanhazdadjoke.com${request.url}` }))
    )

    const search = Effect.fn("ICanHazDadJoke.search")(
      function* (params: typeof GetDadJoke.Type) {
        return yield* httpClientOk.get("/search", {
          acceptJson: true,
          urlParams: { ...params }
        }).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(SearchResponse)),
          Effect.flatMap(({ results }) => Array.head(results)),
          Effect.map((joke) => joke.joke),
          Effect.scoped,
          Effect.orDie
        )
      }
    )

    return {
      search
    } as const
  })
}) { }

class GetDadJoke extends Schema.TaggedRequest<GetDadJoke>()("GetDadJoke", {
  payload: {
    searchTerm: Schema.String.annotations({
      description: "The search term to use to find dad jokes"
    })
  },
  success: Schema.String,
  failure: Schema.Never
}, {
  description: "Get a hilarious dad joke from the icanhazdadjoke API"
}) { }

const DadJokeTools = AiToolkit.empty.add(GetDadJoke)

const DadJokeToolsLayer = DadJokeTools.implement((handlers) =>
  Effect.gen(function* () {
    const icanhazdadjoke = yield* ICanHazDadJoke
    return handlers
      .handle("GetDadJoke", (params) => {
        return Effect.gen(function* () {
          yield* Console.log("🛠️ Tool called: GetDadJoke")
          yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2))
          const result = yield* icanhazdadjoke.search(params)
          yield* Console.log("✅ Tool result:", result)
          return result
        })
      })
  })
).pipe(Layer.provide(ICanHazDadJoke.Default))

const streamingGenerateDadJoke = Effect.gen(function* () {
  yield* Console.log("🤖 Starting streaming dad joke generation...")
  const completions = yield* Completions.Completions
  const tools = yield* DadJokeTools
  yield* Console.log("🔧 Available tools: GetDadJoke")

  const streamResponse = completions.toolkitStream({
    input: "Generate a dad joke",
    tools,
    concurrency: 1
  })

  // Process each chunk as it arrives
  const processStream = streamResponse.pipe(
    Stream.tap((chunk) =>
      Effect.gen(function* () {
        // Extract the text content from the chunk's parts
        if (chunk.response && chunk.response.parts) {
          const parts = Chunk.toReadonlyArray(chunk.response.parts)
          for (const part of parts) {
            if (part._tag === "Text" && part.content) {
              yield* Console.log(`🔄 Delta: "${part.content}"`)
            } else if (part._tag === "ToolCall") {
              yield* Console.log(`🧰 Tool call: ${part.name} with params ${JSON.stringify(part.params)}`)
            }
          }
        }
        // Also log if we have a resolved value (happens with tool results)
        if (Option.isSome(chunk.value)) {
          yield* Console.log(`💡 Result: "${chunk.value.value}"`)
        }
      })
    ),
    Stream.runCollect
  )

  const chunks = yield* processStream
  const lastChunk = Chunk.last(chunks)

  if (Option.isSome(lastChunk)) {
    yield* Console.log("\n🎯 Final response:")
    yield* Console.log(Option.getOrElse(lastChunk.value.value, () => ""))
    return lastChunk.value
  }

  throw new Error("No response received")
})

const Claude3 = AnthropicCompletions.model("claude-3-5-haiku-latest")

const main = Effect.gen(function* () {
  const claude3 = yield* Claude3
  return yield* claude3.provide(streamingGenerateDadJoke)
})

const Anthropic = AnthropicClient.layerConfig({
  apiKey: Config.redacted("ANTHROPIC_API_KEY")
})

const AnthropicWithHttp = Layer.provide(Anthropic, NodeHttpClient.layerUndici)


// Start the server when running the program directly
if (import.meta.url === `file://${process.argv[1]}`) {
  startServer()
} else {
  // If imported as a module, run the main function
  main.pipe(
    Effect.provide([AnthropicWithHttp, DadJokeToolsLayer]),
    Effect.runPromise
  )
}
