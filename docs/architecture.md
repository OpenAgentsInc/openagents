# OpenAgents Architecture Guide

## Overview

This document outlines the architectural principles and patterns used in the OpenAgents monorepo, built on Effect.js. The architecture follows Domain-Driven Design principles with clear separation of concerns across packages.

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

#### Server Package (`@openagentsinc/server`)
- Implements API contracts from domain
- Contains repository implementations
- Manages HTTP server setup
- Handles persistence and external services

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

## Effect.js Patterns

### 1. Effect.gen for Readability

Use generators for sequential async operations:

```typescript
Effect.gen(function*() {
  const user = yield* UserService
  const wallet = yield* user.getWallet()
  return yield* wallet.getBalance()
})
```

### 2. Tagged Errors

Define domain-specific errors with Schema:

```typescript
export class TodoNotFound extends Schema.TaggedError<TodoNotFound>()("TodoNotFound", {
  id: Schema.Number
}) {}
```

### 3. Service Dependencies

Use Effect.Service for dependency injection:

```typescript
const program = Effect.gen(function*() {
  const todos = yield* TodosRepository
  return yield* todos.getAll
})
```

### 4. Layer Composition

Build application layers incrementally:

```typescript
const MainLayer = Layer.mergeAll(
  ApiLive,
  TodosRepository.Default,
  ConfigLive
)
```

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

## Migration Guide

When adding new functionality:

1. **Identify the scope**: Is it shared or package-specific?
2. **Define contracts first**: Create schemas in domain if shared
3. **Implement in packages**: Follow the service pattern
4. **Compose with Layers**: Wire dependencies properly
5. **Add tests**: Test at appropriate boundaries

## Conclusion

This architecture provides:
- Clear separation of concerns
- Type-safe contracts
- Testable components
- Scalable package structure
- Consistent patterns across the codebase

The Effect.js service pattern combined with schema-first development creates a robust foundation for building complex distributed systems while maintaining code clarity and type safety.