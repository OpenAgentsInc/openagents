"use node";

import { createHash } from "node:crypto";
import { action } from "./_generated/server";
import type { Id } from "./_generated/dataModel";
import { v } from "convex/values";
import { api, internal } from "./_generated/api";

function sha256Hex(input: string): string {
  return createHash("sha256").update(input, "utf8").digest("hex");
}

/** Create a post using API key; hashes key and looks up posting identity. */
export const createWithApiKey = action({
  args: {
    title: v.string(),
    content: v.string(),
    apiKey: v.string(),
  },
  handler: async (ctx, args): Promise<Id<"posts">> => {
    const tokenHash = sha256Hex(args.apiKey.trim());
    const postingIdentityId = (await ctx.runQuery(
      internal.identity_tokens.getByTokenHash,
      { token_hash: tokenHash }
    )) as Id<"posting_identities"> | null;
    if (!postingIdentityId) {
      throw new Error("Invalid API key");
    }
    return await ctx.runMutation(api.posts.create, {
      title: args.title.trim(),
      content: args.content.trim(),
      posting_identity_id: postingIdentityId,
    });
  },
});
