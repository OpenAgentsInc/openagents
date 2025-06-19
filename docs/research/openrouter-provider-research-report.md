# OpenRouter API Integration for OpenAgents Effect AI Library

OpenRouter provides a **highly compatible API** that serves as a unified gateway to over 300 AI models while maintaining OpenAI's chat completions format. After thorough investigation, I recommend creating a **dedicated OpenRouter provider** to fully leverage its unique capabilities including intelligent provider routing, automatic failbacks, and cost optimization features.

## API Compatibility with OpenAI

OpenRouter maintains **near-perfect compatibility** with OpenAI's API structure, making it a viable drop-in replacement with minimal code changes. Both APIs share identical endpoint paths (`/v1/chat/completions`), Bearer token authentication, and response schemas. The key differences are additive features rather than breaking changes.

### Streaming Support via SSE
Both APIs support Server-Sent Events streaming with `stream: true`. OpenRouter adds comment payloads to prevent timeouts (`:{"comment": "Keep connection alive"}`) which should be ignored per SSE specifications. Streaming errors are handled inline rather than terminating the stream, requiring adjusted error handling logic.

### Response Format Extensions
OpenRouter normalizes responses across providers while adding metadata:
```typescript
{
  // Standard OpenAI fields
  id: string,
  choices: [...],
  usage: { prompt_tokens, completion_tokens, total_tokens },

  // OpenRouter additions
  provider?: string,        // Which provider served the request
  model_used?: string,      // Actual model (may differ from requested)
  native_finish_reason?: string  // Provider's original finish reason
}
```

## Implementation Architecture Recommendation

### Create a Dedicated OpenRouter Provider

After analyzing OpenRouter's extensive feature set, a dedicated provider implementation offers significant advantages over extending the existing OpenAI provider:

**Unique OpenRouter Features:**
- **Provider Routing**: Intelligent selection across 300+ models with `provider.order` arrays
- **Automatic Fallbacks**: Seamless failover when providers are down or rate-limited
- **Cost Optimization**: Dynamic routing to `:floor` (cheapest) or `:nitro` (fastest) variants
- **Model Variants**: Special suffixes like `:free`, `:online` (web search enabled)
- **BYOK Support**: Bring Your Own Key with 5% fee for enterprise deployments
- **Privacy Controls**: `data_collection: "deny"` for compliance requirements

### Effect.js Service Architecture

```typescript
// Core service definition
export class OpenRouterService extends Context.Tag("OpenRouterService")<
  OpenRouterService,
  {
    completions: (request: OpenRouterRequest) => Effect.Effect<OpenRouterResponse, OpenRouterError>
    stream: (request: OpenRouterRequest) => Effect.Effect<Stream.Stream<OpenRouterStreamChunk, OpenRouterError>, OpenRouterError>
    models: () => Effect.Effect<OpenRouterModel[], OpenRouterError>
    generation: (id: string) => Effect.Effect<GenerationStats, OpenRouterError>
  }
>() {}

// Layer implementation
export const OpenRouterLive = Layer.effect(
  OpenRouterService,
  Effect.gen(function* () {
    const httpClient = yield* HttpClient.HttpClient
    const config = yield* OpenRouterConfig

    return OpenRouterService.of({
      completions: (request) =>
        httpClient.post("/chat/completions", {
          body: HttpBody.json(normalizeRequest(request)),
          headers: buildHeaders(config)
        }).pipe(
          HttpClientResponse.schemaBodyJsonScoped(OpenRouterResponse),
          Effect.mapError(handleOpenRouterError),
          Effect.retry(config.retryPolicy)
        ),

      stream: (request) =>
        httpClient.post("/chat/completions", {
          body: HttpBody.json({ ...request, stream: true }),
          headers: buildHeaders(config)
        }).pipe(
          HttpClientResponse.stream,
          Effect.map(Stream.fromReadableStream),
          Effect.map(parseSSEStream)
        )
    })
  })
)
```

## Model Management in OpenRouter

### Model Naming Convention
Models follow a `{provider}/{model-name}` format with optional variants:
- Standard: `openai/gpt-4o`, `anthropic/claude-3.5-sonnet`
- With variants: `openai/gpt-4:nitro` (fastest), `anthropic/claude-3-haiku:free`
- Auto Router: `openrouter/auto` for dynamic model selection

### Advanced Model Configuration
```typescript
{
  "model": "openai/gpt-4o",
  "models": ["anthropic/claude-3.5-sonnet", "openai/gpt-3.5-turbo"], // Fallbacks
  "provider": {
    "order": ["OpenAI", "Azure"],        // Try providers in order
    "allow_fallbacks": true,
    "require_parameters": true,          // Only providers supporting all params
    "data_collection": "deny",           // Privacy compliance
    "sort": "throughput"                 // or "price" for optimization
  }
}
```

### Rate Limiting and Headers
Required headers include standard Bearer authentication. Optional headers enhance tracking:
```typescript
{
  "Authorization": "Bearer sk-or-v1-...",
  "HTTP-Referer": "https://yourapp.com",  // For app rankings
  "X-Title": "Your App Name"               // For identification
}
```

## Effect.js Integration Patterns

