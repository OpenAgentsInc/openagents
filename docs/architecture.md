# OpenAgents Architecture Guide

## Overview

This document outlines the architectural principles and patterns used in the OpenAgents monorepo, built on Effect. The architecture follows Domain-Driven Design principles with clear separation of concerns across packages.

## Core Architectural Principles

### 1. Schema-First Development

The architecture prioritizes defining data structures and contracts before implementation:

- **API Contracts**: All APIs are defined using `@effect/platform` HttpApi with Schema validation
- **Type Safety**: Schemas provide runtime validation and compile-time type safety
- **Contract-Driven**: Domain defines contracts that other packages implement or consume

### 2. Effect Service Pattern

All major components are implemented as Effect Services:

```typescript
// Service definition pattern
export class MyService extends Effect.Service<MyService>()("namespace/MyService", {
  effect: Effect.gen(function*() {
    // Service implementation
    return { /* service interface */ }
  })
}) {}
```

Benefits:
- Dependency injection via Effect Layers
- Testability through service interfaces
- Clear service boundaries and contracts

### 3. Layer-Based Composition

Services are composed using Effect Layers:

```typescript
const ApiLive = HttpApiBuilder.api(TodosApi).pipe(
  Layer.provide(TodosApiLive)
)
```

This enables:
- Clean dependency management
- Environment-specific implementations
- Easy testing with mock layers

## Package Architecture

### Domain Package (`@openagentsinc/domain`)

**Purpose**: Define shared contracts, types, and API specifications

**What belongs here:**
- API contract definitions using HttpApi
- Shared data schemas and branded types
- Error types and domain exceptions
- Pure business logic rules
- Value objects and entities

**What doesn't belong here:**
- Implementation details
- External service integrations
- UI components
- Infrastructure code

**Example from codebase:**
```typescript
// TodosApi.ts - Domain defines the contract
export class TodosApiGroup extends HttpApiGroup.make("todos")
  .add(HttpApiEndpoint.get("getAllTodos", "/todos").addSuccess(Schema.Array(Todo)))
  .add(HttpApiEndpoint.post("createTodo", "/todos")
    .addSuccess(Todo)
    .setPayload(Schema.Struct({ text: Schema.NonEmptyTrimmedString })))
```

### Application-Specific Packages

These packages implement specific features or integrations:

#### Pylon Package (`@openagentsinc/pylon`)
- Nostr relay server implementation
- WebSocket server for Nostr protocol
- Event storage and subscription management
- Handles persistence and relay operations

#### CLI Package (`@openagentsinc/cli`)
- Consumes API contracts via HTTP clients
- Implements command-line interface
- Contains CLI-specific business logic

#### UI Package (`@openagentsinc/ui`)
- Shared React components
- Design system implementation
- UI utilities and hooks
- No business logic - pure presentation

#### Future Packages Pattern

For planned packages like `@openagentsinc/wallet`, `@openagentsinc/ai`, etc.:

**What belongs in specific packages:**
- Feature-specific implementations
- Integrations with external services
- Package-specific UI components
- Package-specific utilities

**What goes in domain:**
- Shared contracts between packages
- Common error types
- Shared value objects
- Cross-cutting business rules

## Decision Framework: Domain vs Specific Package

### Put in Domain When:

1. **Multiple packages need it**
   - Shared data types (User, Transaction, etc.)
   - Common API contracts
   - Shared business rules

2. **It defines a contract**
   - API specifications
   - Event schemas
   - Integration interfaces

3. **It's pure business logic**
   - Validation rules
   - Business calculations
   - Domain invariants

### Put in Specific Package When:

1. **Single package scope**
   - Feature-specific logic
   - Package-specific utilities
   - Local state management

2. **External integration**
   - Third-party API clients
   - Database repositories
   - Service adapters

3. **Implementation details**
   - How contracts are fulfilled
   - Infrastructure code
   - Package-specific optimizations

