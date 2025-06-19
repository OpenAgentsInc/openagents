/**
 * Persistence module for Psionic framework
 * Provides local-first chat persistence using PGlite
 */

// Schema exports
export {
  conversations,
  messages,
  type Conversation,
  type NewConversation,
  type Message,
  type NewMessage,
  type MessageWithConversation
} from './schema'

// Service exports
export {
  PGliteError,
  PGliteService,
  PGliteServiceLive,
  ConversationRepository,
  ConversationRepositoryLive,
  MessageRepository,
  MessageRepositoryLive,
  PersistenceLive,
  type PGliteConfig
} from './services'

// Client exports
export { ChatClient } from './client'
export { BrowserChatClient } from './browser-client'
export { BrowserEffectChatClient } from './browser-effect-client'

// Browser-safe Effect services
export {
  BrowserPersistenceError,
  BrowserPGliteService,
  BrowserConversationRepository,
  BrowserMessageRepository,
  BrowserPGliteServiceLive,
  BrowserConversationRepositoryLive,
  BrowserMessageRepositoryLive,
  BrowserPersistenceLive
} from './client-services'

// Re-export PGlite types for convenience
export type { PGlite } from '@electric-sql/pglite'