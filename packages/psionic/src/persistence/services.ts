/**
 * Effect services for PGlite persistence
 */
import { PGlite } from "@electric-sql/pglite"
import { electricSync } from "@electric-sql/pglite-sync"
import { live } from "@electric-sql/pglite/live"
import { and, desc, eq } from "drizzle-orm"
import { drizzle } from "drizzle-orm/pglite"
import { Context, Effect, Layer, Schema } from "effect"
import {
  type Conversation,
  conversations,
  type Message,
  messages,
  type NewConversation,
  type NewMessage
} from "./schema"

// Error types
export class PGliteError extends Schema.TaggedError<PGliteError>()(
  "PGliteError",
  {
    message: Schema.String,
    cause: Schema.Unknown
  }
) {}

// PGlite Service
export class PGliteService extends Context.Tag("PGliteService")<
  PGliteService,
  {
    readonly client: PGlite
    readonly db: ReturnType<typeof drizzle>
    readonly query: <T>(
      fn: (db: ReturnType<typeof drizzle>) => Promise<T>
    ) => Effect.Effect<T, PGliteError>
  }
>() {}

// Configuration for PGlite
export interface PGliteConfig {
  readonly databaseName?: string
  readonly enableSync?: boolean
  readonly syncUrl?: string
}

// Live implementation
export const PGliteServiceLive = (config: PGliteConfig = {}) =>
  Layer.effect(
    PGliteService,
    Effect.gen(function*() {
      const databaseName = config.databaseName || "openagents-chat"

      // Initialize PGlite with extensions
      const client = yield* Effect.tryPromise({
        try: async () => {
          const extensions: any = { live }
          if (config.enableSync) {
            extensions.electricSync = electricSync
          }

          const pg = new PGlite(`idb://${databaseName}`, {
            extensions
          })

          // Create tables
          await pg.exec(`
          -- Conversations table
          CREATE TABLE IF NOT EXISTS conversations (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            user_id TEXT NOT NULL DEFAULT 'local',
            title TEXT,
            model TEXT,
            last_message_at TIMESTAMP,
            created_at TIMESTAMP DEFAULT NOW(),
            archived BOOLEAN DEFAULT FALSE,
            metadata JSONB DEFAULT '{}'::jsonb
          );

          -- Messages table
          CREATE TABLE IF NOT EXISTS messages (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system', 'tool')),
            content TEXT NOT NULL,
            model TEXT,
            created_at TIMESTAMP DEFAULT NOW(),
            metadata JSONB DEFAULT '{}'::jsonb
          );

          -- Indexes
          CREATE INDEX IF NOT EXISTS idx_user_created ON conversations(user_id, created_at);
          CREATE INDEX IF NOT EXISTS idx_archived ON conversations(archived);
          CREATE INDEX IF NOT EXISTS idx_conversation_created ON messages(conversation_id, created_at);
          CREATE INDEX IF NOT EXISTS idx_role ON messages(role);
          
          -- Full text search index
          CREATE INDEX IF NOT EXISTS idx_messages_content_fts
          ON messages USING gin(to_tsvector('english', content));
        `)

          return pg
        },
        catch: (error) =>
          new PGliteError({
            message: "Failed to initialize PGlite",
            cause: error
          })
      })

      const db = drizzle(client)

      const query = <T>(
        fn: (db: ReturnType<typeof drizzle>) => Promise<T>
      ) =>
        Effect.tryPromise({
          try: () => fn(db),
          catch: (error) =>
            new PGliteError({
              message: "Query failed",
              cause: error
            })
        })

      return { client, db, query }
    })
  )

// Conversation Repository
export class ConversationRepository extends Context.Tag("ConversationRepository")<
  ConversationRepository,
  {
    create: (conversation: NewConversation) => Effect.Effect<Conversation, PGliteError>
    get: (id: string) => Effect.Effect<Conversation | undefined, PGliteError>
    list: (userId?: string, includeArchived?: boolean) => Effect.Effect<Array<Conversation>, PGliteError>
    update: (id: string, updates: Partial<Conversation>) => Effect.Effect<Conversation | undefined, PGliteError>
    delete: (id: string) => Effect.Effect<boolean, PGliteError>
    archive: (id: string) => Effect.Effect<boolean, PGliteError>
    generateTitle: (conversationId: string) => Effect.Effect<string, PGliteError>
  }
>() {}

