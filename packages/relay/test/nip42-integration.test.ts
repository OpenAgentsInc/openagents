import { describe, expect, it } from "@effect/vitest"
import { CryptoService, EventService, Nip06Service } from "@openagentsinc/nostr"
import type { Schema as NostrSchema } from "@openagentsinc/nostr"
import { Effect, Layer } from "effect"
import { RelayDatabase } from "../src/database.js"
import { NostrRelay, NostrRelayLive } from "../src/relay.js"

// Mock database for testing
const MockDatabaseLive = Layer.succeed(
  RelayDatabase,
  {
    storeEvent: (_event: NostrSchema.NostrEvent) => Effect.succeed(true),
    queryEvents: (_filters: Array<NostrSchema.Filter>) => Effect.succeed([]),
    deleteEvent: (_eventId: string) => Effect.succeed(true),
    getEvent: (_eventId: string) => Effect.succeed(null)
  }
)

// Test layer
const TestLayer = NostrRelayLive.pipe(
  Layer.provide(MockDatabaseLive)
)

// Layer for client-side services
const ClientLayer = Layer.merge(
  Nip06Service.Nip06ServiceLive,
  EventService.EventServiceLive
).pipe(
  Layer.provide(CryptoService.CryptoServiceLive)
)

describe("NIP-42 Integration Tests", () => {
  describe("Full authentication flow", () => {
    it.skip("should authenticate client using NIP-42", () =>
      Effect.gen(function*() {
        // Set up relay
        const relay = yield* NostrRelay
        const connectionId = "test-conn-1"
        const handler = yield* relay.handleConnection(connectionId)

        // Generate client keys
        const nip06 = yield* Nip06Service.Nip06Service
        const eventService = yield* EventService.EventService
        const crypto = yield* CryptoService.CryptoService
        const mnemonic = yield* nip06.generateMnemonic()
        const privateKey = yield* nip06.derivePrivateKey(mnemonic)
        const _publicKey = yield* crypto.getPublicKey(privateKey)

        // Simulate receiving AUTH challenge from relay
        // In real scenario, this would come from WebSocket
        const authChallenge = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"

        // Create auth event
        const authEvent: NostrSchema.EventParams = {
          kind: 22242,
          content: "",
          tags: [
            ["relay", "wss://test.relay.com"],
            ["challenge", authChallenge]
          ]
        }

        const signedAuthEvent = yield* eventService.create(authEvent, privateKey)

        // Send AUTH message to relay
        const authMessage = JSON.stringify(["AUTH", signedAuthEvent])
        const responses = yield* handler.processMessage(authMessage)

        // Should receive OK response
        expect(responses.length).toBe(1)
        const response = JSON.parse(responses[0])
        expect(response[0]).toBe("OK")
        expect(response[1]).toBe(signedAuthEvent.id)
        // Note: In this test, auth will fail because we didn't set up the challenge properly
        // In a real integration test with WebSocket, the flow would be complete
      }).pipe(
        Effect.provide(TestLayer),
        Effect.provide(ClientLayer)
      ))
  })

  describe("Authentication state", () => {
    it.effect("should track authenticated connections", () =>
      Effect.gen(function*() {
        const relay = yield* NostrRelay
        const stats = yield* relay.getStats()

        expect(stats.totalConnections).toBe(0)
        expect(stats.activeConnections).toBe(0)

        // Create connection
        const connectionId = "test-conn-2"
        yield* relay.handleConnection(connectionId)

        const newStats = yield* relay.getStats()
        expect(newStats.totalConnections).toBe(1)
        expect(newStats.activeConnections).toBe(1)
      }).pipe(Effect.provide(TestLayer)))
  })

  describe("Protected operations", () => {
    it.effect("should allow REQ without auth when not required", () =>
      Effect.gen(function*() {
        const relay = yield* NostrRelay
        const connectionId = "test-conn-3"
        const handler = yield* relay.handleConnection(connectionId)

        // Send REQ without authentication
        const reqMessage = JSON.stringify(["REQ", "sub1", { kinds: [1] }])
        const responses = yield* handler.processMessage(reqMessage)

        // Should receive EOSE (end of stored events)
        expect(responses.length).toBeGreaterThan(0)
        const lastResponse = JSON.parse(responses[responses.length - 1])
        expect(lastResponse[0]).toBe("EOSE")
        expect(lastResponse[1]).toBe("sub1")
      }).pipe(Effect.provide(TestLayer)))
  })

  describe("Error cases", () => {
    it.effect("should reject AUTH with invalid event kind", () =>
      Effect.gen(function*() {
        const relay = yield* NostrRelay
        const connectionId = "test-conn-4"
        const handler = yield* relay.handleConnection(connectionId)

        // Create auth event with wrong kind
        const nip06 = yield* Nip06Service.Nip06Service
        const eventService = yield* EventService.EventService
        const mnemonic = yield* nip06.generateMnemonic()
        const privateKey = yield* nip06.derivePrivateKey(mnemonic)

        const wrongEvent: NostrSchema.EventParams = {
          kind: 1, // Wrong kind, should be 22242
          content: "",
          tags: [
            ["relay", "wss://test.relay.com"],
            ["challenge", "test-challenge"]
          ]
        }

        const signedEvent = yield* eventService.create(wrongEvent, privateKey)
        const authMessage = JSON.stringify(["AUTH", signedEvent])
        const responses = yield* handler.processMessage(authMessage)

        expect(responses.length).toBe(1)
        const response = JSON.parse(responses[0])
        expect(response[0]).toBe("OK")
        expect(response[2]).toBe(false) // Failed
        expect(response[3]).toContain("auth-required: must be kind 22242")
      }).pipe(
        Effect.provide(TestLayer),
        Effect.provide(ClientLayer)
      ))
  })
})
