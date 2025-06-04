# Breaking Changes in @effect/ai 0.16.5

## Overview

This document analyzes the breaking changes between @effect/ai 0.2.0 (our current version) and 0.16.5 (latest), with a focus on what needs to change in our codebase.

## Package Structure Changes

### @effect/ai (0.2.0 → 0.16.5)

- **Old Structure**:
  ```typescript
  import {
    AiChat,
    AiError,
    AiInput,
    AiResponse,
    AiRole,
    AiToolkit,
  } from "@effect/ai";
  import { Completions, Tokenizer } from "@effect/ai";
  ```
- **New Structure**:
  ```typescript
  import { AiLanguageModel, AiTool, AiToolkit } from "@effect/ai";
  import { Schema } from "effect";
  ```
- **Key Changes**:
  - Introduction of `AiLanguageModel` as the primary interface
  - Stronger focus on tool integration with `AiTool` and `AiToolkit`
  - Schema-based type definitions for tools and responses
  - Removal of direct Completions/Tokenizer exports (moved to provider packages)

### @effect/ai-openai (0.2.0 → 0.19.5)

- **Old Structure**:
  ```typescript
  import { OpenAiClient, StreamChunk } from "@effect/ai-openai/OpenAiClient";
  import {
    OpenAiCompletions,
    OpenAiConfig,
    OpenAiTokenizer,
  } from "@effect/ai-openai";
  ```
- **New Structure**:
  ```typescript
  import { OpenAiClient } from "@effect/ai-openai";
  import { OpenAiLanguageModel } from "@effect/ai-openai";
  ```
- **Key Changes**:
  - `StreamChunk` is now part of the internal API
  - New `OpenAiLanguageModel` class that implements `AiLanguageModel`
  - Expanded client interface with more operations
  - TypeId-based response types for better type safety

## Core Concepts Evolution

### 1. Language Model Interface

- **Old Approach**:
  ```typescript
  // We had to implement our own AgentLanguageModel interface
  interface AgentLanguageModel {
    streamText(
      options: StreamTextOptions,
    ): Stream.Stream<AiTextChunk, AIProviderError>;
    generateText(
      options: GenerateTextOptions,
    ): Effect.Effect<AiResponse, AIProviderError>;
  }
  ```
- **New Approach**:

  ```typescript
  // Use the built-in AiLanguageModel interface
  interface Service<Config> {
    generateText: <Tools extends AiTool.Any, Options>(
      options: Options & GenerateTextOptions<Tools>,
    ) => Effect.Effect<
      ExtractSuccess<Options>,
      ExtractError<Options>,
      ExtractContext<Options> | Config
    >;

    streamText: <Tools extends AiTool.Any, Options>(
      options: Options & GenerateTextOptions<Tools>,
    ) => Stream.Stream<
      ExtractSuccess<Options>,
      ExtractError<Options>,
      ExtractContext<Options> | Config
    >;

    generateObject: <A, I, R>(
      options: GenerateObjectOptions<A, I, R>,
    ) => Effect.Effect<AiResponse.WithStructuredOutput<A>, AiError, R | Config>;
  }
  ```

Key differences:

1. Generic tool support built into the interface
2. Structured output generation with schemas
3. Richer error and context type extraction
4. Configuration as a generic parameter

### 2. Stream Processing

- **Old Approach**:
  ```typescript
  // Direct Stream.asyncInterrupt usage
  return Stream.asyncInterrupt<AiTextChunk, AIProviderError>((emit) => {
    // Manual stream handling
    const onChunk = (chunk: StreamChunk) => {
      emit.chunk({ text: chunk.text || "" });
    };
    // Manual cleanup
    return Effect.sync(() => {
      cleanup();
    });
  });
  ```
- **New Approach**:
  ```typescript
  // Built-in streaming support with proper typing
  const streamText = (options: AiLanguageModelOptions) =>
    Stream.fromEffect(
      Effect.gen(function* (_) {
        const response = yield* _(makeRequest(options));
        return Stream.fromAsyncIterable(response.data, {
          onError: (error) => new AiError({ message: error.message }),
        });
      }),
    ).pipe(Stream.flatMap(identity));
  ```

### 3. Tool Integration

- **Old Approach**:

  ```typescript
  // Manual function calling implementation
  interface ToolCall {
    name: string;
    arguments: Record<string, unknown>;
  }

  // Manual response handling
  const handleToolCalls = (calls: ToolCall[]) =>
    Effect.forEach(calls, (call) => {
      // Manual dispatch
    });
  ```

- **New Approach**:

  ```typescript
  // Type-safe tool definitions with Schema
  const GetWeather = AiTool.make("GetWeather", {
    description: "Get the weather for a location",
    success: Schema.String,
    failure: Schema.Never,
    parameters: {
      location: Schema.String.annotations({
        description: "The location to get weather for",
      }),
    },
  });

  // Toolkit composition
  class WeatherTools extends AiToolkit.make(GetWeather) {}

  // Automatic tool handling
  const WeatherToolHandlers = WeatherTools.toLayer(
    Effect.gen(function* (_) {
      const weatherService = yield* _(WeatherService);
      return {
        GetWeather: ({ location }) => weatherService.getWeather(location),
      };
    }),
  );
  ```

## Implementation Requirements

### 1. Core Service Implementation