## Example: Wallet Package Architecture

```typescript
// In @openagentsinc/domain/WalletApi.ts
export class WalletTransaction extends Schema.Class<WalletTransaction>("WalletTransaction")({
  id: TransactionId,
  amount: Schema.Number,
  currency: Currency,
  timestamp: Schema.Date
}) {}

export class WalletApiGroup extends HttpApiGroup.make("wallet")
  .add(HttpApiEndpoint.get("getBalance", "/wallet/balance")
    .addSuccess(WalletBalance))
  .add(HttpApiEndpoint.post("createTransaction", "/wallet/transactions")
    .addSuccess(WalletTransaction)
    .setPayload(TransactionRequest))
{}

// In @openagentsinc/wallet/src/WalletService.ts
export class WalletService extends Effect.Service<WalletService>()("wallet/WalletService", {
  effect: Effect.gen(function*() {
    // Implementation specific to wallet functionality
    // Blockchain integration, key management, etc.
  })
}) {}
```

## Advanced Effect Patterns

### 1. Effect.gen for Readability

Use generators for sequential async operations:

```typescript
Effect.gen(function*() {
  const user = yield* UserService
  const wallet = yield* user.getWallet()
  return yield* wallet.getBalance()
})
```

### 2. Tagged Errors with Data.TaggedError

Define domain-specific errors with Schema:

```typescript
export class TodoNotFound extends Data.TaggedError("TodoNotFound")<{
  readonly id: number
}> {}

// Or with Schema for validation
export class ValidationError extends Schema.TaggedError<ValidationError>()("ValidationError", {
  field: Schema.String,
  message: Schema.String
}) {}
```

### 3. Service Pattern with Context

Use Effect.Service for dependency injection with proper context management:

```typescript
export class DatabaseService extends Effect.Service<DatabaseService>()("app/DatabaseService", {
  dependencies: [ConfigService.Default, LoggerService.Default],
  effect: Effect.gen(function*() {
    const config = yield* ConfigService
    const logger = yield* LoggerService

    // Service implementation
    return {
      query: (sql: string) => Effect.tryPromise({
        try: () => db.query(sql),
        catch: (error) => new DatabaseError({ error })
      })
    }
  })
}) {}

// Usage preserves context requirements
const program = Effect.gen(function*() {
  const db = yield* DatabaseService
  return yield* db.query("SELECT * FROM users")
})
```

### 4. Layer Composition and Context Management

Build application layers incrementally with proper context tracking:

```typescript
// Basic layer composition
const MainLayer = Layer.mergeAll(
  ApiLive,
  TodosRepository.Default,
  ConfigLive
)

// Context-aware resolver pattern
const GetTodosResolver = RequestResolver.fromEffect((_: GetTodos) =>
  Effect.andThen(HttpService, (http) =>
    Effect.tryPromise({
      try: () => http.fetch("/todos").then(res => res.json()),
      catch: () => new GetTodosError()
    })
  )
).pipe(RequestResolver.contextFromServices(HttpService))
```

### 5. Request Batching and Caching

Optimize API calls with Effect's batching capabilities:

```typescript
// Define batchable requests
interface GetUserById extends Request.Request<User, GetUserError> {
  readonly _tag: "GetUserById"
  readonly id: number
}

const GetUserById = Request.tagged<GetUserById>("GetUserById")

// Create batched resolver
const GetUserByIdResolver = RequestResolver.makeBatched(
  (requests: ReadonlyArray<GetUserById>) =>
    Effect.tryPromise({
      try: () => fetch("/users/batch", {
        method: "POST",
        body: JSON.stringify({ ids: requests.map(r => r.id) })
      }).then(res => res.json()),
      catch: () => new GetUserError()
    }).pipe(
      Effect.andThen((users) =>
        Effect.forEach(requests, (request, index) =>
          Request.completeEffect(request, Effect.succeed(users[index]))
        )
      )
    )
)

// Enable caching for requests
const getUserById = (id: number) =>
  Effect.request(GetUserById({ id }), GetUserByIdResolver).pipe(
    Effect.withRequestCaching(true)
  )
```

