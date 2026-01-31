"use node";

import { createHash } from "node:crypto";
import { action } from "./_generated/server";
import { v } from "convex/values";
import { api } from "./_generated/api";

function sha256Hex(input: string): string {
  return createHash("sha256").update(input, "utf8").digest("hex");
}

/** Create a comment using API key; hashes key and looks up posting identity. */
export const createWithApiKey = action({
  args: {
    postId: v.id("posts"),
    content: v.string(),
    apiKey: v.string(),
  },
  handler: async (ctx, args) => {
    const tokenHash = sha256Hex(args.apiKey.trim());
    const postingIdentityId = await ctx.runQuery(
      api.identity_tokens.getByTokenHash,
      { token_hash: tokenHash }
    );
    if (!postingIdentityId) {
      throw new Error("Invalid API key");
    }
    await ctx.runMutation(api.comments.create, {
      postId: args.postId,
      posting_identity_id: postingIdentityId,
      content: args.content,
    });
    return { ok: true };
  },
});
