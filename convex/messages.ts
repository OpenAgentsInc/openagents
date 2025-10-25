import { queryGeneric, mutationGeneric } from "convex/server";

type ForThreadArgs = { threadId: string; limit?: number; since?: number };
type CreateArgs = { threadId: string; role?: string; kind?: string; text?: string; data?: any; ts?: number };

export const forThread = queryGeneric(async ({ db }, args: ForThreadArgs) => {
  const { threadId } = args;
  const limit = Math.max(1, Math.min(args.limit ?? 400, 800));
  // Use index + descending + take for newest N
  try {
    // @ts-ignore withIndex/order/take exist on Convex query builders
    const rows = await db
      .query("messages")
      .withIndex('by_thread_ts', (q: any) => q.eq('threadId', threadId))
      .order('desc')
      .take(limit);
    return Array.isArray(rows) ? rows.reverse() : [];
  } catch {
    // Fallback to full collect (rare) with server-side clamp
    const rows = await db
      .query("messages")
      .filter((q) => q.eq(q.field("threadId"), threadId))
      .order('desc' as any)
      .collect();
    return (rows as any[]).slice(0, limit).reverse();
  }
});

export const create = mutationGeneric(async ({ db }, args: CreateArgs) => {
  const { threadId } = args;
  const ts = args.ts ?? Date.now();
  const id = await db.insert("messages", {
    threadId,
    role: args.role,
    kind: args.kind || 'message',
    text: args.text,
    data: args.data,
    ts,
    createdAt: ts,
  });
  return id;
});

export const byId = queryGeneric(async ({ db }, args: { id: string }) => {
  try {
    // @ts-ignore: Convex typed id at runtime
    const doc = await db.get(args.id as any);
    if (doc) return doc;
  } catch {}
  const rows = await db
    .query("messages")
    // @ts-ignore: compare raw string form
    .filter((q) => q.eq(q.field("_id"), (args.id as any)))
    .collect();
  return rows[0] ?? null;
});

export const createDemo = mutationGeneric(async ({ db }, args: { threadId: string }) => {
  const ts = Date.now();
  const id = await db.insert("messages", {
    threadId: args.threadId,
    role: "assistant",
    text: "Hello from Convex messages!",
    kind: 'message',
    ts,
    createdAt: ts,
  });
  return id;
});

export const countForThread = queryGeneric(async ({ db }, args: { threadId: string }) => {
  try {
    const rows = await db
      .query("messages")
      // Use the composite index to select by threadId efficiently
      .withIndex('by_thread_ts', (q: any) => q.eq('threadId', args.threadId))
      .collect();
    return Array.isArray(rows) ? rows.length : 0;
  } catch {
    // Fallback if index isn't available yet
    const rows = await db
      .query("messages")
      .filter((q) => q.eq(q.field("threadId"), args.threadId))
      .collect();
    return Array.isArray(rows) ? rows.length : 0;
  }
});
