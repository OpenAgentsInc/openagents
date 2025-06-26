// Simple in-memory storage for server-side
interface StoredConversation {
  id: string
  title: string
  lastMessageAt?: Date
  createdAt: Date
  model: string
  metadata?: Record<string, any>
}

interface StoredMessage {
  id: string
  conversationId: string
  role: "user" | "assistant"
  content: string
  timestamp: Date
  model?: string
  metadata?: Record<string, any>
}

// In-memory storage
const conversations = new Map<string, StoredConversation>()
const messages = new Map<string, Array<StoredMessage>>()

// Simple ID generation
function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`
}

/**
 * Create a new conversation and return its ID
 */
export async function createConversation(title?: string): Promise<string> {
  const id = generateId()
  const conversation: StoredConversation = {
    id,
    title: title || "New Conversation",
    createdAt: new Date(),
    model: "llama-3.3-70b",
    metadata: {}
  }

  conversations.set(id, conversation)
  messages.set(id, [])

  return id
}

/**
 * Add a message to a conversation
 */
export async function addMessage(
  conversationId: string,
  role: "user" | "assistant",
  content: string
): Promise<void> {
  const conversation = conversations.get(conversationId)
  if (!conversation) {
    throw new Error(`Conversation ${conversationId} not found`)
  }

  const message: StoredMessage = {
    id: generateId(),
    conversationId,
    role,
    content,
    timestamp: new Date(),
    model: "llama-3.3-70b",
    metadata: {}
  }

  const conversationMessages = messages.get(conversationId) || []
  conversationMessages.push(message)
  messages.set(conversationId, conversationMessages)

  // Update last message timestamp
  conversation.lastMessageAt = new Date()
  conversations.set(conversationId, conversation)
}

/**
 * Get all conversations sorted by last message
 */
export async function getConversations() {
  const allConversations = Array.from(conversations.values())

  // Sort by lastMessageAt in descending order
  return allConversations.sort((a, b) => {
    const aTime = a.lastMessageAt?.getTime() || a.createdAt.getTime()
    const bTime = b.lastMessageAt?.getTime() || b.createdAt.getTime()
    return bTime - aTime
  })
}

/**
 * Get a conversation with its messages
 */
export async function getConversationWithMessages(conversationId: string) {
  const conversation = conversations.get(conversationId)
  const conversationMessages = messages.get(conversationId) || []

  if (!conversation) {
    throw new Error("Conversation not found")
  }

  return { conversation, messages: conversationMessages }
}

/**
 * Update conversation title (usually from first message)
 */
export async function updateConversationTitle(
  conversationId: string,
  title: string
): Promise<void> {
  const conversation = conversations.get(conversationId)
  if (!conversation) {
    throw new Error(`Conversation ${conversationId} not found`)
  }

  conversation.title = title
  conversations.set(conversationId, conversation)
}

/**
 * Delete a conversation and all its messages
 */
export async function deleteConversation(conversationId: string): Promise<void> {
  conversations.delete(conversationId)
  messages.delete(conversationId)
}

/**
 * Search conversations by content
 */
export async function searchConversations(query: string) {
  const allConversations = Array.from(conversations.values())

  return allConversations.filter((conv) => conv.title?.toLowerCase().includes(query.toLowerCase()))
}
