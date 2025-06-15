# OpenAgents Nostr Integration Plan

## Executive Summary

This document outlines the comprehensive plan for integrating Nostr protocol support into the OpenAgents monorepo. We will rename the existing 'server' package to 'relay' (to serve as our Nostr relay implementation) and create a new '@openagentsinc/nostr' package that provides Effect-based implementations of Nostr NIPs (Nostr Implementation Possibilities).

The integration leverages Effect's powerful abstractions to create a type-safe, composable, and highly testable Nostr implementation that surpasses existing libraries in terms of developer experience and reliability.

## Architecture Overview

### Package Structure

```
packages/
├── relay/          # Renamed from 'server' - Nostr relay implementation
├── nostr/          # New package - Effect-based NIP implementations
├── domain/         # Extended with Nostr schemas and contracts
└── ...
```

### Design Principles

1. **NIP-Centric Organization**: Each NIP is implemented as a separate Effect Layer/Service
2. **Schema-First Development**: All Nostr data structures defined using Effect Schema
3. **Type-Safe Error Handling**: Leveraging Effect's typed errors for all failure modes
4. **Resource Safety**: Using Effect's Resource/Scope for WebSocket and connection management
5. **Stream-Based Subscriptions**: Effect Streams for real-time event handling
6. **Comprehensive Testing**: Effect-based test utilities with ephemeral relay

## @openagentsinc/nostr Package Design

### Core Architecture

```typescript
// packages/nostr/src/index.ts
export * from './nip01'  // Core protocol
export * from './nip02'  // Contact lists
export * from './nip04'  // Encrypted messages
export * from './nip05'  // DNS identifiers
// ... more NIPs

export * from './core/Event'
export * from './core/Filter'
export * from './core/Relay'
export * from './services'
```

### Service Architecture

#### Core Services

```typescript
// EventService - Core event operations
export class EventService extends Effect.Service<EventService>()("nostr/EventService", {
  effect: Effect.gen(function*() {
    const crypto = yield* CryptoService

    return {
      create: (params: EventParams) => Effect.gen(function*() {
        // Validate and create event
        const event = yield* Schema.decode(NostrEventSchema)(params)
        const id = yield* crypto.hash(event)
        const sig = yield* crypto.sign(event)
        return { ...event, id, sig }
      }),

      verify: (event: NostrEvent) => Effect.gen(function*() {
        const valid = yield* crypto.verifySignature(event)
        if (!valid) return yield* new InvalidSignature({ event })
        return event
      })
    }
  })
}) {}

// RelayService - Relay connection management
export class RelayService extends Effect.Service<RelayService>()("nostr/RelayService", {
  effect: Effect.gen(function*() {
    const subs = new Map<string, Subscription>()

    return {
      connect: (url: string) => Resource.make(
        WebSocketService.connect(url),
        (ws) => WebSocketService.close(ws)
      ),

      subscribe: (filters: Filter[]) => Stream.async<NostrEvent>((emit) => {
        // Implementation
      })
    }
  })
}) {}

// CryptoService - Cryptographic operations
export class CryptoService extends Effect.Service<CryptoService>()("nostr/CryptoService", {
  effect: Effect.gen(function*() {
    return {
      generateKeyPair: () => Effect.sync(() => {
        const privateKey = generatePrivateKey()
        const publicKey = getPublicKey(privateKey)
        return { privateKey, publicKey }
      }),

      sign: (event: UnsignedEvent, privateKey: string) =>
        Effect.try(() => schnorr.sign(event.id, privateKey)),

      verifySignature: (event: NostrEvent) =>
        Effect.try(() => schnorr.verify(event.sig, event.id, event.pubkey))
    }
  })
}) {}
```

### Schema Definitions

