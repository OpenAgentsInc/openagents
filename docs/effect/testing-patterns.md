# Effect-TS Testing Patterns & Best Practices

This document outlines comprehensive testing patterns for Effect-TS services in OpenAgents, based on the implementation of service-level testing for Issue #1269. It provides proven patterns for testing Effect services with high coverage and reliability.

## Overview

Effect-TS testing requires different patterns than traditional Promise-based code. This guide covers:
- Service testing infrastructure
- Test organization patterns
- Performance benchmarking
- Error scenario testing
- Integration testing approaches

## Testing Infrastructure

### Core Test Utilities

```typescript
// setup-service-tests.ts
export const ServiceTestUtils = {
  /**
   * Runs an Effect-based test with proper error handling
   */
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

/**
 * Performance benchmarking utility
 */
export const benchmarkEffect = <A, E>(
  name: string,
  effect: Effect.Effect<A, E, never>,
  maxTimeMs: number
) => Effect.gen(function* () {
  const startTime = Date.now()
  const result = yield* effect
  const duration = Date.now() - startTime
  
  yield* Effect.logInfo(`Benchmark '${name}': ${duration}ms`)
  
  if (duration > maxTimeMs) {
    yield* Effect.fail(new Error(`Benchmark '${name}' exceeded ${maxTimeMs}ms (took ${duration}ms)`))
  }
  
  return result
})
```

### Service Test Template

```typescript
import { Effect } from "effect"
import { describe, it, expect, vi, beforeEach } from "vitest"
import { ServiceTestUtils, benchmarkEffect } from "./setup-service-tests"

// Main service implementation
class TestMyService extends Effect.Service<TestMyService>()("TestMyService", {
  sync: () => {
    // Shared state for the service instance
    let state = {}
    
    return {
      // Service methods
      method1: () => Effect.succeed("result"),
      method2: (input: string) => Effect.gen(function* () {
        // Implementation
      })
    }
  }
}) {}

// Error simulation service
class FailingMyService extends Effect.Service<FailingMyService>()("FailingMyService", {
  sync: () => ({
    method1: () => Effect.fail(new Error("Method 1 failed")),
    method2: (input: string) => Effect.fail(new Error("Method 2 failed"))
  })
}) {}

describe("MyService Tests", () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe("Basic Functionality", () => {
    ServiceTestUtils.runServiceTest(
      "should handle basic operations",
      Effect.gen(function* () {
        const service = yield* TestMyService
        const result = yield* service.method1()
        expect(result).toBe("result")
        return result
      }).pipe(Effect.provide(TestMyService.Default))
    )
  })

  describe("Error Handling", () => {
    ServiceTestUtils.runServiceTest(
      "should handle service failures",
      Effect.gen(function* () {
        const failingService = yield* FailingMyService
        const result = yield* failingService.method1().pipe(Effect.either)
        
        expect(result._tag).toBe("Left")
        if (result._tag === "Left") {
          expect(result.left.message).toBe("Method 1 failed")
        }
        
        return result
      }).pipe(Effect.provide(FailingMyService.Default))
    )
  })

  describe("Performance Benchmarks", () => {
    ServiceTestUtils.runServiceTest(
      "should meet performance requirements",
      benchmarkEffect(
        "Method Performance",
        Effect.gen(function* () {
          const service = yield* TestMyService
          return yield* service.method1()
        }).pipe(Effect.provide(TestMyService.Default)),
        100 // Should complete within 100ms
      )
    )
  })
})
```

## Testing Patterns by Service Type

### 1. Storage Service Testing

