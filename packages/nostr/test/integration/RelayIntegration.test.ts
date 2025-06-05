/**
 * Integration tests for Nostr relay communication
 */

import { Chunk, Console, Duration, Effect, Layer, Stream } from "effect"
import { describe, expect, it } from "vitest"
import { CryptoService, CryptoServiceLive } from "../../src/services/CryptoService.js"
import { EventService, EventServiceLive } from "../../src/services/EventService.js"
import { RelayService, RelayServiceLive } from "../../src/services/RelayService.js"
import { WebSocketServiceLive } from "../../src/services/WebSocketService.js"
import { makeEphemeralRelay } from "../../src/test/EphemeralRelay.js"

describe("Relay Integration Tests", () => {
  const TestLayer = Layer.mergeAll(
    CryptoServiceLive,
    EventServiceLive.pipe(Layer.provide(CryptoServiceLive)),
    WebSocketServiceLive,
    RelayServiceLive.pipe(Layer.provide(WebSocketServiceLive))
  )

  describe("Basic relay operations", () => {
    it("should connect to a relay and publish an event", async () =>
      Effect.gen(function*() {
        // Start ephemeral relay
        const relay = yield* makeEphemeralRelay()
        yield* relay.start()
        
        // Add small delay to ensure server is ready and port is set
        yield* Effect.sleep(Duration.millis(200))
        
        const url = yield* relay.getUrl()

        // Log the URL to debug
        yield* Console.log(`Connecting to relay at: ${url}`)

        // Connect to relay
        const relayService = yield* RelayService
        const connection = yield* relayService.connect(url)

        // Create and publish event
        const crypto = yield* CryptoService
        const eventService = yield* EventService

        const privateKey = yield* crypto.generatePrivateKey()
        const event = yield* eventService.create({
          kind: 1,
          content: "Hello from Effect Nostr!",
          tags: []
        }, privateKey)

        const success = yield* connection.publish(event)
        expect(success).toBe(true)

        // Verify event was stored
        const stored = yield* relay.getStoredEvents()
        expect(stored).toHaveLength(1)
        expect(stored[0].id).toBe(event.id)
      }).pipe(
        Effect.scoped,
        Effect.provide(TestLayer),
        Effect.runPromise
      ), 30000)

    it("should subscribe to events and receive them", async () =>
      Effect.gen(function*() {
        // Start ephemeral relay
        const relay = yield* makeEphemeralRelay()
        yield* relay.start()
        
        // Add small delay to ensure server is ready and port is set
        yield* Effect.sleep(Duration.millis(200))
        
        const url = yield* relay.getUrl()

        // Log the URL to debug
        yield* Console.log(`Connecting to relay at: ${url}`)

        // Connect to relay
        const relayService = yield* RelayService
        const connection = yield* relayService.connect(url)

        // Subscribe to all events
        const subscription = yield* connection.subscribe("test-sub" as any, [{}])

        // Create and publish event
        const crypto = yield* CryptoService
        const eventService = yield* EventService

        const privateKey = yield* crypto.generatePrivateKey()
        const event = yield* eventService.create({
          kind: 1,
          content: "Test event for subscription",
          tags: []
        }, privateKey)

        yield* connection.publish(event)

        // Receive event from subscription
        const received = yield* subscription.events.pipe(
          Stream.take(1),
          Stream.runCollect,
          Effect.map((chunk) => Chunk.unsafeGet(chunk, 0)),
          Effect.timeout(Duration.seconds(1))
        )

        expect(received).toBeDefined()
        expect(received!.id).toBe(event.id)
        expect(received!.content).toBe("Test event for subscription")
      }).pipe(
        Effect.scoped,
        Effect.provide(TestLayer),
        Effect.runPromise
      ), 30000)

    it("should filter events by kind", async () =>
      Effect.gen(function*() {
        // Start ephemeral relay
        const relay = yield* makeEphemeralRelay()
        yield* relay.start()
        
        // Add small delay to ensure server is ready and port is set
        yield* Effect.sleep(Duration.millis(200))
        
        const url = yield* relay.getUrl()

        // Log the URL to debug
        yield* Console.log(`Connecting to relay at: ${url}`)

        // Connect to relay
        const relayService = yield* RelayService
        const connection = yield* relayService.connect(url)

        // Create events with different kinds
        const crypto = yield* CryptoService
        const eventService = yield* EventService
        const privateKey = yield* crypto.generatePrivateKey()

        const event1 = yield* eventService.create({
          kind: 1,
          content: "Kind 1 event",
          tags: []
        }, privateKey)

        const event2 = yield* eventService.create({
          kind: 2,
          content: "Kind 2 event",
          tags: []
        }, privateKey)

        yield* connection.publish(event1)
        yield* connection.publish(event2)

        // Subscribe with kind filter
        const subscription = yield* connection.subscribe("kind-filter" as any, [
          { kinds: [1] }
        ])

        // Should only receive kind 1 event
        const received = yield* subscription.events.pipe(
          Stream.take(1),
          Stream.runCollect,
          Effect.map((chunk) => Chunk.unsafeGet(chunk, 0)),
          Effect.timeout(Duration.seconds(1))
        )

        expect(received!.kind).toBe(1)
        expect(received!.content).toBe("Kind 1 event")
      }).pipe(
        Effect.scoped,
        Effect.provide(TestLayer),
        Effect.runPromise
      ), 30000)

    it("should handle multiple subscriptions", async () =>
      Effect.gen(function*() {
        // Start ephemeral relay
        const relay = yield* makeEphemeralRelay()
        yield* relay.start()
        
        // Add small delay to ensure server is ready and port is set
        yield* Effect.sleep(Duration.millis(200))
        
        const url = yield* relay.getUrl()

        // Log the URL to debug
        yield* Console.log(`Connecting to relay at: ${url}`)

        // Connect to relay
        const relayService = yield* RelayService
        const connection = yield* relayService.connect(url)

        // Create multiple subscriptions
        const sub1 = yield* connection.subscribe("sub1" as any, [{ kinds: [1] }])
        const sub2 = yield* connection.subscribe("sub2" as any, [{ kinds: [2] }])

        // Create and publish events
        const crypto = yield* CryptoService
        const eventService = yield* EventService
        const privateKey = yield* crypto.generatePrivateKey()

        const event1 = yield* eventService.create({
          kind: 1,
          content: "For sub1",
          tags: []
        }, privateKey)

        const event2 = yield* eventService.create({
          kind: 2,
          content: "For sub2",
          tags: []
        }, privateKey)

        yield* connection.publish(event1)
        yield* connection.publish(event2)

        // Each subscription should receive its filtered event
        const received1 = yield* sub1.events.pipe(
          Stream.take(1),
          Stream.runCollect,
          Effect.map((chunk) => Chunk.unsafeGet(chunk, 0)),
          Effect.timeout(Duration.seconds(1))
        )

        const received2 = yield* sub2.events.pipe(
          Stream.take(1),
          Stream.runCollect,
          Effect.map((chunk) => Chunk.unsafeGet(chunk, 0)),
          Effect.timeout(Duration.seconds(1))
        )

        expect(received1!.content).toBe("For sub1")
        expect(received2!.content).toBe("For sub2")

        // Close subscriptions
        yield* connection.close("sub1" as any)
        yield* connection.close("sub2" as any)
      }).pipe(
        Effect.scoped,
        Effect.provide(TestLayer),
        Effect.runPromise
      ), 30000)
  })
})
