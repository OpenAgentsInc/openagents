# Execution Planning | Effect Documentation

Imagine that we’ve refactored our `generateDadJoke` program from our [Getting Started](https://effect.website/docs/ai/getting-started/) guide. Now, instead of handling all errors internally, the code can **fail with domain-specific issues** like network interruptions or provider outages:

```

import type { AiLanguageModel, AiResponse } from "@effect/ai"
import { OpenAiLanguageModel } from "@effect/ai-openai"
import { Data, Effect } from "effect"
class NetworkError extends Data.TaggedError("NetworkError") {}
class ProviderOutage extends Data.TaggedError("ProviderOutage") {}
declare const generateDadJoke: Effect.Effect<
  AiResponse.AiResponse,
  NetworkError | ProviderOutage,
  AiLanguageModel.AiLanguageModel
>
const main = Effect.gen(function*() {
  const gpt4o = yield* OpenAiLanguageModel.model("gpt-4o")
  const response = yield* gpt4o.use(generateDadJoke)
  console.log(response.text)
})
```

This is fine, but what if we instead want to:

- Retry the program a fixed number of times on `NetworkError`s
- Add some backoff delay between retries
- Fallback to a different model provider if OpenAi is down

How can we accomplish such logic?

## The AiPlan Type

The Effect AI integrations provide a robust method for creating **structured execution plans** for your LLM interactions through the `AiPlan` data type. Rather than making a single model call and hoping it succeeds, `AiPlan` lets you describe how to handle retries, fallbacks, and recoverable errors in a clear, declarative way.

This is especially useful when:

- You want to fall back to a secondary model if the primary one is unavailable
- You want to retry on transient errors (e.g. network failures)
- You want to control timing between retry attempts

Think of an `AiPlan` as the **blueprint** for for an LLM interaction, with logic for when to keep trying to interact with one provider and when to switch providers.

```

interface AiPlan<Errors, Provides, Requires> {}
```

An `AiPlan` has three generic type parameters:

- **Errors**: Any errors that can be handled during execution of the plan
- **Provides**: The services that this plan can provide (e.g. `AiLanguageModel`, `AiEmbeddingsModel`)
- **Requires**: The services that this plan requires (e.g. `OpenAiClient`, `AnthropicClient`)

If you’ve used `AiModel` before (via `OpenAiLanguageModel.model()` or similar), you’ll find `AiPlan` familiar. In fact, an `AiModel` _is_ in fact an `AiPlan` with just a single step.

This means you can start by writing your code with plain `AiModel`s, and as your needs become more complex (e.g. adding retries, switching providers), you can upgrade to `AiPlan` without changing how the rest of your code works.

## Defining a Primary Model

The primary entry point to building up an `AiPlan` is the `AiPlan.make` constructor.

This method defines the **primary model** that you would like to use for an LLM interaction, as well as the rules for retrying it under specific conditions.

Use this when you want to:

- Retry a model multiple times
- Customize backoff timing between attempts
- Decide whether to retry based on the error type

**Example** (Creating an `AiPlan` from an `AiModel`)

```

import { AiPlan } from "@effect/ai"
import type { AiLanguageModel, AiResponse } from "@effect/ai"
import { OpenAiLanguageModel } from "@effect/ai-openai"
import { Data, Effect, Schedule } from "effect"
9 collapsed lines
class NetworkError extends Data.TaggedError("NetworkError") {}
class ProviderOutage extends Data.TaggedError("ProviderOutage") {}
declare const generateDadJoke: Effect.Effect<
  AiResponse.AiResponse,
  NetworkError | ProviderOutage,
  AiLanguageModel.AiLanguageModel
>
const DadJokePlan = AiPlan.make({
  model: OpenAiLanguageModel.model("gpt-4o"),
  attempts: 3,
  schedule: Schedule.exponential("100 millis", 1.5),
  while: (error: NetworkError | ProviderOutage) =>
    error._tag === "NetworkError"
})
const main = Effect.gen(function*() {
  const plan = yield* DadJokePlan
  const response = yield* plan.use(generateDadJoke)
  console.log(response.text)
})
```

This plan will:

- Attempt to use OpenAi’s `"gpt-4o"` model up to 3 times
- Wait with an exponential backoff between attempts (starting at 100ms)
- Only re-attempt the call to OpenAi if the error is a `NetworkError`

---

## Adding Fallback Models

To make your interactions with large language models resilient to provider outages, you can define a **fallback** models to use. This will allow the plan to automatically fallback to another model if the previous step in the execution plan fails.

Use this when:

- You want to make your model interactions resilient to provider outages
- You want to potentially have multiple fallback models

**Example** (Adding a Fallback to Anthropic from OpenAi)

```

import { AiPlan } from "@effect/ai"
import type { AiLanguageModel, AiResponse } from "@effect/ai"
import { AnthropicLanguageModel } from "@effect/ai-anthropic"
import { OpenAiLanguageModel } from "@effect/ai-openai"
import { Data, Effect, Schedule } from "effect"
9 collapsed lines
class NetworkError extends Data.TaggedError("NetworkError") {}
class ProviderOutage extends Data.TaggedError("ProviderOutage") {}
declare const generateDadJoke: Effect.Effect<
  AiResponse.AiResponse,
  NetworkError | ProviderOutage,
  AiLanguageModel.AiLanguageModel
>
const DadJokePlan = AiPlan.make({
  model: OpenAiLanguageModel.model("gpt-4o"),
  attempts: 3,
  schedule: Schedule.exponential("100 millis", 1.5),
  while: (error: NetworkError | ProviderOutage) =>
    error._tag === "NetworkError"
}, {
  model: AnthropicLanguageModel.model("claude-3-7-sonnet-latest"),
  attempts: 2,
  schedule: Schedule.exponential("100 millis", 1.5),
  while: (error: NetworkError | ProviderOutage) =>
    error._tag === "ProviderOutage"
})
const main = Effect.gen(function*() {
  const plan = yield* DadJokePlan
  const response = yield* plan.use(generateDadJoke)
  console.log(response.text)
})
```

This plan will:

- Fallback to Anthropic if the OpenAi step of the plan fails
- Attempt to use Anthropic’s `"claude-3-7-sonnet"` model up to 2 times
- Wait with an exponential backoff between attempts (starting at 100ms)
- Only run and / or re-attempt the fallback if the error is a `ProviderOutage`

## End-to-End Usage

The following is the complete program with the desired `AiPlan` fully implemented:

```

import { AiPlan } from "@effect/ai"
import type { AiLanguageModel, AiResponse } from "@effect/ai"
import { AnthropicClient, AnthropicLanguageModel } from "@effect/ai-anthropic"
import { OpenAiClient, OpenAiLanguageModel } from "@effect/ai-openai"
import { NodeHttpClient } from "@effect/platform-node"
import { Config, Data, Effect, Layer, Schedule } from "effect"
class NetworkError extends Data.TaggedError("NetworkError") {}
class ProviderOutage extends Data.TaggedError("ProviderOutage") {}
declare const generateDadJoke: Effect.Effect<
  AiResponse.AiResponse,
  NetworkError | ProviderOutage,
  AiLanguageModel.AiLanguageModel
>
const DadJokePlan = AiPlan.make({
  model: OpenAiLanguageModel.model("gpt-4o"),
  attempts: 3,
  schedule: Schedule.exponential("100 millis", 1.5),
  while: (error: NetworkError | ProviderOutage) =>
    error._tag === "NetworkError"
}, {
  model: AnthropicLanguageModel.model("claude-3-7-sonnet-latest"),
  attempts: 2,
  schedule: Schedule.exponential("100 millis", 1.5),
  while: (error: NetworkError | ProviderOutage) =>
    error._tag === "ProviderOutage"
})
const main = Effect.gen(function*() {
  const plan = yield* DadJokePlan
  const response = yield* plan.use(generateDadJoke)
  console.log(response.text)
})
const Anthropic = AnthropicClient.layerConfig({
  apiKey: Config.redacted("ANTHROPIC_API_KEY")
}).pipe(Layer.provide(NodeHttpClient.layerUndici))
const OpenAi = OpenAiClient.layerConfig({
  apiKey: Config.redacted("OPENAI_API_KEY")
}).pipe(Layer.provide(NodeHttpClient.layerUndici))
main.pipe(
  Effect.provide([Anthropic, OpenAi]),
  Effect.runPromise
)
```

## Notes & Patterns

- You can chain multiple calls together for more complex failover logic
- `AiPlan` is re-usable and can be abstracted into shared logic if desired
- Great for teams needing **multi-provider resilience** or **predictable behavior under failure**
