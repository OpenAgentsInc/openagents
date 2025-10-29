import { queryGeneric, mutationGeneric } from "convex/server";

export const forThread = queryGeneric(async ({ db }, args: { threadId: string }) => {
  let rows: any[] = [];
  try {
    // @ts-ignore
    rows = await db
      .query('acp_state')
      .withIndex('by_thread', (q: any) => q.eq('threadId', args.threadId))
      .collect();
  } catch {
    rows = await db
      .query('acp_state')
      .filter((q) => q.eq(q.field('threadId'), args.threadId))
      .collect();
  }
  return rows[0] ?? null;
});

export const update = mutationGeneric(async ({ db }, args: { threadId: string; currentModeId?: string | null; available_commands_json?: string | null }) => {
  const now = Date.now();
  let rows: any[] = [];
  try {
    // @ts-ignore
    rows = await db
      .query('acp_state')
      .withIndex('by_thread', (q: any) => q.eq('threadId', args.threadId))
      .collect();
  } catch {
    rows = await db
      .query('acp_state')
      .filter((q) => q.eq(q.field('threadId'), args.threadId))
      .collect();
  }
  if (rows.length > 0) {
    const doc = rows[0]!;
    await db.patch(doc._id, {
      currentModeId: typeof args.currentModeId === 'string' ? args.currentModeId : doc.currentModeId,
      available_commands_json: typeof args.available_commands_json === 'string' ? args.available_commands_json : doc.available_commands_json,
      updatedAt: now,
    } as any);
    return doc._id;
  }
  const id = await db.insert('acp_state', {
    threadId: args.threadId,
    currentModeId: typeof args.currentModeId === 'string' ? args.currentModeId : undefined,
    available_commands_json: typeof args.available_commands_json === 'string' ? args.available_commands_json : undefined,
    createdAt: now,
    updatedAt: now,
  } as any);
  return id;
});