```typescript
describe("Storage Service", () => {
  ServiceTestUtils.runServiceTest(
    "should handle cross-platform storage",
    Effect.gen(function* () {
      const storage = yield* StorageService
      
      // Test web platform
      yield* storage.setStorageValue("test-key", "web-value", "web")
      const webResult = yield* storage.getStorageValue("test-key", "web")
      expect(webResult).toBe("web-value")
      
      // Test mobile platform
      yield* storage.setStorageValue("test-key", "mobile-value", "mobile")
      const mobileResult = yield* storage.getStorageValue("test-key", "mobile")
      expect(mobileResult).toBe("mobile-value")
      
      // Verify platform isolation
      expect(webResult).not.toBe(mobileResult)
      
      return { webResult, mobileResult }
    }).pipe(Effect.provide(TestStorageService.Default))
  )

  ServiceTestUtils.runServiceTest(
    "should handle JSON serialization errors",
    Effect.gen(function* () {
      const storage = yield* StorageService
      
      // Manually store invalid JSON
      yield* storage.setInLocalStorage("invalid-json", "{ invalid json }")
      
      // Try to retrieve as JSON
      const result = yield* storage.getStoredJson("invalid-json", "web").pipe(Effect.either)
      
      expect(result._tag).toBe("Left")
      if (result._tag === "Left") {
        expect(result.left.message).toContain("Invalid JSON")
      }
      
      return result
    }).pipe(Effect.provide(TestStorageService.Default))
  )
})
```

### 2. Authentication Service Testing

```typescript
describe("Auth Service", () => {
  ServiceTestUtils.runServiceTest(
    "should support complete OAuth workflow",
    Effect.gen(function* () {
      const auth = yield* AuthService
      
      // 1. Get OAuth configuration
      const config = yield* auth.getOAuthConfig()
      expect(config.clientId).toBeTruthy()
      
      // 2. Start OAuth flow
      const oauthFlow = yield* auth.startOAuthFlow("mobile")
      expect(oauthFlow.authUrl).toContain("platform=mobile")
      
      // 3. Exchange code for token
      const tokenResult = yield* auth.exchangeCodeForToken("auth-code-123", oauthFlow.state)
      expect(tokenResult.access_token).toBeTruthy()
      
      // 4. Store auth data
      yield* auth.storeAuthData(tokenResult.access_token, tokenResult.user)
      
      // 5. Verify authentication state
      const authState = yield* auth.checkStoredAuth()
      expect(authState.isAuthenticated).toBe(true)
      
      // 6. Logout
      const logoutResult = yield* auth.logout()
      expect(logoutResult.isAuthenticated).toBe(false)
      
      return { config, tokenResult, authState, logoutResult }
    }).pipe(Effect.provide(TestAuthService.Default))
  )
})
```

### 3. Streaming Service Testing

```typescript
describe("Streaming Service", () => {
  ServiceTestUtils.runServiceTest(
    "should handle streaming workflow",
    Effect.gen(function* () {
      const claude = yield* StreamingService
      
      // Create session
      const session = yield* claude.createSession("test-session")
      expect(session.isActive).toBe(true)
      
      // Start streaming
      const responseStream = yield* claude.streamResponse("test-session", "Hello!")
      
      // Collect stream chunks
      const chunksChunk = yield* Stream.runCollect(responseStream)
      const chunks = Chunk.toArray(chunksChunk)
      
      expect(chunks.length).toBeGreaterThan(0)
      expect(chunks[0].type).toBe("metadata")
      
      // Verify session state
      const messages = yield* claude.getMessages("test-session")
      expect(messages.length).toBeGreaterThan(0)
      
      return { session, chunks, messages }
    }).pipe(Effect.provide(TestStreamingService.Default))
  )

  ServiceTestUtils.runServiceTest(
    "should handle concurrent streaming sessions",
    Effect.gen(function* () {
      const claude = yield* StreamingService
      
      // Create multiple sessions concurrently
      const sessionIds = ["concurrent-1", "concurrent-2", "concurrent-3"]
      const sessionCreations = sessionIds.map(id => claude.createSession(id))
      
      const sessions = yield* Effect.all(sessionCreations, { concurrency: 3 })
      expect(sessions).toHaveLength(3)
      
      // Stream responses concurrently
      const streamingOperations = sessionIds.map(id => 
        claude.streamResponse(id, `Message to ${id}`)
      )
      
      const streams = yield* Effect.all(streamingOperations, { concurrency: 3 })
      expect(streams).toHaveLength(3)
      
      return { sessions, streams }
    }).pipe(Effect.provide(TestStreamingService.Default))
  )
})
```

## Advanced Testing Patterns

### 1. State Management Testing

