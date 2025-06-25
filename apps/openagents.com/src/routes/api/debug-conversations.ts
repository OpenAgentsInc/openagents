import { HttpServerResponse } from "@effect/platform"
import type { RouteContext } from "@openagentsinc/psionic"
import { Effect } from "effect"
import { getConversations } from "../../lib/chat-client-convex"

/**
 * GET /api/debug-conversations - Debug all conversations
 */
export function debugConversations(_ctx: RouteContext) {
  return Effect.gen(function*() {
    try {
      const conversations = yield* Effect.promise(() => getConversations())
      
      return yield* HttpServerResponse.json({
        totalCount: conversations.length,
        conversations: conversations.map((conv: any) => ({
          id: conv.id,
          title: conv.title,
          lastMessageAt: conv.lastMessageAt,
          createdAt: conv.createdAt,
          model: conv.model,
          metadata: conv.metadata
        })),
        // Show first conversation ID for easy testing
        firstConversationId: conversations.length > 0 ? conversations[0].id : null,
        debugUrl: conversations.length > 0 ? `/api/debug-messages/${conversations[0].id}` : null
      })
    } catch (error) {
      console.error("Failed to debug conversations:", error)
      return yield* HttpServerResponse.json(
        { error: "Failed to load conversations", details: error instanceof Error ? error.message : String(error) },
        { status: 500 }
      )
    }
  })
}