export const ConversationRepositoryLive = Layer.effect(
  ConversationRepository,
  Effect.gen(function*() {
    const pgService = yield* PGliteService

    return {
      create: (conversation) =>
        pgService.query(async (db) => {
          const [created] = await db.insert(conversations).values(conversation).returning()
          return created
        }),

      get: (id) =>
        pgService.query(async (db) => {
          const [conversation] = await db.select().from(conversations).where(eq(conversations.id, id))
          return conversation
        }),

      list: (userId = "local", includeArchived = false) =>
        pgService.query(async (db) => {
          const conditions = [eq(conversations.userId, userId)]
          
          if (!includeArchived) {
            conditions.push(eq(conversations.archived, false))
          }

          return await db.select()
            .from(conversations)
            .where(conditions.length === 1 ? conditions[0] : and(...conditions))
            .orderBy(desc(conversations.lastMessageAt), desc(conversations.createdAt))
        }),

      update: (id, updates) =>
        pgService.query(async (db) => {
          const [updated] = await db.update(conversations)
            .set(updates)
            .where(eq(conversations.id, id))
            .returning()
          return updated
        }),

      delete: (id) =>
        pgService.query(async (db) => {
          const result = await db.delete(conversations).where(eq(conversations.id, id)).returning()
          return result.length > 0
        }),

      archive: (id) =>
        pgService.query(async (db) => {
          const [archived] = await db.update(conversations)
            .set({ archived: true })
            .where(eq(conversations.id, id))
            .returning()
          return !!archived
        }),

      generateTitle: (conversationId) =>
        pgService.query(async (db) => {
          // Get first user message
          const [firstMessage] = await db.select()
            .from(messages)
            .where(and(
              eq(messages.conversationId, conversationId),
              eq(messages.role, "user")
            ))
            .orderBy(messages.createdAt)
            .limit(1)

          if (!firstMessage) {
            return "New Conversation"
          }

          // Generate title from first message (truncate to 50 chars)
          const title = firstMessage.content.slice(0, 50) + (firstMessage.content.length > 50 ? "..." : "")

          // Update conversation with title
          await db.update(conversations)
            .set({ title })
            .where(eq(conversations.id, conversationId))

          return title
        })
    }
  })
)

// Message Repository
export class MessageRepository extends Context.Tag("MessageRepository")<
  MessageRepository,
  {
    send: (message: NewMessage) => Effect.Effect<Message, PGliteError>
    getConversation: (conversationId: string, limit?: number) => Effect.Effect<Array<Message>, PGliteError>
    search: (query: string, userId?: string) => Effect.Effect<Array<Message>, PGliteError>
    delete: (id: string) => Effect.Effect<boolean, PGliteError>
    watchConversation: (
      conversationId: string,
      callback: (messages: Array<Message>) => void
    ) => Effect.Effect<() => void, PGliteError>
  }
>() {}

export const MessageRepositoryLive = Layer.effect(
  MessageRepository,
  Effect.gen(function*() {
    const pgService = yield* PGliteService

    return {
      send: (message) =>
        pgService.query(async (db) => {
          // Insert message
          const [inserted] = await db.insert(messages).values(message).returning()

          // Update conversation last message time
          await db.update(conversations)
            .set({ lastMessageAt: new Date() })
            .where(eq(conversations.id, message.conversationId))

          return inserted
        }),

      getConversation: (conversationId, limit = 100) =>
        pgService.query(async (db) => {
          return await db.select()
            .from(messages)
            .where(eq(messages.conversationId, conversationId))
            .orderBy(desc(messages.createdAt))
            .limit(limit)
        }),

      search: (query, userId = "local") =>
        pgService.query(async () => {
          // Using PostgreSQL full-text search
          const result = await pgService.client.query(
            `
            SELECT m.* FROM messages m
            JOIN conversations c ON m.conversation_id = c.id
            WHERE c.user_id = $1
            AND to_tsvector('english', m.content) @@ plainto_tsquery('english', $2)
            ORDER BY m.created_at DESC
            LIMIT 50
          `,
            [userId, query]
          )

          return result.rows as Array<Message>
        }),

      delete: (id) =>
        pgService.query(async (db) => {
          const result = await db.delete(messages).where(eq(messages.id, id)).returning()
          return result.length > 0
        }),

      watchConversation: (conversationId, callback) =>
        Effect.sync(() => {
          // Poll for changes every 2 seconds
          const intervalId = setInterval(async () => {
            try {
              const result = await pgService.client.query<Message>(
                `SELECT * FROM messages
                 WHERE conversation_id = $1
                 ORDER BY created_at ASC`,
                [conversationId]
              )
              callback(result.rows)
            } catch (error) {
              console.error('Error polling messages:', error)
            }
          }, 2000)

          // Initial fetch
          pgService.client.query<Message>(
            `SELECT * FROM messages
             WHERE conversation_id = $1
             ORDER BY created_at ASC`,
            [conversationId]
          ).then(result => callback(result.rows))
          .catch(error => console.error('Error fetching initial messages:', error))

          // Return cleanup function
          return () => clearInterval(intervalId)
        })
    }
  })
)

// Combined layer for easy usage
export const PersistenceLive = (config?: PGliteConfig) =>
  Layer.mergeAll(
    PGliteServiceLive(config),
    ConversationRepositoryLive,
    MessageRepositoryLive
  )
