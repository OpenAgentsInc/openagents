# Streaming Architecture Guide

**Audience**: Coding Agents  
**Purpose**: Deep dive into Effect-based streaming implementation  
**Last Updated**: 2025-06-23

## Critical Understanding

**THE MOST IMPORTANT RULE**: When converting Effect Streams to Web Streams, you MUST provide all required layers before the conversion. This is the #1 source of streaming bugs.

```typescript
// ❌ WRONG - This will fail with "Service not found" errors
const readableStream = yield* Stream.toReadableStreamEffect(effectStream)

// ✅ CORRECT - Provide all layers before conversion
const readableStream = yield* Stream.toReadableStreamEffect(effectStream).pipe(
  Effect.provide(Layer.merge(
    BunHttpPlatform.layer,
    FetchHttpClient.layer,
    YourServiceLayer
  ))
)
```

## Architecture Overview

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  AI Provider │────▶│Effect Stream │────▶│ SSE Transform│
└──────────────┘     └──────────────┘     └──────┬───────┘
                                                  │
                                                  ▼
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Browser    │◀────│   SSE HTTP   │◀────│ReadableStream│
└──────────────┘     └──────────────┘     └──────────────┘
```

## Core Concepts

### Effect Streams vs Web Streams
- **Effect Stream**: Type-safe, composable streams with error handling
- **ReadableStream**: Web standard streams for HTTP responses
- **Conversion**: `Stream.toReadableStreamEffect()` bridges the gap

### Server-Sent Events (SSE)
SSE is our standard for all streaming responses:
- Text-based protocol over HTTP
- Automatic reconnection
- Simple client implementation
- Works with all browsers

## Implementation Pattern

### 1. Create AI Stream
```typescript
export function streamChat(ctx: RouteContext) {
  return Effect.gen(function* () {
    // Parse request
    const bodyText = yield* ctx.request.text
    const { messages, model, temperature } = JSON.parse(bodyText)
    
    // Get AI client and create stream
    const client = yield* CloudflareClient
    const aiStream = yield* client.stream({
      model,
      messages,
      temperature,
      stream: true
    })
    
    return aiStream
  })
}
```

### 2. Transform to SSE Format
```typescript
const encoder = new TextEncoder()