### Configuration Schema
```typescript
export const OpenRouterConfigSchema = Schema.Struct({
  apiKey: Schema.String.pipe(Schema.nonEmptyString()),
  baseUrl: Schema.String.pipe(Schema.withDefault(() => "https://openrouter.ai/api/v1")),
  referer: Schema.optional(Schema.String),
  title: Schema.optional(Schema.String),
  timeout: Schema.optional(Schema.Number).pipe(Schema.withDefault(() => 30000)),
  retryPolicy: Schema.optional(RetryPolicySchema),
  provider: Schema.optional(ProviderRoutingSchema)
})
```

### Error Handling with Custom Types
```typescript
export type OpenRouterError =
  | OpenRouterValidationError
  | OpenRouterRateLimitError
  | OpenRouterModelNotFoundError
  | OpenRouterProviderError

const handleOpenRouterError = (error: HttpClientError) =>
  Effect.gen(function* () {
    if (error._tag === "ResponseError") {
      const status = error.response.status

      if (status === 429) {
        return new OpenRouterRateLimitError({
          message: "Rate limit exceeded",
          retryAfter: error.response.headers["retry-after"]
        })
      }

      // Parse OpenRouter error response
      const errorBody = yield* parseErrorResponse(error.response)
      return new OpenRouterProviderError({
        message: errorBody.error.message,
        provider: errorBody.provider,
        status
      })
    }

    return new OpenRouterError({ message: error.message })
  })
```

### Streaming Implementation
```typescript
const parseSSEStream = (stream: Stream.Stream<string, never>) =>
  stream.pipe(
    Stream.splitLines,
    Stream.filter(line => line.startsWith("data: ") && line !== "data: [DONE]"),
    Stream.map(line => line.slice(6)),
    Stream.mapEffect(jsonString =>
      Schema.decodeUnknown(OpenRouterStreamChunk)(JSON.parse(jsonString)).pipe(
        Effect.catchAll(() => Effect.succeed(undefined))
      )
    ),
    Stream.filter((chunk): chunk is OpenRouterStreamChunk => chunk !== undefined)
  )
```

### Testing Strategy
```typescript
// Mock service for testing
export const OpenRouterMock = Layer.succeed(
  OpenRouterService,
  OpenRouterService.of({
    completions: (request) =>
      Effect.succeed({
        id: "mock-completion",
        object: "chat.completion",
        created: Date.now(),
        model: request.model,
        choices: [{
          index: 0,
          message: {
            role: "assistant",
            content: `Mock response for: ${request.messages[0]?.content}`
          },
          finish_reason: "stop"
        }],
        usage: { prompt_tokens: 10, completion_tokens: 20, total_tokens: 30 }
      }),

    stream: (request) =>
      Effect.succeed(createMockStream(request))
  })
)
```

## Migration Guide from OpenAI

### Configuration Migration
```typescript
// Existing OpenAI configuration
const openai = new OpenAI({
  apiKey: process.env.OPENAI_API_KEY,
})

// OpenRouter with Effect.js
const OpenRouterConfigLive = Layer.succeed(
  OpenRouterConfig,
  {
    apiKey: process.env.OPENROUTER_API_KEY,
    baseUrl: "https://openrouter.ai/api/v1",
    referer: "https://yourapp.com",
    title: "Your App"
  }
)

// Model name mapping
const modelMapping = {
  'gpt-3.5-turbo': 'openai/gpt-3.5-turbo',
  'gpt-4': 'openai/gpt-4',
  'claude-3-opus': 'anthropic/claude-3-opus'
}
```

### Complete Usage Example
```typescript
const program = Effect.gen(function* () {
  const openRouter = yield* OpenRouterService

  // Basic completion with fallbacks
  const response = yield* openRouter.completions({
    model: "openai/gpt-4o",
    models: ["anthropic/claude-3.5-sonnet", "openai/gpt-3.5-turbo"],
    messages: [
      { role: "user", content: "Explain Effect.js" }
    ],
    temperature: 0.7,
    provider: {
      allow_fallbacks: true,
      data_collection: "deny"
    }
  })

  // Streaming with cost optimization
  const stream = yield* openRouter.stream({
    model: "anthropic/claude-3-haiku:floor", // Cheapest option
    messages: [
      { role: "user", content: "Tell me a story" }
    ]
  })

  yield* stream.pipe(
    Stream.tap(chunk =>
      Effect.logInfo(`Content: ${chunk.choices[0]?.delta?.content || ""}`)
    ),
    Stream.runDrain
  )
})

// Run with full observability
program.pipe(
  Effect.provide(MainLayer),
  withMetrics({ service: "openrouter" }),
  withTracing("openrouter.chat"),
  Effect.runPromise
)
```

## Key Implementation Considerations

**Type Safety**: The dedicated provider enables full TypeScript support for OpenRouter-specific features like provider routing and model variants, preventing runtime errors through compile-time validation.

**Error Resilience**: OpenRouter's automatic fallback mechanism requires careful error handling to distinguish between recoverable provider errors and actual failures. The Effect.js retry policies integrate seamlessly with OpenRouter's built-in resilience features.

**Cost Management**: Leverage the Generation API (`/api/v1/generation/{id}`) to track actual costs per request, especially important when using dynamic model routing or fallbacks that may switch between differently priced models.

**Observability**: Implement comprehensive logging and metrics to track provider usage, fallback patterns, and cost optimization effectiveness. OpenRouter's transparent provider information enables detailed performance analysis across different AI providers.

The combination of OpenRouter's unified API and Effect.js's compositional patterns creates a robust, type-safe foundation for multi-provider AI applications with built-in resilience and cost optimization.
