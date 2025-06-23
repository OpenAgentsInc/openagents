import { BrowserEffectChatClient } from "@openagentsinc/psionic"

// Global chat client instance
let chatClient: BrowserEffectChatClient | null = null

/**
 * Initialize the chat client if not already initialized
 * Returns the singleton instance
 */
export async function getChatClient(): Promise<BrowserEffectChatClient> {
  if (!chatClient) {
    // Initialize the chat client with IndexedDB backend
    chatClient = new BrowserEffectChatClient("openagents-chat")
  }

  return chatClient
}

/**
 * Create a new conversation and return its ID
 */
export async function createConversation(title?: string): Promise<string> {
  const client = await getChatClient()

  const conversation = await client.createConversation({
    title: title || "New Conversation",
    model: "llama-3.3-70b",
    metadata: {} // Provider info can be stored elsewhere if needed
  })

  return conversation.id
}

/**
 * Add a message to a conversation
 */
export async function addMessage(
  conversationId: string,
  role: "user" | "assistant",
  content: string
): Promise<void> {
  const client = await getChatClient()

  await client.sendMessage({
    conversationId,
    role,
    content,
    model: "llama-3.3-70b",
    metadata: {}
  })
}

/**
 * Get all conversations sorted by last message
 */
export async function getConversations() {
  const client = await getChatClient()

  const conversations = await client.listConversations("local", false)

  // Sort by lastMessageAt in descending order
  return conversations.sort((a, b) => {
    const aTime = a.lastMessageAt?.getTime() || 0
    const bTime = b.lastMessageAt?.getTime() || 0
    return bTime - aTime
  })
}

/**
 * Get a conversation with its messages
 */
export async function getConversationWithMessages(conversationId: string) {
  const client = await getChatClient()

  const [conversation, messages] = await Promise.all([
    client.getConversation(conversationId),
    client.getMessages(conversationId, 100) // Get last 100 messages
  ])

  if (!conversation) {
    throw new Error("Conversation not found")
  }

  return { conversation, messages }
}

/**
 * Update conversation title (usually from first message)
 */
export async function updateConversationTitle(
  conversationId: string,
  title: string
): Promise<void> {
  const client = await getChatClient()

  await client.updateConversation(conversationId, { title })
}

/**
 * Delete a conversation and all its messages
 */
export async function deleteConversation(conversationId: string): Promise<void> {
  const client = await getChatClient()

  await client.deleteConversation(conversationId)
}

/**
 * Search conversations by content
 */
export async function searchConversations(query: string) {
  const client = await getChatClient()

  // Get all conversations and filter by title for now
  // TODO: Implement full-text search when available
  const allConversations = await client.listConversations("local", false)

  return allConversations.filter((conv) => conv.title?.toLowerCase().includes(query.toLowerCase()))
}
