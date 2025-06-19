/**
 * Client-side entry point for chat persistence
 * This file gets bundled for browser usage
 */
export { BrowserEffectChatClient as ChatClient } from '@openagentsinc/psionic/browser'

// Re-export types for convenience
export type { 
  Conversation, 
  Message, 
  NewConversation, 
  NewMessage 
} from '@openagentsinc/psionic/browser'