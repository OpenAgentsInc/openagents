/**
 * Browser-safe exports from Psionic
 * Excludes server-only dependencies like JSDOM
 */

// Core templates (browser-safe)
export { css, html } from "./core/templates"

// Browser-safe persistence
export {
  BrowserConversationRepository,
  // Browser Effect client
  BrowserEffectChatClient,
  BrowserMessageRepository,
  // Browser services (if you want to use them directly)
  BrowserPersistenceError,
  BrowserPersistenceLive,
  BrowserPGliteService,
  // Schema types
  type Conversation,
  type Message,
  type MessageWithConversation,
  type NewConversation,
  type NewMessage
} from "./persistence"
