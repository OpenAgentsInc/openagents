import { query } from "./_generated/server";
import { v } from "convex/values";

/** Internal: lookup posting_identity_id by token hash. */
export const getByTokenHash = query({
  args: { token_hash: v.string() },
  handler: async (ctx, args) => {
    const row = await ctx.db
      .query("identity_tokens")
      .withIndex("by_token_hash", (q) => q.eq("token_hash", args.token_hash))
      .first();
    return row ? row.posting_identity_id : null;
  },
});
