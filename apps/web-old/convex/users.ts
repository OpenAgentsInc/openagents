import { v } from "convex/values";
import { internalMutation, internalQuery } from "./_generated/server";
import { fail, requireFound } from "./lib/errors";

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

export const getNostrIdentityForUser = internalQuery({
  args: {
    user_id: v.string(),
  },
  handler: async (ctx, args) => {
    const user = await ctx.db
      .query("users")
      .withIndex("by_user_id", (q) => q.eq("user_id", args.user_id))
      .first();
    if (!user) {
      return null;
    }
    return {
      user_id: user.user_id,
      nostr_pubkey: user.nostr_pubkey,
      nostr_npub: user.nostr_npub,
      nostr_verified_at: user.nostr_verified_at,
      nostr_verification_method: user.nostr_verification_method,
    };
  },
});

export const linkNostrIdentityForUser = internalMutation({
  args: {
    user_id: v.string(),
    pubkey: v.string(),
    npub: v.string(),
    verified_at: v.number(),
    method: v.string(),
  },
  handler: async (ctx, args) => {
    const user = await ctx.db
      .query("users")
      .withIndex("by_user_id", (q) => q.eq("user_id", args.user_id))
      .first();
    const userRecord = requireFound(user, "NOT_FOUND", "User not found");

    const existing = await ctx.db
      .query("users")
      .withIndex("by_nostr_pubkey", (q) => q.eq("nostr_pubkey", args.pubkey))
      .first();
    if (existing && existing.user_id !== args.user_id) {
      fail("CONFLICT", "Nostr pubkey already linked to another user");
    }
    if (userRecord.nostr_pubkey && userRecord.nostr_pubkey !== args.pubkey) {
      fail("CONFLICT", "A different Nostr pubkey is already linked");
    }

    await ctx.db.patch(userRecord._id, {
      nostr_pubkey: args.pubkey,
      nostr_npub: args.npub,
      nostr_verified_at: args.verified_at,
      nostr_verification_method: args.method,
    });

    return {
      user_id: userRecord.user_id,
      nostr_pubkey: args.pubkey,
      nostr_npub: args.npub,
      nostr_verified_at: args.verified_at,
      nostr_verification_method: args.method,
    };
  },
});
