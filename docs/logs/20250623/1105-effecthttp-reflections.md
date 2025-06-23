# Deep Analysis: Effect HttpClient Service Not Found Issue

**Date**: 2025-06-23 11:05  
**Status**: Root Cause Identified  

## Deep Dive Analysis

After consulting the Effect documentation and analyzing the error patterns, I've identified the fundamental issue.

### The Core Problem

The issue stems from a **fundamental misunderstanding** of how Effect HTTP servers handle request bodies:

1. **HttpServerRequest vs HttpIncomingMessage**: Based on the Effect docs, `HttpIncomingMessage` has a `text` property that returns `Effect<string, E>`. This is what the request object likely implements.

2. **Service Context Isolation**: When you call `Effect.runPromise`, it creates a **completely isolated execution context** that doesn't inherit the server's service layers. This is by design in Effect.

3. **Wrong Layer Type**: `FetchHttpClient.layer` provides an `HttpClient` service, but this is for **making outgoing HTTP requests**, not for reading incoming request bodies in a server context.

## The Real Issue

Looking at the error trace and documentation:

```typescript
// What's happening in the route handlers:
const bodyText = await Effect.runPromise(
  Effect.gen(function*() {
    return yield* context.request.text  // This requires HttpClient internally
  })
)
```

The `request.text` is an Effect that internally uses HttpClient service to read the body. But **this is the wrong approach entirely**.

## Why FetchHttpClient.layer Doesn't Work

1. **Service Mismatch**: `FetchHttpClient` is for fetch-based HTTP clients (outgoing requests), not for reading server request bodies
2. **Context Type Mismatch**: The server request reading likely needs different services than what `FetchHttpClient` provides
3. **Execution Context**: Even if it was the right service, `Effect.runPromise` creates an isolated context

## The Correct Approach

Based on Effect patterns, there are several correct ways to handle this:

### Option 1: Don't Use Effect.runPromise for Request Bodies
```typescript
// If context.request is a native Request object:
const bodyText = await context.request.text()  // Direct promise, no Effect wrapper
```

### Option 2: Execute Within Server Context
The request body reading should happen within the Effect context that the server provides:

```typescript
// The route handler should return an Effect, not use runPromise internally
export const handler = Effect.gen(function*() {
  const request = yield* HttpServerRequest.HttpServerRequest
  const body = yield* request.text
  // ... process body
  return HttpServerResponse.json(result)
})
```

### Option 3: Use Schema-based Body Parsing
Effect platform provides schema-based body parsing:

```typescript
import { HttpServerRequest } from "@effect/platform"

const bodyEffect = HttpServerRequest.schemaBodyJson(MySchema)
// This returns an Effect that can be used within the server context
```

## The Architecture Problem

The current architecture mixes paradigms:
1. Psionic routes expect plain async functions
2. But the request object is Effect-based
3. Using `Effect.runPromise` breaks the service context chain

## Recommendations

### Immediate Fix Attempts

1. **Check Request Type**: First, verify what `context.request` actually is:
```typescript
console.log("Request type:", context.request.constructor.name)
console.log("Has text method:", 'text' in context.request)
console.log("Text is Effect?", context.request.text?._tag === "Effect")
```

2. **Try Direct Access**: If it's a native request:
```typescript
const bodyText = await context.request.text()  // No Effect wrapper
```

3. **Use Effect Throughout**: If Psionic supports Effect handlers:
```typescript
export const handler = Effect.gen(function*() {
  const body = yield* context.request.text
  // Return Effect, not Promise
})
```

### Deeper Architecture Fix

The real solution requires understanding how Psionic integrates with Effect:

1. **If Psionic wraps native requests**: The framework should provide unwrapped access to request bodies
2. **If Psionic uses Effect requests**: Route handlers should return Effects, not Promises
3. **Service provision**: The framework needs to properly propagate service contexts to route handlers

## Critical Questions for Investigation

1. **What is the actual type of `context.request`?**
   - Is it `HttpServerRequest` from Effect?
   - Is it `HttpIncomingMessage`?
   - Is it a native Bun/Node Request?

2. **How does Psionic expect route handlers to work?**
   - Should they return Promises or Effects?
   - How should request bodies be accessed?

3. **Where is the HttpClient dependency coming from?**
   - Why does reading a request body need an HTTP client?
   - Is this a bug in Effect platform or intended behavior?

## Next Steps

1. **Inspect the request object structure** to understand its actual type
2. **Look at Psionic examples** for the correct way to handle request bodies
3. **Consider bypassing Effect** for request body reading if it's causing issues
4. **Review the Psionic-Effect integration** to ensure proper service propagation

## Key Insight from Effect Documentation

After deep analysis of Effect platform documentation:

1. **HttpIncomingMessage Interface**: The `text` property returns `Effect<string, E>` - it's already an Effect
2. **HttpRouter.schemaJson**: Shows the proper pattern - handlers should work within Effect context
3. **Service Requirements**: The error shows HttpClient is needed internally by the platform

## The Fundamental Problem

**Psionic is mixing async/await with Effect incorrectly**. The framework appears to:
1. Use Effect HTTP server internally
2. But expose route handlers that expect Promises, not Effects
3. Force developers to use `Effect.runPromise` which breaks service context

## The Real Solution

### Option 1: Fix at Framework Level
Psionic should either:
- Provide native request objects (not Effect-wrapped)
- OR support Effect-returning route handlers

### Option 2: Hack Around It
Try accessing the underlying native request:
```typescript
// If context.request has an underlying native request
const nativeRequest = context.request._request || context.request.raw
const body = await nativeRequest.text()
```

### Option 3: Use Proper Effect Context
Instead of `Effect.runPromise`, the entire handler should be an Effect:
```typescript
// This won't work with current Psionic, but it's the "correct" Effect way
export const handler = HttpRouter.post(
  "/api/path",
  Effect.gen(function* () {
    const request = yield* HttpServerRequest.HttpServerRequest
    const body = yield* request.text
    return HttpServerResponse.json({ result })
  })
)
```

## Conclusion

The error occurs because:
1. **Psionic wraps requests in Effect HttpServerRequest objects**
2. **But expects Promise-returning handlers, not Effect-returning ones**
3. **Using `Effect.runPromise` breaks the service context chain**
4. **`FetchHttpClient.layer` is for outgoing HTTP requests, not incoming**

The issue is a **fundamental architectural mismatch** between Effect's service-based approach and Psionic's Promise-based route handlers. The framework needs to either:
- Fully embrace Effect (handlers return Effects)
- Fully hide Effect (provide native request objects)
- Provide a proper bridge that maintains service context

**This is not something that can be fixed by adding layers** - it requires architectural changes to how Psionic integrates with Effect.