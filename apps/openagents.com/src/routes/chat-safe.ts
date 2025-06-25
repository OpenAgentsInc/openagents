import { createChatViewSafe } from "../components/chat-view-safe"

export async function chatSafe(ctx: { params: { id: string } }) {
  const conversationId = ctx.params.id
  return createChatViewSafe({ conversationId })
}
