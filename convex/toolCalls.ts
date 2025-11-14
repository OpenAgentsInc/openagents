import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import { getAuthUserId } from "@convex-dev/auth/server";

// List tool calls for a specific thread
export const listToolCalls = query({
  args: {
    threadId: v.id("threads"),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    // Verify thread ownership
    const thread = await ctx.db.get(args.threadId);
    if (!thread || thread.userId !== userId) {
      throw new Error("Unauthorized");
    }

    let query = ctx.db
      .query("toolCalls")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .order("desc");

    if (args.limit) {
      query = query.take(args.limit) as any;
    }

    const toolCalls = await query.collect();
    return toolCalls;
  },
});

// Get a specific tool call by tool call ID
export const getToolCall = query({
  args: {
    toolCallId: v.string(),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    const toolCall = await ctx.db
      .query("toolCalls")
      .withIndex("by_tool_call_id", (q) => q.eq("toolCallId", args.toolCallId))
      .first();

    if (!toolCall) {
      return null;
    }

    // Verify ownership
    if (toolCall.userId !== userId) {
      throw new Error("Unauthorized");
    }

    return toolCall;
  },
});

// Upsert a tool call (create or update)
export const upsertToolCall = mutation({
  args: {
    threadId: v.id("threads"),
    toolCallId: v.string(),
    title: v.optional(v.string()),
    kind: v.optional(v.string()),
    status: v.optional(v.string()),
    contentJson: v.optional(v.string()),
    locationsJson: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    // Verify thread ownership
    const thread = await ctx.db.get(args.threadId);
    if (!thread || thread.userId !== userId) {
      throw new Error("Unauthorized");
    }

    // Check if tool call already exists
    const existing = await ctx.db
      .query("toolCalls")
      .withIndex("by_tool_call_id", (q) => q.eq("toolCallId", args.toolCallId))
      .first();

    const now = Date.now();

    if (existing) {
      // Update existing tool call
      const updates: any = {
        updatedAt: now,
      };

      if (args.title !== undefined) updates.title = args.title;
      if (args.kind !== undefined) updates.kind = args.kind;
      if (args.status !== undefined) updates.status = args.status;
      if (args.contentJson !== undefined) updates.contentJson = args.contentJson;
      if (args.locationsJson !== undefined) updates.locationsJson = args.locationsJson;

      await ctx.db.patch(existing._id, updates);
      return existing._id;
    } else {
      // Create new tool call
      const toolCallDbId = await ctx.db.insert("toolCalls", {
        threadId: args.threadId,
        userId,
        toolCallId: args.toolCallId,
        title: args.title,
        kind: args.kind,
        status: args.status ?? "pending",
        contentJson: args.contentJson,
        locationsJson: args.locationsJson,
        createdAt: now,
        updatedAt: now,
      });

      return toolCallDbId;
    }
  },
});

// Update tool call status
export const updateToolCallStatus = mutation({
  args: {
    toolCallId: v.string(),
    status: v.string(),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    const toolCall = await ctx.db
      .query("toolCalls")
      .withIndex("by_tool_call_id", (q) => q.eq("toolCallId", args.toolCallId))
      .first();

    if (!toolCall) {
      throw new Error("Tool call not found");
    }

    // Verify ownership
    if (toolCall.userId !== userId) {
      throw new Error("Unauthorized");
    }

    await ctx.db.patch(toolCall._id, {
      status: args.status,
      updatedAt: Date.now(),
    });
  },
});

// Delete a tool call
export const deleteToolCall = mutation({
  args: {
    toolCallId: v.string(),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    const toolCall = await ctx.db
      .query("toolCalls")
      .withIndex("by_tool_call_id", (q) => q.eq("toolCallId", args.toolCallId))
      .first();

    if (!toolCall) {
      throw new Error("Tool call not found");
    }

    // Verify ownership
    if (toolCall.userId !== userId) {
      throw new Error("Unauthorized");
    }

    await ctx.db.delete(toolCall._id);
  },
});