const sseStream = aiStream.pipe(
  Stream.mapConcat((response) => {
    const chunks: Array<Uint8Array> = []
    
    for (const part of response.parts) {
      if (part._tag === "TextPart") {
        // Format as OpenAI-compatible SSE
        const event = {
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
        
        // SSE format: "data: {json}\n\n"
        const data = `data: ${JSON.stringify(event)}\n\n`
        chunks.push(encoder.encode(data))
      }
      
      if (part._tag === "FinishReasonPart") {
        const event = {
          id: `chatcmpl-${Date.now()}`,
          object: "chat.completion.chunk",
          created: Math.floor(Date.now() / 1000),
          model,
          choices: [{
            index: 0,
            delta: {},
            finish_reason: part.reason
          }]
        }
        chunks.push(encoder.encode(`data: ${JSON.stringify(event)}\n\n`))
      }
    }
    
    return chunks
  }),
  
  // Always end with [DONE] marker
  Stream.concat(Stream.make(encoder.encode("data: [DONE]\n\n")))
)
```

### 3. Convert to ReadableStream (CRITICAL STEP)
```typescript
// This is where most bugs occur!
const readableStream = yield* Stream.toReadableStreamEffect(sseStream).pipe(
  Effect.provide(Layer.merge(
    BunHttpPlatform.layer,      // Required for Bun runtime
    FetchHttpClient.layer,      // Required for HTTP operations
    CloudflareClient.Default    // Your service layer
  ))
)
```

### 4. Return HTTP Response
```typescript
return HttpServerResponse.raw(readableStream, {
  headers: {
    'Content-Type': 'text/event-stream',
    'Cache-Control': 'no-cache',
    'Connection': 'keep-alive',
    'X-Accel-Buffering': 'no'  // Disable nginx buffering
  }
})
```

## Complete Example

```typescript
export function cloudflareChat(ctx: RouteContext) {
  return Effect.gen(function* () {
    // 1. Parse request
    const bodyText = yield* ctx.request.text
    const { messages, model = "@cf/meta/llama-4-scout-17b-16e-instruct" } = JSON.parse(bodyText)
    
    // 2. Get client and create AI stream
    const client = yield* CloudflareClient
    const aiStream = yield* client.stream({
      model,
      messages,
      max_tokens: 4096,
      stream: true
    })
    
    // 3. Transform to SSE
    const encoder = new TextEncoder()
    const sseStream = aiStream.pipe(
      Stream.mapConcat((response) => {
        const chunks: Array<Uint8Array> = []
        
        for (const part of response.parts) {
          if (part._tag === "TextPart") {
            const chunk = {
              id: "chatcmpl-" + Math.random().toString(36).substring(2),
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
      Stream.concat(Stream.make(encoder.encode("data: [DONE]\n\n"))),
      Stream.catchAll((error) => {
        console.error("Streaming error:", error)
        return Stream.make(encoder.encode(`data: {"error": "${error}"}\n\n`))
      })
    )
    
    // 4. Convert to ReadableStream WITH ALL LAYERS
    const readableStream = yield* Stream.toReadableStreamEffect(sseStream).pipe(
      Effect.provide(Layer.merge(
        BunHttpPlatform.layer,
        FetchHttpClient.layer,
        CloudflareClient.Default
      ))
    )
    
    // 5. Return SSE response
    return HttpServerResponse.raw(readableStream, {
      headers: {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive'
      }
    })
  }).pipe(
    Effect.catchAll((error: any) => {
      console.error("Cloudflare API error:", error)
      return HttpServerResponse.json({ error: error.message }, { status: 500 })
    })
  )
}
```

## Stream Operations

### Buffering
For high-throughput streams:
```typescript
const bufferedStream = aiStream.pipe(
  Stream.buffer({ capacity: 100 })
)
```

### Rate Limiting
To prevent overwhelming clients:
```typescript
const throttledStream = aiStream.pipe(
  Stream.throttle({
    elements: 10,
    duration: "1 second"
  })
)
```

### Timeout Handling
For slow providers:
```typescript
const timedStream = aiStream.pipe(
  Stream.timeoutFail({
    duration: "30 seconds",
    onTimeout: () => new StreamTimeoutError()
  })
)
```

### Error Recovery
Graceful error handling:
```typescript
const safeStream = aiStream.pipe(
  Stream.catchAll((error) => {
    // Log error
    console.error("Stream error:", error)
    
    // Return error message as stream content
    return Stream.make({
      _tag: "TextPart",
      text: "I apologize, but I encountered an error. Please try again."
    } as const)
  })
)
```

## Client-Side Implementation

### JavaScript EventSource
```javascript
const eventSource = new EventSource('/api/cloudflare/chat', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ messages, model })
})

eventSource.onmessage = (event) => {
  if (event.data === '[DONE]') {
    eventSource.close()
    return
  }
  
  const chunk = JSON.parse(event.data)
  const content = chunk.choices[0]?.delta?.content || ''
  appendToUI(content)
}

eventSource.onerror = (error) => {
  console.error('SSE error:', error)
  eventSource.close()
}
```

### Fetch API Streaming
```javascript
const response = await fetch('/api/cloudflare/chat', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ messages, model })
})

const reader = response.body.getReader()
const decoder = new TextDecoder()

