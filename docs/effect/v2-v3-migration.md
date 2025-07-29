# Effect-TS v2 to v3 Migration Guide

This document captures the key migration patterns and challenges encountered during OpenAgents' transition from Effect-TS v2 to v3, based on the comprehensive testing implementation for Issue #1269.

## Overview

Effect-TS v3 introduces significant API changes that improve type safety, performance, and developer experience. However, these changes require careful migration of existing code patterns.

### Version Context
- **Current Version**: Effect v3.17.2
- **Migration Scope**: All service-level code and testing infrastructure
- **Compatibility**: Breaking changes in core APIs require comprehensive updates

## Major API Changes

### 1. Service Definition Pattern

**v2 Pattern (Deprecated)**:
```typescript
// Old Context.GenericTag pattern
const MyService = Context.GenericTag<MyService>("MyService")

interface MyService {
  method1: () => Effect.Effect<string, never, never>
  method2: (input: string) => Effect.Effect<void, MyError, never>
}
```

**v3 Pattern (Current)**:
```typescript
// New Effect.Service pattern
class MyService extends Effect.Service<MyService>()("MyService", {
  sync: () => ({
    method1: () => Effect.succeed("result"),
    method2: (input: string) => Effect.gen(function* () {
      // Implementation
    })
  })
}) {}
```

### 2. TestClock API Removal

**v2 Pattern (No longer available)**:
```typescript
import { TestClock, TestContext } from "effect/TestClock"

// Complex time testing - REMOVED in v3
const testLayer = Layer.mergeAll(
  TestClock.live,
  TestContext.live,
  MyService.Default
)
```

**v3 Pattern (Simplified)**:
```typescript
// Direct Effect patterns without TestClock
const test = Effect.gen(function* () {
  const service = yield* MyService
  const result = yield* service.method()
  return result
}).pipe(Effect.provide(MyService.Default))

// Use Effect.sleep for time-based testing
yield* Effect.sleep("10 millis")
```

### 3. Stream Processing Updates

**v2 Pattern**:
```typescript
// Direct method access (may not exist)
const chunks = yield* Stream.runCollect(stream)
const array = chunks.toArray() // Method may not exist
```

**v3 Pattern**:
```typescript
// Static method usage
const chunksChunk = yield* Stream.runCollect(stream)
const array = Chunk.toArray(chunksChunk) // Use static Chunk.toArray
```

### 4. Layer Composition

**v2 Pattern**:
```typescript
// Complex layer merging
const testLayer = Layer.mergeAll(
  ServiceA.live,
  ServiceB.live,
  ServiceC.live
)
```

**v3 Pattern**:
```typescript
// Simplified Default pattern
Effect.provide(ServiceA.Default)
// Or for multiple services
Effect.provide(Layer.mergeAll(ServiceA.Default, ServiceB.Default))
```

## Migration Strategies

### 1. Service Migration

**Step 1: Update Service Definition**
```typescript
// Before (v2)
const StorageService = Context.GenericTag<StorageService>("StorageService")

// After (v3)
class StorageService extends Effect.Service<StorageService>()("StorageService", {
  sync: () => ({
    get: (key: string) => Effect.gen(function* () {
      // Implementation
    }),
    set: (key: string, value: string) => Effect.sync(() => {
      // Implementation
    })
  })
}) {}
```

**Step 2: Update Service Usage**
```typescript
// Service access remains the same
const service = yield* StorageService
const result = yield* service.get("key")
```

### 2. Test Migration

**Step 1: Remove TestClock Dependencies**
```typescript
// Remove these imports
// import { TestClock, TestContext } from "effect/TestClock"

// Replace with direct Effect testing
const testEffect = Effect.gen(function* () {
  const service = yield* MyService
  // Test implementation
}).pipe(Effect.provide(MyService.Default))
```

**Step 2: Simplify Test Utilities**
```typescript
// Create reusable test runner
export const ServiceTestUtils = {
  runServiceTest: <A, E>(
    description: string,
    effect: Effect.Effect<A, E, never>
  ) => {
    it(description, async () => {
      const result = await Effect.runPromise(effect)
      return result
    })
  }
}
```

### 3. Error Handling Migration

**v2 and v3 Consistent**:
```typescript
// Error handling patterns remain largely the same
Effect.gen(function* () {
  const result = yield* someOperation().pipe(Effect.either)
  
  if (result._tag === "Left") {
    // Handle error
  } else {
    // Handle success
  }
})
```

