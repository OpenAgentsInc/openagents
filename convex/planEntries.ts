import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import { getAuthUserId } from "@convex-dev/auth/server";

// Get plan entries for a specific thread
export const getPlan = query({
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

    const plan = await ctx.db
      .query("planEntries")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .first();

    // Verify ownership
    if (plan && plan.userId !== userId) {
      throw new Error("Unauthorized");
    }

    return plan;
  },
});

// Upsert plan entries for a thread
export const upsertPlan = mutation({
  args: {
    threadId: v.string(), // ACP session ID string
    entriesJson: v.string(),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    // Note: threadId is a session ID string, not a Convex thread ID
    // ACP sessions manage their own authorization

    // Check if plan already exists for this thread
    const existing = await ctx.db
      .query("planEntries")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .first();

    const now = Date.now();

    if (existing) {
      // Update existing plan
      await ctx.db.patch(existing._id, {
        entriesJson: args.entriesJson,
        updatedAt: now,
      });
      return existing._id;
    } else {
      // Create new plan
      const planId = await ctx.db.insert("planEntries", {
        threadId: args.threadId,
        userId,
        entriesJson: args.entriesJson,
        createdAt: now,
        updatedAt: now,
      });
      return planId;
    }
  },
});

// Delete plan entries for a thread
export const deletePlan = mutation({
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

    const plan = await ctx.db
      .query("planEntries")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .first();

    if (plan) {
      // Verify ownership
      if (plan.userId !== userId) {
        throw new Error("Unauthorized");
      }

      await ctx.db.delete(plan._id);
    }
  },
});
