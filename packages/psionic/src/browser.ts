/**
 * Browser-safe exports from Psionic
 * Excludes server-only dependencies like JSDOM
 */

// Core templates (browser-safe)
export { html, css } from './core/templates'

// Browser-safe persistence
export {
  // Schema types
  type Conversation,
  type NewConversation,
  type Message,
  type NewMessage,
  type MessageWithConversation,
  // Browser Effect client
  BrowserEffectChatClient,
  // Browser services (if you want to use them directly)
  BrowserPersistenceError,
  BrowserPGliteService,
  BrowserConversationRepository,
  BrowserMessageRepository,
  BrowserPersistenceLive
} from './persistence'