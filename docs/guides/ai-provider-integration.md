# AI Provider Integration Guide

**Audience**: Coding Agents  
**Purpose**: Detailed guide for working with AI providers in the OpenAgents architecture  
**Last Updated**: 2025-06-23

## Overview

The OpenAgents AI package (`@openagentsinc/ai`) provides a unified, Effect-based interface for integrating with multiple AI providers. All providers follow consistent patterns for configuration, streaming, and error handling.

## Architecture Overview

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   API Routes    │     │   API Routes    │     │   API Routes    │
│ /api/cloudflare │     │ /api/openrouter │     │   /api/ollama   │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                        AI Package (@openagentsinc/ai)           │
├─────────────────┬─────────────────┬─────────────────────────────┤
│ CloudflareClient│ OpenRouterClient│      OllamaClient           │
├─────────────────┴─────────────────┴─────────────────────────────┤
│                    Common Types & Interfaces                     │
│              (StreamOptions, Message, Response)                  │
└─────────────────────────────────────────────────────────────────┘
```

## Core Types

### Message Format
All providers use OpenAI-compatible message format:
```typescript
interface Message {
  role: "system" | "user" | "assistant"
  content: string
}
```

### Stream Response Parts
```typescript
// Text content part
interface TextPart {
  _tag: "TextPart"
  text: string
}

// Tool/function call part
interface ToolCallPart {
  _tag: "ToolCallPart"
  toolCallId: string
  toolName: string
  args: unknown
}

// Tool response part
interface ToolResponsePart {
  _tag: "ToolResponsePart"
  toolCallId: string
  toolName: string
  response: unknown
}

// Finish reason part
interface FinishReasonPart {
  _tag: "FinishReasonPart"
  reason: "stop" | "length" | "tool_calls"
}

// Usage statistics part
interface UsagePart {
  _tag: "UsagePart"
  usage: {
    promptTokens: number
    completionTokens: number
    totalTokens: number
  }
}
```

## Provider Implementations

### 1. Cloudflare Workers AI

**Models**: `@cf/meta/llama-4-scout-17b-16e-instruct`, `@cf/meta/llama-3.3-70b-instruct-fp8-fast`, `@cf/deepseek-ai/deepseek-r1-distill-qwen-32b`, `@cf/meta/llama-3.1-8b-instruct`

#### Configuration
```typescript
class CloudflareConfig extends Context.Tag("CloudflareConfig")<
  CloudflareConfig,
  {
    accountId: string
    apiKey: string
  }
>() {}

// Layer setup
const CloudflareConfigLive = Layer.succeed(CloudflareConfig, {
  accountId: process.env.CLOUDFLARE_ACCOUNT_ID!,
  apiKey: process.env.CLOUDFLARE_API_KEY!
})
```

#### Client Implementation
```typescript
class CloudflareClient extends Effect.Service<CloudflareClient>()("CloudflareClient", {
  effect: Effect.gen(function* () {
    const config = yield* CloudflareConfig
    const http = yield* HttpClient.HttpClient
    
    return {
      stream: ({ model, messages, temperature = 0.7 }) =>
        Effect.gen(function* () {
          const response = yield* http.request.post(
            `https://api.cloudflare.com/client/v4/accounts/${config.accountId}/ai/run/${model}`
          ).pipe(
            HttpClient.request.setHeader("Authorization", `Bearer ${config.apiKey}`),
            HttpClient.request.jsonBody({
              messages,
              stream: true,
              temperature
            }),
            HttpClient.response.stream
          )
          
          // Parse SSE stream
          return Stream.fromReadableStream(() => response, (error) => 
            new CloudflareError({ message: "Stream error", cause: error })
          ).pipe(
            // Transform to parts
            Stream.map(parseSSEChunk),
            Stream.filterMap(identity)
          )
        })
    }
  })
}) {}
```

### 2. OpenRouter

**Models**: 100+ models including GPT-4, Claude, Llama, Mistral

#### Configuration
```typescript
class OpenRouterConfig extends Context.Tag("OpenRouterConfig")<
  OpenRouterConfig,
  {
    apiKey: string
    baseUrl?: string
    siteUrl?: string
    appName?: string
  }
