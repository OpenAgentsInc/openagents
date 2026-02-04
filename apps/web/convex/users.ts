import { v } from 'convex/values';
import { internalMutation, mutation } from './_generated/server';
import { requireFound } from './lib/errors';
import { requireUser } from './lib/users';

export const ensureUser = mutation({
  args: {},
  handler: async (ctx) => {
    const user = await requireUser(ctx);
    return {
      user_id: user.user_id,
      email: user.email ?? null,
      name: user.name ?? null,
      access_enabled: user.access_enabled ?? false,
    };
  },
});

export const upsertAgentUser = internalMutation({
  args: {
    user_id: v.string(),
    name: v.optional(v.string()),
    metadata: v.optional(v.any()),
    nostr_pubkey: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const existing = await ctx.db
      .query('users')
      .withIndex('by_user_id', (q) => q.eq('user_id', args.user_id))
      .first();

    const now = Date.now();
    if (existing) {
      const patch: Record<string, unknown> = {
        updated_at: now,
      };
      if (args.name !== undefined) {
        patch.name = args.name;
      }
      if (args.metadata !== undefined) {
        patch.metadata = args.metadata;
      }
      if (args.nostr_pubkey !== undefined) {
        patch.nostr_pubkey = args.nostr_pubkey;
      }
      if (existing.kind !== 'agent') {
        patch.kind = 'agent';
      }
      if (Object.keys(patch).length > 1) {
        await ctx.db.patch(existing._id, patch);
      }
      return {
        user_id: existing.user_id,
        created: false,
      };
    }

    const record: {
      user_id: string;
      kind: string;
      name?: string;
      metadata?: unknown;
      nostr_pubkey?: string;
      created_at: number;
      updated_at: number;
    } = {
      user_id: args.user_id,
      kind: 'agent',
      created_at: now,
      updated_at: now,
    };

    if (args.name !== undefined) {
      record.name = args.name;
    }
    if (args.metadata !== undefined) {
      record.metadata = args.metadata;
    }
    if (args.nostr_pubkey !== undefined) {
      record.nostr_pubkey = args.nostr_pubkey;
    }

    const userId = await ctx.db.insert('users', record);
    const created = await ctx.db.get(userId);
    const user = requireFound(created, 'NOT_FOUND', 'User not found after creation');
    return {
      user_id: user.user_id,
      created: true,
    };
  },
});
