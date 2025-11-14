import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import { getAuthUserId } from "@convex-dev/auth/server";

// Append an ACP event to the log
export const appendEvent = mutation({
  args: {
    sessionId: v.optional(v.string()),
    clientThreadDocId: v.optional(v.string()),
    threadId: v.optional(v.id("threads")),
    updateKind: v.optional(v.string()),
    payload: v.string(),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    // If threadId is provided, verify ownership
    if (args.threadId) {
      const thread = await ctx.db.get(args.threadId);
      if (!thread || thread.userId !== userId) {
        throw new Error("Unauthorized");
      }
    }

    const eventId = await ctx.db.insert("acpEvents", {
      userId,
      sessionId: args.sessionId,
      clientThreadDocId: args.clientThreadDocId,
      threadId: args.threadId,
      updateKind: args.updateKind,
      payload: args.payload,
      timestamp: Date.now(),
    });

    return eventId;
  },
});

// Get events by session
export const getEventsBySession = query({
  args: {
    sessionId: v.string(),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    let query = ctx.db
      .query("acpEvents")
      .withIndex("by_session", (q) => q.eq("sessionId", args.sessionId))
      .order("asc");

    if (args.limit) {
      query = query.take(args.limit) as any;
    }

    const events = await query.collect();

    // Filter to only user's events
    return events.filter((event) => event.userId === userId);
  },
});

// Get events by thread
export const getEventsByThread = query({
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
      .query("acpEvents")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .order("asc");

    if (args.limit) {
      query = query.take(args.limit) as any;
    }

    const events = await query.collect();
    return events;
  },
});

// Get recent events for a user
export const getRecentEvents = query({
  args: {
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    let query = ctx.db
      .query("acpEvents")
      .withIndex("by_user", (q) => q.eq("userId", userId))
      .order("desc");

    if (args.limit) {
      query = query.take(args.limit) as any;
    } else {
      query = query.take(100) as any; // Default limit
    }

    const events = await query.collect();
    return events;
  },
});

// Get events in a time range
export const getEventsByTimeRange = query({
  args: {
    startTime: v.number(),
    endTime: v.number(),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    let query = ctx.db
      .query("acpEvents")
      .withIndex("by_timestamp")
      .order("asc");

    if (args.limit) {
      query = query.take(args.limit) as any;
    }

    const events = await query.collect();

    // Filter by time range and user
    return events.filter(
      (event) =>
        event.userId === userId &&
        event.timestamp >= args.startTime &&
        event.timestamp <= args.endTime
    );
  },
});
