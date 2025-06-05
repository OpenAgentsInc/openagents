/**
 * Integration tests for relay pool functionality
 */

import { Chunk, Duration, Effect, HashMap, Layer, Stream } from "effect"
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

        const url1 = relay1.url
        const url2 = relay2.url
        const url3 = relay3.url

        // Add small delay to ensure servers are ready
        yield* Effect.sleep(Duration.millis(100))

        // Connect to pool
        const poolService = yield* RelayPoolService
        const pool = yield* poolService.connect([url1, url2, url3])

        // Check connection status
        const status = yield* pool.getConnectionStatus()
        expect(HashMap.size(status)).toBe(3)
        expect(HashMap.get(status, url1)).toEqual({ _tag: "Some", value: "connected" })
        expect(HashMap.get(status, url2)).toEqual({ _tag: "Some", value: "connected" })
        expect(HashMap.get(status, url3)).toEqual({ _tag: "Some", value: "connected" })
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

        const url1 = relay1.url
        const url2 = relay2.url

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
        expect(HashMap.get(results, url1)).toEqual({ _tag: "Some", value: true })
        expect(HashMap.get(results, url2)).toEqual({ _tag: "Some", value: true })

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

        const url1 = relay1.url
        const url2 = relay2.url

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

        // Should receive event only once
        const received = yield* subscription.events.pipe(
          Stream.take(1),
          Stream.runCollect,
          Effect.map((chunk) => Chunk.unsafeGet(chunk, 0)),
          Effect.timeout(Duration.seconds(1))
        )

        expect(received!.id).toBe(event.id)

        // Check which relays saw the event
        const seenOn = yield* subscription.seenOn.pipe(
          Effect.map(HashMap.get(event.id)),
          Effect.map((opt) => opt._tag === "Some" ? opt.value : [])
        )

        expect(seenOn).toHaveLength(2)
        expect(seenOn).toContain(url1)
        expect(seenOn).toContain(url2)
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
        const url1 = relay1.url
        const url2 = "ws://invalid-relay.test:9999" // This will fail

        // Add small delay to ensure server is ready
        yield* Effect.sleep(Duration.millis(100))

        // Connect to pool
        const poolService = yield* RelayPoolService
        const pool = yield* poolService.connect([url1, url2])

        // Should have one connected, one disconnected
        const status = yield* pool.getConnectionStatus()
        expect(HashMap.get(status, url1)).toEqual({ _tag: "Some", value: "connected" })
        expect(HashMap.get(status, url2)).toEqual({ _tag: "Some", value: "disconnected" })

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
        expect(HashMap.get(results, url1)).toEqual({ _tag: "Some", value: true })
        expect(HashMap.get(results, url2)).toEqual({ _tag: "Some", value: false })
      }).pipe(
        Effect.scoped,
        Effect.provide(TestLayer),
        Effect.runPromise
      ), 30000)
  })
})
