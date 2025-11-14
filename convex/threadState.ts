import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import { getAuthUserId } from "@convex-dev/auth/server";

// Get thread state (current mode and available commands)
export const getThreadState = query({
  args: {
    threadId: v.string(), // ACP session ID string
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    // Note: threadId is a session ID string, not a Convex thread ID
    // ACP sessions manage their own authorization

    const state = await ctx.db
      .query("threadState")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .first();

    // Verify ownership
    if (state && state.userId !== userId) {
      throw new Error("Unauthorized");
    }

    return state;
  },
});

// Upsert thread state
export const upsertThreadState = mutation({
  args: {
    threadId: v.string(), // ACP session ID string
    currentModeId: v.optional(v.string()),
    availableCommandsJson: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    // Note: threadId is a session ID string, not a Convex thread ID
    // ACP sessions manage their own authorization

    // Check if state already exists for this thread
    const existing = await ctx.db
      .query("threadState")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .first();

    const now = Date.now();

    if (existing) {
      // Update existing state
      const updates: any = {
        updatedAt: now,
      };

      if (args.currentModeId !== undefined) {
        updates.currentModeId = args.currentModeId;
      }
      if (args.availableCommandsJson !== undefined) {
        updates.availableCommandsJson = args.availableCommandsJson;
      }

      await ctx.db.patch(existing._id, updates);
      return existing._id;
    } else {
      // Create new state
      const stateId = await ctx.db.insert("threadState", {
        threadId: args.threadId,
        userId,
        currentModeId: args.currentModeId,
        availableCommandsJson: args.availableCommandsJson,
        createdAt: now,
        updatedAt: now,
      });
      return stateId;
    }
  },
});

// Update current mode
export const updateCurrentMode = mutation({
  args: {
    threadId: v.string(), // ACP session ID string
    currentModeId: v.string(),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    // Note: threadId is a session ID string, not a Convex thread ID
    // ACP sessions manage their own authorization

    const state = await ctx.db
      .query("threadState")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .first();

    const now = Date.now();

    if (state) {
      await ctx.db.patch(state._id, {
        currentModeId: args.currentModeId,
        updatedAt: now,
      });
    } else {
      await ctx.db.insert("threadState", {
        threadId: args.threadId,
        userId,
        currentModeId: args.currentModeId,
        availableCommandsJson: undefined,
        createdAt: now,
        updatedAt: now,
      });
    }
  },
});

// Update available commands
export const updateAvailableCommands = mutation({
  args: {
    threadId: v.string(), // ACP session ID string
    availableCommandsJson: v.string(),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    // Note: threadId is a session ID string, not a Convex thread ID
    // ACP sessions manage their own authorization

    const state = await ctx.db
      .query("threadState")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .first();

    const now = Date.now();

    if (state) {
      await ctx.db.patch(state._id, {
        availableCommandsJson: args.availableCommandsJson,
        updatedAt: now,
      });
    } else {
      await ctx.db.insert("threadState", {
        threadId: args.threadId,
        userId,
        currentModeId: undefined,
        availableCommandsJson: args.availableCommandsJson,
        createdAt: now,
        updatedAt: now,
      });
    }
  },
});

// Delete thread state
export const deleteThreadState = mutation({
  args: {
    threadId: v.string(), // ACP session ID string
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    // Note: threadId is a session ID string, not a Convex thread ID
    // ACP sessions manage their own authorization

    const state = await ctx.db
      .query("threadState")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .first();

    if (state) {
      // Verify ownership
      if (state.userId !== userId) {
        throw new Error("Unauthorized");
      }

      await ctx.db.delete(state._id);
    }
  },
});