### 6. Resource Management with Scope

Ensure proper resource cleanup:

```typescript
const managedResource = Effect.acquireRelease(
  // Acquire
  Effect.sync(() => {
    console.log("Acquiring resource")
    return { resource: "database-connection" }
  }),
  // Release
  (resource) => Effect.sync(() => {
    console.log("Releasing resource")
    // Cleanup logic
  })
)

// Use with scoped
const program = Effect.scoped(
  Effect.gen(function*() {
    const resource = yield* managedResource
    // Use resource
    return yield* performOperation(resource)
  })
)
```

### 7. Stream Processing

Handle data streams efficiently:

```typescript
import { Stream } from "effect"

const processLargeDataset = Stream.fromIterable(largeDataset).pipe(
  Stream.mapEffect((item) => processItem(item)),
  Stream.tap((result) => Effect.log(`Processed: ${result.id}`)),
  Stream.buffer(100), // Buffer for performance
  Stream.runCollect
)
```

### 8. Structured Concurrency

Manage concurrent operations safely:

```typescript
// Concurrent execution with controlled parallelism
const processTodos = Effect.forEach(
  todos,
  (todo) => notifyOwner(todo),
  { concurrency: 10, batching: true }
)

// Race multiple operations
const fastestProvider = Effect.race(
  fetchFromProviderA(),
  fetchFromProviderB()
)

// Fork and join pattern
Effect.gen(function*() {
  const fiber1 = yield* Effect.fork(longRunningTask1)
  const fiber2 = yield* Effect.fork(longRunningTask2)

  const [result1, result2] = yield* Effect.all([
    Fiber.join(fiber1),
    Fiber.join(fiber2)
  ])
})
```

### 9. AI/LLM Integration Pattern

Integrate AI capabilities using @effect/ai:

```typescript
// Define AI model abstractly
const ChatModel = OpenAiLanguageModel.model("gpt-4o")

// Create AI service with provider-agnostic interface
export class AiService extends Effect.Service<AiService>()("app/AiService", {
  effect: Effect.gen(function*() {
    const model = yield* ChatModel

    return {
      generateText: (prompt: string) =>
        model.use(
          AiLanguageModel.generateText({ prompt })
        ),

      generateWithTools: (prompt: string) =>
        model.use(
          AiLanguageModel.generateText({
            prompt,
            tools: AppToolkit // Your AiToolkit
          })
        )
    }
  }),
  dependencies: [
    OpenAiClient.layerConfig({
      apiKey: Config.redacted("OPENAI_API_KEY")
    })
  ]
}) {}

// Define tools for AI
class AppToolkit extends AiToolkit.make(
  AiTool.make("SearchDatabase", {
    description: "Search application database",
    parameters: {
      query: Schema.String
    },
    success: Schema.Array(SearchResult),
    failure: Schema.Never
  })
) {}

// Implement tool handlers
const AppToolHandlers = AppToolkit.toLayer(
  Effect.gen(function*() {
    const db = yield* DatabaseService
    return {
      SearchDatabase: ({ query }) => db.search(query)
    }
  })
)
```

### 10. Configuration Management

Handle configuration with validation:

