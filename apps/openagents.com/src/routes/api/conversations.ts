import type { HttpServerRequest } from "@effect/platform"
import { HttpServerResponse } from "@effect/platform"
import type { RouteContext } from "@openagentsinc/psionic"
import { Effect } from "effect"
import { addMessage, createConversation, getConversations, updateConversationTitle } from "../../lib/chat-client"

/**
 * GET /api/conversations - List all conversations
 */
export function listConversations(
  _ctx: RouteContext
): Effect.Effect<HttpServerResponse.HttpServerResponse, never, never> {
  return Effect.gen(function*() {
    try {
      const conversations = yield* Effect.promise(() => getConversations())
      return yield* HttpServerResponse.json(conversations)
    } catch (error) {
      console.error("Failed to list conversations:", error)
      return yield* HttpServerResponse.json(
        { error: "Failed to load conversations" },
        { status: 500 }
      )
    }
  })
}

/**
 * POST /api/conversations - Create a new conversation
 */
export function createConversationRoute(
  ctx: RouteContext
): Effect.Effect<HttpServerResponse.HttpServerResponse, never, HttpServerRequest.HttpServerRequest> {
  return Effect.gen(function*() {
    console.log("🔍 CONVERSATIONS API: createConversationRoute called")

    try {
      // Now we can access request.text directly within the Effect context!
      const bodyText = yield* ctx.request.text
      console.log("✅ Successfully read request body using Effect")

      const body = JSON.parse(bodyText)
      const title = body.title || "New Conversation"

      const id = yield* Effect.promise(() => createConversation(title))

      return yield* HttpServerResponse.json({ id, title })
    } catch (error) {
      console.error("Failed to create conversation:", error)
      return yield* HttpServerResponse.json(
        { error: "Failed to create conversation" },
        { status: 500 }
      )
    }
  })
}

/**
 * PATCH /api/conversations/:id - Update conversation title
 */
export function updateConversation(
  ctx: RouteContext
): Effect.Effect<HttpServerResponse.HttpServerResponse, never, HttpServerRequest.HttpServerRequest> {
  return Effect.gen(function*() {
    try {
      const bodyText = yield* ctx.request.text
      const body = JSON.parse(bodyText)
      const { title } = body

      if (!title) {
        return yield* HttpServerResponse.json(
          { error: "Title is required" },
          { status: 400 }
        )
      }

      yield* Effect.promise(() => updateConversationTitle(ctx.params.id, title))

      return yield* HttpServerResponse.json({ success: true })
    } catch (error) {
      console.error("Failed to update conversation:", error)
      return yield* HttpServerResponse.json(
        { error: "Failed to update conversation" },
        { status: 500 }
      )
    }
  })
}

/**
 * POST /api/conversations/:id/messages - Add a message to a conversation
 */
export function addMessageRoute(
  ctx: RouteContext
): Effect.Effect<HttpServerResponse.HttpServerResponse, never, HttpServerRequest.HttpServerRequest> {
  return Effect.gen(function*() {
    console.log("🔍 CONVERSATIONS API: addMessageRoute called")
    try {
      const bodyText = yield* ctx.request.text
      console.log("✅ Successfully read message body using Effect")

      const body = JSON.parse(bodyText)
      const { content, role } = body

      if (!role || !content) {
        return yield* HttpServerResponse.json(
          { error: "Role and content are required" },
          { status: 400 }
        )
      }

      yield* Effect.promise(() => addMessage(ctx.params.id, role, content))

      return yield* HttpServerResponse.json({ success: true })
    } catch (error) {
      console.error("Failed to add message:", error)
      return yield* HttpServerResponse.json(
        { error: "Failed to add message" },
        { status: 500 }
      )
    }
  })
}