>() {}
```

#### Special Headers
OpenRouter supports optional headers for analytics:
```typescript
const headers = {
  "Authorization": `Bearer ${config.apiKey}`,
  "HTTP-Referer": config.siteUrl || "https://openagents.com",
  "X-Title": config.appName || "OpenAgents"
}
```

### 3. Ollama (Local Models)

**Models**: Any model pulled via `ollama pull`

#### Configuration
```typescript
class OllamaConfig extends Context.Tag("OllamaConfig")<
  OllamaConfig,
  {
    baseUrl: string  // Default: "http://localhost:11434"
  }
>() {}
```

#### Streaming Implementation
```typescript
// Ollama uses native streaming API
const stream = yield* Effect.tryPromise({
  try: async () => {
    const response = await fetch(`${config.baseUrl}/api/chat`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        model,
        messages,
        stream: true
      })
    })
    
    // Parse NDJSON stream
    return parseNDJSON(response.body)
  },
  catch: (error) => new OllamaError({ cause: error })
})
```

## API Route Pattern

All AI routes follow this pattern:

```typescript
export function chatRoute(ctx: RouteContext) {
  return Effect.gen(function* () {
    // 1. Parse request
    const bodyText = yield* ctx.request.text
    const { messages, model, temperature } = JSON.parse(bodyText)
    
    // 2. Get client
    const client = yield* ProviderClient
    
    // 3. Create stream
    const aiStream = yield* client.stream({
      model,
      messages,
      temperature
    })
    
    // 4. Transform to SSE
    const encoder = new TextEncoder()
    const sseStream = transformToSSE(aiStream, encoder)
    
    // 5. Convert to ReadableStream
    const readableStream = yield* Stream.toReadableStreamEffect(sseStream).pipe(
      Effect.provide(Layer.merge(
        BunHttpPlatform.layer,
        FetchHttpClient.layer,
        ProviderClient.Default
      ))
    )
    
    // 6. Return response
    return HttpServerResponse.raw(readableStream, {
      headers: {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive'
      }
    })
  }).pipe(
    Effect.catchAll((error) => {
      console.error("Chat API error:", error)
      return HttpServerResponse.json(
        { error: "Failed to process chat request" },
        { status: 500 }
      )
    })
  )
}
```

## SSE Format Transformation

All providers must output OpenAI-compatible SSE format:

```typescript
function transformToSSE(
  stream: Stream.Stream<AIPart, CloudflareError>,
  encoder: TextEncoder
): Stream.Stream<Uint8Array, never> {
  return stream.pipe(
    Stream.mapConcat((part) => {
      const chunks: Array<Uint8Array> = []
      
      switch (part._tag) {
        case "TextPart":
          chunks.push(encoder.encode(
            `data: ${JSON.stringify({
              id: `chatcmpl-${Date.now()}`,
              object: "chat.completion.chunk",
              created: Math.floor(Date.now() / 1000),
              model,
              choices: [{
                index: 0,
                delta: { content: part.text },
                finish_reason: null
              }]
            })}\n\n`
          ))
          break
          
        case "FinishReasonPart":
          chunks.push(encoder.encode(
            `data: ${JSON.stringify({
              id: `chatcmpl-${Date.now()}`,
              object: "chat.completion.chunk",
              created: Math.floor(Date.now() / 1000),
              model,
              choices: [{
                index: 0,
                delta: {},
                finish_reason: part.reason
              }]
            })}\n\n`
          ))
          break
          
        case "UsagePart":
          chunks.push(encoder.encode(
            `data: ${JSON.stringify({
              id: `chatcmpl-${Date.now()}`,
              object: "chat.completion.chunk",
              created: Math.floor(Date.now() / 1000),
              model,
              usage: {
                prompt_tokens: part.usage.promptTokens,
                completion_tokens: part.usage.completionTokens,
                total_tokens: part.usage.totalTokens
              }
            })}\n\n`
          ))
          break
      }
      
      return chunks
    }),
    // Always end with [DONE]
    Stream.concat(Stream.make(encoder.encode("data: [DONE]\n\n")))
  )
}
```

## Error Handling

### Provider-Specific Errors
```typescript
// Each provider has its own error type
export class CloudflareError extends Data.TaggedError("CloudflareError")<{
  message: string
  statusCode?: number
  cause?: unknown
}> {}

