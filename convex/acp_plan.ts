import { queryGeneric, mutationGeneric } from "convex/server";

export const forThread = queryGeneric(async ({ db }, args: { threadId: string }) => {
  let rows: any[] = [];
  try {
    // @ts-ignore withIndex
    rows = await db
      .query('acp_plan')
      .withIndex('by_thread', (q: any) => q.eq('threadId', args.threadId))
      .collect();
  } catch {
    rows = await db
      .query('acp_plan')
      .filter((q) => q.eq(q.field('threadId'), args.threadId))
      .collect();
  }
  return rows[0] ?? null;
});

export const set = mutationGeneric(async ({ db }, args: { threadId: string; entries_json: string }) => {
  const now = Date.now();
  let rows: any[] = [];
  try {
    // @ts-ignore
    rows = await db
      .query('acp_plan')
      .withIndex('by_thread', (q: any) => q.eq('threadId', args.threadId))
      .collect();
  } catch {
    rows = await db
      .query('acp_plan')
      .filter((q) => q.eq(q.field('threadId'), args.threadId))
      .collect();
  }
  if (rows.length > 0) {
    const doc = rows[0]!;
    await db.patch(doc._id, { entries_json: args.entries_json, updatedAt: now } as any);
    return doc._id;
  }
  const id = await db.insert('acp_plan', { threadId: args.threadId, entries_json: args.entries_json, createdAt: now, updatedAt: now } as any);
  return id;
});

