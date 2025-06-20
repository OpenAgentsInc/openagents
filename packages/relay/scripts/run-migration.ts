/**
 * Database migration script for PlanetScale
 * Handles adding missing columns to existing tables
 */
import { Client } from "@planetscale/database"
import "dotenv/config"

const client = new Client({
  host: process.env.DATABASE_HOST!,
  username: process.env.DATABASE_USERNAME!,
  password: process.env.DATABASE_PASSWORD!,
  database: process.env.DATABASE_NAME!
})

async function runMigration() {
  console.log("🚀 Starting database migration...\n")

  try {
    // Check if channels table exists
    const tablesResult = await client.execute("SHOW TABLES LIKE \"channels\"")

    if (tablesResult.rows.length === 0) {
      console.log("📝 Channels table does not exist, creating from scratch...")
      // Run the full migration SQL
      const migrationSql = `
        CREATE TABLE channels (
          id varchar(64) NOT NULL,
          name varchar(255),
          about text,
          picture varchar(500),
          creator_pubkey varchar(64) NOT NULL,
          message_count bigint DEFAULT 0,
          last_message_at timestamp,
          created_at timestamp NOT NULL DEFAULT (now()),
          updated_at timestamp NOT NULL DEFAULT (now()) ON UPDATE CURRENT_TIMESTAMP,
          CONSTRAINT channels_id PRIMARY KEY(id)
        );

        CREATE INDEX idx_name ON channels (name);
        CREATE INDEX idx_creator ON channels (creator_pubkey);
        CREATE INDEX idx_last_message ON channels (last_message_at);
        CREATE INDEX idx_message_count ON channels (message_count);
      `

      const statements = migrationSql.split(";").filter((s) => s.trim())
      for (const statement of statements) {
        if (statement.trim()) {
          await client.execute(statement.trim())
        }
      }
      console.log("✅ Channels table created successfully")
    } else {
      console.log("📝 Channels table exists, checking for missing columns...")

      // Get current columns
      const columnsResult = await client.execute("DESCRIBE channels")
      const existingColumns = columnsResult.rows.map((row: any) => row.Field)

      console.log("Current columns:", existingColumns)

      // Check for missing columns and add them
      const requiredColumns = [
        { name: "creator_pubkey", sql: "ADD COLUMN creator_pubkey VARCHAR(64) NOT NULL DEFAULT \"\"" },
        { name: "message_count", sql: "ADD COLUMN message_count BIGINT DEFAULT 0" },
        { name: "last_message_at", sql: "ADD COLUMN last_message_at TIMESTAMP NULL" },
        {
          name: "updated_at",
          sql: "ADD COLUMN updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP"
        }
      ]

      for (const column of requiredColumns) {
        if (!existingColumns.includes(column.name)) {
          console.log(`➕ Adding missing column: ${column.name}`)
          await client.execute(`ALTER TABLE channels ${column.sql}`)
        } else {
          console.log(`✅ Column ${column.name} already exists`)
        }
      }

      // Add missing indexes
      const missingIndexes = [
        { name: "idx_creator", sql: "CREATE INDEX IF NOT EXISTS idx_creator ON channels(creator_pubkey)" },
        { name: "idx_last_message", sql: "CREATE INDEX IF NOT EXISTS idx_last_message ON channels(last_message_at)" },
        { name: "idx_message_count", sql: "CREATE INDEX IF NOT EXISTS idx_message_count ON channels(message_count)" }
      ]

      for (const index of missingIndexes) {
        try {
          console.log(`📊 Adding index: ${index.name}`)
          await client.execute(index.sql)
        } catch {
          // Index might already exist, ignore
          console.log(`ℹ️  Index ${index.name} may already exist`)
        }
      }
    }

    // Verify final structure
    console.log("\n🔍 Verifying final table structure...")
    const finalColumns = await client.execute("DESCRIBE channels")
    console.log("\nFinal channels table structure:")
    finalColumns.rows.forEach((row: any) => {
      console.log(
        `  ${row.Field} (${row.Type}) ${row.Null === "NO" ? "NOT NULL" : "NULL"} ${
          row.Default ? `DEFAULT ${row.Default}` : ""
        }`
      )
    })

    console.log("\n✅ Migration completed successfully!")
  } catch (error) {
    console.error("❌ Migration failed:", error)
    process.exit(1)
  }
}

// Run the migration
runMigration().then(() => {
  console.log("🎉 All done!")
  process.exit(0)
})
