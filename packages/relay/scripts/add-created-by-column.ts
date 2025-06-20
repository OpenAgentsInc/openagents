#!/usr/bin/env bun

/**
 * Migration script to add created_by column to channels table
 * Run: bun scripts/add-created-by-column.ts
 */

import { sql } from "drizzle-orm"
import { drizzle } from "drizzle-orm/mysql2"
import mysql from "mysql2/promise"

async function addCreatedByColumn() {
  console.log("🔧 Adding created_by column to channels table...")

  // Create database connection
  const connection = await mysql.createConnection({
    host: process.env.DATABASE_HOST || "aws.connect.psdb.cloud",
    username: process.env.DATABASE_USERNAME!,
    password: process.env.DATABASE_PASSWORD!,
    database: process.env.DATABASE_NAME || "openagents-com",
    ssl: { rejectUnauthorized: true }
  })

  const db = drizzle(connection)

  try {
    // Check if column already exists
    console.log("📋 Checking current table structure...")
    const [columns] = await connection.execute(
      `SHOW COLUMNS FROM channels LIKE 'created_by'`
    ) as [Array<any>, any]

    if (columns.length > 0) {
      console.log("✅ created_by column already exists")
      await connection.end()
      return
    }

    // Add the created_by column
    console.log("📝 Adding created_by column...")
    await db.execute(sql`
      ALTER TABLE channels 
      ADD COLUMN created_by VARCHAR(64) NOT NULL DEFAULT ''
    `)

    // Update existing rows to copy creator_pubkey to created_by
    console.log("🔄 Copying creator_pubkey values to created_by...")
    await db.execute(sql`
      UPDATE channels 
      SET created_by = creator_pubkey 
      WHERE created_by = ''
    `)

    console.log("✅ Successfully added created_by column and updated existing data")
  } catch (error) {
    console.error("❌ Migration failed:", error)
    throw error
  } finally {
    await connection.end()
  }
}

// Run the migration
addCreatedByColumn().catch((error) => {
  console.error("Migration failed:", error)
  process.exit(1)
})