```typescript
// Core event schema
export const NostrEventSchema = Schema.Struct({
  id: Schema.String.pipe(
    Schema.pattern(/^[0-9a-f]{64}$/),
    Schema.brand("EventId")
  ),
  pubkey: Schema.String.pipe(
    Schema.pattern(/^[0-9a-f]{64}$/),
    Schema.brand("PublicKey")
  ),
  created_at: Schema.Number.pipe(Schema.int()),
  kind: Schema.Number.pipe(Schema.int(), Schema.between(0, 65535)),
  tags: Schema.Array(Schema.Array(Schema.String)),
  content: Schema.String,
  sig: Schema.String.pipe(
    Schema.pattern(/^[0-9a-f]{128}$/),
    Schema.brand("Signature")
  )
})

// Filter schema
export const FilterSchema = Schema.Struct({
  ids: Schema.optional(Schema.Array(EventId)),
  authors: Schema.optional(Schema.Array(PublicKey)),
  kinds: Schema.optional(Schema.Array(Schema.Number)),
  since: Schema.optional(Schema.Number),
  until: Schema.optional(Schema.Number),
  limit: Schema.optional(Schema.Number),
  // Tag filters
  "#e": Schema.optional(Schema.Array(EventId)),
  "#p": Schema.optional(Schema.Array(PublicKey)),
  // ... more tag filters
})

// Message schemas
export const ClientMessageSchema = Schema.Union(
  Schema.Tuple(
    Schema.Literal("EVENT"),
    NostrEventSchema
  ),
  Schema.Tuple(
    Schema.Literal("REQ"),
    Schema.String.pipe(Schema.brand("SubscriptionId")),
    Schema.Array(FilterSchema)
  ),
  Schema.Tuple(
    Schema.Literal("CLOSE"),
    Schema.String.pipe(Schema.brand("SubscriptionId"))
  )
)
```

### NIP Implementation Pattern

Each NIP follows a consistent pattern:

```typescript
// packages/nostr/src/nip01/index.ts
export * from './Event'
export * from './Filter'
export * from './Message'
export * from './Relay'

// Layer composition
export const Nip01Live = Layer.mergeAll(
  EventService.Default,
  RelayService.Default,
  CryptoService.Default
)

// packages/nostr/src/nip04/index.ts
export class Nip04Service extends Effect.Service<Nip04Service>()("nostr/Nip04Service", {
  dependencies: [CryptoService.Default],
  effect: Effect.gen(function*() {
    const crypto = yield* CryptoService

    return {
      encrypt: (content: string, recipientPubkey: string, senderPrivkey: string) =>
        Effect.gen(function*() {
          const sharedSecret = yield* crypto.computeSharedSecret(recipientPubkey, senderPrivkey)
          const encrypted = yield* crypto.aesEncrypt(content, sharedSecret)
          return encrypted
        }),

      decrypt: (content: string, senderPubkey: string, recipientPrivkey: string) =>
        Effect.gen(function*() {
          const sharedSecret = yield* crypto.computeSharedSecret(senderPubkey, recipientPrivkey)
          const decrypted = yield* crypto.aesDecrypt(content, sharedSecret)
          return decrypted
        })
    }
  })
}) {}
```

### Testing Infrastructure

```typescript
// Ephemeral relay for testing
export class EphemeralRelayService extends Effect.Service<EphemeralRelayService>()("test/EphemeralRelayService", {
  effect: Effect.gen(function*() {
    const events = new Map<string, NostrEvent>()
    const subscriptions = new Map<string, Subscription>()

    return {
      start: () => Effect.acquireRelease(
        Effect.sync(() => {
          const server = createWebSocketServer({ port: 0 })
          // Implementation
          return server
        }),
        (server) => Effect.sync(() => server.close())
      ),

      // Test helpers
      getStoredEvents: () => Effect.succeed(Array.from(events.values())),
      clearEvents: () => Effect.sync(() => events.clear())
    }
  })
}) {}

// Test utilities
export const withEphemeralRelay = <A, E, R>(
  effect: Effect.Effect<A, E, R>
): Effect.Effect<A, E, Exclude<R, EphemeralRelayService>> =>
  Effect.scoped(
    Effect.gen(function*() {
      const relay = yield* EphemeralRelayService
      const server = yield* relay.start()
      return yield* effect
    })
  ).pipe(Effect.provide(EphemeralRelayService.Test))
```