while (true) {
  const { done, value } = await reader.read()
  if (done) break
  
  const chunk = decoder.decode(value)
  const lines = chunk.split('\n')
  
  for (const line of lines) {
    if (line.startsWith('data: ')) {
      const data = line.slice(6)
      if (data === '[DONE]') continue
      
      try {
        const event = JSON.parse(data)
        const content = event.choices[0]?.delta?.content || ''
        appendToUI(content)
      } catch (e) {
        console.error('Parse error:', e)
      }
    }
  }
}
```

## Common Issues and Solutions

### Issue 1: "Service not found" Errors
**Cause**: Missing layers when converting to ReadableStream
**Solution**: Always provide all required layers before conversion

```typescript
// Add debug logging to verify layers
const readableStream = yield* Effect.gen(function* () {
  console.log("Converting stream with layers...")
  
  return yield* Stream.toReadableStreamEffect(sseStream).pipe(
    Effect.provide(Layer.merge(
      BunHttpPlatform.layer,
      FetchHttpClient.layer,
      CloudflareClient.Default
    ))
  )
})
```

### Issue 2: Stream Hangs Forever
**Cause**: Stream not properly terminated
**Solution**: Always end with [DONE] marker

```typescript
const sseStream = transformStream.pipe(
  // Ensure stream ends
  Stream.concat(Stream.make(encoder.encode("data: [DONE]\n\n"))),
  Stream.ensuring(Effect.sync(() => console.log("Stream completed")))
)
```

### Issue 3: Buffering Issues
**Cause**: Proxy or CDN buffering SSE responses
**Solution**: Add anti-buffering headers

```typescript
return HttpServerResponse.raw(readableStream, {
  headers: {
    'Content-Type': 'text/event-stream',
    'Cache-Control': 'no-cache',
    'Connection': 'keep-alive',
    'X-Accel-Buffering': 'no',      // Nginx
    'X-Content-Type-Options': 'nosniff',
    'Transfer-Encoding': 'chunked'
  }
})
```

### Issue 4: Memory Leaks
**Cause**: Streams not properly closed
**Solution**: Use Effect.scoped and proper cleanup

```typescript
const streamWithCleanup = Effect.scoped(
  Effect.gen(function* () {
    const stream = yield* createStream()
    
    yield* Effect.addFinalizer(() =>
      Effect.sync(() => console.log("Cleaning up stream"))
    )
    
    return stream
  })
)
```

## Testing Streams

### Unit Testing
```typescript
import { Stream, Effect, TestClock } from "effect"

test("stream transformation", async () => {
  const input = Stream.make(
    { _tag: "TextPart", text: "Hello" },
    { _tag: "TextPart", text: " World" },
    { _tag: "FinishReasonPart", reason: "stop" }
  )
  
  const output = await pipe(
    input,
    transformToSSE,
    Stream.runCollect,
    Effect.runPromise
  )
  
  expect(output).toContainEqual(
    expect.stringContaining("Hello")
  )
})
```

### Integration Testing
```typescript
test("streaming endpoint", async () => {
  const response = await fetch("/api/cloudflare/chat", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      messages: [{ role: "user", content: "Hi" }],
      model: "@cf/meta/llama-4-scout-17b-16e-instruct"
    })
  })
  
  expect(response.headers.get("content-type")).toBe("text/event-stream")
  
  const text = await response.text()
  expect(text).toContain("data: ")
  expect(text).toContain("[DONE]")
})
```

## Performance Optimization

### 1. Use Generators for Large Streams
```typescript
function* generateChunks(text: string, chunkSize = 10) {
  for (let i = 0; i < text.length; i += chunkSize) {
    yield text.slice(i, i + chunkSize)
  }
}

const chunkedStream = Stream.fromIterable(generateChunks(longText))
```

### 2. Parallel Stream Processing
```typescript
const parallelStream = Stream.merge(
  stream1,
  stream2,
  stream3
).pipe(
  Stream.buffer({ capacity: 100 })
)
```

### 3. Memory-Efficient Transformations
```typescript
// Use mapConcat instead of flatMap for arrays
const efficient = stream.pipe(
  Stream.mapConcat((item) => processItem(item))
)

// Avoid collecting entire stream
const bad = await Stream.runCollect(stream)  // Loads all in memory
const good = await Stream.runForEach(stream, (item) => process(item))
```

## Debugging Techniques

### Stream Tapping
```typescript
const debugStream = stream.pipe(
  Stream.tap((item) => 
    Effect.sync(() => console.log("Stream item:", item))
  ),
  Stream.tapError((error) =>
    Effect.sync(() => console.error("Stream error:", error))
  )
)
```

### Stream Metrics
```typescript
const meteredStream = stream.pipe(
  Stream.scan(0, (count, _) => count + 1),
  Stream.tap((count) =>
    Effect.sync(() => {
      if (count % 100 === 0) {
        console.log(`Processed ${count} items`)
      }
    })
  )
)
```

---

**Golden Rule**: Always provide ALL required layers before converting Effect Streams to ReadableStreams. This single rule will prevent 90% of streaming bugs.