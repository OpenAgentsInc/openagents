import { Effect, Layer } from "effect"
import mysql from "mysql2/promise"
import { afterAll, beforeAll, describe, expect, test } from "vitest"
import { RelayDatabase, RelayDatabaseLive } from "../src/database.js"
import { createTestEvent, generateTestKeypair, TestNip06Live } from "./test-helpers.js"

describe("Relay Database Tests", () => {
  let connection: mysql.Connection | null = null
  let testLayer: Layer.Layer<RelayDatabase | any>

  beforeAll(async () => {
    // Skip if no database credentials
    if (!process.env.DATABASE_HOST || process.env.DATABASE_HOST === "localhost") {
      console.log("⚠️  Skipping database tests - no PlanetScale credentials")
      return
    }

    // Create MySQL connection for cleanup
    connection = await mysql.createConnection({
      host: process.env.DATABASE_HOST,
      user: process.env.DATABASE_USERNAME,
      password: process.env.DATABASE_PASSWORD,
      database: process.env.DATABASE_NAME,
      ssl: { rejectUnauthorized: false }
    })

    // RelayDatabaseLive reads config from environment
    // The test setup.ts already loads environment variables

    // Merge all required layers
    testLayer = Layer.merge(
      RelayDatabaseLive,
      TestNip06Live
    )

    // Clear test data
    await connection.execute("DELETE FROM event_tags WHERE event_id LIKE \"test%\"")
    await connection.execute("DELETE FROM events WHERE id LIKE \"test%\"")
  })

  afterAll(async () => {
    if (connection) {
      await connection.end()
    }
  })

  test("should store and retrieve events", async () => {
    if (!connection) return

    const program = Effect.gen(function*() {
      const db = yield* RelayDatabase

      // Generate test keypair
      const { privateKey, publicKey } = yield* generateTestKeypair()

      // Create test event
      const event = yield* createTestEvent(
        privateKey,
        publicKey,
        "Database test event " + Date.now(),
        1,
        [["t", "test"], ["client", "relay-test"]]
      )

      // Prefix ID with 'test' for cleanup
      event.id = "test" + event.id.slice(4)

      // Store event
      const stored = yield* db.storeEvent(event)
      expect(stored).toBe(true)

      // Retrieve by ID
      const retrieved = yield* db.getEvent(event.id)
      expect(retrieved).toBeDefined()
      expect(retrieved?.content).toBe(event.content)
      expect(retrieved?.pubkey).toBe(event.pubkey)

      // Query by filters
      const events = yield* db.queryEvents([{
        ids: [event.id]
      }])
      expect(events.length).toBe(1)
      expect(events[0].id).toBe(event.id)
    })

    await Effect.runPromise(
      program.pipe(Effect.provide(testLayer))
    )
  }, 30000) // 30 second timeout

  test("should filter events by author", async () => {
    if (!connection) return

    const program = Effect.gen(function*() {
      const db = yield* RelayDatabase

      // Generate two test keypairs
      const keypair1 = yield* generateTestKeypair(0)
      const keypair2 = yield* generateTestKeypair(1)

      // Create events from both authors
      const event1 = yield* createTestEvent(
        keypair1.privateKey,
        keypair1.publicKey,
        "Event from author 1",
        1
      )
      event1.id = "test" + event1.id.slice(4)

      const event2 = yield* createTestEvent(
        keypair2.privateKey,
        keypair2.publicKey,
        "Event from author 2",
        1
      )
      event2.id = "test" + event2.id.slice(4)

      // Store both events
      yield* db.storeEvent(event1)
      yield* db.storeEvent(event2)

      // Query by author
      const author1Events = yield* db.queryEvents([{
        authors: [keypair1.publicKey]
      }])

      // Should only get events from author 1
      expect(author1Events.length).toBeGreaterThanOrEqual(1)
      expect(author1Events.every((e) => e.pubkey === keypair1.publicKey)).toBe(true)
    })

    await Effect.runPromise(
      program.pipe(Effect.provide(testLayer))
    )
  }, 30000)

  test("should handle tag queries", async () => {
    if (!connection) return

    const program = Effect.gen(function*() {
      const db = yield* RelayDatabase

      const { privateKey, publicKey } = yield* generateTestKeypair()

      // Create event with specific tags
      const event = yield* createTestEvent(
        privateKey,
        publicKey,
        "Tagged event",
        1,
        [["t", "nostr"], ["t", "test"], ["subject", "testing"]]
      )
      event.id = "test" + event.id.slice(4)

      // Store event
      yield* db.storeEvent(event)

      // Query by tag
      const taggedEvents = yield* db.queryEvents([{
        "#t": ["nostr"]
      }])

      expect(taggedEvents.some((e) => e.id === event.id)).toBe(true)

      // Query by multiple tag values
      const multiTagEvents = yield* db.queryEvents([{
        "#t": ["nostr", "test"]
      }])

      expect(multiTagEvents.some((e) => e.id === event.id)).toBe(true)
    })

    await Effect.runPromise(
      program.pipe(Effect.provide(testLayer))
    )
  }, 30000)

  test("should respect time-based filters", async () => {
    if (!connection) return

    const program = Effect.gen(function*() {
      const db = yield* RelayDatabase

      const { privateKey, publicKey } = yield* generateTestKeypair()

      // Create event with specific timestamp
      const now = Math.floor(Date.now() / 1000)
      const event = yield* createTestEvent(
        privateKey,
        publicKey,
        "Time-based test event",
        1
      )
      event.id = "test" + event.id.slice(4)
      event.created_at = now

      yield* db.storeEvent(event)

      // Query with since filter
      const sinceEvents = yield* db.queryEvents([{
        since: now - 60, // Last minute
        kinds: [1]
      }])

      expect(sinceEvents.some((e) => e.id === event.id)).toBe(true)

      // Query with until filter (should not include future events)
      const untilEvents = yield* db.queryEvents([{
        until: now - 120, // 2 minutes ago
        kinds: [1]
      }])

      expect(untilEvents.some((e) => e.id === event.id)).toBe(false)
    })

    await Effect.runPromise(
      program.pipe(Effect.provide(testLayer))
    )
  }, 30000)
})
