/**
 * Convex functions for chat message operations
 * @since 1.0.0
 */

import { v } from "convex/values"
import { query, mutation } from "./_generated/server"

/**
 * Create a new chat message
 */
export const create = mutation({
  args: {
    session_id: v.string(),
    entry_uuid: v.string(),
    entry_type: v.string(),
    role: v.optional(v.string()),
    content: v.optional(v.string()),
    thinking: v.optional(v.string()),
    summary: v.optional(v.string()),
    model: v.optional(v.string()),
    token_usage: v.optional(v.object({
      input_tokens: v.number(),
      output_tokens: v.number(),
      total_tokens: v.number()
    })),
    cost: v.optional(v.number()),
    timestamp: v.number(),
    turn_count: v.optional(v.number()),
    tool_name: v.optional(v.string()),
    tool_input: v.optional(v.any()),
    tool_use_id: v.optional(v.string()),
    tool_output: v.optional(v.string()),
    tool_is_error: v.optional(v.boolean())
  },
  handler: async (ctx, args) => {
    return await ctx.db.insert("messages", args)
  }
})

/**
 * Get messages for a session
 */
export const listBySession = query({
  args: {
    sessionId: v.string(),
    limit: v.optional(v.number()),
    offset: v.optional(v.number())
  },
  handler: async (ctx, args) => {
    const limit = args.limit ?? 50
    const offset = args.offset ?? 0

    // Get messages ordered by timestamp
    const messages = await ctx.db
      .query("messages")
      .withIndex("by_session_id", q => q.eq("session_id", args.sessionId))
      .order("asc")
      .collect()

    // Apply pagination
    return messages.slice(offset, offset + limit)
  }
})

/**
 * Get recent messages for a session
 */
export const getRecent = query({
  args: {
    sessionId: v.string(),
    limit: v.optional(v.number())
  },
  handler: async (ctx, args) => {
    const messages = await ctx.db
      .query("messages")
      .withIndex("by_session_id", q => q.eq("session_id", args.sessionId))
      .order("desc")
      .take(args.limit ?? 20)

    // Return in chronological order
    return messages.reverse()
  }
})

/**
 * Get message by entry UUID
 */
export const getByUuid = query({
  args: { entryUuid: v.string() },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("messages")
      .filter(q => q.eq(q.field("entry_uuid"), args.entryUuid))
      .first()
  }
})

/**
 * Get messages by type
 */
export const listByType = query({
  args: {
    sessionId: v.string(),
    entryType: v.string(),
    limit: v.optional(v.number())
  },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("messages")
      .withIndex("by_entry_type", q => q.eq("entry_type", args.entryType))
      .filter(q => q.eq(q.field("session_id"), args.sessionId))
      .order("desc")
      .take(args.limit ?? 20)
  }
})

/**
 * Get tool usage messages
 */
export const listToolUsage = query({
  args: {
    sessionId: v.string(),
    toolName: v.optional(v.string()),
    limit: v.optional(v.number())
  },
  handler: async (ctx, args) => {
    let messages = await ctx.db
      .query("messages")
      .withIndex("by_entry_type", q => q.eq("entry_type", "tool_use"))
      .filter(q => q.eq(q.field("session_id"), args.sessionId))
      .collect()

    if (args.toolName) {
      messages = messages.filter(m => m.tool_name === args.toolName)
    }

    return messages
      .sort((a, b) => b.timestamp - a.timestamp)
      .slice(0, args.limit ?? 20)
  }
})

/**
 * Get messages by tool use ID (for linking tool_use and tool_result)
 */
export const listByToolUseId = query({
  args: { toolUseId: v.string() },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("messages")
      .withIndex("by_tool_use_id", q => q.eq("tool_use_id", args.toolUseId))
      .collect()
  }
})

/**
 * Update message content (for streaming updates)
 */
