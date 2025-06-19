/**
 * Browser-safe Effect services for chat persistence
 * Uses Effect but avoids any platform-specific dependencies
 */
import { Effect, Layer, Context, Data } from 'effect'
import { PGlite } from '@electric-sql/pglite'
import { drizzle } from 'drizzle-orm/pglite'
import { eq, desc } from 'drizzle-orm'
import { 
  conversations, 
  messages, 
  type Conversation, 
  type Message, 
  type NewConversation, 
  type NewMessage 
} from './schema'

// Error types
export class BrowserPersistenceError extends Data.TaggedError<BrowserPersistenceError>('BrowserPersistenceError')<{
  message: string
  cause?: unknown
}> {}

// PGlite service for browser
export class BrowserPGliteService extends Context.Tag('BrowserPGliteService')<
  BrowserPGliteService,
  {
    readonly db: ReturnType<typeof drizzle>
    readonly pg: PGlite
  }
>() {}

// Repository services
export class BrowserConversationRepository extends Context.Tag('BrowserConversationRepository')<
  BrowserConversationRepository,
  {
    readonly create: (conversation: NewConversation) => Effect.Effect<Conversation, BrowserPersistenceError>
    readonly get: (id: string) => Effect.Effect<Conversation | undefined, BrowserPersistenceError>
    readonly list: (userId?: string, includeArchived?: boolean) => Effect.Effect<Conversation[], BrowserPersistenceError>
    readonly update: (id: string, updates: Partial<Conversation>) => Effect.Effect<Conversation | undefined, BrowserPersistenceError>
    readonly delete: (id: string) => Effect.Effect<boolean, BrowserPersistenceError>
    readonly archive: (id: string) => Effect.Effect<boolean, BrowserPersistenceError>
    readonly generateTitle: (conversationId: string) => Effect.Effect<boolean, BrowserPersistenceError>
  }
>() {}

export class BrowserMessageRepository extends Context.Tag('BrowserMessageRepository')<
  BrowserMessageRepository,
  {
    readonly send: (message: NewMessage) => Effect.Effect<Message, BrowserPersistenceError>
    readonly getConversation: (conversationId: string, limit?: number) => Effect.Effect<Message[], BrowserPersistenceError>
    readonly delete: (id: string) => Effect.Effect<boolean, BrowserPersistenceError>
    readonly search: (query: string, userId?: string) => Effect.Effect<Message[], BrowserPersistenceError>
    readonly watchConversation: (conversationId: string, callback: (messages: Message[]) => void) => Effect.Effect<() => void, BrowserPersistenceError>
  }
>() {}

