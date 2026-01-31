import { mutation, query } from "./_generated/server";
import { v } from "convex/values";

export const listByPost = query({
  args: { postId: v.id("posts") },
  handler: async (ctx, args) => {
    const rows = await ctx.db
      .query("comments")
      .withIndex("by_post_id_and_created_at", (q) =>
        q.eq("post_id", args.postId)
      )
      .order("asc")
      .collect();
    const withAuthor = await Promise.all(
      rows.map(async (c) => {
        const authorName =
          c.posting_identity_id != null
            ? (await ctx.db.get(c.posting_identity_id))?.name
            : c.author;
        return {
          id: c._id,
          post_id: c.post_id,
          content: c.content,
          created_at: c.created_at ?? 0,
          author_name: authorName ?? "Unknown",
        };
      })
    );
    return withAuthor;
  },
});

/** Internal: create a comment with a known posting_identity_id (used by action). */
export const create = mutation({
  args: {
    postId: v.id("posts"),
    posting_identity_id: v.id("posting_identities"),
    content: v.string(),
  },
  handler: async (ctx, args) => {
    const post = await ctx.db.get(args.postId);
    if (!post) throw new Error("Post not found");
    const now = Date.now();
    return await ctx.db.insert("comments", {
      post_id: args.postId,
      posting_identity_id: args.posting_identity_id,
      content: args.content.trim(),
      created_at: now,
    });
  },
});