## Relay (Relay) Implementation

### Architecture Updates

The existing 'server' package will be renamed to 'relay' and extended with Nostr relay capabilities:

```typescript
// packages/relay/src/NostrRelay.ts
export class NostrRelayService extends Effect.Service<NostrRelayService>()("relay/NostrRelayService", {
  dependencies: [
    DatabaseService.Default,
    WebSocketService.Default,
    ConfigService.Default
  ],
  effect: Effect.gen(function*() {
    const db = yield* DatabaseService
    const ws = yield* WebSocketService
    const config = yield* ConfigService

    return {
      handleConnection: (socket: WebSocket) => Effect.gen(function*() {
        const clientId = yield* generateClientId()
        
        yield* Stream.fromWebSocket(socket).pipe(
          Stream.mapEffect((message) => handleMessage(clientId, message)),
          Stream.runDrain
        )
      }),

      handleMessage: (clientId: string, message: NostrMessage) =>
        Effect.gen(function*() {
          switch (message[0]) {
            case "EVENT":
              return yield* handleEvent(message[1])
            case "REQ":
              return yield* handleSubscription(clientId, message[1], message.slice(2))
            case "CLOSE":
              return yield* handleClose(clientId, message[1])
          }
        })
    }
  })
}) {}
```

### Database Schema

```typescript
// Event storage with Effect SQL
export const EventRepository = Repository.make("events", {
  id: Schema.String.pipe(Schema.brand("EventId")),
  pubkey: Schema.String.pipe(Schema.brand("PublicKey")),
  created_at: Schema.Number,
  kind: Schema.Number,
  content: Schema.String,
  sig: Schema.String.pipe(Schema.brand("Signature")),
  tags: Schema.Array(Schema.Array(Schema.String)),
  deleted_at: Schema.Option(Schema.Number)
})

// Tag indexing
export const TagRepository = Repository.make("event_tags", {
  event_id: Schema.String.pipe(Schema.brand("EventId")),
  tag_name: Schema.String,
  tag_value: Schema.String,
  tag_order: Schema.Number
})
```

## Implementation Phases

### Phase 1: Foundation (Weeks 1-2)

1. **Package Setup**
   - Rename 'server' to 'relay'
   - Create '@openagentsinc/nostr' package
   - Set up build configuration

2. **Core Schemas**
   - Define all NIP-01 schemas in Effect Schema
   - Create branded types for type safety
   - Add validation utilities

3. **Basic Services**
   - Implement CryptoService
   - Implement EventService
   - Create test infrastructure with EphemeralRelay

### Phase 2: NIP-01 Implementation (Weeks 3-4)

1. **Relay Client**
   - WebSocket connection management
   - Message serialization/deserialization
   - Subscription handling with Effect Streams

2. **Relay Relay**
   - WebSocket server setup
   - Event storage and retrieval
   - Filter matching logic
   - Basic relay information (NIP-11)

3. **Testing**
   - Unit tests for all components
   - Integration tests with ephemeral relay
   - Performance benchmarks

### Phase 3: Essential NIPs (Weeks 5-8)

1. **Privacy & Security**
   - NIP-04: Encrypted Direct Messages
   - NIP-44: Versioned Encryption
   - NIP-42: Authentication

2. **Usability**
   - NIP-05: DNS Identifiers
   - NIP-19: Bech32 Encoding
   - NIP-02: Contact Lists

3. **Advanced Features**
   - NIP-09: Event Deletion
   - NIP-13: Proof of Work
   - NIP-28: Public Chat

### Phase 4: Integration (Weeks 9-10)

1. **Domain Integration**
   - Add Nostr schemas to domain package
   - Define cross-package contracts
   - Create Nostr API endpoints

