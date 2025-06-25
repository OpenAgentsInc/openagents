import { HttpServerResponse } from "@effect/platform"
import type { RouteContext } from "@openagentsinc/psionic"
import { Effect } from "effect"
import { getConversationWithMessages } from "../../lib/chat-client-convex"

/**
 * GET /api/debug-messages/:id - Debug conversation messages
 */
export function debugMessages(ctx: RouteContext) {
  return Effect.gen(function*() {
    try {
      const conversationId = ctx.params.id
      
      if (!conversationId) {
        return yield* HttpServerResponse.json(
          { error: "Conversation ID required" },
          { status: 400 }
        )
      }
      
      const result = yield* Effect.promise(() => getConversationWithMessages(conversationId))
      
      // Return raw data for debugging
      return yield* HttpServerResponse.json({
        conversation: result.conversation,
        messageCount: result.messages.length,
        messages: result.messages.map((msg: any, index: number) => ({
          index,
          id: msg.id,
          role: msg.role,
          contentLength: msg.content?.length || 0,
          contentPreview: msg.content?.substring(0, 100) + (msg.content?.length > 100 ? '...' : ''),
          metadata: msg.metadata,
          timestamp: msg.timestamp
        })),
        // Include first 3 full messages for detailed inspection
        fullMessages: result.messages.slice(0, 3)
      })
    } catch (error) {
      console.error("Failed to debug messages:", error)
      return yield* HttpServerResponse.json(
        { error: "Failed to load conversation", details: error instanceof Error ? error.message : String(error) },
        { status: 500 }
      )
    }
  })
}