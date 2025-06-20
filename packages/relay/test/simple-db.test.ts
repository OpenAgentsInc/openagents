import { eq } from "drizzle-orm"
import { drizzle } from "drizzle-orm/mysql2"
import mysql from "mysql2/promise"
import { describe, expect, test } from "vitest"
import * as schema from "../src/schema.js"

const TEST_TIMEOUT = 30000

describe("Simple Database Connection Test", () => {
  test("should connect and perform basic operations", { timeout: TEST_TIMEOUT }, async () => {
    // Skip if no credentials
    if (!process.env.DATABASE_HOST || process.env.DATABASE_HOST === "localhost") {
      console.log("⚠️  Skipping - no database credentials")
      return
    }

    const connection = await mysql.createConnection({
      host: process.env.DATABASE_HOST,
      user: process.env.DATABASE_USERNAME,
      password: process.env.DATABASE_PASSWORD,
      database: process.env.DATABASE_NAME,
      ssl: { rejectUnauthorized: false }
    })

    const db = drizzle(connection, { schema, mode: "default" })

    // Create test event with proper length constraints
    const testId = "simpletest" + Date.now().toString(16)
    const paddedId = testId.padEnd(64, "0").slice(0, 64)

    const testEvent = {
      id: paddedId,
      pubkey: "testpubkey".padEnd(64, "0").slice(0, 64),
      created_at: Math.floor(Date.now() / 1000),
      kind: 1,
      tags: JSON.stringify([["test", "simple"]]),
      content: "Simple database test",
      sig: "0".repeat(128)
    }

    // Insert
    await db.insert(schema.events).values(testEvent)

    // Query back
    const events = await db.select()
      .from(schema.events)
      .where(eq(schema.events.id, paddedId))
      .limit(1)

    expect(events.length).toBe(1)
    expect(events[0].content).toBe(testEvent.content)

    // Clean up
    await db.delete(schema.events).where(eq(schema.events.id, paddedId))

    await connection.end()
  })

  test("should verify database schema", { timeout: TEST_TIMEOUT }, async () => {
    if (!process.env.DATABASE_HOST || process.env.DATABASE_HOST === "localhost") {
      return
    }

    const connection = await mysql.createConnection({
      host: process.env.DATABASE_HOST,
      user: process.env.DATABASE_USERNAME,
      password: process.env.DATABASE_PASSWORD,
      database: process.env.DATABASE_NAME,
      ssl: { rejectUnauthorized: false }
    })

    // Check tables exist
    const [tables] = await connection.query(
      `
      SELECT table_name 
      FROM information_schema.tables 
      WHERE table_schema = ?
      ORDER BY table_name
    `,
      [process.env.DATABASE_NAME]
    )

    const tableNames = (tables as Array<any>).map((t) => t.TABLE_NAME || t.table_name)

    // Expected tables
    const expectedTables = ["events", "event_tags", "agent_profiles", "service_offerings", "channels"]
    expectedTables.forEach((table) => {
      expect(tableNames).toContain(table)
    })

    await connection.end()
  })
})
