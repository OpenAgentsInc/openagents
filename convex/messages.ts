import { queryGeneric, mutationGeneric } from "convex/server";

type ForThreadArgs = { threadId: string; limit?: number; since?: number };
type CreateArgs = { threadId: string; role?: string; kind?: string; text?: string; data?: any; ts?: number };

export const forThread = queryGeneric(async ({ db }, args: ForThreadArgs) => {
  const { threadId } = args;
  const limit = Math.max(1, Math.min(args.limit ?? 400, 800));
  // Prefer indexed, descending read of the newest N messages to stay under Convex read byte limits
  const builder = db
    .query("messages")
    .withIndex?.('by_thread_ts', (q: any) => q.eq('threadId', threadId))
    ?? db.query("messages").filter((q) => q.eq(q.field("threadId"), threadId));
  const ordered = (builder as any).order?.('desc') ?? builder;
  const page = (ordered as any).take?.(limit) ?? (await (ordered as any).collect());
  const rows = Array.isArray(page) ? page.slice(0, limit).reverse() : [];
  return rows;
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
  const rows = await db
    .query("messages")
    .withIndex?.('threadId', (q: any) => q.eq('threadId', args.threadId))
    .filter((q) => q.eq(q.field("threadId"), args.threadId))
    .collect();
  return Array.isArray(rows) ? rows.length : 0;
});