```typescript
export const OllamaAgentLanguageModelLive = Layer.effect(
  AgentLanguageModel,
  Effect.gen(function* (_) {
    const client = yield* _(OllamaAsOpenAIClient);
    const config = yield* _(OllamaConfig);

    const makeService = Effect.gen(function* (_) {
      return AiLanguageModel.make({
        generateText: (options) =>
          Effect.gen(function* (_) {
            const response = yield* _(
              client.createChatCompletion({
                model: config.model,
                messages: formatMessages(options.prompt),
                tools: options.tools,
                tool_choice: options.toolChoice,
              }),
            );

            return mapResponseToAiResponse(response);
          }),

        streamText: (options) =>
          Stream.fromEffect(
            Effect.gen(function* (_) {
              const response = yield* _(
                client.createChatCompletionStream({
                  model: config.model,
                  messages: formatMessages(options.prompt),
                  tools: options.tools,
                  tool_choice: options.toolChoice,
                }),
              );

              return Stream.fromAsyncIterable(response.data, {
                onError: (error) =>
                  new AiError({
                    message: error.message,
                    cause: error,
                  }),
              });
            }),
          ).pipe(Stream.flatMap(identity), Stream.map(mapChunkToAiResponse)),
      });
    });

    return yield* _(makeService);
  }),
);
```

### 2. Response Type Mapping

```typescript
interface AiResponse {
  text: string;
  toolCalls?: Array<{
    id: string;
    name: string;
    arguments: Record<string, unknown>;
  }>;
  metadata?: {
    usage?: {
      promptTokens: number;
      completionTokens: number;
      totalTokens: number;
    };
  };
}

const mapResponseToAiResponse = (
  response: Generated.CreateChatCompletionResponse,
): AiResponse => ({
  text: response.choices[0]?.message?.content || "",
  toolCalls: response.choices[0]?.message?.tool_calls?.map((call) => ({
    id: call.id,
    name: call.function.name,
    arguments: JSON.parse(call.function.arguments),
  })),
  metadata: {
    usage: response.usage && {
      promptTokens: response.usage.prompt_tokens,
      completionTokens: response.usage.completion_tokens,
      totalTokens: response.usage.total_tokens,
    },
  },
});
```

### 3. Error Handling

```typescript
class AiError extends Data.TaggedError("AiError")<{
  message: string;
  cause?: unknown;
}> {}

const mapErrorToAiError = (error: unknown): AiError => {
  if (error instanceof AiError) return error;

  if (error instanceof Error) {
    return new AiError({
      message: error.message,
      cause: error,
    });
  }

  return new AiError({
    message: String(error),
  });
};
```

## Required Changes in Our Codebase

### 1. ChatOrchestratorService

- Replace custom retry logic with built-in retry support
- Update stream handling to use new patterns
- Implement tool support using AiToolkit

```typescript
export const ChatOrchestratorServiceLive = Layer.effect(
  ChatOrchestratorService,
  Effect.gen(function* (_) {
    const telemetry = yield* _(TelemetryService);
    const model = yield* _(OpenAiLanguageModel.model("gpt-4"));

    return ChatOrchestratorService.of({
      streamConversation: ({ messages, options }) => {
        const stream = model.use(
          AiLanguageModel.streamText({
            prompt: JSON.stringify({ messages }),
            ...options,
          }),
        );

        return Stream.fromEffect(stream);
      },
    });
  }),
);
```

### 2. OllamaAgentLanguageModelLive

- Implement the new AiLanguageModel interface
- Use Schema for response types
- Update stream chunk handling

```typescript
export const OllamaAgentLanguageModelLive = Layer.effect(
  AgentLanguageModel,
  Effect.gen(function* (_) {
    const client = yield* _(OllamaAsOpenAIClient);

    return AiLanguageModel.make({
      streamText: (options) =>
        Effect.gen(function* (_) {
          // Implementation using new patterns
        }),
      generateText: (options) =>
        Effect.gen(function* (_) {
          // Implementation using new patterns
        }),
    });
  }),
);
```

### 3. Test Updates

- Update mocks to match new interfaces
- Use provided test utilities from @effect/ai
- Update Layer composition patterns

```typescript
const mockModel = AiLanguageModel.make({
  streamText: () => Stream.make({ text: "test" }),
  generateText: () => Effect.succeed({ text: "test" }),
});

const testLayer = Layer.succeed(AiLanguageModel, mockModel);
```

## Migration Strategy

1. **Phase 1: Core Interface Updates**

   - Update all language model implementations to use new AiLanguageModel interface
   - Migrate stream handling to new patterns
   - Update response type handling

2. **Phase 2: Tool Integration**

   - Define tools using AiTool.make
   - Create toolkits for different capabilities
   - Update orchestrator to handle tool calls

3. **Phase 3: Testing & Validation**

   - Update all test files
   - Verify streaming behavior
   - Test tool integration

4. **Phase 4: Error Handling & Retry Logic**
   - Implement new retry patterns
   - Update error mapping
   - Verify error propagation

## Benefits of Migration

1. **Type Safety**

   - Better type inference
   - Schema-based validation
   - Stricter interfaces

2. **Tool Integration**

   - Built-in support for function calling
   - Type-safe tool definitions
   - Better error handling

3. **Streaming Improvements**

   - More reliable stream handling
   - Better backpressure support
   - Improved cancellation

4. **Testing**
   - Better testing utilities
   - More predictable behavior
   - Easier mocking

## Risks and Mitigation

1. **Breaking Changes**

   - Extensive testing required
   - Gradual migration possible
   - Clear rollback path

2. **Performance Impact**

   - Monitor stream performance
   - Test with high load
   - Profile memory usage

3. **Integration Points**
   - Test all providers
   - Verify UI integration
   - Check error handling
