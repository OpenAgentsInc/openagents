/**
 * Convex-backed messages per thread for chat rehydration.
 * UIMessage parts are stored as JSON; list returns messages in order.
 */
import { v } from 'convex/values';
import type { Id } from './_generated/dataModel';
import { mutation, query } from './_generated/server';

export const list = query({
  args: {
    threadId: v.id('threads'),
  },
  handler: async (ctx, args) => {
    const identity = (await ctx.auth.getUserIdentity()) as { subject?: string } | null;
    if (!identity?.subject) return [];
    const thread = await ctx.db.get('threads', args.threadId);
    if (!thread || thread.user_id !== identity.subject) return [];
    const docs = await ctx.db
      .query('thread_messages')
      .withIndex('by_thread_id_order', (q) => q.eq('thread_id', args.threadId))
      .order('asc')
      .collect();
    return docs.map((doc) => ({
      id: doc.message_id,
      role: doc.role as 'user' | 'assistant' | 'system',
      parts: JSON.parse(doc.parts_json) as Array<{ type: string; text?: string; [key: string]: unknown }>,
      metadata: undefined as Record<string, unknown> | undefined,
    }));
  },
});

export const setMessages = mutation({
  args: {
    threadId: v.id('threads'),
    messages: v.array(
      v.object({
        id: v.string(),
        role: v.string(),
        parts: v.any(),
        metadata: v.optional(v.any()),
      }),
    ),
  },
  handler: async (ctx, args) => {
    const identity = (await ctx.auth.getUserIdentity()) as { subject?: string } | null;
    if (!identity?.subject) throw new Error('not authenticated');
    const thread = await ctx.db.get('threads', args.threadId);
    if (!thread || thread.user_id !== identity.subject) throw new Error('thread not found');
    const existing = await ctx.db
      .query('thread_messages')
      .withIndex('by_thread_id_order', (q) => q.eq('thread_id', args.threadId))
      .collect();
    for (const row of existing) {
      await ctx.db.delete(row._id);
    }
    const now = Date.now();
    for (let i = 0; i < args.messages.length; i++) {
      const m = args.messages[i];
      await ctx.db.insert('thread_messages', {
        thread_id: args.threadId,
        message_id: m.id,
        role: m.role,
        parts_json: JSON.stringify(m.parts ?? []),
        order: i,
        created_at: now,
      });
    }
    await ctx.db.patch('threads', args.threadId, { updated_at: now });
    return undefined;
  },
});