// Implementation layers
export const BrowserPGliteServiceLive = (databaseName = 'openagents-chat') => Layer.effect(
  BrowserPGliteService,
  Effect.gen(function* () {
    const pg = yield* Effect.tryPromise({
      try: async () => {
        const client = new PGlite(`idb://${databaseName}`)
        await client.waitReady
        
        // Initialize tables
        await client.exec(`
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
        
        return client
      },
      catch: (error) => new BrowserPersistenceError({ 
        message: 'Failed to initialize PGlite', 
        cause: error 
      })
    })
    
    const db = drizzle(pg)
    
    return { db, pg }
  })
)

export const BrowserConversationRepositoryLive = Layer.effect(
  BrowserConversationRepository,
  Effect.gen(function* () {
    const { db } = yield* BrowserPGliteService
    
    return {
      create: (conversation) => Effect.tryPromise({
        try: async () => {
          const [result] = await db.insert(conversations).values(conversation).returning()
          return result
        },
        catch: (error) => new BrowserPersistenceError({ 
          message: 'Failed to create conversation', 
          cause: error 
        })
      }),
      
      get: (id) => Effect.tryPromise({
        try: async () => {
          const [result] = await db.select().from(conversations).where(eq(conversations.id, id))
          return result
        },
        catch: (error) => new BrowserPersistenceError({ 
          message: `Failed to get conversation ${id}`, 
          cause: error 
        })
      }),
      
      list: (userId = 'local', includeArchived = false) => Effect.tryPromise({
        try: async () => {
          let query = db.select().from(conversations).where(eq(conversations.userId, userId))
          
          if (!includeArchived) {
            query = query.where(eq(conversations.archived, false))
          }
          
          return query.orderBy(desc(conversations.lastMessageAt), desc(conversations.createdAt))
        },
        catch: (error) => new BrowserPersistenceError({ 
          message: 'Failed to list conversations', 
          cause: error 
        })
      }),
      
      update: (id, updates) => Effect.tryPromise({
        try: async () => {
          const [result] = await db.update(conversations)
            .set(updates)
            .where(eq(conversations.id, id))
            .returning()
          return result
        },
        catch: (error) => new BrowserPersistenceError({ 
          message: `Failed to update conversation ${id}`, 
          cause: error 
        })
      }),
      
      delete: (id) => Effect.tryPromise({
        try: async () => {
          const result = await db.delete(conversations).where(eq(conversations.id, id))
          return result.rowsAffected > 0
        },
        catch: (error) => new BrowserPersistenceError({ 
          message: `Failed to delete conversation ${id}`, 
          cause: error 
        })
      }),
      
      archive: (id) => Effect.tryPromise({
        try: async () => {
          const [result] = await db.update(conversations)
            .set({ archived: true })
            .where(eq(conversations.id, id))
            .returning()
          return !!result
        },
        catch: (error) => new BrowserPersistenceError({ 
          message: `Failed to archive conversation ${id}`, 
          cause: error 
        })
      }),
      
      generateTitle: (conversationId) => Effect.gen(function* () {
        const conversation = yield* Effect.tryPromise({
          try: () => db.select().from(conversations).where(eq(conversations.id, conversationId)).then(r => r[0]),
          catch: (error) => new BrowserPersistenceError({ 
            message: 'Failed to get conversation for title generation', 
            cause: error 
          })
        })
        
        if (!conversation || conversation.title) {
          return false
        }
        
        const firstMessage = yield* Effect.tryPromise({
          try: () => db.select().from(messages)
            .where(eq(messages.conversationId, conversationId))
            .orderBy(messages.createdAt)
            .limit(1)
            .then(r => r[0]),
          catch: (error) => new BrowserPersistenceError({ 
            message: 'Failed to get first message', 
            cause: error 
          })
        })
        
        if (firstMessage && firstMessage.role === 'user') {
          const title = firstMessage.content.slice(0, 50) + (firstMessage.content.length > 50 ? '...' : '')
          yield* Effect.tryPromise({
            try: () => db.update(conversations)
              .set({ title })
              .where(eq(conversations.id, conversationId)),
            catch: (error) => new BrowserPersistenceError({ 
              message: 'Failed to update title', 
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
  Effect.gen(function* () {
    const { db, pg } = yield* BrowserPGliteService
    
    return {
      send: (message) => Effect.tryPromise({
        try: async () => {
          return await db.transaction(async (tx) => {
            const [newMessage] = await tx.insert(messages).values(message).returning()
            
            await tx.update(conversations)
              .set({ lastMessageAt: new Date() })
              .where(eq(conversations.id, message.conversationId))
            
            return newMessage
          })
        },
        catch: (error) => new BrowserPersistenceError({ 
          message: 'Failed to send message', 
          cause: error 
        })
      }),
      
      getConversation: (conversationId, limit) => Effect.tryPromise({
        try: async () => {
          let query = db.select().from(messages)
            .where(eq(messages.conversationId, conversationId))
            .orderBy(desc(messages.createdAt))
          
          if (limit) {
            query = query.limit(limit)
          }
          
          // Reverse to get chronological order
          const results = await query
          return results.reverse()
        },
        catch: (error) => new BrowserPersistenceError({ 
          message: `Failed to get messages for conversation ${conversationId}`, 
          cause: error 
        })
      }),
      
      delete: (id) => Effect.tryPromise({
        try: async () => {
          const result = await db.delete(messages).where(eq(messages.id, id))
          return result.rowsAffected > 0
        },
        catch: (error) => new BrowserPersistenceError({ 
          message: `Failed to delete message ${id}`, 
          cause: error 
        })
      }),
      
      search: (query, userId = 'local') => Effect.tryPromise({
        try: async () => {
          const result = await pg.query(`
            SELECT m.* FROM messages m
            JOIN conversations c ON m.conversation_id = c.id
            WHERE c.user_id = $1
            AND m.content ILIKE $2
            ORDER BY m.created_at DESC
            LIMIT 50
          `, [userId, `%${query}%`])
          
          return result.rows as Message[]
        },
        catch: (error) => new BrowserPersistenceError({ 
          message: 'Failed to search messages', 
          cause: error 
        })
      }),
      
      watchConversation: (conversationId, callback) => Effect.sync(() => {
        // Simple polling implementation for now
        // TODO: Use PGlite live queries when available
        let lastCount = 0
        const checkInterval = setInterval(async () => {
          try {
            const messages = await db.select().from(messages)
              .where(eq(messages.conversationId, conversationId))
              .orderBy(messages.createdAt)
            
            if (messages.length !== lastCount) {
              lastCount = messages.length
              callback(messages)
            }
          } catch (error) {
            console.error('Watch error:', error)
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