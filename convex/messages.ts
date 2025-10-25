import { queryGeneric, mutationGeneric } from "convex/server";

type ForThreadArgs = { threadId: string; limit?: number | bigint | string; since?: number | bigint | string };
type CreateArgs = { threadId: string; role?: string; kind?: string; text?: string; data?: any; ts?: number };

function toNum(v: unknown, fallback: number): number {
  if (typeof v === 'number') return v;
  if (typeof v === 'bigint') return Number(v);
  if (typeof v === 'string') {
    const n = Number(v);
    return Number.isFinite(n) ? n : fallback;
  }
  return fallback;
}

export const forThread = queryGeneric(async ({ db }, args: ForThreadArgs) => {
  const { threadId } = args;
  const limit = Math.max(1, Math.min(toNum(args.limit, 400), 800));
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
  // Count only primary chat messages (assistant/user). Exclude reasoning/tool items.
  try {
    // @ts-ignore convex query builder supports withIndex + filter
    const rows = await db
      .query("messages")
      .withIndex('by_thread_ts', (q: any) => q.eq('threadId', args.threadId))
      .filter((q: any) =>
        q.or(
          q.eq(q.field('kind'), 'message'),
          q.or(q.eq(q.field('role'), 'assistant'), q.eq(q.field('role'), 'user'))
        )
      )
      .collect();
    return Array.isArray(rows) ? rows.length : 0;
  } catch {
    const rows = await db
      .query("messages")
      .filter((q) =>
        q.and(
          q.eq(q.field("threadId"), args.threadId),
          q.or(
            q.eq(q.field('kind'), 'message'),
            q.or(q.eq(q.field('role'), 'assistant'), q.eq(q.field('role'), 'user'))
          )
        )
      )
      .collect();
    return Array.isArray(rows) ? rows.length : 0;
  }
});
