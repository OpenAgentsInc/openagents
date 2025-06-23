import { createChatView } from "../components/chat-view"

export async function chat(ctx: { params: { id: string } }) {
  const conversationId = ctx.params.id
  return createChatView({ conversationId })
}