## Common Migration Issues

### 1. Service Constructor Confusion

**Problem**: Mixing v2 and v3 service patterns
```typescript
// Wrong - mixing patterns
const MyService = Context.GenericTag<MyService>("MyService")
class MyService extends Effect.Service<MyService>()("MyService", {
  // This won't work correctly
})
```

**Solution**: Use only v3 pattern
```typescript
// Correct v3 pattern
class MyService extends Effect.Service<MyService>()("MyService", {
  sync: () => ({
    // Methods here
  })
}) {}
```

### 2. TestClock Dependency Errors

**Problem**: Compilation errors with TestClock imports
```
error TS2307: Cannot find module 'effect/TestClock'
```

**Solution**: Remove TestClock completely
```typescript
// Remove
import { TestClock } from "effect/TestClock"

// Replace with direct Effect patterns
yield* Effect.sleep("10 millis") // For time-based testing
```

### 3. Stream Chunk Processing

**Problem**: Method not found on Chunk type
```
error TS2339: Property 'toArray' does not exist on type 'Chunk<T>'
```

**Solution**: Use static Chunk methods
```typescript
// Wrong
const array = chunk.toArray()

// Correct
const array = Chunk.toArray(chunk)
```

## Best Practices for v3

### 1. Service Design

```typescript
class MyService extends Effect.Service<MyService>()("MyService", {
  sync: () => {
    // Shared state can be initialized here
    let state = initialState
    
    return {
      // Pure methods
      getValue: () => Effect.succeed(state),
      
      // Side-effect methods
      setValue: (value: any) => Effect.sync(() => {
        state = value
      }),
      
      // Async methods
      fetchData: () => Effect.gen(function* () {
        const data = yield* Effect.tryPromise({
          try: () => fetch('/api/data'),
          catch: (error) => new NetworkError(error)
        })
        return data
      })
    }
  }
}) {}
```

### 2. Testing Patterns

```typescript
describe("MyService", () => {
  const testService = Effect.gen(function* () {
    const service = yield* MyService
    return service
  }).pipe(Effect.provide(MyService.Default))
  
  ServiceTestUtils.runServiceTest(
    "should handle basic operations",
    Effect.gen(function* () {
      const service = yield* MyService
      const result = yield* service.getValue()
      expect(result).toBeDefined()
      return result
    }).pipe(Effect.provide(MyService.Default))
  )
})
```

### 3. Error Handling

```typescript
// Define tagged errors
class MyServiceError extends Data.TaggedError("MyServiceError")<{
  operation: string
  cause: unknown
}> {}

// Use in service
class MyService extends Effect.Service<MyService>()("MyService", {
  sync: () => ({
    riskyOperation: () => Effect.gen(function* () {
      try {
        const result = yield* Effect.tryPromise({
          try: () => performOperation(),
          catch: (error) => new MyServiceError({
            operation: "performOperation",
            cause: error
          })
        })
        return result
      } catch (error) {
        yield* Effect.fail(new MyServiceError({
          operation: "riskyOperation",
          cause: error
        }))
      }
    })
  })
}) {}
```

## Performance Considerations

### Bundle Size Impact
- **v3 Service Pattern**: More efficient tree-shaking
- **Removed TestClock**: Reduces testing bundle size
- **Simplified Layers**: Faster runtime service resolution

### Runtime Performance
- **Service Creation**: v3 pattern is more efficient
- **Memory Usage**: Better garbage collection with simplified patterns
- **Type Checking**: Improved compile-time performance

## Migration Checklist

- [ ] Update all `Context.GenericTag` to `Effect.Service` pattern
- [ ] Remove all `TestClock` and `TestContext` imports
- [ ] Update `chunk.toArray()` to `Chunk.toArray(chunk)`
- [ ] Simplify Layer compositions where possible
- [ ] Update test utilities to use direct Effect patterns
- [ ] Verify all service dependencies use `.Default` pattern
- [ ] Test all error handling still works correctly
- [ ] Validate performance hasn't regressed

## Conclusion

The v2 to v3 migration requires systematic updates but results in cleaner, more performant code. The key is to migrate service definitions first, then update usage patterns, and finally simplify testing infrastructure.

The patterns established in this migration (particularly for Issue #1269) provide a solid foundation for future Effect-TS development in OpenAgents.