export class OpenRouterError extends Data.TaggedError("OpenRouterError")<{
  message: string
  code?: string
  statusCode?: number
}> {}

export class OllamaError extends Data.TaggedError("OllamaError")<{
  message: string
  cause?: unknown
}> {}
```

### Error Recovery in Streams
```typescript
const safeStream = aiStream.pipe(
  Stream.catchAll((error) => {
    console.error("Stream error:", error)
    
    // Return error as SSE message
    return Stream.make({
      _tag: "TextPart",
      text: "I apologize, but I encountered an error. Please try again."
    } as const)
  })
)
```

## Adding a New Provider

### Step 1: Define Types
```typescript
// In packages/ai/src/providers/NewProvider.ts

// Configuration
export class NewProviderConfig extends Context.Tag("NewProviderConfig")<
  NewProviderConfig,
  {
    apiKey: string
    endpoint?: string
  }
>() {}

// Error type
export class NewProviderError extends Data.TaggedError("NewProviderError")<{
  message: string
  cause?: unknown
}> {}
```

### Step 2: Implement Client
```typescript
export class NewProviderClient extends Effect.Service<NewProviderClient>()("NewProviderClient", {
  effect: Effect.gen(function* () {
    const config = yield* NewProviderConfig
    const http = yield* HttpClient.HttpClient
    
    return {
      stream: (options: StreamOptions) => 
        Effect.gen(function* () {
          // Implementation
        }),
        
      complete: (options: CompleteOptions) =>
        Effect.gen(function* () {
          // Implementation  
        })
    }
  }),
  dependencies: [NewProviderConfig.Default, HttpClient.layer]
}) {}
```

### Step 3: Create Layers
```typescript
export const NewProviderConfigLive = Layer.effect(
  NewProviderConfig,
  Effect.gen(function* () {
    const apiKey = process.env.NEW_PROVIDER_API_KEY
    
    if (!apiKey) {
      return yield* Effect.fail(
        new ConfigError({ message: "NEW_PROVIDER_API_KEY not set" })
      )
    }
    
    return { apiKey }
  })
)

export const NewProviderClientLive = NewProviderClient.Default.pipe(
  Layer.provide(NewProviderConfigLive)
)
```

### Step 4: Add API Route
```typescript
// In apps/openagents.com/src/routes/api/newprovider.ts
export function newProviderChat(ctx: RouteContext) {
  return Effect.gen(function* () {
    const bodyText = yield* ctx.request.text
    const { messages, model } = JSON.parse(bodyText)
    
    const client = yield* NewProviderClient
    const stream = yield* client.stream({ messages, model })
    
    // Transform and return SSE stream
    // ... (same pattern as other providers)
  }).pipe(
    Effect.provide(NewProviderClientLive),
    Effect.catchTag("NewProviderError", (error) =>
      HttpServerResponse.json({ error: error.message }, { status: 500 })
    )
  )
}
```

## Testing Providers

### Unit Testing
```typescript
test("NewProvider streaming", async () => {
  const TestConfig = Layer.succeed(NewProviderConfig, {
    apiKey: "test-key"
  })
  
  const result = await pipe(
    NewProviderClient.stream({
      model: "test-model",
      messages: [{ role: "user", content: "Hello" }]
    }),
    Stream.runCollect,
    Effect.provide(NewProviderClient.Default),
    Effect.provide(TestConfig),
    Effect.runPromise
  )
  
  expect(result.length).toBeGreaterThan(0)
})
```

### Integration Testing
```typescript
// Test actual API endpoint
const response = await fetch("http://localhost:3003/api/newprovider/chat", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({
    model: "test-model",
    messages: [{ role: "user", content: "Test message" }]
  })
})

