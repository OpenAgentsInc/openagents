/**
 * Browser-safe Effect services for chat persistence
 * Uses Effect but avoids any platform-specific dependencies
 */
import { type PGlite } from "@electric-sql/pglite"
import { and, desc, eq } from "drizzle-orm"
import { type drizzle } from "drizzle-orm/pglite"
import { Context, Effect, Layer, Schema } from "effect"
import { initializePGlite } from "./browser-pglite"
import {
  type Conversation,
  conversations,
  type Message,
  messages,
  type NewConversation,
  type NewMessage
} from "./schema"

// Error types
export class BrowserPersistenceError extends Schema.TaggedError<BrowserPersistenceError>()(
  "BrowserPersistenceError",
  {
    message: Schema.String,
    cause: Schema.Unknown
  }
) {}

// PGlite service for browser
export class BrowserPGliteService extends Context.Tag("BrowserPGliteService")<
  BrowserPGliteService,
  {
    readonly db: ReturnType<typeof drizzle>
    readonly pg: PGlite
  }
>() {}

// Repository services
export class BrowserConversationRepository extends Context.Tag("BrowserConversationRepository")<
  BrowserConversationRepository,
  {
    readonly create: (conversation: NewConversation) => Effect.Effect<Conversation, BrowserPersistenceError>
    readonly get: (id: string) => Effect.Effect<Conversation | undefined, BrowserPersistenceError>
    readonly list: (
      userId?: string,
      includeArchived?: boolean
    ) => Effect.Effect<Array<Conversation>, BrowserPersistenceError>
    readonly update: (
      id: string,
      updates: Partial<Conversation>
    ) => Effect.Effect<Conversation | undefined, BrowserPersistenceError>
    readonly delete: (id: string) => Effect.Effect<boolean, BrowserPersistenceError>
    readonly archive: (id: string) => Effect.Effect<boolean, BrowserPersistenceError>
    readonly generateTitle: (conversationId: string) => Effect.Effect<boolean, BrowserPersistenceError>
  }
>() {}

export class BrowserMessageRepository extends Context.Tag("BrowserMessageRepository")<
  BrowserMessageRepository,
  {
    readonly send: (message: NewMessage) => Effect.Effect<Message, BrowserPersistenceError>
    readonly getConversation: (
      conversationId: string,
      limit?: number
    ) => Effect.Effect<Array<Message>, BrowserPersistenceError>
    readonly delete: (id: string) => Effect.Effect<boolean, BrowserPersistenceError>
    readonly search: (query: string, userId?: string) => Effect.Effect<Array<Message>, BrowserPersistenceError>
    readonly watchConversation: (
      conversationId: string,
      callback: (messages: Array<Message>) => void
    ) => Effect.Effect<() => void, BrowserPersistenceError>
  }
>() {}

// Implementation layers
export const BrowserPGliteServiceLive = (databaseName = "openagents-chat") =>
  Layer.effect(
    BrowserPGliteService,
    Effect.gen(function*() {
      // Initialize PGlite outside of Effect context to avoid Node.js dependencies
      const { pg, db } = yield* Effect.tryPromise({
        try: () => initializePGlite(databaseName),
        catch: (error) =>
          new BrowserPersistenceError({
            message: "Failed to initialize PGlite",
            cause: error
          })
      })

      return { db, pg }
    })
  )

