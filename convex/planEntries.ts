import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import { getAuthUserId } from "@convex-dev/auth/server";

// Get plan entries for a specific thread
export const getPlan = query({
  args: {
    threadId: v.id("threads"),
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

    const plan = await ctx.db
      .query("planEntries")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .first();

    return plan;
  },
});

// Upsert plan entries for a thread
export const upsertPlan = mutation({
  args: {
    threadId: v.id("threads"),
    entriesJson: v.string(),
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
    threadId: v.id("threads"),
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

    const plan = await ctx.db
      .query("planEntries")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .first();

    if (plan) {
      await ctx.db.delete(plan._id);
    }
  },
});