```typescript
const AppConfig = Schema.Struct({
  port: Schema.Number.pipe(Schema.between(1, 65535)),
  apiKey: Schema.Redacted(Schema.String),
  environment: Schema.Literal("development", "production"),
  database: Schema.Struct({
    host: Schema.String,
    port: Schema.Number,
    name: Schema.String
  })
})

const ConfigLive = Layer.effect(
  ConfigService,
  Config.all({
    port: Config.number("PORT").pipe(Config.withDefault(3000)),
    apiKey: Config.redacted("API_KEY"),
    environment: Config.literal("NODE_ENV")("development", "production"),
    database: Config.all({
      host: Config.string("DB_HOST"),
      port: Config.number("DB_PORT"),
      name: Config.string("DB_NAME")
    })
  }).pipe(
    Effect.map(Schema.decodeUnknownSync(AppConfig))
  )
)

## Best Practices

### 1. Package Independence
- Packages should be independently deployable
- Minimize inter-package dependencies
- Use domain for shared contracts only

### 2. Schema Evolution
- Version your schemas when breaking changes occur
- Use Schema transformations for backward compatibility
- Document schema changes in changesets

### 3. Testing Strategy
- Test domain logic with pure functions
- Use Layer composition for integration tests
- Mock external services at the service boundary

### 4. Error Handling
- Use tagged errors for domain errors
- Let Effect handle error propagation
- Provide meaningful error messages

### 5. Configuration
- Use Effect Config for environment variables
- Define config schemas in domain
- Provide config via Layers

## Performance Considerations

### 1. Request Optimization
- Use batching for multiple similar requests
- Enable caching for frequently accessed data
- Implement request deduplication
- Consider using `Effect.withRequestBatching(false)` for time-sensitive operations

### 2. Concurrency Control
```typescript
// Control concurrency to prevent resource exhaustion
const options = {
  concurrency: 10,      // Limit parallel operations
  batching: true,       // Enable request batching
  discard: false        // Keep all results
}

Effect.forEach(items, processItem, options)
```

### 3. Memory Management
- Use streams for large datasets
- Implement pagination for API responses
- Clean up resources with proper scoping
- Monitor fiber lifecycles in long-running applications

## AI-Specific Architecture Patterns

### For @openagentsinc/ai Package

**Domain Layer (shared contracts):**
```typescript
// In domain/src/ai/
export interface AiRequest extends Schema.Schema<AiRequest> {
  readonly prompt: string
  readonly model: ModelIdentifier
  readonly temperature?: number
}

export interface AiResponse extends Schema.Schema<AiResponse> {
  readonly text: string
  readonly usage: TokenUsage
  readonly model: ModelIdentifier
}
```

**AI Package Implementation:**
```typescript
// In packages/ai/src/
export class AiOrchestrator extends Effect.Service<AiOrchestrator>()("ai/AiOrchestrator", {
  effect: Effect.gen(function*() {
    // Provider-agnostic orchestration
    const providers = yield* AiProviders

    return {
      route: (request: AiRequest) =>
        // Route to appropriate provider based on model
        providers.getProvider(request.model).pipe(
          Effect.andThen(provider => provider.complete(request))
        )
    }
  })
}) {}
```

### For @openagentsinc/commander Package

**Integration with AI:**
```typescript
export class CommanderService extends Effect.Service<CommanderService>()("commander/CommanderService", {
  dependencies: [AiService.Default],
  effect: Effect.gen(function*() {
    const ai = yield* AiService

    return {
      executeCommand: (command: Command) =>
        Effect.gen(function*() {
          // Parse command intent using AI
          const intent = yield* ai.parseIntent(command)
          // Execute based on intent
          return yield* executeIntent(intent)
        })
    }
  })
}) {}
```

### For @openagentsinc/wallet Package

**Secure State Management:**
```typescript
export class WalletState extends Effect.Service<WalletState>()("wallet/WalletState", {
  effect: Effect.gen(function*() {
    const ref = yield* SubscriptionRef.make({
      balance: 0,
      transactions: []
    })

    return {
      updateBalance: (amount: number) =>
        SubscriptionRef.update(ref, state => ({
          ...state,
          balance: state.balance + amount
        })),

      subscribe: ref.changes
    }
  })
}) {}
```

## Testing Strategy Extensions

### 1. AI Service Testing
```typescript
const TestAiService = Layer.succeed(
  AiService,
  {
    generateText: () => Effect.succeed({
      text: "Test response",
      usage: { promptTokens: 10, completionTokens: 20 }
    })
  }
)