2. **CLI Integration**
   - Add Nostr commands to CLI
   - Event publishing tools
   - Relay management

3. **Documentation**
   - API documentation
   - Usage examples
   - Migration guides

## Testing Strategy

### Unit Testing

```typescript
describe("EventService", () => {
  it("creates valid events", () =>
    Effect.gen(function*() {
      const eventService = yield* EventService
      const event = yield* eventService.create({
        kind: 1,
        content: "Hello Nostr!",
        tags: []
      })

      expect(event.id).toMatch(/^[0-9a-f]{64}$/)
      expect(event.sig).toMatch(/^[0-9a-f]{128}$/)
    }).pipe(
      Effect.provide(EventService.Test),
      Effect.runPromise
    )
  )
})
```

### Integration Testing

```typescript
describe("Relay Integration", () => {
  it("handles subscriptions", () =>
    withEphemeralRelay(
      Effect.gen(function*() {
        const relay = yield* RelayService
        const conn = yield* relay.connect("ws://localhost:8080")
        
        const events = yield* relay.subscribe([
          { kinds: [1], limit: 10 }
        ]).pipe(
          Stream.take(10),
          Stream.runCollect
        )

        expect(events).toHaveLength(10)
      })
    )
  )
})
```

## Performance Considerations

1. **Connection Pooling**: Reuse WebSocket connections across subscriptions
2. **Event Deduplication**: Use bloom filters for quick duplicate detection
3. **Query Optimization**: Index tags for efficient filtering
4. **Stream Buffering**: Use Effect's buffering strategies for high throughput
5. **Concurrent Processing**: Leverage Effect's fiber-based concurrency

## Security Considerations

1. **Input Validation**: All inputs validated with Effect Schema
2. **Rate Limiting**: Built-in rate limiting using Effect's Semaphore
3. **Authentication**: NIP-42 implementation with Effect's security patterns
4. **Key Management**: Secure key storage with Effect's Redacted type
5. **Error Handling**: No sensitive data in error messages

## Migration Path

### From nostr-tools

```typescript
// Before (nostr-tools)
import { SimplePool } from 'nostr-tools'
const pool = new SimplePool()
const events = await pool.querySync(relays, filters)

// After (Effect)
import { RelayPool } from '@openagentsinc/nostr'
const events = yield* RelayPool.query(relays, filters).pipe(
  Effect.provide(RelayPoolLive)
)
```

### From NDK

```typescript
// Before (NDK)
const ndk = new NDK()
await ndk.connect()
const events = await ndk.fetchEvents(filter)

// After (Effect)
const events = yield* NostrClient.fetchEvents(filter).pipe(
  Effect.provide(NostrClientLive)
)
```

## Future Enhancements

1. **NIP-90 Support**: Data Vending Machines for AI integration
2. **Actor Model**: Rivet actors for distributed relay operation
3. **Performance**: WASM modules for crypto operations
4. **Offline Support**: pglite integration for local relay
5. **Advanced NIPs**: MLS encryption (NIP-EE), custom privacy NIPs

## Success Metrics

1. **Code Quality**
   - 100% type coverage
   - >90% test coverage
   - Zero runtime type errors

2. **Performance**
   - <50ms event validation
   - >10k events/second throughput
   - <100MB memory for 100k events

3. **Developer Experience**
   - Comprehensive IntelliSense
   - Clear error messages
   - Minimal boilerplate

## Conclusion

This plan leverages Effect's powerful abstractions to create a best-in-class Nostr implementation that surpasses existing libraries in type safety, composability, and developer experience. By building on the excellent patterns from SNSTR while adding Effect's capabilities, we create a foundation for the next generation of decentralized applications.

The modular, NIP-centric architecture ensures that new protocol features can be added incrementally without breaking existing functionality. The comprehensive testing infrastructure guarantees reliability, while Effect's performance optimizations ensure scalability to millions of users.