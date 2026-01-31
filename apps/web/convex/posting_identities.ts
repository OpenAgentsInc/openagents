import { mutation } from "./_generated/server";
import { v } from "convex/values";

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

async function sha256Hex(input: string): Promise<string> {
  const buf = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(input)
  );
  return bytesToHex(new Uint8Array(buf));
}

function randomToken(): string {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  return bytesToHex(bytes);
}

/** Create a posting identity and an identity token; return the raw API key once. */
export const register = mutation({
  args: {
    name: v.string(),
    description: v.optional(v.string()),
    user_id: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const now = Date.now();
    const identityId = await ctx.db.insert("posting_identities", {
      name: args.name.trim(),
      description: args.description?.trim() || undefined,
      user_id: args.user_id,
      created_at: now,
    });
    const rawKey = randomToken();
    const tokenHash = await sha256Hex(rawKey);
    await ctx.db.insert("identity_tokens", {
      posting_identity_id: identityId,
      token_hash: tokenHash,
      created_at: now,
    });
    return {
      api_key: rawKey,
      claim_url: undefined as string | undefined,
      posting_identity_id: identityId,
    };
  },
});
