# OpenAgents Effect Architecture Guide

**Audience**: Coding Agents  
**Purpose**: Comprehensive guide to understanding and working with the OpenAgents Effect-based architecture  
**Last Updated**: 2025-06-23

## Table of Contents
1. [Core Architectural Principles](#core-architectural-principles)
2. [Effect Foundation](#effect-foundation)
3. [Psionic Framework](#psionic-framework)
4. [AI Provider Architecture](#ai-provider-architecture)
5. [Streaming Implementation](#streaming-implementation)
6. [Service Layer Pattern](#service-layer-pattern)
7. [Error Handling Strategy](#error-handling-strategy)
8. [Common Patterns and Anti-Patterns](#common-patterns-and-anti-patterns)

## Core Architectural Principles

### 1. Effect-First Design
Every async operation in the codebase uses Effect, not Promises. This provides:
- Type-safe error handling
- Dependency injection via Layers
- Composable streaming
- Resource management

### 2. Service-Oriented Architecture
All capabilities are exposed as Effect Services:
```typescript
// Service definition pattern
export class MyService extends Effect.Service<MyService>()("MyService", {
  effect: Effect.gen(function* () {
    // Service implementation
    return {
      myMethod: (param: string) => Effect.succeed(result)
    }
  })
}) {}
```

### 3. Layer-Based Composition
Dependencies are provided through Layers, not imported directly:
```typescript
// Layer composition pattern
const AppLive = Layer.merge(
  DatabaseLayer,
  HttpClientLayer,
  ConfigLayer
)
```

## Effect Foundation

### Core Concepts You Must Understand

#### 1. Effect<Success, Error, Requirements>
The core type representing a computation that:
- Succeeds with type `Success`
- Fails with type `Error`
- Requires services of type `Requirements`

```typescript
// Example: Reading from database
const getUser: Effect.Effect<User, DatabaseError, Database> = 
  Effect.gen(function* () {
    const db = yield* Database
    return yield* db.query("SELECT * FROM users WHERE id = ?", [userId])
  })
```

#### 2. Generator Functions (Effect.gen)
We use generator syntax for readable async code:
```typescript
Effect.gen(function* () {
  const a = yield* effectA  // "await" an Effect
  const b = yield* effectB
  return a + b
})
```

#### 3. Services and Layers
Services are singletons provided through the Effect context:
```typescript
// Define service
class Database extends Effect.Service<Database>()("Database", {
  effect: Effect.succeed({
    query: (sql: string) => Effect.succeed([])
  })
}) {}

// Provide service
const program = myEffect.pipe(
  Effect.provide(Database.Default)
)
```

## Psionic Framework

### Overview
Psionic is our custom Effect-based web framework (packages/psionic). It wraps Effect's HTTP server with a familiar Express-like API while maintaining Effect semantics.

### Route Handler Types
```typescript
type RouteHandler =
  | ((context: RouteContext) => Effect.Effect<any, any, any>)  // Effect handler
  | ((context: any) => string | Promise<string> | Response | Promise<Response> | any)  // Legacy
```

### Route Context
Every route receives a context with the HTTP request:
```typescript
interface RouteContext {
  request: HttpServerRequest.HttpServerRequest  // Effect HTTP request
  params: Record<string, string>               // Route parameters
}
```

### Creating Routes
```typescript
// Effect-based route (PREFERRED)
app.get('/api/users', (ctx) => Effect.gen(function* () {
  const users = yield* UserService.getAll()
  return HttpServerResponse.json(users)
}))

// Reading request body
app.post('/api/users', (ctx) => Effect.gen(function* () {
  const bodyText = yield* ctx.request.text
  const body = JSON.parse(bodyText)
  // Process body...
  return HttpServerResponse.json({ success: true })
}))
```

### CRITICAL: Effect Detection
Psionic detects Effect returns using `Effect.isEffect()`:
```typescript
// In Psionic's route handler
const handlerResult = handler(context)
if (Effect.isEffect(handlerResult)) {
  return yield* handlerResult  // Execute within server context
}
```

## AI Provider Architecture

### Provider Interface Pattern
All AI providers (@openagentsinc/ai) follow a consistent pattern:

```typescript
// Provider service definition
export class CloudflareClient extends Effect.Service<CloudflareClient>()("CloudflareClient", {
  effect: Effect.gen(function* () {
    const config = yield* CloudflareConfig
    
    return {
      // Streaming method returns Effect Stream
      stream: (options: StreamOptions) => 
        Stream.fromAsyncIterable(streamGenerator(options), onError)
        
      // Non-streaming method returns Effect
      complete: (options: CompleteOptions) =>
        Effect.tryPromise({
          try: () => fetch(endpoint, { body: JSON.stringify(options) }),
          catch: (error) => new CloudflareError({ error })
        })
    }
  })
}) {}
```

### Provider Layers
Each provider has configuration and client layers:
```typescript
// Configuration layer
export const CloudflareConfigLive = Layer.succeed(
  CloudflareConfig,
  { apiKey: process.env.CLOUDFLARE_API_KEY! }
)

// Client layer with dependencies
export const CloudflareClientLive = CloudflareClient.Default.pipe(
  Layer.provide(CloudflareConfigLive),
  Layer.provide(FetchHttpClient.layer)
)
```

### Supported Providers
1. **Cloudflare Workers AI** (`@cf/meta/llama-*` models)
2. **OpenRouter** (100+ models via unified API)
3. **Ollama** (local models)
4. **OpenAI** (GPT models)
5. **Anthropic** (Claude models)

## Streaming Implementation

### Server-Sent Events (SSE) Pattern
All streaming responses use SSE format for compatibility:

```typescript
// SSE Response Headers
const SSE_HEADERS = {
  'Content-Type': 'text/event-stream',
  'Cache-Control': 'no-cache',
  'Connection': 'keep-alive'
}
```

### Stream Transformation Pipeline
```typescript
export function streamChat(ctx: RouteContext) {
  return Effect.gen(function* () {
    // 1. Parse request
    const bodyText = yield* ctx.request.text
    const { messages, model } = JSON.parse(bodyText)
    
    // 2. Get AI client and create stream
    const client = yield* CloudflareClient
    const aiStream = yield* client.stream({
      model,
      messages,
      stream: true
    })
    
    // 3. Transform to SSE format
    const encoder = new TextEncoder()
    const sseStream = aiStream.pipe(
      Stream.mapConcat((response) => {
        const chunks: Array<Uint8Array> = []
        
        for (const part of response.parts) {
          if (part._tag === "TextPart") {
            // Format as OpenAI-compatible SSE
            const chunk = {
              id: `chatcmpl-${Date.now()}`,
              object: "chat.completion.chunk",
              created: Math.floor(Date.now() / 1000),
              model,
              choices: [{
                index: 0,
                delta: { content: part.text },
                finish_reason: null
              }]
            }
            chunks.push(encoder.encode(`data: ${JSON.stringify(chunk)}\n\n`))
          }
        }
        return chunks
      }),
      // Add [DONE] marker
      Stream.concat(Stream.make(encoder.encode("data: [DONE]\n\n")))
    )
    
    // 4. Convert to ReadableStream with all required layers
    const readableStream = yield* Stream.toReadableStreamEffect(sseStream).pipe(
      Effect.provide(Layer.merge(
        BunHttpPlatform.layer,
        FetchHttpClient.layer,
        CloudflareClient.Default
      ))
    )
    
    // 5. Return as HTTP response
    return HttpServerResponse.raw(readableStream, {
      headers: SSE_HEADERS
    })
  })
}
```

### CRITICAL: Layer Provision for Streams
When converting Effect Streams to ReadableStreams, you MUST provide all layers:
```typescript
// ❌ WRONG - Missing layers
const readable = yield* Stream.toReadableStreamEffect(stream)

// ✅ CORRECT - All layers provided
const readable = yield* Stream.toReadableStreamEffect(stream).pipe(
  Effect.provide(requiredLayers)
)
```

## Service Layer Pattern

### Database Services (Relay Package)
```typescript
export class RelayDatabase extends Effect.Service<RelayDatabase>()("RelayDatabase", {
  effect: Effect.gen(function* () {
    const sql = yield* SqlClient.SqlClient
    
    return {
      getEvents: (filter: Filter) => Effect.gen(function* () {
        const query = buildQuery(filter)
        const result = yield* sql`${query}`
        return result.map(rowToEvent)
      })
    }
  })
}) {}
```

### HTTP Client Pattern
Always use `@effect/platform/HttpClient` for external requests:
```typescript
const response = yield* HttpClient.request.post(url).pipe(
  HttpClient.request.jsonBody(data),
  Effect.flatMap(HttpClient.response.json),
  Effect.scoped
)
```

## Error Handling Strategy

### Tagged Errors
All errors extend `Data.TaggedError` for type safety:
```typescript
export class CloudflareError extends Data.TaggedError("CloudflareError")<{
  message: string
  cause?: unknown
}> {}

export class ConfigError extends Data.TaggedError("ConfigError")<{
  message: string
}> {}
```

### Error Handling in Routes
```typescript
app.post('/api/chat', (ctx) => 
  Effect.gen(function* () {
    // Implementation
  }).pipe(
    Effect.catchTag("CloudflareError", (error) =>
      HttpServerResponse.json(
        { error: "AI service unavailable" },
        { status: 503 }
      )
    ),
    Effect.catchTag("ConfigError", (error) =>
      HttpServerResponse.json(
        { error: "Missing configuration" },
        { status: 500 }
      )
    )
  )
)
```

## Common Patterns and Anti-Patterns

### ✅ DO: Use Effect Throughout
```typescript
// Good: Effect all the way
export function handler(ctx: RouteContext) {
  return Effect.gen(function* () {
    const data = yield* fetchData()
    const processed = yield* processData(data)
    return HttpServerResponse.json(processed)
  })
}
```

### ❌ DON'T: Mix Promises and Effects
```typescript
// Bad: Creates isolated context
export async function handler(ctx: RouteContext) {
  const result = await Effect.runPromise(
    fetchData().pipe(Effect.provide(DatabaseLayer))
  )
  return HttpServerResponse.json(result)
}
```

### ✅ DO: Compose Layers Properly
```typescript
// Good: Merge all required layers
const AppLive = Layer.merge(
  Layer.merge(
    BunHttpPlatform.layer,
    FetchHttpClient.layer
  ),
  Layer.merge(
    DatabaseLayer,
    ConfigLayer
  )
)
```

### ❌ DON'T: Use .pipe(Effect.orDie) on HttpServerResponse
```typescript
// Bad: Corrupts Effect structure
return HttpServerResponse.json(data).pipe(Effect.orDie)

// Good: Return directly
return HttpServerResponse.json(data)
```

### ✅ DO: Handle Request Body in Effect Context
```typescript
// Good: Read body within Effect
const bodyText = yield* ctx.request.text
const body = JSON.parse(bodyText)
```

### ❌ DON'T: Use async/await for Request Methods
```typescript
// Bad: Breaks Effect context
const bodyText = await ctx.request.text()  // This is a Promise!
```

## Testing Patterns

### Testing Effect Services
```typescript
import { Effect, Layer, TestContext } from "effect"

test("UserService.create", async () => {
  const TestUserService = Layer.succeed(Database, {
    query: () => Effect.succeed([{ id: 1, name: "Test" }])
  })
  
  const result = await Effect.runPromise(
    UserService.create({ name: "Test" }).pipe(
      Effect.provide(TestUserService)
    )
  )
  
  expect(result.name).toBe("Test")
})
```

## Debugging Tips

### 1. Service Not Found Errors
If you see "Service not found", check:
- Is the service provided in the Layer?
- Are you using Effect.runPromise (creates new context)?
- Is the route handler returning an Effect?

### 2. Streaming Not Working
Check:
- Are all layers provided before `toReadableStreamEffect`?
- Is the response using correct SSE headers?
- Is the stream properly terminated with [DONE]?

### 3. Type Errors with Effect
- Use `Effect.gen(function* () {})` not `async/await`
- Yield Effects with `yield*` not `await`
- Return Effects from route handlers, not Promises

## Quick Reference

### Creating a New API Route
```typescript
// 1. Define error types
class MyError extends Data.TaggedError("MyError")<{ message: string }> {}

// 2. Create route handler
export function myRoute(ctx: RouteContext) {
  return Effect.gen(function* () {
    // Read request
    const body = yield* ctx.request.json
    
    // Call services
    const result = yield* MyService.process(body)
    
    // Return response
    return HttpServerResponse.json(result)
  }).pipe(
    Effect.catchTag("MyError", (error) =>
      HttpServerResponse.json({ error: error.message }, { status: 400 })
    )
  )
}

// 3. Register route
app.post('/api/my-route', myRoute)
```

### Adding a New AI Provider
```typescript
// 1. Define config
class ProviderConfig extends Context.Tag("ProviderConfig")<
  ProviderConfig,
  { apiKey: string; baseUrl: string }
>() {}

// 2. Define client service
class ProviderClient extends Effect.Service<ProviderClient>()("ProviderClient", {
  effect: Effect.gen(function* () {
    const config = yield* ProviderConfig
    
    return {
      stream: (options) => createStream(config, options),
      complete: (options) => createCompletion(config, options)
    }
  })
}) {}

// 3. Create layers
const ProviderConfigLive = Layer.succeed(ProviderConfig, {
  apiKey: process.env.PROVIDER_API_KEY!,
  baseUrl: "https://api.provider.com"
})

const ProviderClientLive = ProviderClient.Default.pipe(
  Layer.provide(ProviderConfigLive)
)
```

## Resources

- [Effect Documentation](https://effect.website)
- [Effect Platform HTTP](https://effect.website/docs/guides/platform/http)
- [Effect Streaming Guide](https://effect.website/docs/guides/streaming)
- Internal Docs: `/docs/logs/20250623/` for implementation history

---

**Remember**: When in doubt, follow the Effect patterns. The entire codebase is built on Effect's principles of type safety, composability, and explicit error handling.