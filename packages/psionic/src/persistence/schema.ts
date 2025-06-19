/**
 * Database schema definitions for chat persistence
 * Using Drizzle ORM with PGlite
 */
import { pgTable, text, timestamp, jsonb, uuid, boolean, index } from 'drizzle-orm/pg-core'
import type { InferSelectModel, InferInsertModel } from 'drizzle-orm'

// Conversations table
export const conversations = pgTable('conversations', {
  id: uuid('id').defaultRandom().primaryKey(),
  userId: text('user_id').notNull().default('local'), // Default for local-only usage
  title: text('title'),
  model: text('model'), // Selected AI model
  lastMessageAt: timestamp('last_message_at'),
  createdAt: timestamp('created_at').defaultNow().notNull(),
  archived: boolean('archived').default(false),
  metadata: jsonb('metadata').$type<{
    systemPrompt?: string
    temperature?: number
    maxTokens?: number
    // Other model-specific settings
  }>().default({}),
}, (table) => ({
  userIdIdx: index('idx_user_created').on(table.userId, table.createdAt),
  archivedIdx: index('idx_archived').on(table.archived),
}))

// Messages table
export const messages = pgTable('messages', {
  id: uuid('id').defaultRandom().primaryKey(),
  conversationId: uuid('conversation_id').notNull().references(() => conversations.id, { onDelete: 'cascade' }),
  role: text('role').notNull().$type<'user' | 'assistant' | 'system' | 'tool'>(),
  content: text('content').notNull(), // Main text content
  model: text('model'), // AI model used (null for user messages)
  createdAt: timestamp('created_at').defaultNow().notNull(),
  metadata: jsonb('metadata').$type<{
    // Message parts for complex messages
    parts?: Array<{
      _tag: string
      text?: string
      url?: string
      data?: string
      mediaType?: string
      // Tool call specific
      id?: string
      name?: string
      params?: unknown
      result?: unknown
    }>
    // Token usage
    tokens?: {
      input: number
      output: number
    }
    // Tool calls made by assistant
    toolCalls?: Array<{
      id: string
      name: string
      params: unknown
    }>
    // File/image attachments
    attachments?: Array<{
      type: 'image' | 'file'
      url?: string
      data?: string // base64
      mediaType?: string
      name?: string
    }>
    // Any other provider-specific metadata
    [key: string]: unknown
  }>().default({}),
}, (table) => ({
  conversationIdx: index('idx_conversation_created').on(table.conversationId, table.createdAt),
  roleIdx: index('idx_role').on(table.role),
  // Full-text search index will be created via raw SQL
}))

// Type exports
export type Conversation = InferSelectModel<typeof conversations>
export type NewConversation = InferInsertModel<typeof conversations>
export type Message = InferSelectModel<typeof messages>
export type NewMessage = InferInsertModel<typeof messages>

// Helper type for message with conversation info
export type MessageWithConversation = Message & {
  conversation: Conversation
}