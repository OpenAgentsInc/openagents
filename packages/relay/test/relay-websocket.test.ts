import { Effect } from "effect"
import { Elysia } from "elysia"
import { afterAll, beforeAll, describe, expect, test } from "vitest"
import { WebSocket } from "ws"
import { createRelayPlugin } from "../src/psionic-plugin.js"
import { createTestEvent, generateTestKeypair, TestNip06Live } from "./test-helpers.js"

const TEST_TIMEOUT = 30000

describe("Relay WebSocket Integration", () => {
  let app: Elysia | null = null
  const port = 3456
  const relayUrl = `ws://localhost:${port}/relay`

  beforeAll(async () => {
    // Skip if no database credentials
    if (!process.env.DATABASE_HOST || process.env.DATABASE_HOST === "localhost") {
      console.log("⚠️  Skipping WebSocket tests - no database credentials")
      return
    }

    // Skip if not running in Bun (required for WebSocket support)
    if (typeof Bun === "undefined") {
      console.log("⚠️  Skipping WebSocket tests - requires Bun runtime for WebSocket support")
      return
    }

    // Create Elysia app with relay plugin
    app = new Elysia()
      .use(createRelayPlugin({
        path: "/relay",
        enableMetrics: true,
        rateLimitEnabled: false
      }))
      .listen(port)

    // Wait for server to start
    await new Promise((resolve) => setTimeout(resolve, 1000))
  })

  afterAll(async () => {
    if (app) {
      await app.stop()
    }
  })

  test("should connect to relay WebSocket", { timeout: TEST_TIMEOUT }, async () => {
    if (!app) return

    const ws = new WebSocket(relayUrl)

    await new Promise<void>((resolve, reject) => {
      ws.on("open", () => resolve())
      ws.on("error", reject)
      setTimeout(() => reject(new Error("Connection timeout")), 5000)
    })

    expect(ws.readyState).toBe(WebSocket.OPEN)
    ws.close()
  })

  test("should store and retrieve events", { timeout: TEST_TIMEOUT }, async () => {
    if (!app) return

    const program = Effect.gen(function*() {
      const { privateKey, publicKey } = yield* generateTestKeypair()
      const event = yield* createTestEvent(
        privateKey,
        publicKey,
        "WebSocket test event " + Date.now()
      )

      const ws = new WebSocket(relayUrl)
      const messages: Array<any> = []

      ws.on("message", (data) => {
        messages.push(JSON.parse(data.toString()))
      })

      // Wait for connection
      yield* Effect.promise(() =>
        new Promise<void>((resolve, reject) => {
          ws.on("open", () => resolve())
          ws.on("error", reject)
          setTimeout(() => reject(new Error("Connection timeout")), 5000)
        })
      )

      // Send event
      ws.send(JSON.stringify(["EVENT", event]))

      // Wait for OK response
      yield* Effect.sleep("500 millis")

      const okMsg = messages.find((m) => m[0] === "OK" && m[1] === event.id)
      expect(okMsg).toBeDefined()
      expect(okMsg[2]).toBe(true) // Success

      // Subscribe and retrieve
      messages.length = 0
      const subId = "test-sub-" + Date.now()
      ws.send(JSON.stringify(["REQ", subId, { ids: [event.id] }]))

      yield* Effect.sleep("500 millis")

      const eventMsg = messages.find((m) => m[0] === "EVENT" && m[1] === subId)
      expect(eventMsg).toBeDefined()
      expect(eventMsg[2].id).toBe(event.id)
      expect(eventMsg[2].content).toBe(event.content)

      ws.close()
    })

    await Effect.runPromise(program.pipe(Effect.provide(TestNip06Live)))
  })

  test("should handle multiple concurrent connections", { timeout: TEST_TIMEOUT }, async () => {
    if (!app) return

    const program = Effect.gen(function*() {
      const connections = 5
      const sockets: Array<WebSocket> = []

      // Create multiple connections
      for (let i = 0; i < connections; i++) {
        const ws = new WebSocket(relayUrl)
        yield* Effect.promise(() =>
          new Promise<void>((resolve, reject) => {
            ws.on("open", () => resolve())
            ws.on("error", reject)
            setTimeout(() => reject(new Error("Connection timeout")), 5000)
          })
        )
        sockets.push(ws)
      }

      expect(sockets.every((ws) => ws.readyState === WebSocket.OPEN)).toBe(true)

      // Generate test event
      const { privateKey, publicKey } = yield* generateTestKeypair()
      const event = yield* createTestEvent(
        privateKey,
        publicKey,
        "Broadcast test event"
      )

      // Set up message collectors
      const messageCollectors = sockets.map(() => [] as Array<any>)
      sockets.forEach((ws, i) => {
        ws.on("message", (data) => {
          messageCollectors[i].push(JSON.parse(data.toString()))
        })
      })

      // Subscribe on all connections
      const subId = "broadcast-test"
      sockets.forEach((ws) => {
        ws.send(JSON.stringify(["REQ", subId, { kinds: [1], limit: 10 }]))
      })

      yield* Effect.sleep("500 millis")

      // Send event from first connection
      sockets[0].send(JSON.stringify(["EVENT", event]))

      yield* Effect.sleep("1 second")

      // All connections should receive the event
      const receivedCounts = messageCollectors.map((msgs) =>
        msgs.filter((m) => m[0] === "EVENT" && m[2]?.id === event.id).length
      )

      // At least the other connections should receive it
      expect(receivedCounts.filter((count) => count > 0).length).toBeGreaterThanOrEqual(connections - 1)

      // Clean up
      sockets.forEach((ws) => ws.close())
    })

    await Effect.runPromise(program.pipe(Effect.provide(TestNip06Live)))
  })

  test("should handle subscription filters correctly", { timeout: TEST_TIMEOUT }, async () => {
    if (!app) return

    const program = Effect.gen(function*() {
      const ws = new WebSocket(relayUrl)
      const messages: Array<any> = []

      ws.on("message", (data) => {
        messages.push(JSON.parse(data.toString()))
      })

      yield* Effect.promise(() =>
        new Promise<void>((resolve, reject) => {
          ws.on("open", () => resolve())
          ws.on("error", reject)
        })
      )

      // Create events with different kinds
      const { privateKey, publicKey } = yield* generateTestKeypair()

      const textNote = yield* createTestEvent(privateKey, publicKey, "Text note", 1)
      const metadata = yield* createTestEvent(privateKey, publicKey, "{\"name\":\"Test\"}", 0)
      const reaction = yield* createTestEvent(privateKey, publicKey, "+", 7, [["e", "someeventid"]])

      // Send all events
      ws.send(JSON.stringify(["EVENT", textNote]))
      ws.send(JSON.stringify(["EVENT", metadata]))
      ws.send(JSON.stringify(["EVENT", reaction]))

      yield* Effect.sleep("500 millis")

      // Subscribe to only kind 1 events
      messages.length = 0
      const subId = "kind-filter-test"
      ws.send(JSON.stringify(["REQ", subId, { kinds: [1] }]))

      yield* Effect.sleep("500 millis")

      const kind1Events = messages.filter((m) => m[0] === "EVENT" && m[1] === subId && m[2].kind === 1)
      const otherKindEvents = messages.filter((m) => m[0] === "EVENT" && m[1] === subId && m[2].kind !== 1)

      expect(kind1Events.length).toBeGreaterThan(0)
      expect(otherKindEvents.length).toBe(0)

      ws.close()
    })

    await Effect.runPromise(program.pipe(Effect.provide(TestNip06Live)))
  })
})
