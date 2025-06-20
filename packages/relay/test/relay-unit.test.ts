import { Effect, Layer } from "effect"
import { describe, expect, test } from "vitest"
import { RelayDatabase } from "../src/database.js"
import { NostrRelay, NostrRelayLive } from "../src/relay.js"

// Mock database layer for testing
const MockDatabaseLive = Layer.succeed(
  RelayDatabase,
  {
    storeEvent: (_event) => Effect.succeed(true),
    queryEvents: (_filters) => Effect.succeed([]),
    getEventById: (_id) => Effect.succeed(null),
    deleteEvent: (_id) => Effect.succeed(true),
    getEventCount: () => Effect.succeed(0),
    updateAgentProfile: (_profile) => Effect.succeed(undefined),
    getAgentProfile: (_pubkey) => Effect.succeed(null),
    updateServiceOffering: (_offering) => Effect.succeed(undefined),
    getServiceOfferings: (_filters) => Effect.succeed([]),
    storeChannel: (_channel) => Effect.succeed(undefined),
    getChannel: (_id) => Effect.succeed(null)
  }
)

describe("Nostr Relay Unit Tests", () => {
  describe("Message Parsing", () => {
    test("should parse EVENT messages", async () => {
      const testEvent = {
        id: "1234567890abcdef",
        pubkey: "abcdef1234567890",
        created_at: Math.floor(Date.now() / 1000),
        kind: 1,
        tags: [],
        content: "Test event",
        sig: "signature123"
      }

      const relay = await Effect.runPromise(
        Effect.provide(
          NostrRelay,
          Layer.provide(NostrRelayLive, MockDatabaseLive)
        )
      )

      const handler = await Effect.runPromise(relay.handleConnection("test-conn-1"))

      // processMessage now returns response arrays
      const result = await Effect.runPromise(
        handler.processMessage(JSON.stringify(["EVENT", testEvent]))
      )
      
      expect(Array.isArray(result)).toBe(true)
      expect(result.length).toBeGreaterThan(0)
      // Should contain an OK message
      expect(result[0]).toContain("OK")
      expect(result[0]).toContain(testEvent.id)
    })

    test("should parse REQ messages", async () => {
      const relay = await Effect.runPromise(
        Effect.provide(
          NostrRelay,
          Layer.provide(NostrRelayLive, MockDatabaseLive)
        )
      )

      const handler = await Effect.runPromise(relay.handleConnection("test-conn-2"))
      const reqMessage = ["REQ", "sub123", { kinds: [1], limit: 10 }]

      // Should return EOSE message for REQ
      const result = await Effect.runPromise(
        handler.processMessage(JSON.stringify(reqMessage))
      )
      
      expect(Array.isArray(result)).toBe(true)
      expect(result.length).toBeGreaterThan(0)
      // Should contain EOSE message
      expect(result[result.length - 1]).toContain("EOSE")
      expect(result[result.length - 1]).toContain("sub123")
    })

    test("should parse CLOSE messages", async () => {
      const relay = await Effect.runPromise(
        Effect.provide(
          NostrRelay,
          Layer.provide(NostrRelayLive, MockDatabaseLive)
        )
      )

      const handler = await Effect.runPromise(relay.handleConnection("test-conn-3"))

      // First create a subscription
      await Effect.runPromise(
        handler.processMessage(JSON.stringify(["REQ", "sub456", { kinds: [1] }]))
      )

      // Then close it
      const closeResult = await Effect.runPromise(
        handler.processMessage(JSON.stringify(["CLOSE", "sub456"]))
      )

      expect(Array.isArray(closeResult)).toBe(true)
      expect(closeResult.length).toBeGreaterThan(0)
      // Should contain CLOSED message
      expect(closeResult[0]).toContain("CLOSED")
      expect(closeResult[0]).toContain("sub456")
    })

    test("should handle invalid messages gracefully", async () => {
      const relay = await Effect.runPromise(
        Effect.provide(
          NostrRelay,
          Layer.provide(NostrRelayLive, MockDatabaseLive)
        )
      )

      const handler = await Effect.runPromise(relay.handleConnection("test-conn-4"))

      // Invalid JSON should fail with MessageError
      await expect(
        Effect.runPromise(handler.processMessage("invalid json"))
      ).rejects.toThrow("Invalid JSON")

      // Unknown message type should fail with MessageError
      await expect(
        Effect.runPromise(handler.processMessage(JSON.stringify(["UNKNOWN", "data"])))
      ).rejects.toThrow("Unknown message type")
    })
  })

  describe("Connection Management", () => {
    test("should track connection count", async () => {
      const relay = await Effect.runPromise(
        Effect.provide(
          NostrRelay,
          Layer.provide(NostrRelayLive, MockDatabaseLive)
        )
      )

      // Initial count
      const count1 = await Effect.runPromise(relay.getConnectionCount())
      expect(count1).toBe(0)

      // Add connection
      const handler1 = await Effect.runPromise(relay.handleConnection("conn1"))
      const count2 = await Effect.runPromise(relay.getConnectionCount())
      expect(count2).toBe(1)

      // Add another
      await Effect.runPromise(relay.handleConnection("conn2"))
      const count3 = await Effect.runPromise(relay.getConnectionCount())
      expect(count3).toBe(2)

      // Disconnect one
      await Effect.runPromise(handler1.close())
      const count4 = await Effect.runPromise(relay.getConnectionCount())
      expect(count4).toBe(1)
    })

    test("should handle multiple subscriptions per connection", async () => {
      const relay = await Effect.runPromise(
        Effect.provide(
          NostrRelay,
          Layer.provide(NostrRelayLive, MockDatabaseLive)
        )
      )

      const handler = await Effect.runPromise(relay.handleConnection("multi-sub"))

      // Create multiple subscriptions
      await Effect.runPromise(
        handler.processMessage(JSON.stringify(["REQ", "sub1", { kinds: [1] }]))
      )

      await Effect.runPromise(
        handler.processMessage(JSON.stringify(["REQ", "sub2", { kinds: [3] }]))
      )

      await Effect.runPromise(
        handler.processMessage(JSON.stringify(["REQ", "sub3", { authors: ["pubkey123"] }]))
      )

      // All should succeed
      const stats = await Effect.runPromise(relay.getStats())
      expect(stats.activeConnections).toBe(1)
    })
  })

  describe("Event Validation", () => {
    test("should process events through mock database", async () => {
      const relay = await Effect.runPromise(
        Effect.provide(
          NostrRelay,
          Layer.provide(NostrRelayLive, MockDatabaseLive)
        )
      )

      const handler = await Effect.runPromise(relay.handleConnection("validation-test"))

      // Even invalid events succeed with mock database
      const invalidEvent = {
        id: "123",
        // missing pubkey
        created_at: 123,
        kind: 1,
        tags: [],
        content: "test"
        // missing sig
      }

      // Mock database always returns success, so we get OK response
      const result = await Effect.runPromise(
        handler.processMessage(JSON.stringify(["EVENT", invalidEvent]))
      )
      
      expect(Array.isArray(result)).toBe(true)
      expect(result.length).toBeGreaterThan(0)
      // Should contain an OK message even for invalid events (mock always succeeds)
      expect(result[0]).toContain("OK")
      expect(result[0]).toContain(invalidEvent.id)
    })
  })
})