// Use in tests
const testProgram = program.pipe(
  Effect.provide(TestAiService)
)
```

### 2. Snapshot Testing for Schemas
```typescript
import { AST } from "@effect/schema"

test("schema compatibility", () => {
  const ast = AST.from(UserSchema)
  expect(ast).toMatchSnapshot()
})
```

### 3. Property-Based Testing
```typescript
import { Arbitrary } from "@effect/schema"

const userArb = Arbitrary.make(UserSchema)

test.prop([userArb])("user validation", (user) => {
  const encoded = Schema.encode(UserSchema)(user)
  const decoded = Schema.decode(UserSchema)(encoded)
  expect(decoded).toEqual(user)
})
```

## Observability and Monitoring

### 1. Structured Logging
```typescript
const tracedService = Effect.gen(function*() {
  yield* Effect.log("Starting operation", {
    level: "info",
    service: "wallet",
    operation: "transfer"
  })

  return yield* operation.pipe(
    Effect.tap(() => Effect.log("Operation completed")),
    Effect.tapError((error) =>
      Effect.log("Operation failed", { error, level: "error" })
    )
  )
})
```

### 2. Metrics Collection
```typescript
const metrics = Metric.counter("api_requests", {
  description: "API request count",
  unit: "count"
})

const tracked = operation.pipe(
  Effect.tap(() => Metric.increment(metrics)),
  Effect.withSpan("api.request", {
    attributes: { endpoint: "/users" }
  })
)
```

### 3. Distributed Tracing
```typescript
const program = Effect.gen(function*() {
  yield* Effect.annotateCurrentSpan({
    "user.id": userId,
    "request.id": requestId
  })

  return yield* processRequest(request)
}).pipe(Effect.withSpan("process.request"))
```

## Migration Guide

When adding new functionality:

1. **Identify the scope**: Is it shared or package-specific?
2. **Define contracts first**: Create schemas in domain if shared
3. **Design for testability**: Use dependency injection via services
4. **Implement in packages**: Follow the service pattern
5. **Consider performance**: Apply batching/caching where appropriate
6. **Compose with Layers**: Wire dependencies properly
7. **Add comprehensive tests**: Unit, integration, and property-based
8. **Document patterns**: Update this guide with new patterns

## Security Considerations

### 1. Sensitive Data Handling
```typescript
const SecureConfig = Schema.Struct({
  apiKey: Schema.Redacted(Schema.String),
  privateKey: Schema.Redacted(Schema.String)
})

// Redacted values won't appear in logs
Effect.log("Config loaded", { config })
// Output: Config loaded { config: { apiKey: <redacted>, privateKey: <redacted> } }
```

### 2. Input Validation
```typescript
const validateInput = <A, I>(schema: Schema.Schema<A, I>) =>
  (input: unknown) =>
    Schema.decode(schema)(input).pipe(
      Effect.mapError(error => new ValidationError({ error }))
    )
```

### 3. Rate Limiting
```typescript
const rateLimiter = Semaphore.make(10).pipe(
  Effect.andThen(semaphore => ({
    withLimit: <A, E, R>(effect: Effect.Effect<A, E, R>) =>
      semaphore.withPermit(effect)
  }))
)
```

## Conclusion

This architecture provides:
- Clear separation of concerns
- Type-safe contracts with runtime validation
- Testable components through dependency injection
- Scalable package structure
- Consistent patterns across the codebase
- Performance optimization through batching and caching
- Provider-agnostic AI integration
- Comprehensive error handling and observability

The Effect ecosystem provides a robust foundation for building complex distributed systems while maintaining code clarity, type safety, and performance. The patterns outlined here scale from simple CRUD operations to complex AI orchestration and real-time systems.
