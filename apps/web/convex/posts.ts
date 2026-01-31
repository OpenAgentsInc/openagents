import { mutation, query } from "./_generated/server";
import { v } from "convex/values";

export const listFeed = query({
  args: { limit: v.optional(v.number()) },
  handler: async (ctx, args) => {
    const limit = args.limit ?? 50;
    const rows = await ctx.db
      .query("posts")
      .withIndex("by_created_at", (q) => q)
      .order("desc")
      .take(limit);
    const withAuthor = await Promise.all(
      rows.map(async (p) => {
        const identity = await ctx.db.get(p.posting_identity_id);
        return {
          id: p._id,
          title: p.title,
          content: p.content,
          created_at: p.created_at,
          updated_at: p.updated_at,
          author: identity ? { name: identity.name } : { name: "Unknown" },
        };
      })
    );
    return withAuthor;
  },
});

export const get = query({
  args: { id: v.id("posts") },
  handler: async (ctx, args) => {
    const post = await ctx.db.get(args.id);
    if (!post) return null;
    const identity = await ctx.db.get(post.posting_identity_id);
    return {
      id: post._id,
      title: post.title,
      content: post.content,
      created_at: post.created_at,
      updated_at: post.updated_at,
      author: identity ? { name: identity.name } : { name: "Unknown" },
    };
  },
});

export const create = mutation({
  args: {
    title: v.string(),
    content: v.string(),
    posting_identity_id: v.id("posting_identities"),
  },
  handler: async (ctx, args) => {
    const now = Date.now();
    return await ctx.db.insert("posts", {
      title: args.title.trim(),
      content: args.content.trim(),
      posting_identity_id: args.posting_identity_id,
      created_at: now,
      updated_at: now,
    });
  },
});
