```typescript
// packages/chat-persistence/src/index.ts
import { PGlite } from '@electric-sql/pglite';
import { live } from '@electric-sql/pglite/live';
import { electricSync } from '@electric-sql/pglite-sync';
import { Effect, Layer, Context, pipe, Schema } from 'effect';
import { drizzle } from 'drizzle-orm/pglite';
import { pgTable, text, timestamp, jsonb, uuid, boolean, index } from 'drizzle-orm/pg-core';
import { eq, desc, and, gte } from 'drizzle-orm';
import type { InferSelectModel, InferInsertModel } from 'drizzle-orm';

// Schema definitions using Drizzle
export const messages = pgTable('messages', {
  id: uuid('id').defaultRandom().primaryKey(),
  conversationId: uuid('conversation_id').notNull(),
  userId: text('user_id').notNull(),
  content: text('content').notNull(),
  metadata: jsonb('metadata').$type<{
    model?: string;
    tokens?: number;
    attachments?: Array<{ type: string; url: string }>;
  }>(),
  createdAt: timestamp('created_at').defaultNow().notNull(),
  updatedAt: timestamp('updated_at').defaultNow().notNull(),
  deleted: boolean('deleted').default(false),
}, (table) => ({
  conversationIdx: index('idx_conversation_created').on(table.conversationId, table.createdAt),
  userIdx: index('idx_user_created').on(table.userId, table.createdAt),
}));

export const conversations = pgTable('conversations', {
  id: uuid('id').defaultRandom().primaryKey(),
  userId: text('user_id').notNull(),
  title: text('title'),
  lastMessageAt: timestamp('last_message_at'),
  metadata: jsonb('metadata').$type<{
    participants?: string[];
    tags?: string[];
  }>(),
  createdAt: timestamp('created_at').defaultNow().notNull(),
  archived: boolean('archived').default(false),
});

// Types
export type Message = InferSelectModel<typeof messages>;
export type NewMessage = InferInsertModel<typeof messages>;
export type Conversation = InferSelectModel<typeof conversations>;

// PGlite Service with Effect
export class PGliteError extends Schema.TaggedError<PGliteError>()(
  "PGliteError",
  {
    message: Schema.String,
    cause: Schema.Unknown,
  }
) {}

export class PGliteService extends Context.Tag("PGliteService")<
  PGliteService,
  {
    readonly client: PGlite;
    readonly db: ReturnType<typeof drizzle>;
    readonly query: <T>(
      fn: (db: ReturnType<typeof drizzle>) => Promise<T>
    ) => Effect.Effect<T, PGliteError>;
  }
>() {}

// Live implementation
export const PGliteServiceLive = Layer.effect(
  PGliteService,
  Effect.gen(function* () {
    // Initialize PGlite with extensions
    const client = yield* Effect.tryPromise({
      try: async () => {
        const pg = new PGlite('idb://openagents-chat', {
          extensions: {
            live,
            electricSync,
          }
        });

        // Create tables
        await pg.exec(`
          CREATE TABLE IF NOT EXISTS messages (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            conversation_id UUID NOT NULL,
            user_id TEXT NOT NULL,
            content TEXT NOT NULL,
            metadata JSONB,
            created_at TIMESTAMP DEFAULT NOW(),
            updated_at TIMESTAMP DEFAULT NOW(),
            deleted BOOLEAN DEFAULT FALSE
          );

          CREATE TABLE IF NOT EXISTS conversations (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            user_id TEXT NOT NULL,
            title TEXT,
            last_message_at TIMESTAMP,
            metadata JSONB,
            created_at TIMESTAMP DEFAULT NOW(),
            archived BOOLEAN DEFAULT FALSE
          );

          -- Full text search
          CREATE INDEX IF NOT EXISTS idx_messages_content_fts
          ON messages USING gin(to_tsvector('english', content));
        `);

        return pg;
      },
      catch: (error) => new PGliteError({
        message: "Failed to initialize PGlite",
        cause: error
      })
    });

    const db = drizzle(client);

    const query = <T>(
      fn: (db: ReturnType<typeof drizzle>) => Promise<T>
    ) => Effect.tryPromise({
      try: () => fn(db),
      catch: (error) => new PGliteError({
        message: "Query failed",
        cause: error
      })
    });

    return { client, db, query };
  })
);

// Message Repository
export class MessageRepository extends Context.Tag("MessageRepository")<
  MessageRepository,
  {
    send: (message: NewMessage) => Effect.Effect<Message, PGliteError>;
    getConversation: (conversationId: string, limit?: number) => Effect.Effect<Message[], PGliteError>;
    search: (query: string, userId: string) => Effect.Effect<Message[], PGliteError>;
    watchConversation: (conversationId: string, callback: (messages: Message[]) => void) => Effect.Effect<() => void, PGliteError>;
  }
>() {}

export const MessageRepositoryLive = Layer.effect(
  MessageRepository,
  Effect.gen(function* () {
    const pgService = yield* PGliteService;

    return {
      send: (message) =>
        pgService.query(async (db) => {
          const [inserted] = await db.insert(messages).values(message).returning();

          // Update conversation last message time
          await db.update(conversations)
            .set({ lastMessageAt: new Date() })
            .where(eq(conversations.id, message.conversationId));

          return inserted;
        }),

      getConversation: (conversationId, limit = 50) =>
        pgService.query(async (db) => {
          return await db.select()
            .from(messages)
            .where(and(
              eq(messages.conversationId, conversationId),
              eq(messages.deleted, false)
            ))
            .orderBy(desc(messages.createdAt))
            .limit(limit);
        }),

      search: (query, userId) =>
        pgService.query(async (db) => {
          // Using PostgreSQL full-text search
          const result = await pgService.client.query(`
            SELECT * FROM messages
            WHERE user_id = $1
            AND deleted = false
            AND to_tsvector('english', content) @@ plainto_tsquery('english', $2)
            ORDER BY created_at DESC
            LIMIT 50
          `, [userId, query]);

          return result.rows as Message[];
        }),

      watchConversation: (conversationId, callback) =>
        Effect.gen(function* () {
          const { client } = yield* PGliteService;

          // Use PGlite's live query feature
          const query = client.live.query<Message>(
            `SELECT * FROM messages
             WHERE conversation_id = $1 AND deleted = false
             ORDER BY created_at DESC`,
            [conversationId],
            (result) => callback(result.rows)
          );

          // Return cleanup function
          return () => query.unsubscribe();
        })
    };
  })
);

// Sync Service for Electric SQL
export class SyncService extends Context.Tag("SyncService")<
  SyncService,
  {
    start: () => Effect.Effect<void, PGliteError>;
    stop: () => Effect.Effect<void, PGliteError>;
    syncStatus: () => Effect.Effect<{ connected: boolean; lastSync: Date | null }, PGliteError>;
  }
>() {}

export const SyncServiceLive = Layer.effect(
  SyncService,
  Effect.gen(function* () {
    const pgService = yield* PGliteService;
    let electricClient: any = null;

    return {
      start: () => Effect.gen(function* () {
        const { client } = yield* PGliteService;

        // Initialize Electric sync
        electricClient = await client.electric.syncShapeToTable({
          shape: {
            url: process.env.ELECTRIC_URL || 'http://localhost:3000/v1/shape',
            table: 'messages',
            where: `user_id = '${getCurrentUserId()}'`,
          },
          table: 'messages',
          primaryKey: ['id'],
        });

        // Sync conversations too
        await client.electric.syncShapeToTable({
          shape: {
            url: process.env.ELECTRIC_URL || 'http://localhost:3000/v1/shape',
            table: 'conversations',
            where: `user_id = '${getCurrentUserId()}'`,
          },
          table: 'conversations',
          primaryKey: ['id'],
        });
      }),

      stop: () => Effect.gen(function* () {
        if (electricClient) {
          electricClient.stop();
          electricClient = null;
        }
      }),

      syncStatus: () => Effect.succeed({
        connected: electricClient !== null,
        lastSync: new Date(), // Would track actual sync times
      })
    };
  })
);

// Usage with Elysia/Bun HTTP server
export const createChatHandlers = (app: any) => {
  // Initialize services
  const runtime = Effect.runSync(
    Effect.gen(function* () {
      const pgService = yield* PGliteService;
      const messageRepo = yield* MessageRepository;
      const syncService = yield* SyncService;

      // Start sync
      yield* syncService.start();

      return { pgService, messageRepo, syncService };
    }).pipe(
      Effect.provide(
        Layer.mergeAll(
          PGliteServiceLive,
          MessageRepositoryLive,
          SyncServiceLive
        )
      )
    )
  );

  // HTTP endpoints
  app.post('/messages', async ({ body }: any) => {
    const result = await Effect.runPromise(
      runtime.messageRepo.send(body)
    );
    return result;
  });

  app.get('/conversations/:id/messages', async ({ params }: any) => {
    const messages = await Effect.runPromise(
      runtime.messageRepo.getConversation(params.id)
    );
    return messages;
  });

  app.get('/messages/search', async ({ query }: any) => {
    const results = await Effect.runPromise(
      runtime.messageRepo.search(query.q, query.userId)
    );
    return results;
  });

  // WebSocket for live updates
  app.ws('/conversations/:id/live', {
    open(ws: any) {
      const conversationId = ws.data.params.id;

      Effect.runPromise(
        runtime.messageRepo.watchConversation(
          conversationId,
          (messages) => ws.send(JSON.stringify(messages))
        )
      ).then(unsubscribe => {
        ws.data.unsubscribe = unsubscribe;
      });
    },
    close(ws: any) {
      if (ws.data.unsubscribe) {
        ws.data.unsubscribe();
      }
    }
  });
};

// Vanilla TypeScript usage (no framework)
export class ChatClient {
  private runtime: any;
  private listeners: Map<string, Set<(messages: Message[]) => void>> = new Map();

  constructor() {
    this.initialize();
  }

  private async initialize() {
    this.runtime = await Effect.runPromise(
      Effect.gen(function* () {
        return {
          messageRepo: yield* MessageRepository,
          syncService: yield* SyncService,
        };
      }).pipe(
        Effect.provide(
          Layer.mergeAll(
            PGliteServiceLive,
            MessageRepositoryLive,
            SyncServiceLive
          )
        )
      )
    );

    // Start sync
    await Effect.runPromise(this.runtime.syncService.start());
  }

  async sendMessage(message: NewMessage): Promise<Message> {
    return Effect.runPromise(this.runtime.messageRepo.send(message));
  }

  async getMessages(conversationId: string, limit?: number): Promise<Message[]> {
    return Effect.runPromise(
      this.runtime.messageRepo.getConversation(conversationId, limit)
    );
  }

  async searchMessages(query: string, userId: string): Promise<Message[]> {
    return Effect.runPromise(
      this.runtime.messageRepo.search(query, userId)
    );
  }

  // Subscribe to live updates
  subscribeToConversation(
    conversationId: string,
    callback: (messages: Message[]) => void
  ): () => void {
    Effect.runPromise(
      this.runtime.messageRepo.watchConversation(conversationId, callback)
    ).then(unsubscribe => {
      // Store for cleanup
      if (!this.listeners.has(conversationId)) {
        this.listeners.set(conversationId, new Set());
      }
      this.listeners.get(conversationId)!.add(callback);
    });

    // Return unsubscribe function
    return () => {
      const listeners = this.listeners.get(conversationId);
      if (listeners) {
        listeners.delete(callback);
      }
    };
  }
}

// Helper function (implement based on your auth)
function getCurrentUserId(): string {
  // Get from your auth system
  return 'current-user-id';
}

// Export everything needed
export { PGlite, Effect, Layer, Schema };
```
