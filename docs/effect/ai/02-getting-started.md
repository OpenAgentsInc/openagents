# Getting Started | Effect Documentation

In this getting started guide, we will demonstrate how to generate a simple text completion using an LLM provider (OpenAi) using the Effect AI integration packages.

We’ll walk through:

- Writing provider-agnostic logic to interact with an LLM
- Declaring the specific LLM model to use for the interaction
- Using a provider integration to make the program executable

## Installation

First, we will need to install the base `@effect/ai` package to gain access to the core AI abstractions. In addition, we will need to install at least one provider integration package (in this case `@effect/ai-openai`):

- [npm](#tab-panel-30)
- [pnpm](#tab-panel-31)
- [Yarn](#tab-panel-32)
- [Bun](#tab-panel-33)

```

# Install the base package for the core abstractions (always required)
npm install @effect/ai
# Install one (or more) provider integrations
npm install @effect/ai-openai
# Also add the core Effect package (if not already installed)
npm install effect
```

## Define an Interaction with a Language Model

First let’s define a simple interaction with a large language model (LLM):

**Example** (Using the `AiLanguageModel` Service to Generate a Dad Joke)

```

import { AiLanguageModel } from "@effect/ai"
import { Effect } from "effect"
// Using `AiLanguageModel` will add it to your program's requirements
//
//          ┌─── Effect<AiResponse, AiError, AiLanguageModel>
//          ▼
const generateDadJoke = Effect.gen(function*() {
  // Use the `AiLanguageModel` to generate some text
  const response = yield* AiLanguageModel.generateText({
    prompt: "Generate a dad joke"
  })
  // Log the generated text to the console
  console.log(response.text)
  // Return the response
  return response
})
```

## Select a Provider

Next, we need to select which model provider we want to use:

**Example** (Using a Model Provider to Satisfy the `AiLanguageModel` Requirement)

```

import { AiLanguageModel } from "@effect/ai"
import { OpenAiLanguageModel } from "@effect/ai-openai"
import { Effect } from "effect"
7 collapsed lines
const generateDadJoke = Effect.gen(function*() {
  const response = yield* AiLanguageModel.generateText({
    prompt: "Generate a dad joke"
  })
  console.log(response.text)
  return response
})
// Create an `AiModel` which provides a concrete implementation of
// `AiLanguageModel` and requires an `OpenAiClient`
//
//      ┌─── AiModel<AiLanguageModel, OpenAiClient>
//      ▼
const Gpt4o = OpenAiLanguageModel.model("gpt-4o")
// Building an `AiModel` adds its requirements to the program
//
//     ┌─── Effect<void, AiError, OpenAiClient>
//     ▼
const main = Effect.gen(function*() {
  // Build the `AiModel` into a `Provider`
  const gpt4o = yield* Gpt4o
  // Use the implementation of `AiLanguageModel` for the
  // `generateDadJoke` program
  const response = yield* gpt4o.use(generateDadJoke)
})
```

Before moving on, it is important that we understand the purpose of the `AiModel` data type.

## Understanding `AiModel`

The `AiModel` data type represents a **provider-specific implementation** of one or more services, such as `AiLanguageModel` or `AiEmbeddingsModel`. It is the primary way that you can plug a real large language model into your program.

```

export interface AiModel<Provides, Requires> {}
```

An `AiModel` has two generic type parameters:

- **Provides** - the services this model will provide when built
- **Requires** - the services this model will require to be built

This allows Effect to track which services should be added to the requirements of the program that builds the `AiModel`, as well as which services the built `AiModel` can provide.

### Creating an `AiModel`

To create an `AiModel`, you can use the model-specific factory from one of Effect’s provider integration packages.

**Example** (Defining an `AiModel` to Interact with OpenAi)

```

import { OpenAiLanguageModel } from "@effect/ai-openai"
//      ┌─── AiModel<AiLanguageModel, OpenAiClient>
//      ▼
const Gpt4o = OpenAiLanguageModel.model("gpt-4o")
```

This creates an `AiModel` that:

- **Provides** an OpenAi-specific implementation of the `AiLanguageModel` service using `"gpt-4o"`
- **Requires** an `OpenAiClient` to be built

### Building an `AiModel`

When you build an `AiModel<Provides, Requires>`, you get back a `Provider<Provides>`:

```

Effect.gen(function*() {
  //      ┌─── Provider<AiLanguageModel>
  //      ▼
  const gpt4o = yield* Gpt4o
})
```

A `Provider` has a single `.use(...)` method

which allows you to “use” the services provided by the `AiModel` to run a particular program, removing those services from that program’s requirements.

### Benefits of `AiModel`

There are several benefits to this approach:

**Reusability**

You can `.use(...)` the same built model as many times as you like.

For example, we can re-use the same built model to provide `AiLanguageModel` to multiple calls to `generateDadJoke`:

```

import { OpenAiLanguageModel } from "@effect/ai-openai"
import { AiLanguageModel } from "@effect/ai"
import { Effect } from "effect"
7 collapsed lines
const generateDadJoke = Effect.gen(function*() {
  const response = yield* AiLanguageModel.generateText({
    prompt: "Generate a dad joke"
  })
  console.log(response.text)
  return response
})
const Gpt4o = OpenAiLanguageModel.model("gpt-4o")
const main = Effect.gen(function*() {
  const gpt = yield* Gpt4o
  const res1 = yield* gpt.use(generateDadJoke)
  const res2 = yield* gpt.use(generateDadJoke)
  const res3 = yield* gpt.use(generateDadJoke)
})
```

**Flexibility**

If we know that one model or provider performs better at a given task than another, we can freely mix and match models and providers together.

For example, if we know Anthropic’s Claude generates some really great dad jokes, we can mix it into our existing program with just a few lines of code:

**Example** (Mixing Multiple Providers and Models)

```

import { AnthropicLanguageModel } from "@effect/ai-anthropic"
import { OpenAiLanguageModel } from "@effect/ai-openai"
import { AiLanguageModel } from "@effect/ai"
import { Effect } from "effect"
7 collapsed lines
const generateDadJoke = Effect.gen(function*() {
  const response = yield* AiLanguageModel.generateText({
    prompt: "Generate a dad joke"
  })
  console.log(response.text)
  return response
})
const Gpt4o = OpenAiLanguageModel.model("gpt-4o")
const Claude37 = AnthropicLanguageModel.model("claude-3-7-sonnet-latest")
//      ┌─── Effect<void, AiError, AnthropicClient | OpenAiClient>
//      ▼
const main = Effect.gen(function*() {
  const gpt = yield* Gpt4o
  const claude = yield* Claude37
  const res1 = yield* gpt.use(generateDadJoke)
  const res2 = yield* gpt.use(generateDadJoke)
  const res3 = yield* claude.use(generateDadJoke)
})
```

Because Effect performs type-level dependency tracking, we can see that an `AnthropicClient` must now also be provided to make our program runnable.

**Abstractability**

Because there is separation between _building_ an `AiModel` and _providing_ its services, we can very nicely support the service constructor pattern.

For example, in the code below the `main` program is only dependent upon the `DadJokes` service. All AI requirements are abstracted away into `Layer` composition.

**Example** (Abstracting LLM Interactions into a Service)

```

import { AnthropicLanguageModel } from "@effect/ai-anthropic"
import { OpenAiLanguageModel } from "@effect/ai-openai"
import { AiLanguageModel } from "@effect/ai"
import { Effect } from "effect"
const Gpt4o = OpenAiLanguageModel.model("gpt-4o")
const Claude37 = AnthropicLanguageModel.model("claude-3-7-sonnet-latest")
class DadJokes extends Effect.Service<DadJokes>()("app/DadJokes", {
  effect: Effect.gen(function*() {
    const gpt = yield* Gpt4o
    const claude = yield* Claude37
7 collapsed lines
    const generateDadJoke = Effect.gen(function*() {
      const response = yield* AiLanguageModel.generateText({
        prompt: "Generate a dad joke"
      })
      console.log(response.text)
      return response
    })
    return {
      generateDadJoke: gpt.use(generateDadJoke),
      generateReallyGroanInducingDadJoke: claude.use(generateDadJoke)
    }
  })
}) {}
// Programs which utilize the `DadJokes` service have no knowledge of
// any AI requirements
//
//     ┌─── Effect<void, AiError, DadJokes>
//     ▼
const main = Effect.gen(function*() {
  const dadJokes = yield* DadJokes
  const res1 = yield* dadJokes.generateDadJoke
  const res2 = yield* dadJokes.generateReallyGroanInducingDadJoke
})
// The AI requirements are abstracted away into `Layer` composition
//
//         ┌─── Layer<DadJokes, never, AnthropicClient | OpenAiClient>
//         ▼
DadJokes.Default
```

## Create a Provider Client

To make our code executable, we must finish satisfying our program’s requirements.

Let’s take another look at our program from earlier:

```

import { OpenAiLanguageModel } from "@effect/ai-openai"
import { AiLanguageModel } from "@effect/ai"
import { Effect } from "effect"
7 collapsed lines
const generateDadJoke = Effect.gen(function*() {
  const response = yield* AiLanguageModel.generateText({
    prompt: "Generate a dad joke"
  })
  console.log(response.text)
  return response
})
const Gpt4o = OpenAiLanguageModel.model("gpt-4o")
//     ┌─── Effect<void, AiError, OpenAiClient>
//     ▼
const main = Effect.gen(function*() {
  const gpt4o = yield* Gpt4o
  const response = yield* gpt4o.use(generateDadJoke)
})
```

We can see that our `main` program still requires us to provide an `OpenAiClient`.

Each of our provider integration packages exports a client module that can be used to construct a client for that provider.

**Example** (Creating a Client Layer for a Model Provider)

```

import { OpenAiClient, OpenAiLanguageModel } from "@effect/ai-openai"
import { AiLanguageModel } from "@effect/ai"
import { Config, Effect } from "effect"
14 collapsed lines
const generateDadJoke = Effect.gen(function*() {
  const response = yield* AiLanguageModel.generateText({
    prompt: "Generate a dad joke"
  })
  console.log(response.text)
  return response
})
const Gpt4o = OpenAiLanguageModel.model("gpt-4o")
const main = Effect.gen(function*() {
  const gpt4o = yield* Gpt4o
  const response = yield* gpt4o.use(generateDadJoke)
})
// Create a `Layer` which produces an `OpenAiClient` and requires
// an `HttpClient`
//
//      ┌─── Layer<OpenAiClient, never, HttpClient>
//      ▼
const OpenAi = OpenAiClient.layerConfig({
  apiKey: Config.redacted("OPENAI_API_KEY")
})
```

In the code above, we use the `layerConfig` constructor from the `OpenAiClient` module to create a `Layer` which will produce an `OpenAiClient`. The `layerConfig` constructor allows us to read in configuration variables using Effect’s [configuration system](https://effect.website/docs/configuration/).

The provider clients also have a dependency on an `HttpClient` implementation to avoid any platform dependencies. This way, you can provide whichever `HttpClient` implementation is most appropriate for the platform your code is running upon.

For example, if we know we are going to run this code in NodeJS, we can utilize the `NodeHttpClient` module from `@effect/platform-node` to provide an `HttpClient` implementation:

```

import { OpenAiClient, OpenAiLanguageModel } from "@effect/ai-openai"
import { AiLanguageModel } from "@effect/ai"
import { NodeHttpClient } from "@effect/platform-node"
import { Config, Effect, Layer } from "effect"
14 collapsed lines
const generateDadJoke = Effect.gen(function*() {
  const response = yield* AiLanguageModel.generateText({
    prompt: "Generate a dad joke"
  })
  console.log(response.text)
  return response
})
const Gpt4o = OpenAiLanguageModel.model("gpt-4o")
const main = Effect.gen(function*() {
  const gpt4o = yield* Gpt4o
  const response = yield* gpt4o.use(generateDadJoke)
})
// Create a `Layer` which produces an `OpenAiClient` and requires
// an `HttpClient`
//
//      ┌─── Layer<OpenAiClient, never, HttpClient>
//      ▼
const OpenAi = OpenAiClient.layerConfig({
  apiKey: Config.redacted("OPENAI_API_KEY")
})
// Provide a platform-specific implementation of `HttpClient` to our
// OpenAi layer
//
//        ┌─── Layer<OpenAiClient, never, never>
//        ▼
const OpenAiWithHttp = Layer.provide(OpenAi, NodeHttpClient.layerUndici)
```

## Running the Program

Now that we have a `Layer` which provides us with an `OpenAiClient`, we’re ready to make our `main` program runnable.

Our final program looks like the following:

```

import { OpenAiClient, OpenAiLanguageModel } from "@effect/ai-openai"
import { AiLanguageModel } from "@effect/ai"
import { NodeHttpClient } from "@effect/platform-node"
import { Config, Effect, Layer } from "effect"
const generateDadJoke = Effect.gen(function*() {
  const response = yield* AiLanguageModel.generateText({
    prompt: "Generate a dad joke"
  })
  console.log(response.text)
  return response
})
const Gpt4o = OpenAiLanguageModel.model("gpt-4o")
const main = Effect.gen(function*() {
  const gpt4o = yield* Gpt4o
  const response = yield* gpt4o.use(generateDadJoke)
})
const OpenAi = OpenAiClient.layerConfig({
  apiKey: Config.redacted("OPENAI_API_KEY")
})
const OpenAiWithHttp = Layer.provide(OpenAi, NodeHttpClient.layerUndici)
main.pipe(
  Effect.provide(OpenAiWithHttp),
  Effect.runPromise
)
```
