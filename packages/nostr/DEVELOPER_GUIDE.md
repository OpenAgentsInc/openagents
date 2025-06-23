# Nostr Package Developer Guide

## Getting Started

This guide helps developers understand and contribute to the `@openagentsinc/nostr` package.

## Architecture Overview

The package follows Effect patterns throughout:

```typescript
// All services are defined as Effect Context Tags
export class MyService extends Context.Tag("MyService")<
  MyService,
  {
    readonly doSomething: (input: string) => Effect.Effect<Output, MyError>
  }
>() {}

// Implementations are provided as Layers
export const MyServiceLive = Layer.succeed(
  MyService,
  {
    doSomething: (input) => Effect.gen(function* () {
      // Implementation
    })
  }
)
```

## Directory Structure

```
packages/nostr/
├── src/
│   ├── core/              # Shared types and utilities
│   │   ├── Errors.ts      # Tagged error definitions
│   │   └── Schema.ts      # Common schemas (keys, events, etc.)
│   │
│   ├── services/          # Core Nostr protocol services
│   │   ├── WebSocketService.ts      # Low-level WebSocket handling
│   │   ├── RelayService.ts          # High-level relay interface
│   │   ├── CryptoService.ts         # Cryptographic operations
│   │   ├── EventService.ts          # Event creation/validation
│   │   ├── RelayPoolService.ts      # Multi-relay management
│   │   └── RelayReconnectService.ts # Auto-reconnection logic
│   │
│   ├── nips/              # New NIP implementations (NIP-02, 04, 05, 09, 19, 44)
│   │   └── nipXX.ts       # One file per NIP
│   │
│   ├── nip06/            # Legacy structure (to be migrated)
│   ├── nip28/            # Legacy structure (to be migrated)
│   ├── nip90/            # Legacy structure (to be migrated)
│   │
│   ├── agent-profile/    # OpenAgents-specific extensions
│   │
│   └── index.ts          # Public API exports
│
├── test/                 # Test files mirror src structure
├── README.md            # User documentation
├── IMPLEMENTATION_STATUS.md  # Current state of implementations
└── DEVELOPER_GUIDE.md   # This file
```

## Development Workflow

### 1. Setting Up

```bash
# Install dependencies
pnpm install

# Build the package
pnpm build

# Run tests
pnpm test

# Run tests in watch mode
pnpm test:watch
```

### 2. Adding a New NIP

#### Step 1: Create the implementation file

```typescript
// src/nips/nip99.ts
import { Context, Data, Effect, Layer, Schema } from "effect"

// Define schemas for the NIP
export const MyNipData = Schema.Struct({
  // ... fields
})

// Define errors
export class Nip99Error extends Data.TaggedError("Nip99Error")<{
  reason: "invalid_data" | "network_error" | "not_supported"
  message: string
  cause?: unknown
}> {}

// Define service interface
export class Nip99Service extends Context.Tag("nips/Nip99Service")<
  Nip99Service,
  {
    readonly doOperation: (data: MyNipData) => Effect.Effect<Result, Nip99Error>
  }
>() {}

// Implement service
export const Nip99ServiceLive = Layer.succeed(
  Nip99Service,
  {
    doOperation: (data) => Effect.gen(function* () {
      // Implementation
    })
  }
)

// Export utility functions for direct use
export const doOperation = (data: MyNipData) =>
  Effect.serviceWithEffect(Nip99Service, (service) => service.doOperation(data))
```

#### Step 2: Add tests

```typescript
// test/nips/nip99.test.ts
import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import * as Nip99 from "../../src/nips/nip99.js"

describe("NIP-99", () => {
  it("should perform operation", () => {
    const program = Effect.gen(function* () {
      const result = yield* Nip99.doOperation({ /* data */ })
      expect(result).toEqual(/* expected */)
    })

    Effect.runSync(program.pipe(
      Effect.provide(Nip99.Nip99ServiceLive)
    ))
  })
})
```

#### Step 3: Export from index

```typescript
// src/index.ts
export * as nip99 from "./nips/nip99.js"
```

#### Step 4: Update documentation

- Add to README.md
- Update IMPLEMENTATION_STATUS.md

### 3. Code Style Guidelines

#### Use Effect.gen for async code

```typescript
// ✅ Good
const myFunction = Effect.gen(function* () {
  const result = yield* someEffect
  return result + 1
})

// ❌ Bad - using Promise
const myFunction = async () => {
  const result = await somePromise
  return result + 1
}
```

#### Always define branded types

```typescript
// ✅ Good
export const EventId = Schema.String.pipe(
  Schema.brand("EventId"),
  Schema.pattern(/^[0-9a-f]{64}$/)
)
export type EventId = Schema.Schema.Type<typeof EventId>

// ❌ Bad
export type EventId = string
```

