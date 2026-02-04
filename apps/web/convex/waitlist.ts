import { v } from 'convex/values';
import { mutation } from './_generated/server';

export const joinWaitlist = mutation({
  args: {
    email: v.string(),
    source: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const normalized = args.email.trim().toLowerCase();
    if (!normalized) {
      throw new Error('Email is required');
    }
    const existing = await ctx.db
      .query('waitlist')
      .withIndex('by_email', (q) => q.eq('email', normalized))
      .first();
    if (existing) {
      return {
        id: existing._id,
        joined: false,
        approved: existing.approved ?? false,
      };
    }
    const id = await ctx.db.insert('waitlist', {
      email: normalized,
      source: args.source ?? 'hatchery',
      created_at: Date.now(),
      approved: false,
    });
    return { id, joined: true, approved: false };
  },
});