expect(response.headers.get("content-type")).toBe("text/event-stream")
```

## Performance Considerations

### 1. Stream Buffering
Use `Stream.buffer` for high-throughput providers:
```typescript
const bufferedStream = aiStream.pipe(
  Stream.buffer({ capacity: 100 })
)
```

### 2. Timeout Handling
Add timeouts for slow providers:
```typescript
const timedStream = aiStream.pipe(
  Stream.timeoutFail({
    duration: "30 seconds",
    onTimeout: () => new ProviderError({ message: "Request timeout" })
  })
)
```

### 3. Resource Management
Always use `Effect.scoped` for HTTP requests:
```typescript
const response = yield* HttpClient.request.post(url).pipe(
  HttpClient.request.jsonBody(body),
  Effect.scoped  // Ensures proper cleanup
)
```

## Debugging Tips

### Enable Debug Logging
```typescript
const debugStream = aiStream.pipe(
  Stream.tap((part) => 
    Effect.sync(() => console.log("Stream part:", part))
  )
)
```

### Trace HTTP Requests
```typescript
const client = http.pipe(
  HttpClient.mapRequest(HttpClientRequest.prependUrl("https://api.example.com")),
  HttpClient.tapRequest((req) =>
    Effect.sync(() => console.log("Request:", req))
  )
)
```

## Vector Embedding Services

The AI package provides embedding services for semantic search and vector operations, supporting multiple embedding providers beyond OpenAI.

### MessageEmbeddingService

**Purpose**: Converts chat messages into vector embeddings for semantic search.

```typescript
class MessageEmbeddingService extends Context.Tag("MessageEmbeddingService")<
  MessageEmbeddingService,
  MessageEmbeddingService.Service
>() {}

export declare namespace MessageEmbeddingService {
  export interface Service {
    readonly embedMessage: (content: MessageContent) => Effect.Effect<EmbeddingResult, AiError>
    readonly embedMessages: (contents: ReadonlyArray<MessageContent>) => Effect.Effect<ReadonlyArray<EmbeddingResult>, AiError>
  }
}
```

**Message Content Structure**:
```typescript
export interface MessageContent {
  readonly content?: string | undefined
  readonly entry_type: string
  readonly role?: string | undefined
  readonly thinking?: string | undefined
  readonly summary?: string | undefined
  readonly tool_output?: string | undefined
}
```

**Configuration Options**:
```typescript
export interface MessageEmbeddingConfig {
  readonly model: string                    // e.g., "text-embedding-3-small"
  readonly includeThinking?: boolean        // Include thinking content (default: true)
  readonly includeToolOutputs?: boolean     // Include tool outputs (default: true)
  readonly maxTokens?: number              // Token limit (default: 8192)
}
```

### ConvexEmbeddingService

**Purpose**: Integrates embeddings with Convex database for storage and vector search.

```typescript
class ConvexEmbeddingService extends Context.Tag("ConvexEmbeddingService")<
  ConvexEmbeddingService,
  ConvexEmbeddingService.Service
>() {}

