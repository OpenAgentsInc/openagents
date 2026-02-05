/**
 * Agent users and API keys (key-based auth, no WorkOS).
 * Used by API worker: signup creates agent + key; lookup by key hash resolves principal.
 */
import { v } from 'convex/values';
import { internalMutation, internalQuery } from './_generated/server';

const OA_AGENT_KEY_HMAC_SECRET = process.env.OA_AGENT_KEY_HMAC_SECRET ?? '';
const KEY_PREFIX = 'oak_live_';

function randomHex(bytes: number): string {
  const arr = new Uint8Array(bytes);
  crypto.getRandomValues(arr);
  return Array.from(arr, (b) => b.toString(16).padStart(2, '0')).join('');
}

async function hmacSha256Hex(secret: string, message: string): Promise<string> {
  if (!secret) {
    throw new Error('OA_AGENT_KEY_HMAC_SECRET must be set in Convex env');
  }
  const encoder = new TextEncoder();
  const key = await crypto.subtle.importKey(
    'raw',
    encoder.encode(secret),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  );
  const sig = await crypto.subtle.sign(
    'HMAC',
    key,
    encoder.encode(message),
  );
  return Array.from(new Uint8Array(sig))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

/**
 * Create an agent user and one API key. Returns one-time apiKey; caller must not log it.
 * Called from control HTTP only (API worker with x-oa-control-key).
 */
export const createAgentUserAndKey = internalMutation({
  args: {
    handle: v.optional(v.string()),
    owner_workos_user_id: v.optional(v.string()),
    scopes: v.optional(v.array(v.string())),
    description: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const now = Date.now();
    const agentUserId = await ctx.db.insert('agent_users', {
      handle: args.handle ?? undefined,
      owner_workos_user_id: args.owner_workos_user_id ?? undefined,
      status: 'active',
      created_at: now,
      updated_at: now,
    });

    const rawKey = `${KEY_PREFIX}${randomHex(32)}`;
    const keyHash = await hmacSha256Hex(OA_AGENT_KEY_HMAC_SECRET, rawKey);
    const keyId = `oak_${randomHex(8)}`;

    await ctx.db.insert('agent_api_keys', {
      agent_user_id: agentUserId,
      key_id: keyId,
      key_hash: keyHash,
      scopes: args.scopes ?? ['openclaw:read', 'openclaw:write'],
      description: args.description,
      created_at: now,
    });

    return {
      agentUserId: agentUserId,
      apiKey: rawKey,
      keyId,
    };
  },
});

/**
 * Resolve agent principal by key hash. Used by API worker after hashing the provided key.
 * Returns null if key not found or revoked.
 */
export const getAgentByKeyHash = internalQuery({
  args: {
    key_hash: v.string(),
  },
  handler: async (ctx, args) => {
    const key = await ctx.db
      .query('agent_api_keys')
      .withIndex('by_key_hash', (q) => q.eq('key_hash', args.key_hash))
      .first();
    if (!key || key.revoked_at) {
      return null;
    }
    const user = await ctx.db.get('agent_users', key.agent_user_id);
    if (!user || user.status !== 'active') {
      return null;
    }
    return {
      agent_user_id: key.agent_user_id,
      scopes: key.scopes,
    };
  },
});

/**
 * Update last_used_at for an agent API key (by key hash). Called by API worker after successful auth.
 */
export const touchAgentKeyLastUsed = internalMutation({
  args: {
    key_hash: v.string(),
  },
  handler: async (ctx, args) => {
    const key = await ctx.db
      .query('agent_api_keys')
      .withIndex('by_key_hash', (q) => q.eq('key_hash', args.key_hash))
      .first();
    if (key) {
      await ctx.db.patch(key._id, { last_used_at: Date.now() });
    }
    return null;
  },
});