export const BrowserConversationRepositoryLive = Layer.effect(
  BrowserConversationRepository,
  Effect.gen(function*() {
    const { db } = yield* BrowserPGliteService

    return {
      create: (conversation) =>
        Effect.tryPromise({
          try: async () => {
            const [result] = await db.insert(conversations).values(conversation).returning()
            return result
          },
          catch: (error) =>
            new BrowserPersistenceError({
              message: "Failed to create conversation",
              cause: error
            })
        }),

      get: (id) =>
        Effect.tryPromise({
          try: async () => {
            const [result] = await db.select().from(conversations).where(eq(conversations.id, id))
            return result
          },
          catch: (error) =>
            new BrowserPersistenceError({
              message: `Failed to get conversation ${id}`,
              cause: error
            })
        }),

      list: (userId = "local", includeArchived = false) =>
        Effect.tryPromise({
          try: async () => {
            const conditions = [eq(conversations.userId, userId)]
            
            if (!includeArchived) {
              conditions.push(eq(conversations.archived, false))
            }

            return db.select()
              .from(conversations)
              .where(conditions.length === 1 ? conditions[0] : and(...conditions))
              .orderBy(desc(conversations.lastMessageAt), desc(conversations.createdAt))
          },
          catch: (error) =>
            new BrowserPersistenceError({
              message: "Failed to list conversations",
              cause: error
            })
        }),

      update: (id, updates) =>
        Effect.tryPromise({
          try: async () => {
            const [result] = await db.update(conversations)
              .set(updates)
              .where(eq(conversations.id, id))
              .returning()
            return result
          },
          catch: (error) =>
            new BrowserPersistenceError({
              message: `Failed to update conversation ${id}`,
              cause: error
            })
        }),

      delete: (id) =>
        Effect.tryPromise({
          try: async () => {
            const result = await db.delete(conversations).where(eq(conversations.id, id)).returning()
            return result.length > 0
          },
          catch: (error) =>
            new BrowserPersistenceError({
              message: `Failed to delete conversation ${id}`,
              cause: error
            })
        }),

      archive: (id) =>
        Effect.tryPromise({
          try: async () => {
            const [result] = await db.update(conversations)
              .set({ archived: true })
              .where(eq(conversations.id, id))
              .returning()
            return !!result
          },
          catch: (error) =>
            new BrowserPersistenceError({
              message: `Failed to archive conversation ${id}`,
              cause: error
            })
        }),

      generateTitle: (conversationId) =>
        Effect.gen(function*() {
          const conversation = yield* Effect.tryPromise({
            try: () => db.select().from(conversations).where(eq(conversations.id, conversationId)).then((r) => r[0]),
            catch: (error) =>
              new BrowserPersistenceError({
                message: "Failed to get conversation for title generation",
                cause: error
              })
          })

          if (!conversation || conversation.title) {
            return false
          }

          const firstMessage = yield* Effect.tryPromise({
            try: () =>
              db.select().from(messages)
                .where(eq(messages.conversationId, conversationId))
                .orderBy(messages.createdAt)
                .limit(1)
                .then((r) => r[0]),
            catch: (error) =>
              new BrowserPersistenceError({
                message: "Failed to get first message",
                cause: error
              })
          })

          if (firstMessage && firstMessage.role === "user") {
            const title = firstMessage.content.slice(0, 50) + (firstMessage.content.length > 50 ? "..." : "")
            yield* Effect.tryPromise({
              try: () =>
                db.update(conversations)
                  .set({ title })
                  .where(eq(conversations.id, conversationId)),
              catch: (error) =>
                new BrowserPersistenceError({
                  message: "Failed to update title",
                  cause: error
                })
            })
            return true
          }

          return false
        })
    }
  })
)

export const BrowserMessageRepositoryLive = Layer.effect(
  BrowserMessageRepository,
  Effect.gen(function*() {
    const { db, pg } = yield* BrowserPGliteService

    return {
      send: (message) =>
        Effect.tryPromise({
          try: async () => {
            return await db.transaction(async (tx) => {
              const [newMessage] = await tx.insert(messages).values(message).returning()

              await tx.update(conversations)
                .set({ lastMessageAt: new Date() })
                .where(eq(conversations.id, message.conversationId))

              return newMessage
            })
          },
          catch: (error) =>
            new BrowserPersistenceError({
              message: "Failed to send message",
              cause: error
            })
        }),

      getConversation: (conversationId, limit) =>
        Effect.tryPromise({
          try: async () => {
            const baseQuery = db.select().from(messages)
              .where(eq(messages.conversationId, conversationId))
              .orderBy(desc(messages.createdAt))

            const results = limit 
              ? await baseQuery.limit(limit)
              : await baseQuery

            // Reverse to get chronological order
            return results.reverse()
          },
          catch: (error) =>
            new BrowserPersistenceError({
              message: `Failed to get messages for conversation ${conversationId}`,
              cause: error
            })
        }),

      delete: (id) =>
        Effect.tryPromise({
          try: async () => {
            const result = await db.delete(messages).where(eq(messages.id, id)).returning()
            return result.length > 0
          },
          catch: (error) =>
            new BrowserPersistenceError({
              message: `Failed to delete message ${id}`,
              cause: error
            })
        }),

      search: (query, userId = "local") =>
        Effect.tryPromise({
          try: async () => {
            const result = await pg.query(
              `
            SELECT m.* FROM messages m
            JOIN conversations c ON m.conversation_id = c.id
            WHERE c.user_id = $1
            AND m.content ILIKE $2
            ORDER BY m.created_at DESC
            LIMIT 50
          `,
              [userId, `%${query}%`]
            )

            return result.rows as Array<Message>
          },
          catch: (error) =>
            new BrowserPersistenceError({
              message: "Failed to search messages",
              cause: error
            })
        }),

      watchConversation: (conversationId, callback) =>
        Effect.sync(() => {
          // Simple polling implementation for now
          // TODO: Use PGlite live queries when available
          let lastCount = 0
          const checkInterval = setInterval(async () => {
            try {
              const messageList = await db.select()
                .from(messages)
                .where(eq(messages.conversationId, conversationId))
                .orderBy(messages.createdAt)

              if (messageList.length !== lastCount) {
                lastCount = messageList.length
                callback(messageList)
              }
            } catch (error) {
              console.error("Watch error:", error)
            }
          }, 1000)

          return () => clearInterval(checkInterval)
        })
    }
  })
)

// Combined persistence layer for browser
export const BrowserPersistenceLive = (databaseName?: string) =>
  BrowserConversationRepositoryLive.pipe(
    Layer.provide(BrowserMessageRepositoryLive),
    Layer.provide(BrowserPGliteServiceLive(databaseName))
  )
