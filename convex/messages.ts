import { queryGeneric, mutationGeneric } from "convex/server";

type ForThreadArgs = { threadId: string; limit?: number | bigint | string; since?: number | bigint | string };
type CreateArgs = { threadId: string; role?: string; kind?: string; text?: string; data?: any; ts?: number };
type UpsertStreamedArgs = { threadId: string; itemId: string; role?: string; kind?: string; text?: string; ts?: number; seq?: number };
type FinalizeStreamedArgs = { threadId: string; itemId: string; text?: string };

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

// Create or overwrite a streamed message for (threadId, itemId).
// Sets partial=true. If a row already exists, patches text (replace) and seq/updatedAt.
export const upsertStreamed = mutationGeneric(async ({ db }, args: UpsertStreamedArgs) => {
  const now = typeof args.ts === 'number' ? args.ts : Date.now();
  const kind = args.kind || 'message';
  let rows: any[] = [];
  try {
    // @ts-ignore composite index
    rows = await db
      .query('messages')
      .withIndex('by_thread_item', (q: any) => q.eq('threadId', args.threadId).eq('itemId', args.itemId))
      .collect();
  } catch {
    rows = await db
      .query('messages')
      .filter((q) => q.and(q.eq(q.field('threadId'), args.threadId), q.eq(q.field('itemId'), args.itemId)))
      .collect();
  }
  if (rows.length > 0) {
    const doc = rows[0] as any;
    await db.patch(doc._id, {
      role: args.role ?? doc.role,
      kind,
      text: args.text ?? doc.text,
      partial: true,
      seq: typeof args.seq === 'number' ? args.seq : (doc.seq ?? 0),
      updatedAt: now,
    } as any);
    return doc._id;
  }
  const id = await db.insert('messages', {
    threadId: args.threadId,
    itemId: args.itemId,
    role: args.role,
    kind,
    text: args.text ?? '',
    partial: true,
    seq: typeof args.seq === 'number' ? args.seq : 0,
    ts: now,
    createdAt: now,
    updatedAt: now,
  } as any);
  return id;
});

// Append a delta to an existing streamed message. No-op if not found.

// Finalize a streamed message (partial=false). Optionally set final text.
export const finalizeStreamed = mutationGeneric(async ({ db }, args: FinalizeStreamedArgs) => {
  let rows: any[] = [];
  try {
    // @ts-ignore composite index
    rows = await db
      .query('messages')
      .withIndex('by_thread_item', (q: any) => q.eq('threadId', args.threadId).eq('itemId', args.itemId))
      .collect();
  } catch {
    rows = await db
      .query('messages')
      .filter((q) => q.and(q.eq(q.field('threadId'), args.threadId), q.eq(q.field('itemId'), args.itemId)))
      .collect();
  }
  if (rows.length === 0) return null;
  const row = rows[0] as any;
  await db.patch(row._id, { text: typeof args.text === 'string' ? args.text : row.text, partial: false, updatedAt: Date.now() } as any);
  return row._id;
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
