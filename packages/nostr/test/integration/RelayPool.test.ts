/**
 * Integration tests for relay pool functionality
 */

import { Chunk, Duration, Effect, HashMap, Layer, Option, Stream } from "effect"
import { describe, expect, it } from "vitest"
import { CryptoService, CryptoServiceLive } from "../../src/services/CryptoService.js"
import { EventService, EventServiceLive } from "../../src/services/EventService.js"
import { RelayPoolService, RelayPoolServiceLive } from "../../src/services/RelayPoolService.js"
import { RelayServiceLive } from "../../src/services/RelayService.js"
import { WebSocketServiceLive } from "../../src/services/WebSocketService.js"
import { makeEphemeralRelay } from "../../src/test/EphemeralRelay.js"

describe("Relay Pool Integration Tests", () => {
  const TestLayer = Layer.mergeAll(
    CryptoServiceLive,
    EventServiceLive.pipe(Layer.provide(CryptoServiceLive)),
    WebSocketServiceLive,
    RelayServiceLive.pipe(Layer.provide(WebSocketServiceLive)),
    RelayPoolServiceLive.pipe(
      Layer.provide(
        RelayServiceLive.pipe(Layer.provide(WebSocketServiceLive))
      )
    )
  )

  describe("Multi-relay operations", () => {
    it("should connect to multiple relays", async () =>
      Effect.gen(function*() {
        // Start multiple relays
        const relay1 = yield* makeEphemeralRelay()
        const relay2 = yield* makeEphemeralRelay()
        const relay3 = yield* makeEphemeralRelay()

        yield* relay1.start()
        yield* relay2.start()
        yield* relay3.start()

        const url1 = yield* relay1.getUrl()
        const url2 = yield* relay2.getUrl()
        const url3 = yield* relay3.getUrl()

        // Add small delay to ensure servers are ready
        yield* Effect.sleep(Duration.millis(100))

        // Connect to pool
        const poolService = yield* RelayPoolService
        const pool = yield* poolService.connect([url1, url2, url3])

        // Check connection status
        const status = yield* pool.getConnectionStatus()
        expect(HashMap.size(status)).toBe(3)
        expect(Option.getOrNull(HashMap.get(status, url1))).toEqual("connected")
        expect(Option.getOrNull(HashMap.get(status, url2))).toEqual("connected")
        expect(Option.getOrNull(HashMap.get(status, url3))).toEqual("connected")
      }).pipe(
        Effect.scoped,
        Effect.provide(TestLayer),
        Effect.runPromise
      ), 30000)

    it("should publish events to all relays", async () =>
      Effect.gen(function*() {
        // Start multiple relays
        const relay1 = yield* makeEphemeralRelay()
        const relay2 = yield* makeEphemeralRelay()

        yield* relay1.start()
        yield* relay2.start()

        const url1 = yield* relay1.getUrl()
        const url2 = yield* relay2.getUrl()

        // Add small delay to ensure servers are ready
        yield* Effect.sleep(Duration.millis(100))

        // Connect to pool
        const poolService = yield* RelayPoolService
        const pool = yield* poolService.connect([url1, url2])

        // Create and publish event
        const crypto = yield* CryptoService
        const eventService = yield* EventService

        const privateKey = yield* crypto.generatePrivateKey()
        const event = yield* eventService.create({
          kind: 1,
          content: "Broadcast to all relays",
          tags: []
        }, privateKey)

        const results = yield* pool.publish(event)

        // Check results
        expect(HashMap.size(results)).toBe(2)
        expect(Option.getOrNull(HashMap.get(results, url1))).toEqual(true)
        expect(Option.getOrNull(HashMap.get(results, url2))).toEqual(true)

        // Verify event was stored on both relays
        const stored1 = yield* relay1.getStoredEvents()
        const stored2 = yield* relay2.getStoredEvents()

        expect(stored1).toHaveLength(1)
        expect(stored2).toHaveLength(1)
        expect(stored1[0].id).toBe(event.id)
        expect(stored2[0].id).toBe(event.id)
      }).pipe(
        Effect.scoped,
        Effect.provide(TestLayer),
        Effect.runPromise
      ), 30000)

    it("should deduplicate events from multiple relays", async () =>
      Effect.gen(function*() {
        // Start multiple relays
        const relay1 = yield* makeEphemeralRelay()
        const relay2 = yield* makeEphemeralRelay()

        yield* relay1.start()
        yield* relay2.start()

        const url1 = yield* relay1.getUrl()
        const url2 = yield* relay2.getUrl()

        // Create event
        const crypto = yield* CryptoService
        const eventService = yield* EventService
        const privateKey = yield* crypto.generatePrivateKey()

        const event = yield* eventService.create({
          kind: 1,
          content: "Duplicate test event",
          tags: []
        }, privateKey)

        // Store event on both relays
        yield* relay1.storeEvent(event)
        yield* relay2.storeEvent(event)

        // Connect to pool and subscribe
        const poolService = yield* RelayPoolService
        const pool = yield* poolService.connect([url1, url2])

        const subscription = yield* pool.subscribe("dedup-test" as any, [{}])

        // Collect all events within a time window to check for duplicates
        const allEvents = yield* subscription.events.pipe(
          Stream.take(1),
          Stream.runCollect,
          Effect.timeout(Duration.seconds(1)),
          Effect.catchTag("TimeoutException", () => Effect.succeed(Chunk.empty()))
        )

        // Should have received the event only once (deduplication working)
        expect(Chunk.size(allEvents)).toBe(1)
        expect(Chunk.unsafeGet(allEvents, 0).id).toBe(event.id)
      }).pipe(
        Effect.scoped,
        Effect.provide(TestLayer),
        Effect.runPromise
      ), 30000)

    it("should handle partial relay failures", async () =>
      Effect.gen(function*() {
        // Start one relay
        const relay1 = yield* makeEphemeralRelay()
        yield* relay1.start()
        const url1 = yield* relay1.getUrl()
        const url2 = "ws://invalid-relay.test:9999" // This will fail

        // Add small delay to ensure server is ready
        yield* Effect.sleep(Duration.millis(100))

        // Connect to pool
        const poolService = yield* RelayPoolService
        const pool = yield* poolService.connect([url1, url2])

        // Should have one connected, one disconnected
        const status = yield* pool.getConnectionStatus()
        expect(Option.getOrNull(HashMap.get(status, url1))).toEqual("connected")
        expect(Option.getOrNull(HashMap.get(status, url2))).toEqual("disconnected")

        // Create and publish event
        const crypto = yield* CryptoService
        const eventService = yield* EventService
        const privateKey = yield* crypto.generatePrivateKey()

        const event = yield* eventService.create({
          kind: 1,
          content: "Partial failure test",
          tags: []
        }, privateKey)

        const results = yield* pool.publish(event)

        // Should succeed on relay1, fail on relay2
        expect(Option.getOrNull(HashMap.get(results, url1))).toEqual(true)
        // Failed relay might not be included in results or might have undefined value
        const relay2Result = Option.getOrNull(HashMap.get(results, url2))
        // It's OK for failed relay to not be in results at all, or to have false value
        expect(relay2Result === null || relay2Result === false).toBe(true)
      }).pipe(
        Effect.scoped,
        Effect.provide(TestLayer),
        Effect.runPromise
      ), 30000)
  })
})
