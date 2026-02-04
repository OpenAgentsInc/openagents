/**
 * Convex-backed thread index (chats/projects).
 * Used by the left sidebar and (later) Hatchery workspace graph.
 */
import { v } from 'convex/values';
import { mutation, query } from './_generated/server';

export const list = query({
  args: {
    archived: v.optional(v.boolean()),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    const identity = (await ctx.auth.getUserIdentity()) as { subject?: string } | null;
    if (!identity?.subject) return [];
    const limit = Math.min(args.limit ?? 50, 100);
    const archived = args.archived ?? false;
    const docs = await ctx.db
      .query('threads')
      .withIndex('by_user_id_archived', (q) =>
        q.eq('user_id', identity.subject!).eq('archived', archived)
      )
      .order('desc')
      .take(limit);
    return docs.map((doc) => ({
      _id: doc._id,
      _creationTime: doc._creationTime,
      user_id: doc.user_id,
      title: doc.title,
      kind: doc.kind ?? undefined,
      archived: doc.archived ?? false,
      created_at: doc.created_at,
      updated_at: doc.updated_at,
    }));
  },
});

export const get = query({
  args: {
    threadId: v.id('threads'),
  },
  handler: async (ctx, args) => {
    const identity = (await ctx.auth.getUserIdentity()) as { subject?: string } | null;
    if (!identity?.subject) return null;
    const doc = await ctx.db.get(args.threadId);
    if (!doc || doc.user_id !== identity.subject) return null;
    return {
      _id: doc._id,
      _creationTime: doc._creationTime,
      user_id: doc.user_id,
      title: doc.title,
      kind: doc.kind ?? undefined,
      archived: doc.archived ?? false,
      created_at: doc.created_at,
      updated_at: doc.updated_at,
    };
  },
});

export const create = mutation({
  args: {
    title: v.optional(v.string()),
    kind: v.optional(v.union(v.literal('chat'), v.literal('project'), v.literal('openclaw'))),
  },
  handler: async (ctx, args) => {
    const identity = (await ctx.auth.getUserIdentity()) as { subject?: string } | null;
    if (!identity?.subject) throw new Error('not authenticated');
    const now = Date.now();
    const title = args.title?.trim() || 'New Chat';
    const id = await ctx.db.insert('threads', {
      user_id: identity.subject,
      title,
      kind: args.kind,
      archived: false,
      created_at: now,
      updated_at: now,
    });
    return id;
  },
});

export const updateTitle = mutation({
  args: {
    threadId: v.id('threads'),
    title: v.string(),
  },
  handler: async (ctx, args) => {
    const identity = (await ctx.auth.getUserIdentity()) as { subject?: string } | null;
    if (!identity?.subject) throw new Error('not authenticated');
    const doc = await ctx.db.get(args.threadId);
    if (!doc || doc.user_id !== identity.subject) throw new Error('thread not found');
    await ctx.db.patch(args.threadId, {
      title: args.title.trim() || doc.title,
      updated_at: Date.now(),
    });
    return undefined;
  },
});

export const archive = mutation({
  args: {
    threadId: v.id('threads'),
    archived: v.boolean(),
  },
  handler: async (ctx, args) => {
    const identity = (await ctx.auth.getUserIdentity()) as { subject?: string } | null;
    if (!identity?.subject) throw new Error('not authenticated');
    const doc = await ctx.db.get(args.threadId);
    if (!doc || doc.user_id !== identity.subject) throw new Error('thread not found');
    await ctx.db.patch(args.threadId, {
      archived: args.archived,
      updated_at: Date.now(),
    });
    return undefined;
  },
});
