# Tool Use | Effect Documentation

Language models are great at generating text, but often we need them to take **real-world actions**, such as querying an API, accessing a database, or calling a service. Most LLM providers support this through **tool use** (also known as _function calling_), where you expose specific operations in your application that the model can invoke.

Based on the input it receives, a model may choose to **invoke (or call)** one or more tools to augment its response. Your application then runs the corresponding logic for the tool using the parameters provided by the model. You then return the result to the model, allowing it to include the output in its final response.

The `AiToolkit` simplifies tool integration by offering a structured, type-safe approach to defining tools. It takes care of all the wiring between the model and your application - all you have to do is define the tool and implement its behavior.

Let’s walk through a complete example of how to define, implement, and use a tool that fetches a dad joke from the [icanhazdadjoke.com](https://icanhazdadjoke.com/) API.

### 1\. Define the Tool

We start by defining a tool that the language model will have access to using the `AiTool.make` constructor.

This constructor accepts several parameters that allow us to fully describe the tool to the language model:

- `description`: Provides an optional description of the tool
- `success`: The type of value the tool will return if it succeeds
- `failure`: The type of value the tool will return if it fails
- `parameters`: The parameters that the tool should be called with

**Example** (Defining a Tool)

```

import { AiTool } from "@effect/ai"
import { Schema } from "effect"
const GetDadJoke = AiTool.make("GetDadJoke", {
  description: "Get a hilarious dad joke from the ICanHazDadJoke API",
  success: Schema.String,
  failure: Schema.Never,
  parameters: {
    searchTerm: Schema.String.annotations({
      description: "The search term to use to find dad jokes"
    })
  }
})
```

Based on the above, a request to call the `GetDadJoke` tool:

- Takes a single `searchTerm` parameter
- Will return a string if it succeeds (i.e. the joke)
- Does not have any expected failure scenarios

### 2\. Create a Toolkit

Once we have a tool request defined, we can create an `AiToolkit`, which is a collection of tools that the model will have access to.

**Example** (Creating an `AiToolkit`)

```

import { AiTool, AiToolkit } from "@effect/ai"
import { Schema } from "effect"
class DadJokeTools extends AiToolkit.make(
  AiTool.make("GetDadJoke", {
    description: "Get a hilarious dad joke from the ICanHazDadJoke API",
    success: Schema.String,
    failure: Schema.Never,
    parameters: {
      searchTerm: Schema.String.annotations({
        description: "The search term to use to find dad jokes"
      })
    }
  })
) {}
```

### 3\. Implement the Logic

The `.toLayer(...)` method on an `AiToolkit` allows you to define the handlers for each tool in the toolkit. Because `.toLayer(...)` takes an `Effect`, we can access services from our application to implement the tool call handlers.

**Example** (Implementing an `AiToolkit`)

```

import { AiTool, AiToolkit } from "@effect/ai"
import {
  HttpClient,
  HttpClientRequest,
  HttpClientResponse
} from "@effect/platform"
import { NodeHttpClient } from "@effect/platform-node"
import { Array, Effect, Schema } from "effect"
38 collapsed lines
class DadJoke extends Schema.Class<DadJoke>("DadJoke")({
  id: Schema.String,
  joke: Schema.String
}) {}
class SearchResponse extends Schema.Class<SearchResponse>("SearchResponse")({
  results: Schema.Array(DadJoke)
}) {}
class ICanHazDadJoke extends Effect.Service<ICanHazDadJoke>()("ICanHazDadJoke", {
  dependencies: [NodeHttpClient.layerUndici],
  effect: Effect.gen(function*() {
    const httpClient = yield* HttpClient.HttpClient
    const httpClientOk = httpClient.pipe(
      HttpClient.filterStatusOk,
      HttpClient.mapRequest(HttpClientRequest.prependUrl("https://icanhazdadjoke.com"))
    )
    const search = Effect.fn("ICanHazDadJoke.search")(
      function*(searchTerm: string) {
        return yield* httpClientOk.get("/search", {
          acceptJson: true,
          urlParams: { searchTerm }
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
}) {}
class DadJokeTools extends AiToolkit.make(
  AiTool.make("GetDadJoke", {
    description: "Get a hilarious dad joke from the ICanHazDadJoke API",
    success: Schema.String,
    failure: Schema.Never,
    parameters: {
      searchTerm: Schema.String.annotations({
        description: "The search term to use to find dad jokes"
      })
    }
  })
) {}
const DadJokeToolHandlers = DadJokeTools.toLayer(
  Effect.gen(function*() {
    // Access the `ICanHazDadJoke` service
    const icanhazdadjoke = yield* ICanHazDadJoke
    return {
      // Implement the handler for the `GetDadJoke` tool call request
      GetDadJoke: ({ searchTerm }) => icanhazdadjoke.search(searchTerm)
    }
  })
)
```

In the code above:

- We access the `ICanHazDadJoke` service from our application
- Register a handler for the `GetDadJoke` tool using `.handle("GetDadJoke", ...)`
- Use the `.search` method on our `ICanHazDadJoke` service to search for a dad joke based on the tool call parameters

The result of calling `.toLayer` on an `AiToolkit` is a `Layer` that contains the handlers for all the tools in our toolkit.

Because of this, it is quite simple to test an `AiToolkit` by using `.toLayer` to create a separate `Layer` specifically for testing.

### 4\. Give the Tools to the Model

Once the tools are defined and implemented, you can pass them along to the model at request time. Behind the scenes, the model is given a structured description of each tool and can choose to call one or more of them when responding to input.

**Example** (Using an `AiToolkit` in `Completions.toolkit`)

```

import { AiLanguageModel, AiTool, AiToolkit } from "@effect/ai"
import { Effect, Schema } from "effect"
class DadJokeTools extends AiToolkit.make(
  AiTool.make("GetDadJoke", {
    description: "Get a hilarious dad joke from the ICanHazDadJoke API",
    success: Schema.String,
    failure: Schema.Never,
    parameters: {
      searchTerm: Schema.String.annotations({
        description: "The search term to use to find dad jokes"
      })
    }
  })
) {}
const generateDadJoke = AiLanguageModel.generateText({
  prompt: "Generate a dad joke about pirates",
  tools: DadJokeTools
})
```

### 5\. Bring It All Together

To make the program executable, we must provide the implementation of our tool call handlers:

**Example** (Providing the Tool Call Handlers to a Program)

```

import { AiLanguageModel, AiTool, AiToolkit } from "@effect/ai"
import { OpenAiClient, OpenAiLanguageModel } from "@effect/ai-openai"
import {
  HttpClient,
  HttpClientRequest,
  HttpClientResponse
} from "@effect/platform"
import { NodeHttpClient } from "@effect/platform-node"
import { Array, Config, Effect, Layer, Schema } from "effect"
51 collapsed lines
class DadJoke extends Schema.Class<DadJoke>("DadJoke")({
  id: Schema.String,
  joke: Schema.String
}) {}
class SearchResponse extends Schema.Class<SearchResponse>("SearchResponse")({
  results: Schema.Array(DadJoke)
}) {}
class ICanHazDadJoke extends Effect.Service<ICanHazDadJoke>()("ICanHazDadJoke", {
  dependencies: [NodeHttpClient.layerUndici],
  effect: Effect.gen(function*() {
    const httpClient = yield* HttpClient.HttpClient
    const httpClientOk = httpClient.pipe(
      HttpClient.filterStatusOk,
      HttpClient.mapRequest(HttpClientRequest.prependUrl("https://icanhazdadjoke.com"))
    )
    const search = Effect.fn("ICanHazDadJoke.search")(
      function*(searchTerm: string) {
        return yield* httpClientOk.get("/search", {
          acceptJson: true,
          urlParams: { searchTerm }
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
}) {}
class DadJokeTools extends AiToolkit.make(
  AiTool.make("GetDadJoke", {
    description: "Get a hilarious dad joke from the ICanHazDadJoke API",
    success: Schema.String,
    failure: Schema.Never,
    parameters: {
      searchTerm: Schema.String.annotations({
        description: "The search term to use to find dad jokes"
      })
    }
  })
) {}
const DadJokeToolHandlers = DadJokeTools.toLayer(
  Effect.gen(function*() {
    const icanhazdadjoke = yield* ICanHazDadJoke
    return {
      GetDadJoke: ({ searchTerm }) => icanhazdadjoke.search(searchTerm)
    }
  })
).pipe(Layer.provide(ICanHazDadJoke.Default))
10 collapsed lines
const generateDadJoke = AiLanguageModel.generateText({
  prompt: "Generate a dad joke about pirates",
  tools: DadJokeTools
})
const program = Effect.gen(function*() {
  const model = yield* OpenAiLanguageModel.model("gpt-4o")
  const response = yield* model.use(generateDadJoke)
  console.log(response.text)
})
const OpenAi = OpenAiClient.layerConfig({
  apiKey: Config.redacted("OPENAI_API_KEY")
}).pipe(Layer.provide(NodeHttpClient.layerUndici))
program.pipe(
  Effect.provide([OpenAi, DadJokeToolHandlers]),
  Effect.runPromise
)
```

## Benefits

**Type Safe**

Every tool is fully described using Effect’s `Schema`, including inputs, outputs, and descriptions.

**Effect Native**

Tool call behavior is defined using Effect, so they can leverage all the power of Effect. This is especially useful when you need to access other services to support the implementation of your tool call handlers.

**Injectable**

Because implementing the handlers for an `AiToolkit` results in a `Layer`, providing alternate implementation of tool call handlers in different environments is as simple as providing a different `Layer` to your program.

**Separation of Concerns**

The definition of a tool call request is cleanly separated from both the implementation of the tool behavior, as well as the business logic that calls the model.
