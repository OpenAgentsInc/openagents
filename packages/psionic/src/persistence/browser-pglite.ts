/**
 * Browser-safe PGlite initialization
 * This module handles PGlite setup outside of Effect to avoid Node.js dependencies
 */
import { PGlite } from "@electric-sql/pglite"
import { drizzle } from "drizzle-orm/pglite"

let pgInstance: PGlite | null = null
let dbInstance: ReturnType<typeof drizzle> | null = null
let initPromise: Promise<void> | null = null

export async function initializePGlite(databaseName = "openagents-chat") {
  if (pgInstance && dbInstance) {
    return { pg: pgInstance, db: dbInstance }
  }

  if (initPromise) {
    await initPromise
    return { pg: pgInstance!, db: dbInstance! }
  }

  initPromise = (async () => {
    try {
      // Initialize PGlite with browser-specific settings
      pgInstance = new PGlite(`idb://${databaseName}`)
      await pgInstance.waitReady

      // Initialize Drizzle
      dbInstance = drizzle(pgInstance)

      // Create tables
      await pgInstance.exec(`
        CREATE TABLE IF NOT EXISTS conversations (
          id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
          user_id TEXT NOT NULL DEFAULT 'local',
          title TEXT,
          model TEXT,
          last_message_at TIMESTAMP,
          created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
          archived BOOLEAN DEFAULT FALSE,
          metadata JSONB DEFAULT '{}'::jsonb
        );

        CREATE TABLE IF NOT EXISTS messages (
          id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
          conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
          role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system', 'tool')),
          content TEXT NOT NULL,
          model TEXT,
          created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
          metadata JSONB DEFAULT '{}'::jsonb
        );

        CREATE INDEX IF NOT EXISTS idx_conversations_user_id ON conversations(user_id);
        CREATE INDEX IF NOT EXISTS idx_conversations_created_at ON conversations(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_messages_conversation_id ON messages(conversation_id);
        CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at);
      `)
    } catch (error) {
      console.error("Failed to initialize PGlite:", error)
      throw error
    }
  })()

  await initPromise
  return { pg: pgInstance!, db: dbInstance! }
}

export function getPGliteInstances() {
  if (!pgInstance || !dbInstance) {
    throw new Error("PGlite not initialized. Call initializePGlite() first.")
  }
  return { pg: pgInstance, db: dbInstance }
}