#### Use tagged errors

```typescript
// ✅ Good
export class MyError extends Data.TaggedError("MyError")<{
  reason: "not_found" | "invalid_input"
  message: string
}> {}

// ❌ Bad
throw new Error("Something went wrong")
```

#### Resource management

```typescript
// ✅ Good - automatic cleanup
const program = Effect.acquireUseRelease(
  acquire: connectToRelay(url),
  use: (connection) => doSomething(connection),
  release: (connection) => disconnect(connection)
)

// ❌ Bad - manual cleanup
const connection = await connect(url)
try {
  await doSomething(connection)
} finally {
  await disconnect(connection)
}
```

### 4. Testing Best Practices

#### Use Effect test utilities

```typescript
import { describe, expect, it } from "@effect/vitest"

describe("MyService", () => {
  it("should handle errors", () => {
    const program = myOperation().pipe(
      Effect.flip // Convert success to failure and vice versa
    )
    
    const result = Effect.runSyncExit(program)
    expect(Exit.isSuccess(result)).toBe(true)
  })
})
```

#### Test both success and failure cases

```typescript
it("should succeed with valid input", () => {
  const result = Effect.runSync(
    myOperation(validInput).pipe(
      Effect.provide(MyServiceLive)
    )
  )
  expect(result).toEqual(expectedOutput)
})

it("should fail with invalid input", () => {
  const result = Effect.runSyncExit(
    myOperation(invalidInput).pipe(
      Effect.provide(MyServiceLive)
    )
  )
  expect(Exit.isFailure(result)).toBe(true)
})
```

### 5. Common Patterns

#### Service composition

```typescript
const MyServiceLive = Layer.effect(
  MyService,
  Effect.gen(function* () {
    const crypto = yield* CryptoService
    const relay = yield* RelayService
    
    return {
      myMethod: (input) => Effect.gen(function* () {
        const signed = yield* crypto.sign(input)
        return yield* relay.publish(signed)
      })
    }
  })
).pipe(
  Layer.provide(CryptoServiceLive),
  Layer.provide(RelayServiceLive)
)
```

#### Retries and error recovery

```typescript
const resilientOperation = (input: string) =>
  myOperation(input).pipe(
    Effect.retry({
      times: 3,
      delay: Duration.exponential("100 millis")
    }),
    Effect.catchTag("NetworkError", () => 
      Effect.succeed(fallbackValue)
    )
  )
```

#### Concurrent operations

```typescript
const processMultiple = (items: Array<Item>) =>
  Effect.forEach(items, processItem, {
    concurrency: 5,
    batching: true
  })
```

### 6. Performance Considerations

1. **Use streaming for large datasets**
   ```typescript
   Stream.fromIterable(events).pipe(
     Stream.mapEffect(processEvent),
     Stream.runDrain
   )
   ```

2. **Cache expensive computations**
   ```typescript
   const cached = Effect.cached(expensiveOperation)
   ```

3. **Batch operations when possible**
   ```typescript
   Effect.all(operations, { batching: true })
   ```

### 7. Debugging Tips

#### Enable Effect tracing

```typescript
import { NodeRuntime } from "@effect/platform-node"

NodeRuntime.runMain(
  program.pipe(Effect.withSpan("my-operation"))
)
```

#### Use Effect.tap for debugging

```typescript
myOperation.pipe(
  Effect.tap((value) => Console.log("Value:", value)),
  Effect.tapError((error) => Console.error("Error:", error))
)
```

#### Pretty print errors

```typescript
import { Cause } from "effect"

Effect.runPromise(program).catch((cause) => {
  console.error(Cause.pretty(cause))
})
```

### 8. Common Pitfalls

1. **Forgetting to yield effects**
   ```typescript
   // ❌ Wrong
   Effect.gen(function* () {
     someEffect() // Effect not executed!
   })
   
   // ✅ Correct
   Effect.gen(function* () {
     yield* someEffect()
   })
   ```

2. **Not providing required services**
   ```typescript
   // ❌ Will fail at runtime
   Effect.runSync(programNeedingService)
   
   // ✅ Correct
   Effect.runSync(
     programNeedingService.pipe(
       Effect.provide(RequiredServiceLive)
     )
   )
   ```

3. **Using mutable state**
   ```typescript
   // ❌ Bad
   let counter = 0
   const increment = Effect.sync(() => counter++)
   
   // ✅ Good
   const counter = Ref.make(0)
   const increment = counter.pipe(
     Effect.flatMap((ref) => Ref.update(ref, (n) => n + 1))
   )
   ```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes following the guidelines above
4. Add tests for new functionality
5. Update documentation
6. Submit a pull request

## Questions?

- Check existing implementations for examples
- Review Effect documentation
- Ask in the OpenAgents Discord