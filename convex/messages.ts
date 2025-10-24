import { queryGeneric, mutationGeneric } from "convex/server";

type ForThreadArgs = { threadId: string };
type CreateArgs = { threadId: string; role: string; text: string; ts?: number };

export const forThread = queryGeneric(async ({ db }, args: ForThreadArgs) => {
  const { threadId } = args;
  const rows = await db
    .query("messages")
    .filter((q) => q.eq(q.field("threadId"), threadId))
    .order("asc")
    .collect();
  return rows;
});

export const create = mutationGeneric(async ({ db }, args: CreateArgs) => {
  const { threadId, role, text } = args;
  const ts = args.ts ?? Date.now();
  const id = await db.insert("messages", {
    threadId,
    role,
    text,
    ts,
    createdAt: ts,
  });
  // Bump thread.updatedAt when a new message arrives
  try {
    const thr = await db
      .query('threads')
      .filter((q) => q.eq(q.field('threadId'), threadId))
      .collect();
    if (thr.length > 0) {
      await db.patch(thr[0]!._id, { updatedAt: ts } as any);
    }
  } catch {}
  return id;
});

export const createDemo = mutationGeneric(async ({ db }, args: { threadId: string }) => {
  const ts = Date.now();
  const id = await db.insert("messages", {
    threadId: args.threadId,
    role: "assistant",
    text: "Hello from Convex messages!",
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
