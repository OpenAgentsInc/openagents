import { v } from "convex/values";
import { internalMutation, internalQuery } from "./_generated/server";
import { requireFound } from "./lib/errors";

export const upsertUser = internalMutation({
  args: {
    user_id: v.string(),
    email: v.optional(v.string()),
    name: v.optional(v.string()),
    image: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const existing = await ctx.db
      .query("users")
      .withIndex("by_user_id", (q) => q.eq("user_id", args.user_id))
      .first();

    if (existing) {
      const updates: Record<string, string> = {};
      if (args.email !== undefined && args.email !== existing.email) {
        updates.email = args.email;
      }
      if (args.name !== undefined && args.name !== existing.name) {
        updates.name = args.name;
      }
      if (args.image !== undefined && args.image !== existing.image) {
        updates.image = args.image;
      }
      if (Object.keys(updates).length > 0) {
        await ctx.db.patch(existing._id, updates);
      }
      return (await ctx.db.get(existing._id)) ?? existing;
    }

    const now = Date.now();
    const userId = await ctx.db.insert("users", {
      user_id: args.user_id,
      email: args.email ?? "",
      name: args.name ?? "",
      image: args.image ?? "",
      credits: 100,
      plan: "free",
      created_at: now,
    });

    const user = await ctx.db.get(userId);
    return requireFound(user, "NOT_FOUND", "User not found after creation");
  },
});

export const getUserByExternalId = internalQuery({
  args: {
    user_id: v.string(),
  },
  handler: async (ctx, args) => {
    return ctx.db
      .query("users")
      .withIndex("by_user_id", (q) => q.eq("user_id", args.user_id))
      .first();
  },
});