```typescript
ServiceTestUtils.runServiceTest(
  "should maintain consistent state across operations",
  Effect.gen(function* () {
    const service = yield* StatefulService
    
    // Initial state
    const initial = yield* service.getState()
    expect(initial).toEqual(defaultState)
    
    // Concurrent state updates
    const updates = Array.from({ length: 10 }, (_, i) =>
      service.updateState(`update-${i}`)
    )
    
    yield* Effect.all(updates, { concurrency: 5 })
    
    // Verify final state consistency
    const final = yield* service.getState()
    expect(final.updateCount).toBe(10)
    
    return { initial, final }
  }).pipe(Effect.provide(StatefulService.Default))
)
```

### 2. Resource Cleanup Testing

```typescript
ServiceTestUtils.runServiceTest(
  "should cleanup resources properly",
  Effect.gen(function* () {
    const service = yield* ResourceService
    
    // Create resources
    const resource1 = yield* service.createResource("resource-1")
    const resource2 = yield* service.createResource("resource-2")
    
    // Verify resources exist
    const beforeCleanup = yield* service.listResources()
    expect(beforeCleanup).toHaveLength(2)
    
    // Cleanup all resources
    yield* service.cleanup()
    
    // Verify resources are gone
    const afterCleanup = yield* service.listResources()
    expect(afterCleanup).toHaveLength(0)
    
    return { beforeCleanup, afterCleanup }
  }).pipe(Effect.provide(ResourceService.Default))
)
```

### 3. Time-Based Testing

```typescript
ServiceTestUtils.runServiceTest(
  "should handle time-based operations",
  Effect.gen(function* () {
    const service = yield* TimerService
    
    const startTime = Date.now()
    
    // Start timer
    yield* service.startTimer("test-timer", 100) // 100ms
    
    // Wait for completion
    yield* Effect.sleep("150 millis")
    
    // Check timer completion
    const status = yield* service.getTimerStatus("test-timer")
    const endTime = Date.now()
    
    expect(status.completed).toBe(true)
    expect(endTime - startTime).toBeGreaterThan(100)
    
    return { startTime, endTime, status }
  }).pipe(Effect.provide(TimerService.Default))
)
```

## Performance Testing

### Benchmark Requirements

```typescript
// Performance requirements by service type
const PERFORMANCE_REQUIREMENTS = {
  STORAGE: {
    READ: 50,    // ms
    WRITE: 100,  // ms
    DELETE: 50   // ms
  },
  AUTH: {
    TOKEN_CHECK: 100,    // ms
    TOKEN_EXCHANGE: 200, // ms
    LOGOUT: 50           // ms
  },
  STREAMING: {
    SESSION_CREATE: 100, // ms
    MESSAGE_SEND: 200,   // ms
    STREAM_COLLECT: 500  // ms
  },
  APM: {
    DEVICE_ID: 100,     // ms
    TRACK_ACTION: 50,   // ms
    GET_METRICS: 100    // ms
  }
}
```

### Benchmark Implementation

```typescript
describe("Performance Benchmarks", () => {
  Object.entries(PERFORMANCE_REQUIREMENTS.STORAGE).forEach(([operation, maxTime]) => {
    ServiceTestUtils.runServiceTest(
      `${operation} should be fast`,
      benchmarkEffect(
        operation,
        Effect.gen(function* () {
          const storage = yield* StorageService
          
          switch (operation) {
            case 'READ':
              yield* storage.setStorageValue("bench-key", "bench-value")
              return yield* storage.getStorageValue("bench-key")
            
            case 'WRITE':
              return yield* storage.setStorageValue("bench-key", "bench-value")
            
            case 'DELETE':
              yield* storage.setStorageValue("bench-key", "bench-value")
              return yield* storage.removeStorageValue("bench-key")
          }
        }).pipe(Effect.provide(StorageService.Default)),
        maxTime
      )
    )
  })
})
```

## Error Testing Patterns

### 1. Comprehensive Error Coverage

```typescript
describe("Error Scenarios", () => {
  const errorScenarios = [
    { name: "Network failure", service: "networkService", method: "fetch" },
    { name: "Storage full", service: "storageService", method: "write" },
    { name: "Invalid input", service: "validationService", method: "validate" }
  ]

  errorScenarios.forEach(({ name, service, method }) => {
    ServiceTestUtils.runServiceTest(
      `should handle ${name}`,
      Effect.gen(function* () {
        const failingService = yield* FailingService
        const result = yield* failingService[method]().pipe(Effect.either)
        
        expect(result._tag).toBe("Left")
        if (result._tag === "Left") {
          expect(result.left.message).toContain(name.toLowerCase())
        }
        
        return result
      }).pipe(Effect.provide(FailingService.Default))
    )
  })
})
```