export declare namespace ConvexEmbeddingService {
  export interface Service {
    readonly embedAndStore: (message: ConvexMessage) => Effect.Effect<string, AiError>
    readonly embedAndStoreBatch: (messages: ReadonlyArray<ConvexMessage>) => Effect.Effect<ReadonlyArray<{message_id: string; embedding_id: string}>, AiError>
    readonly searchSimilar: (options: SimilaritySearchOptions) => Effect.Effect<ReadonlyArray<SimilaritySearchResult>, AiError>
    readonly searchSimilarByText: (options: TextSearchOptions) => Effect.Effect<ReadonlyArray<SimilaritySearchResult>, AiError>
    readonly getStats: (session_id?: string) => Effect.Effect<EmbeddingStats, AiError>
  }
}
```

### Embedding Provider Support

**Local/Self-Hosted Options** (Recommended):
- **Ollama** - Local embedding models via HTTP API
- **Hugging Face Transformers** - Local model inference
- **Sentence Transformers** - Local embedding generation
- **Custom HTTP endpoints** - Any compatible embedding service

**Cloud Options**:
- **OpenAI** - text-embedding-3-small, text-embedding-ada-002
- **Cohere** - embed-english-v3.0, embed-multilingual-v3.0
- **Voyage AI** - voyage-large-2, voyage-code-2

### Embedding Provider Implementation Pattern

```typescript
// Example: Ollama embedding provider
export class OllamaEmbeddingProvider extends Effect.Service<OllamaEmbeddingProvider>()("OllamaEmbeddingProvider", {
  effect: Effect.gen(function* () {
    const config = yield* OllamaConfig
    const http = yield* HttpClient.HttpClient
    
    return {
      embed: (text: string, model: string) =>
        Effect.gen(function* () {
          const response = yield* http.request.post(`${config.baseUrl}/api/embeddings`).pipe(
            HttpClient.request.jsonBody({
              model,
              prompt: text
            }),
            HttpClient.response.json
          )
          
          return {
            embedding: response.embedding,
            model,
            dimensions: response.embedding.length
          }
        })
    }
  })
}) {}
```

### Integration with Overlord Sync

The Overlord sync system integrates embedding generation during message synchronization:

```typescript
// CLI options for embedding control
const enableEmbeddingsOption = Options.boolean("enable-embeddings").pipe(
  Options.withDescription("Generate vector embeddings for semantic search"),
  Options.withDefault(false)
)

const embeddingModelOption = Options.string("embedding-model").pipe(
  Options.withDescription("Embedding model to use"),
  Options.withDefault("text-embedding-3-small")
)

// Usage in sync commands
overlord transport --enable-embeddings --embedding-model="nomic-embed-text"
overlord import --enable-embeddings --embedding-model="all-MiniLM-L6-v2"
```

**Embedding Generation Flow**:
1. Message parsed from JSONL
2. Content filtered (skip empty/summary entries)
3. Text extracted from message content, thinking, and tool outputs
4. Embedding generated via configured provider
5. Vector stored in Convex with 1536-dimensional index
6. Embedding failures logged but don't break sync

### Provider Configuration Examples

**Ollama (Local)**:
```typescript
export const OllamaEmbeddingConfigLive = Layer.succeed(OllamaConfig, {
  baseUrl: "http://localhost:11434",
  model: "nomic-embed-text"  // Local embedding model
})
```

**Hugging Face (Local)**:
```typescript
export const HuggingFaceEmbeddingConfigLive = Layer.succeed(HuggingFaceConfig, {
  modelPath: "./models/all-MiniLM-L6-v2",  // Local model directory
  maxLength: 512
})
```

**Custom HTTP Service**:
```typescript
export const CustomEmbeddingConfigLive = Layer.succeed(CustomEmbeddingConfig, {
  endpoint: "http://your-embedding-service.com/embed",
  apiKey: process.env.CUSTOM_EMBEDDING_KEY,
  model: "your-model-name"
})
```

### Performance Considerations

**Batch Processing**:
```typescript
// Process embeddings in configurable batches
const batchSize = config.batchSize ?? 10
for (let i = 0; i < messages.length; i += batchSize) {
  const batch = messages.slice(i, i + batchSize)
  yield* embeddingService.embedMessages(batch)
}
```

**Content Filtering**:
```typescript
// Only generate embeddings for meaningful content
const shouldGenerateEmbedding = (messageRecord: any): boolean => {
  if (messageRecord.entry_type === "summary") return false
  
  const hasContent = messageRecord.content?.trim().length > 0
  const hasThinking = messageRecord.thinking?.trim().length > 0
  const hasToolOutput = messageRecord.tool_output?.trim().length > 0
  
  return hasContent || hasThinking || hasToolOutput
}
```

**Error Resilience**:
```typescript
// Embedding failures don't break sync
yield* generateEmbeddingForMessage(messageId, messageRecord, embeddingConfig).pipe(
  Effect.catchAll((error) =>
    Effect.logWarning(`Failed to generate embedding for message ${messageRecord.entry_uuid}: ${error}`)
  )
)
```

---

**Remember**: All AI providers must maintain compatibility with the OpenAI API format for seamless frontend integration. For embeddings, prefer local/self-hosted solutions to avoid external API dependencies and costs.