export const updateContent = mutation({
  args: {
    entryUuid: v.string(),
    content: v.optional(v.string()),
    thinking: v.optional(v.string())
  },
  handler: async (ctx, args) => {
    const message = await ctx.db
      .query("messages")
      .filter(q => q.eq(q.field("entry_uuid"), args.entryUuid))
      .first()

    if (!message) {
      throw new Error(`Message not found: ${args.entryUuid}`)
    }

    const updateData: any = {}
    if (args.content !== undefined) updateData.content = args.content
    if (args.thinking !== undefined) updateData.thinking = args.thinking

    return await ctx.db.patch(message._id, updateData)
  }
})

/**
 * Update message fields (comprehensive update for fixing imported data)
 */
export const update = mutation({
  args: {
    entryUuid: v.string(),
    content: v.optional(v.string()),
    thinking: v.optional(v.string()),
    tool_name: v.optional(v.string()),
    tool_input: v.optional(v.any()),
    tool_use_id: v.optional(v.string()),
    tool_output: v.optional(v.string()),
    tool_is_error: v.optional(v.boolean())
  },
  handler: async (ctx, args) => {
    const message = await ctx.db
      .query("messages")
      .filter(q => q.eq(q.field("entry_uuid"), args.entryUuid))
      .first()

    if (!message) {
      throw new Error(`Message not found: ${args.entryUuid}`)
    }

    const updateData: any = {}
    if (args.content !== undefined) updateData.content = args.content
    if (args.thinking !== undefined) updateData.thinking = args.thinking
    if (args.tool_name !== undefined) updateData.tool_name = args.tool_name
    if (args.tool_input !== undefined) updateData.tool_input = args.tool_input
    if (args.tool_use_id !== undefined) updateData.tool_use_id = args.tool_use_id
    if (args.tool_output !== undefined) updateData.tool_output = args.tool_output
    if (args.tool_is_error !== undefined) updateData.tool_is_error = args.tool_is_error

    return await ctx.db.patch(message._id, updateData)
  }
})

/**
 * Get session message statistics
 */
export const getSessionStats = query({
  args: { sessionId: v.string() },
  handler: async (ctx, args) => {
    const messages = await ctx.db
      .query("messages")
      .withIndex("by_session_id", q => q.eq("session_id", args.sessionId))
      .collect()

    const totalMessages = messages.length
    const userMessages = messages.filter(m => m.entry_type === "user").length
    const assistantMessages = messages.filter(m => m.entry_type === "assistant").length
    const toolUses = messages.filter(m => m.entry_type === "tool_use").length
    
    const totalTokens = messages.reduce((sum, m) => 
      sum + (m.token_usage?.total_tokens ?? 0), 0
    )
    
    const totalCost = messages.reduce((sum, m) => 
      sum + (m.cost ?? 0), 0
    )

    const firstMessage = messages.reduce((first, m) => 
      !first || m.timestamp < first.timestamp ? m : first, null as any
    )
    
    const lastMessage = messages.reduce((last, m) => 
      !last || m.timestamp > last.timestamp ? m : last, null as any
    )

    return {
      totalMessages,
      userMessages,
      assistantMessages,
      toolUses,
      totalTokens,
      totalCost,
      firstMessageAt: firstMessage?.timestamp,
      lastMessageAt: lastMessage?.timestamp,
      averageTokensPerMessage: totalMessages > 0 ? totalTokens / totalMessages : 0
    }
  }
})

/**
 * Search messages by content
 */
export const search = query({
  args: {
    sessionId: v.string(),
    searchTerm: v.string(),
    limit: v.optional(v.number())
  },
  handler: async (ctx, args) => {
    const messages = await ctx.db
      .query("messages")
      .withIndex("by_session_id", q => q.eq("session_id", args.sessionId))
      .collect()

    const searchTerm = args.searchTerm.toLowerCase()
    const matchingMessages = messages.filter(m => {
      const content = (m.content || "").toLowerCase()
      const thinking = (m.thinking || "").toLowerCase()
      const summary = (m.summary || "").toLowerCase()
      
      return content.includes(searchTerm) || 
             thinking.includes(searchTerm) || 
             summary.includes(searchTerm)
    })

    return matchingMessages
      .sort((a, b) => b.timestamp - a.timestamp)
      .slice(0, args.limit ?? 20)
  }
})