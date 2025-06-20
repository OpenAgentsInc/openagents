/**
 * Persistence module for Psionic framework
 * Provides local-first chat persistence using PGlite
 */

// Schema exports
export {
  type Conversation,
  conversations,
  type Message,
  messages,
  type MessageWithConversation,
  type NewConversation,
  type NewMessage
} from "./schema"

// Service exports
export {
  ConversationRepository,
  ConversationRepositoryLive,
  MessageRepository,
  MessageRepositoryLive,
  PersistenceLive,
  type PGliteConfig,
  PGliteError,
  PGliteService,
  PGliteServiceLive
} from "./services"

// Client exports
export { BrowserChatClient } from "./browser-client"
export { BrowserEffectChatClient } from "./browser-effect-client"
export { ChatClient } from "./client"

// Browser-safe Effect services
export {
  BrowserConversationRepository,
  BrowserConversationRepositoryLive,
  BrowserMessageRepository,
  BrowserMessageRepositoryLive,
  BrowserPersistenceError,
  BrowserPersistenceLive,
  BrowserPGliteService,
  BrowserPGliteServiceLive
} from "./client-services"

// Re-export PGlite types for convenience
export type { PGlite } from "@electric-sql/pglite"
