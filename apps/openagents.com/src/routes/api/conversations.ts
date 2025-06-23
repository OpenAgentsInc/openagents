import { Effect } from "effect"
import { addMessage, createConversation, getConversations, updateConversationTitle } from "../../lib/chat-client"

/**
 * GET /api/conversations - List all conversations
 */
export async function listConversations(_ctx: any) {
  try {
    const conversations = await getConversations()
    return new Response(JSON.stringify(conversations), {
      headers: { "Content-Type": "application/json" }
    })
  } catch (error) {
    console.error("Failed to list conversations:", error)
    return new Response(JSON.stringify({ error: "Failed to load conversations" }), {
      status: 500,
      headers: { "Content-Type": "application/json" }
    })
  }
}

/**
 * POST /api/conversations - Create a new conversation
 */
export async function createConversationRoute(ctx: any) {
  try {
    // Parse the request body from Effect HttpServerRequest
    const bodyText = await Effect.runPromise(
      Effect.gen(function*() {
        const request = ctx.request
        return yield* request.text
      }) as Effect.Effect<string, never, never>
    )

    const body = JSON.parse(bodyText)
    const title = body.title || "New Conversation"

    const id = await createConversation(title)

    return new Response(JSON.stringify({ id, title }), {
      headers: { "Content-Type": "application/json" }
    })
  } catch (error) {
    console.error("Failed to create conversation:", error)
    return new Response(JSON.stringify({ error: "Failed to create conversation" }), {
      status: 500,
      headers: { "Content-Type": "application/json" }
    })
  }
}

/**
 * PATCH /api/conversations/:id - Update conversation title
 */
export async function updateConversation(ctx: any) {
  try {
    // Parse the request body from Effect HttpServerRequest
    const bodyText = await Effect.runPromise(
      Effect.gen(function*() {
        const request = ctx.request
        return yield* request.text
      }) as Effect.Effect<string, never, never>
    )

    const body = JSON.parse(bodyText)
    const { title } = body

    if (!title) {
      return new Response(JSON.stringify({ error: "Title is required" }), {
        status: 400,
        headers: { "Content-Type": "application/json" }
      })
    }

    await updateConversationTitle(ctx.params.id, title)

    return new Response(JSON.stringify({ success: true }), {
      headers: { "Content-Type": "application/json" }
    })
  } catch (error) {
    console.error("Failed to update conversation:", error)
    return new Response(JSON.stringify({ error: "Failed to update conversation" }), {
      status: 500,
      headers: { "Content-Type": "application/json" }
    })
  }
}

/**
 * POST /api/conversations/:id/messages - Add a message to a conversation
 */
export async function addMessageRoute(ctx: any) {
  try {
    // Parse the request body from Effect HttpServerRequest
    const bodyText = await Effect.runPromise(
      Effect.gen(function*() {
        const request = ctx.request
        return yield* request.text
      }) as Effect.Effect<string, never, never>
    )

    const body = JSON.parse(bodyText)
    const { content, role } = body

    if (!role || !content) {
      return new Response(JSON.stringify({ error: "Role and content are required" }), {
        status: 400,
        headers: { "Content-Type": "application/json" }
      })
    }

    await addMessage(ctx.params.id, role, content)

    return new Response(JSON.stringify({ success: true }), {
      headers: { "Content-Type": "application/json" }
    })
  } catch (error) {
    console.error("Failed to add message:", error)
    return new Response(JSON.stringify({ error: "Failed to add message" }), {
      status: 500,
      headers: { "Content-Type": "application/json" }
    })
  }
}
