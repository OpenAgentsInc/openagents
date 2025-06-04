# Introduction to Effect AI | Effect Documentation

Welcome to the documentation for Effect’s AI integration packages — a set of libraries designed to make working with large language models (LLMs) seamless, flexible, and provider-agnostic.

These packages enable you to write programs that describe _what_ you want to do with an LLM — generating completions, handling chat interactions, running function calls — without having to commit to _how_ or _where_ those operations are executed.

The core package, [`@effect/ai`](https://www.npmjs.com/package/@effect/ai), provides a high-level, unified interface for modeling LLM interactions, independent of any specific provider. Once you’re ready to run your program, you can plug in the services your program requires from our LLM provider integration packages.

This separation of concerns allows you to:

- Write clean, declarative business logic without worrying about provider-specific quirks
- Easily swap between or combine providers at runtime or during testing
- Take advantage of Effect’s features when building AI-driven workflows

Whether you’re building an intelligent agent, an interactive chat app, or a system that leverages LLMs for background tasks, Effect’s AI packages offer the flexibility and control you need!

Let’s dive in!

## Why Effect for AI?

Integrating LLMs isn’t just about sending API requests — it’s handling streaming output, retries, rate limits, timeouts, and user-driven side effects, all while keeping your system stable and responsive. Effect provides simple, composable building blocks to model these workflows in a **safe**, **declarative**, and **composable** manner.

By using Effect for your LLM interactions you’ll benefit from:

- 🧩 **Provider-Agnostic Architecture**
  Write your business logic once, and defer choosing the underlying provider (OpenAI, Anthropic, local models, mocks, etc.) until runtime

- 🧪 **Fully Testable**
  Because LLM interactions are modeled via Effect services, you can mock, simulate, or snapshot responses just by providing an alternative implementation

- 🧵 **Structured Concurrency**
  Run concurrent LLM calls, cancel stale requests, stream partial results, or race multiple providers — all safely managed by Effect’s structured concurrency model

- 🔍 **Observability**
  Leverage Effect’s built-in tracing, logging, and metrics to instrument your LLM interactions to gain deep insight into performance bottlenecks or failures in production

…and much more!

## Core Concepts

Effect’s AI integrations are built around the idea of **provider-agnostic programming**. Instead of hardcoding calls to a specific LLM provider’s API, you describe your interaction using the services provided by the base `@effect/ai` package.

These services expose capabilities such as:

- **Generating Text** – single-shot text generation
- **Generating Embeddings** – vector representations of text for search or retrieval
- **Tool Calling** – structured outputs and tool usage
- **Streaming** – incremental output for memory efficiency and responsiveness

Each of these services is defined as an _Effect service_ — meaning they can be injected, composed, and tested just like any other dependency in the Effect ecosystem.

This decoupling lets you write your AI code as a pure description of what you want to happen, and resolve _how_ it happens later — whether by wiring up OpenAI, Anthropic, a mock service for tests, or even your own custom LLM backend.

---

## Packages

Effect’s AI ecosystem is composed of several focused packages:

### `@effect/ai`

Defines the core abstractions for interacting with LLM provider services. This package defines the generic services and helper utilities needed to build AI-powered applications in a provider-agnostic way.

Use this package to:

- Define your application’s interaction with an LLM
- Structure chat or completion flows using Effect
- Build type-safe, declarative AI logic

For detailed API documentation, see the [API Reference](https://effect-ts.github.io/effect/docs/ai/ai).

### `@effect/ai-openai`

Concrete implementations of services from `@effect/ai` backed by the [OpenAI API](https://platform.openai.com/).

Supported services include:

- `AiLanguageModel` (via OpenAI’s [Chat Completions API](https://platform.openai.com/docs/api-reference/chat))
- `AiEmbeddingsModel` (via OpenAI’s [Embeddings API](https://platform.openai.com/docs/api-reference/embeddings))

For detailed API documentation, see the [API Reference](https://effect-ts.github.io/effect/docs/ai/openai).

### `@effect/ai-anthropic`

Concrete implementations of services from `@effect/ai` backed by the [Anthropic API](https://docs.anthropic.com/en/api/getting-started).

Supported services include:

- `AiLanguageModel` (via Anthropic’s [Messages API](https://docs.anthropic.com/en/api/messages))

For detailed API documentation, see the [API Reference](https://effect-ts.github.io/effect/docs/ai/anthropic).
