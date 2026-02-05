import { v } from 'convex/values';
import { internalMutation, internalQuery, mutation, query } from './_generated/server';
import { getUser, requireUser } from './lib/users';
import { fail, requireFound } from './lib/errors';

const generateToken = (): string => {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
};

const hashToken = async (token: string): Promise<string> => {
  const encoder = new TextEncoder();
  const data = encoder.encode(token);
  const hashBuffer = await crypto.subtle.digest('SHA-256', data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map((byte) => byte.toString(16).padStart(2, '0')).join('');
};

export const createApiToken = mutation({
  args: {
    name: v.string(),
  },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx);

    const token = generateToken();
    const tokenHash = await hashToken(token);

    const tokenId = await ctx.db.insert('api_tokens', {
      user_id: user.user_id,
      token_hash: tokenHash,
      name: args.name,
      created_at: Date.now(),
    });

    return {
      token,
      tokenId,
    };
  },
});

export const listApiTokens = query({
  args: {},
  handler: async (ctx) => {
    const user = await getUser(ctx);
    if (!user) {
      return [];
    }

    const tokens = await ctx.db
      .query('api_tokens')
      .withIndex('by_user_id', (q) => q.eq('user_id', user.user_id))
      .collect();

    return tokens.map(({ token_hash, ...rest }) => rest);
  },
});

export const revokeApiToken = mutation({
  args: {
    tokenId: v.id('api_tokens'),
  },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx);

    const token = await ctx.db.get('api_tokens', args.tokenId);
    const tokenRecord = requireFound(token, 'NOT_FOUND', 'Token not found');

    if (tokenRecord.user_id !== user.user_id) {
      fail('UNAUTHORIZED', 'Unauthorized');
    }

    await ctx.db.delete('api_tokens', args.tokenId);
    return null;
  },
});

export const resolveApiToken = internalQuery({
  args: {
    token: v.string(),
  },
  handler: async (ctx, args) => {
    const tokenHash = await hashToken(args.token);
    const token = await ctx.db
      .query('api_tokens')
      .withIndex('by_token_hash', (q) => q.eq('token_hash', tokenHash))
      .first();

    if (!token) {
      return null;
    }
    if (token.expires_at && token.expires_at < Date.now()) {
      return null;
    }

    return {
      tokenId: token._id,
      tokenHash,
      user_id: token.user_id,
    };
  },
});

export const updateApiTokenLastUsed = internalMutation({
  args: {
    tokenHash: v.string(),
  },
  handler: async (ctx, args) => {
    const token = await ctx.db
      .query('api_tokens')
      .withIndex('by_token_hash', (q) => q.eq('token_hash', args.tokenHash))
      .first();

    if (token) {
      await ctx.db.patch('api_tokens', token._id, {
        last_used_at: Date.now(),
      });
    }

    return null;
  },
});

export const issueApiTokenForUser = internalMutation({
  args: {
    user_id: v.string(),
    name: v.string(),
  },
  handler: async (ctx, args) => {
    const user = await ctx.db
      .query('users')
      .withIndex('by_user_id', (q) => q.eq('user_id', args.user_id))
      .first();
    if (!user) {
      throw new Error(`User with user_id ${args.user_id} not found`);
    }

    const token = generateToken();
    const tokenHash = await hashToken(token);

    const tokenId = await ctx.db.insert('api_tokens', {
      user_id: user.user_id,
      token_hash: tokenHash,
      name: args.name,
      created_at: Date.now(),
    });

    return {
      token,
      tokenId,
      tokenHash,
    };
  },
});
