/**
 * Browser-safe chat persistence client using PGlite directly
 * This avoids Effect runtime to prevent Node.js dependencies in the browser
 */
import { PGlite } from "@electric-sql/pglite"
import { and, desc, eq } from "drizzle-orm"
import { drizzle } from "drizzle-orm/pglite"
import {
  type Conversation,
  conversations,
  type Message,
  messages,
  type NewConversation,
  type NewMessage
} from "./schema"

export class BrowserChatClient {
  private db: ReturnType<typeof drizzle> | null = null
  private pg: PGlite | null = null
  private initialized = false
  private initPromise: Promise<void> | null = null

  constructor(private databaseName = "openagents-chat") {}

  private async init() {
    if (this.initialized) return
    if (this.initPromise) return this.initPromise

    this.initPromise = this._init()
    await this.initPromise
  }

  private async _init() {
    try {
      // Initialize PGlite with IndexedDB storage
      this.pg = new PGlite(`idb://${this.databaseName}`)
      await this.pg.waitReady

      // Initialize Drizzle
      this.db = drizzle(this.pg)

      // Create tables if they don't exist
      await this.pg.exec(`
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

        -- Create indexes for performance
        CREATE INDEX IF NOT EXISTS idx_conversations_user_id ON conversations(user_id);
        CREATE INDEX IF NOT EXISTS idx_conversations_created_at ON conversations(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_messages_conversation_id ON messages(conversation_id);
        CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at);
        CREATE INDEX IF NOT EXISTS idx_messages_content_search ON messages USING gin(to_tsvector('english', content));
      `)

      this.initialized = true
    } catch (error) {
      console.error("Failed to initialize PGlite:", error)
      throw error
    }
  }

  async createConversation(data: Partial<NewConversation> = {}): Promise<Conversation> {
    await this.init()
    if (!this.db) throw new Error("Database not initialized")

    const conversation: NewConversation = {
      userId: data.userId || "local",
      title: data.title || null,
      model: data.model || null,
      metadata: data.metadata || {},
      archived: false
    }

    const result = await this.db.insert(conversations).values(conversation).returning()
    return result[0]
  }

  async getConversation(id: string): Promise<Conversation | undefined> {
    await this.init()
    if (!this.db) throw new Error("Database not initialized")

    const result = await this.db.select().from(conversations).where(eq(conversations.id, id))
    return result[0]
  }

  async listConversations(userId = "local", includeArchived = false): Promise<Array<Conversation>> {
    await this.init()
    if (!this.db) throw new Error("Database not initialized")

    const conditions = [eq(conversations.userId, userId)]
    
    if (!includeArchived) {
      conditions.push(eq(conversations.archived, false))
    }
    
    return this.db.select()
      .from(conversations)
      .where(conditions.length === 1 ? conditions[0] : and(...conditions))
      .orderBy(desc(conversations.lastMessageAt), desc(conversations.createdAt))
  }

  async updateConversation(id: string, updates: Partial<Conversation>): Promise<Conversation | undefined> {
    await this.init()
    if (!this.db) throw new Error("Database not initialized")

    const result = await this.db.update(conversations)
      .set(updates)
      .where(eq(conversations.id, id))
      .returning()

    return result[0]
  }

  async deleteConversation(id: string): Promise<boolean> {
    await this.init()
    if (!this.db) throw new Error("Database not initialized")

    const result = await this.db.delete(conversations).where(eq(conversations.id, id)).returning()
    return result.length > 0
  }

  async archiveConversation(id: string): Promise<boolean> {
    await this.init()
    if (!this.db) throw new Error("Database not initialized")

    const result = await this.db.update(conversations)
      .set({ archived: true })
      .where(eq(conversations.id, id))
      .returning()

    return result.length > 0
  }

  async sendMessage(message: NewMessage): Promise<Message> {
    await this.init()
    if (!this.db) throw new Error("Database not initialized")

    // Start a transaction
    return await this.db.transaction(async (tx) => {
      // Insert the message
      const [newMessage] = await tx.insert(messages).values(message).returning()

      // Update conversation's lastMessageAt
      await tx.update(conversations)
        .set({ lastMessageAt: new Date() })
        .where(eq(conversations.id, message.conversationId))

      // Auto-generate title if this is the first user message
      if (message.role === "user") {
        const conversation = await tx.select().from(conversations)
          .where(eq(conversations.id, message.conversationId))
          .limit(1)

        if (conversation[0] && !conversation[0].title) {
          const title = message.content.slice(0, 50) + (message.content.length > 50 ? "..." : "")
          await tx.update(conversations)
            .set({ title })
            .where(eq(conversations.id, message.conversationId))
        }
      }

      return newMessage
    })
  }

  async getMessages(conversationId: string, limit?: number): Promise<Array<Message>> {
    await this.init()
    if (!this.db) throw new Error("Database not initialized")

    const baseQuery = this.db.select().from(messages)
      .where(eq(messages.conversationId, conversationId))
      .orderBy(messages.createdAt)

    return limit ? baseQuery.limit(limit) : baseQuery
  }

  async deleteMessage(id: string): Promise<boolean> {
    await this.init()
    if (!this.db) throw new Error("Database not initialized")

    const result = await this.db.delete(messages).where(eq(messages.id, id)).returning()
    return result.length > 0
  }

  async searchMessages(query: string, userId = "local"): Promise<Array<Message>> {
    await this.init()
    if (!this.db || !this.pg) throw new Error("Database not initialized")

    // Use PostgreSQL full-text search
    const result = await this.pg.query(
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
  }

  // Live query support (polling-based)
  subscribeToConversation(
    conversationId: string,
    callback: (messages: Array<Message>) => void
  ): () => void {
    // Simple polling implementation
    let lastCount = 0
    const checkInterval = setInterval(async () => {
      try {
        const messageList = await this.getMessages(conversationId)
        if (messageList.length !== lastCount) {
          lastCount = messageList.length
          callback(messageList)
        }
      } catch (error) {
        console.error("Subscription error:", error)
      }
    }, 1000)

    return () => clearInterval(checkInterval)
  }

  // Helper method to create a conversation with an initial message
  async startConversation(
    initialMessage: string,
    model?: string,
    metadata?: any
  ): Promise<{ conversation: Conversation; message: Message }> {
    const conversation = await this.createConversation({
      model,
      metadata
    })

    const message = await this.sendMessage({
      conversationId: conversation.id,
      role: "user",
      content: initialMessage,
      metadata: {}
    })

    return { conversation, message }
  }

  // Clean up resources
  async close() {
    if (this.pg) {
      await this.pg.close()
      this.pg = null
      this.db = null
      this.initialized = false
    }
  }
}