### 2. Error Recovery Testing

```typescript
ServiceTestUtils.runServiceTest(
  "should recover from transient failures",
  Effect.gen(function* () {
    const service = yield* RetryService
    
    // This should fail initially but succeed on retry
    const result = yield* service.unreliableOperation().pipe(
      Effect.retry(Schedule.exponential("100 millis").pipe(Schedule.recurs(3)))
    )
    
    expect(result).toBeTruthy()
    
    return result
  }).pipe(Effect.provide(RetryService.Default))
)
```

## Integration Testing

### Service Interaction Testing

```typescript
describe("Integration Tests", () => {
  ServiceTestUtils.runServiceTest(
    "should coordinate multiple services",
    Effect.gen(function* () {
      const auth = yield* AuthService
      const storage = yield* StorageService
      const apm = yield* APMService
      
      // 1. Track login attempt
      yield* apm.trackAction("login_attempt")
      
      // 2. Perform authentication
      const authResult = yield* auth.exchangeCodeForToken("valid-code", "state")
      
      // 3. Store auth data
      yield* storage.setStoredJson("auth", authResult)
      
      // 4. Track successful login
      yield* apm.trackAction("login_success")
      
      // 5. Verify complete workflow
      const storedAuth = yield* storage.getStoredJson("auth")
      const metrics = yield* apm.getSessionMetrics()
      
      expect(storedAuth).toEqual(authResult)
      expect(metrics.actionsCount).toBe(2)
      
      return { authResult, storedAuth, metrics }
    }).pipe(
      Effect.provide(
        Layer.mergeAll(
          AuthService.Default,
          StorageService.Default,
          APMService.Default
        )
      )
    )
  )
})
```

## Best Practices

### 1. Test Organization

```typescript
describe("ServiceName Tests", () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe("Core Functionality", () => {
    // Basic feature tests
  })

  describe("Error Handling", () => {
    // Error scenario tests
  })

  describe("Performance Benchmarks", () => {
    // Performance requirement tests
  })

  describe("Integration Tests", () => {
    // Cross-service interaction tests
  })

  describe("Concurrent Operations", () => {
    // Thread safety and concurrency tests
  })

  describe("Cleanup", () => {
    // Resource cleanup tests
  })
})
```

### 2. Test Data Management

```typescript
// Test data factories
const createTestUser = (overrides: Partial<User> = {}): User => ({
  id: "test-user-123",
  email: "test@example.com",
  name: "Test User",
  ...overrides
})

const createTestSession = (overrides: Partial<Session> = {}): Session => ({
  id: "test-session-" + Math.random().toString(36).substr(2, 9),
  isActive: true,
  messages: [],
  lastActivity: Date.now(),
  ...overrides
})
```

### 3. Assertion Patterns

```typescript
// Effect-specific assertions
const assertEffectSuccess = <A>(effect: Effect.Effect<A, any, any>) =>
  Effect.gen(function* () {
    const result = yield* effect.pipe(Effect.either)
    expect(result._tag).toBe("Right")
    return result._tag === "Right" ? result.right : undefined
  })

const assertEffectFailure = <E>(effect: Effect.Effect<any, E, any>) =>
  Effect.gen(function* () {
    const result = yield* effect.pipe(Effect.either)
    expect(result._tag).toBe("Left")
    return result._tag === "Left" ? result.left : undefined
  })
```

## Conclusion

These testing patterns provide comprehensive coverage for Effect-TS services while maintaining high performance and reliability. The key principles are:

1. **Service Isolation**: Each test should run with independent service instances
2. **Error Coverage**: Test both success and failure scenarios comprehensively
3. **Performance Validation**: Include benchmarks for all critical operations
4. **Integration Testing**: Verify cross-service interactions work correctly
5. **Resource Management**: Always test cleanup and resource disposal

Following these patterns ensures robust, maintainable tests that catch issues early and provide confidence in service reliability.