import { eq } from "drizzle-orm"
import { drizzle } from "drizzle-orm/mysql2"
import { Effect } from "effect"
import { Elysia } from "elysia"
import mysql from "mysql2/promise"
import { afterAll, beforeAll, describe, expect, test } from "vitest"
import { WebSocket } from "ws"
import { createRelayPlugin } from "../src/psionic-plugin.js"
import * as schema from "../src/schema.js"
import { createTestEvent, generateTestKeypair, TestNip06Live } from "./test-helpers.js"

// Configure test timeout
const TEST_TIMEOUT = 30000

describe("Nostr Relay Integration Tests", () => {
  let app: Elysia
  let db: any
  let connection: mysql.Connection
  const port = 3456
  const relayUrl = `ws://localhost:${port}/relay`

  beforeAll(async () => {
    // Skip if no database credentials or not running in Bun
    if (!process.env.DATABASE_HOST || process.env.DATABASE_HOST === "localhost") {
      console.log("⚠️  Skipping integration tests - no database credentials")
      return
    }

    // Skip if not running in Bun (required for WebSocket support)
    if (typeof Bun === "undefined") {
      console.log("⚠️  Skipping integration tests - requires Bun runtime for WebSocket support")
      return
    }

    // Create database connection
    connection = await mysql.createConnection({
      host: process.env.DATABASE_HOST,
      user: process.env.DATABASE_USERNAME,
      password: process.env.DATABASE_PASSWORD,
      database: process.env.DATABASE_NAME || "openagents_relay",
      ssl: { rejectUnauthorized: false }
    })

    db = drizzle(connection, { schema, mode: "default" })

    // Clear test data
    await db.delete(schema.events).execute()

    // Create Elysia app with relay
    app = new Elysia()
      .use(createRelayPlugin({
        path: "/relay",
        enableMetrics: true
      }))
      .listen(port)

    // Wait for server to start
    await new Promise((resolve) => setTimeout(resolve, 1000))
  })

  afterAll(async () => {
    if (app) {
      await app.stop()
    }
    if (connection) {
      await connection.end()
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
      const testContent = "Integration test event " + Date.now()
      const event = yield* createTestEvent(privateKey, publicKey, testContent)

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

      // Send event
      ws.send(JSON.stringify(["EVENT", event]))

      // Wait for OK response
      yield* Effect.sleep("500 millis")

      const okMsg = messages.find((m) => m[0] === "OK")
      expect(okMsg).toBeDefined()
      expect(okMsg[1]).toBe(event.id)
      expect(okMsg[2]).toBe(true) // Success

      // Subscribe and retrieve
      messages.length = 0
      const subId = "test-sub-" + Date.now()
      ws.send(JSON.stringify(["REQ", subId, { ids: [event.id] }]))

      yield* Effect.sleep("500 millis")

      const eventMsg = messages.find((m) => m[0] === "EVENT" && m[1] === subId)
      expect(eventMsg).toBeDefined()
      expect(eventMsg[2].id).toBe(event.id)
      expect(eventMsg[2].content).toBe(testContent)

      ws.close()
    })

    await Effect.runPromise(program.pipe(Effect.provide(TestNip06Live)))
  })

  test("should filter events by author", { timeout: TEST_TIMEOUT }, async () => {
    if (!app) return

    const program = Effect.gen(function*() {
      const keypair1 = yield* generateTestKeypair(0)
      const keypair2 = yield* generateTestKeypair(1)

      const event1 = yield* createTestEvent(keypair1.privateKey, keypair1.publicKey, "Author 1 event")
      const event2 = yield* createTestEvent(keypair2.privateKey, keypair2.publicKey, "Author 2 event")

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

      // Send both events
      ws.send(JSON.stringify(["EVENT", event1]))
      ws.send(JSON.stringify(["EVENT", event2]))

      yield* Effect.sleep("500 millis")

      // Subscribe to author 1 only
      messages.length = 0
      const subId = "author-sub-" + Date.now()
      ws.send(JSON.stringify(["REQ", subId, { authors: [keypair1.publicKey] }]))

      yield* Effect.sleep("500 millis")

      const eventMessages = messages.filter((m) => m[0] === "EVENT" && m[1] === subId)
      expect(eventMessages.length).toBe(1)
      expect(eventMessages[0][2].pubkey).toBe(keypair1.publicKey)

      ws.close()
    })

    await Effect.runPromise(program.pipe(Effect.provide(TestNip06Live)))
  })

  test("should handle tag queries", { timeout: TEST_TIMEOUT }, async () => {
    if (!app) return

    const program = Effect.gen(function*() {
      const { privateKey, publicKey } = yield* generateTestKeypair()
      const taggedEvent = yield* createTestEvent(
        privateKey,
        publicKey,
        "Tagged event",
        1,
        [["t", "nostr"], ["t", "test"]]
      )

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

      // Send tagged event
      ws.send(JSON.stringify(["EVENT", taggedEvent]))

      yield* Effect.sleep("500 millis")

      // Query by tag
      messages.length = 0
      const subId = "tag-sub-" + Date.now()
      ws.send(JSON.stringify(["REQ", subId, { "#t": ["nostr"] }]))

      yield* Effect.sleep("500 millis")

      const eventMsg = messages.find((m) => m[0] === "EVENT" && m[1] === subId)
      expect(eventMsg).toBeDefined()
      expect(eventMsg[2].id).toBe(taggedEvent.id)

      // Verify tags in database
      const dbTags = yield* Effect.promise(() =>
        db.select().from(schema.event_tags).where(eq(schema.event_tags.event_id, taggedEvent.id))
      )
      expect(dbTags.length).toBe(2)
      expect(dbTags.find((t) => t.tag_value === "nostr")).toBeDefined()
      expect(dbTags.find((t) => t.tag_value === "test")).toBeDefined()

      ws.close()
    })

    await Effect.runPromise(program.pipe(Effect.provide(TestNip06Live)))
  })

  test("should respect subscription limits", { timeout: TEST_TIMEOUT }, async () => {
    if (!app) return

    const program = Effect.gen(function*() {
      const { privateKey, publicKey } = yield* generateTestKeypair()

      // Create multiple events
      const events = []
      for (let i = 0; i < 10; i++) {
        const event = yield* createTestEvent(privateKey, publicKey, `Event ${i}`)
        events.push(event)
      }

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

      // Send all events
      for (const event of events) {
        ws.send(JSON.stringify(["EVENT", event]))
      }

      yield* Effect.sleep("1 second")

      // Subscribe with limit
      messages.length = 0
      const subId = "limit-sub-" + Date.now()
      ws.send(JSON.stringify(["REQ", subId, { authors: [publicKey], limit: 5 }]))

      yield* Effect.sleep("500 millis")

      const eventMessages = messages.filter((m) => m[0] === "EVENT" && m[1] === subId)
      expect(eventMessages.length).toBe(5)

      // Should receive EOSE
      const eoseMsg = messages.find((m) => m[0] === "EOSE" && m[1] === subId)
      expect(eoseMsg).toBeDefined()

      ws.close()
    })

    await Effect.runPromise(program.pipe(Effect.provide(TestNip06Live)))
  })

  test("should handle CLOSE message", { timeout: TEST_TIMEOUT }, async () => {
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

      // Create subscription
      const subId = "close-test-" + Date.now()
      ws.send(JSON.stringify(["REQ", subId, { kinds: [1], limit: 10 }]))

      yield* Effect.sleep("200 millis")

      // Close subscription
      ws.send(JSON.stringify(["CLOSE", subId]))

      // Send an event that would match
      const { privateKey, publicKey } = yield* generateTestKeypair()
      const event = yield* createTestEvent(privateKey, publicKey, "Should not be received")
      ws.send(JSON.stringify(["EVENT", event]))

      yield* Effect.sleep("500 millis")

      // Should not receive the event for closed subscription
      const eventAfterClose = messages.find((m) =>
        m[0] === "EVENT" &&
        m[1] === subId &&
        m[2].id === event.id
      )
      expect(eventAfterClose).toBeUndefined()

      ws.close()
    })

    await Effect.runPromise(program.pipe(Effect.provide(TestNip06Live)))
  })
})
