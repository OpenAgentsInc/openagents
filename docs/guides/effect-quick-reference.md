# Effect Quick Reference for Coding Agents

**Purpose**: Quick lookup for common Effect patterns in OpenAgents  
**Last Updated**: 2025-06-23

## Most Important Rules

1. **NEVER mix Promises and Effects** - Use Effect throughout
2. **ALWAYS provide layers before Stream.toReadableStreamEffect**
3. **USE Effect.gen with yield\*, not async/await**
4. **RETURN Effects from route handlers, not Promises**

## Common Patterns

### Basic Route Handler
```typescript
export function myRoute(ctx: RouteContext) {
  return Effect.gen(function* () {
    const body = yield* ctx.request.json
    const result = yield* MyService.process(body)
    return HttpServerResponse.json(result)
  })
}
```

### Route with Error Handling
```typescript
export function myRoute(ctx: RouteContext) {
  return Effect.gen(function* () {
    // Implementation
  }).pipe(
    Effect.catchTag("MyError", (error) =>
      HttpServerResponse.json({ error: error.message }, { status: 400 })
    ),
    Effect.catchAll((error) =>
      HttpServerResponse.json({ error: "Internal error" }, { status: 500 })
    )
  )
}
```

### Streaming Route (CRITICAL PATTERN)
```typescript
export function streamRoute(ctx: RouteContext) {
  return Effect.gen(function* () {
    // 1. Create Effect stream
    const effectStream = yield* createStream()
    
    // 2. Transform to SSE
    const sseStream = effectStream.pipe(transformToSSE)
    
    // 3. Convert to ReadableStream WITH LAYERS
    const readableStream = yield* Stream.toReadableStreamEffect(sseStream).pipe(
      Effect.provide(Layer.merge(
        BunHttpPlatform.layer,
        FetchHttpClient.layer,
        YourServiceLayer
      ))
    )
    
    // 4. Return response
    return HttpServerResponse.raw(readableStream, {
      headers: {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive'
      }
    })
  })
}
```

### Service Definition
```typescript
export class MyService extends Effect.Service<MyService>()("MyService", {
  effect: Effect.gen(function* () {
    const config = yield* MyConfig
    
    return {
      myMethod: (param: string) => Effect.gen(function* () {
        // Implementation
        return result
      })
    }
  })
}) {}
```

### Configuration Layer
```typescript
export const MyConfigLive = Layer.effect(
  MyConfig,
  Effect.gen(function* () {
    const value = process.env.MY_VALUE
    if (!value) {
      return yield* Effect.fail(new ConfigError({ message: "MY_VALUE not set" }))
    }
    return { value }
  })
)
```

### Error Types
```typescript
export class MyError extends Data.TaggedError("MyError")<{
  message: string
  cause?: unknown
}> {}
```

### HTTP Requests
```typescript
const response = yield* HttpClient.request.post(url).pipe(
  HttpClient.request.jsonBody(data),
  HttpClient.request.setHeader("Authorization", `Bearer ${token}`),
  Effect.flatMap(HttpClient.response.json),
  Effect.scoped
)
```

### Reading Request Body
```typescript
// Text body
const text = yield* ctx.request.text
const data = JSON.parse(text)

// JSON body (when available)
const data = yield* ctx.request.json

// Form data
const formData = yield* ctx.request.formData
```

## Common Mistakes to Avoid

### ❌ Using Effect.runPromise in Routes
```typescript
// WRONG - Creates isolated context
export async function handler(ctx) {
  const result = await Effect.runPromise(myEffect)
  return result
}
```

### ❌ Forgetting Layers for Streams
```typescript
// WRONG - Missing layers
const readable = yield* Stream.toReadableStreamEffect(stream)
```

### ❌ Using .pipe(Effect.orDie) on Responses
```typescript
// WRONG - Corrupts Effect structure
return HttpServerResponse.json(data).pipe(Effect.orDie)
```

### ❌ Mixing async/await with Effect
```typescript
// WRONG
const data = await ctx.request.text()  // This is a Promise!

// CORRECT
const data = yield* ctx.request.text   // This is an Effect!
```

## Debug Helpers

### Console Logging in Effects
```typescript
yield* Effect.log("Debug message")
yield* Effect.logDebug("Detailed debug info")
yield* Effect.logError("Error occurred")
```

### Tapping for Side Effects
```typescript
const result = yield* myEffect.pipe(
  Effect.tap((value) => Effect.sync(() => console.log("Value:", value))),
  Effect.tapError((error) => Effect.sync(() => console.error("Error:", error)))
)
```

### Service Inspection
```typescript
const service = yield* MyService
console.log("Service methods:", Object.keys(service))
```

## Layer Composition Patterns

### Simple Merge
```typescript
const AppLive = Layer.merge(Layer1, Layer2)
```

### Nested Merge
```typescript
const AppLive = Layer.merge(
  Layer.merge(Layer1, Layer2),
  Layer.merge(Layer3, Layer4)
)
```

### With Dependencies
```typescript
const ServiceLive = ServiceLayer.Default.pipe(
  Layer.provide(ConfigLayer),
  Layer.provide(HttpClientLayer)
)
```

## SSE Format
```typescript
// Standard SSE chunk format
const chunk = `data: ${JSON.stringify({
  id: "chatcmpl-" + Date.now(),
  object: "chat.completion.chunk",
  created: Math.floor(Date.now() / 1000),
  model: model,
  choices: [{
    index: 0,
    delta: { content: text },
    finish_reason: null
  }]
})}\n\n`

// End marker
const done = "data: [DONE]\n\n"
```

## Testing Patterns

### Basic Effect Test
```typescript
const result = await Effect.runPromise(
  myEffect.pipe(Effect.provide(TestLayer))
)
expect(result).toBe(expected)
```

### Stream Test
```typescript
const items = await pipe(
  myStream,
  Stream.runCollect,
  Effect.provide(TestLayer),
  Effect.runPromise
)
expect(Array.from(items)).toEqual([...])
```

## Environment Variables
```typescript
// Safe access with validation
const apiKey = yield* Effect.fromNullable(process.env.API_KEY).pipe(
  Effect.mapError(() => new ConfigError({ message: "API_KEY not set" }))
)
```

---

**Remember**: When in doubt, look at existing code patterns in the codebase. The patterns are consistent throughout.