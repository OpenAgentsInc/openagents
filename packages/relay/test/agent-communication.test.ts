import { Effect } from "effect"
import { describe, expect, test } from "vitest"
import { WebSocket } from "ws"
import { createTestEvent, generateTestKeypair, TestNip06Live } from "./test-helpers.js"

/**
 * Test suite for agent communication via relay endpoints
 * Verifies NIP-OA, NIP-28, and NIP-90 agent-specific features
 */

describe("Agent Communication Tests", () => {
  test("should support agent profile metadata (NIP-OA)", { timeout: 10000 }, async () => {
    if (!process.env.DATABASE_HOST || process.env.DATABASE_HOST === "localhost") {
      console.log("⚠️  Skipping - no database credentials")
      return
    }

    const program = Effect.gen(function*() {
      const { privateKey, publicKey } = yield* generateTestKeypair()
      
      // Create agent profile metadata event (kind 0)
      const agentProfile = yield* createTestEvent(
        privateKey,
        publicKey,
        JSON.stringify({
          name: "Test Agent",
          about: "An automated test agent",
          picture: "https://example.com/agent.png",
          nip05: "testagent@openagents.com",
          lud16: "testagent@openagents.com",
          // Agent-specific metadata
          agent: {
            version: "1.0.0",
            capabilities: ["text-generation", "code-analysis"],
            model: "test-model-v1"
          }
        }),
        0 // kind 0 for metadata
      )

      return { agentProfile, publicKey }
    })

    const result = await Effect.runPromise(program.pipe(Effect.provide(TestNip06Live)))
    expect(result.agentProfile).toBeDefined()
    expect(result.agentProfile.kind).toBe(0)
    expect(result.agentProfile.pubkey).toBe(result.publicKey)
  })

  test("should support agent-to-agent messages (NIP-28)", { timeout: 10000 }, async () => {
    if (!process.env.DATABASE_HOST || process.env.DATABASE_HOST === "localhost") {
      console.log("⚠️  Skipping - no database credentials")
      return
    }

    const program = Effect.gen(function*() {
      // Generate two agent keypairs
      const agent1 = yield* generateTestKeypair(1)
      const agent2 = yield* generateTestKeypair(2)
      
      // Create public channel creation event (kind 40)
      const channelEvent = yield* createTestEvent(
        agent1.privateKey,
        agent1.publicKey,
        JSON.stringify({
          name: "Agent Coordination Channel",
          about: "Channel for agent-to-agent communication",
          picture: ""
        }),
        40, // kind 40 for channel creation
        [["e", "channel-" + Date.now()]]
      )

      // Create channel message from agent2 (kind 42)
      const channelMessage = yield* createTestEvent(
        agent2.privateKey,
        agent2.publicKey,
        "Hello from Agent 2, ready to coordinate",
        42, // kind 42 for channel message
        [
          ["e", channelEvent.id, "", "root"],
          ["p", agent1.publicKey]
        ]
      )

      return { channelEvent, channelMessage, agent1, agent2 }
    })

    const result = await Effect.runPromise(program.pipe(Effect.provide(TestNip06Live)))
    expect(result.channelEvent.kind).toBe(40)
    expect(result.channelMessage.kind).toBe(42)
    expect(result.channelMessage.tags).toContainEqual(["p", result.agent1.publicKey])
  })

  test("should support data vending requests (NIP-90)", { timeout: 10000 }, async () => {
    if (!process.env.DATABASE_HOST || process.env.DATABASE_HOST === "localhost") {
      console.log("⚠️  Skipping - no database credentials")
      return
    }

    const program = Effect.gen(function*() {
      const client = yield* generateTestKeypair(1)
      const serviceProvider = yield* generateTestKeypair(2)
      
      // Create DVM request event (kind 5000-5999)
      const dvmRequest = yield* createTestEvent(
        client.privateKey,
        client.publicKey,
        JSON.stringify({
          task: "text-generation",
          params: {
            prompt: "Generate a test response",
            max_tokens: 100
          }
        }),
        5001, // Text generation DVM kind
        [
          ["i", "prompt:Generate a test response"],
          ["param", "max_tokens", "100"],
          ["output", "text/plain"]
        ]
      )

      // Create DVM response event (kind 6000-6999)
      const dvmResponse = yield* createTestEvent(
        serviceProvider.privateKey,
        serviceProvider.publicKey,
        "This is a test response from the DVM service provider",
        6001, // Text generation result
        [
          ["request", dvmRequest.id],
          ["e", dvmRequest.id],
          ["p", client.publicKey],
          ["status", "success"]
        ]
      )

      // Create DVM feedback event (kind 7000)
      const dvmFeedback = yield* createTestEvent(
        client.privateKey,
        client.publicKey,
        "",
        7000, // DVM feedback
        [
          ["e", dvmResponse.id],
          ["p", serviceProvider.publicKey],
          ["rating", "5"],
          ["comment", "Excellent service"]
        ]
      )

      return { dvmRequest, dvmResponse, dvmFeedback }
    })

    const result = await Effect.runPromise(program.pipe(Effect.provide(TestNip06Live)))
    
    // Verify DVM request
    expect(result.dvmRequest.kind).toBe(5001)
    expect(result.dvmRequest.tags).toContainEqual(["output", "text/plain"])
    
    // Verify DVM response
    expect(result.dvmResponse.kind).toBe(6001)
    expect(result.dvmResponse.tags).toContainEqual(["request", result.dvmRequest.id])
    
    // Verify DVM feedback
    expect(result.dvmFeedback.kind).toBe(7000)
    expect(result.dvmFeedback.tags).toContainEqual(["rating", "5"])
  })

  test("should handle agent service announcements", { timeout: 10000 }, async () => {
    if (!process.env.DATABASE_HOST || process.env.DATABASE_HOST === "localhost") {
      console.log("⚠️  Skipping - no database credentials")
      return
    }

    const program = Effect.gen(function*() {
      const agent = yield* generateTestKeypair()
      
      // Create service announcement event (kind 31990)
      const serviceAnnouncement = yield* createTestEvent(
        agent.privateKey,
        agent.publicKey,
        JSON.stringify({
          name: "AI Text Generation Service",
          about: "High-quality text generation using advanced models",
          amount: "1000", // sats per request
          endpoints: ["wss://relay.openagents.com"],
          kinds: [5001], // Text generation DVM
          status: "active"
        }),
        31990, // Service announcement
        [
          ["d", "text-generation-service"],
          ["k", "5001"],
          ["amount", "1000", "sats"],
          ["status", "active"]
        ]
      )

      return { serviceAnnouncement, agent }
    })

    const result = await Effect.runPromise(program.pipe(Effect.provide(TestNip06Live)))
    expect(result.serviceAnnouncement.kind).toBe(31990)
    expect(result.serviceAnnouncement.tags).toContainEqual(["k", "5001"])
    expect(result.serviceAnnouncement.tags).toContainEqual(["status", "active"])
  })
})