import { createChatView } from "../components/chat-view"

export async function chat(ctx: { params: { id: string } }) {
  console.log("=== CHAT ROUTE HANDLER ===")
  console.log("ctx:", ctx)
  console.log("ctx.params:", ctx.params)
  console.log("ctx.params.id:", ctx.params.id)

  const conversationId = ctx.params.id
  console.log("Extracted conversationId:", conversationId)

  try {
    const result = await createChatView({ conversationId })
    console.log("createChatView returned successfully")
    return result
  } catch (error) {
    console.error("ERROR in chat route:", error)
    throw error
  }
}
