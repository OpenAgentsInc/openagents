import { queryGeneric, mutationGeneric } from "convex/server";

export const forThread = queryGeneric(async ({ db }, args: { threadId: string }) => {
  try {
    // @ts-ignore withIndex/order/take exists
    return await db
      .query('acp_tool_calls')
      .withIndex('by_thread_updated', (q: any) => q.eq('threadId', args.threadId))
      .order('asc')
      .collect();
  } catch {
    return await db
      .query('acp_tool_calls')
      .filter((q) => q.eq(q.field('threadId'), args.threadId))
      .collect();
  }
});

export const upsert = mutationGeneric(async ({ db }, args: {
  threadId: string;
  toolCallId: string;
  title: string;
  kind: string;
  status: string;
  content_json?: string;
  locations_json?: string;
}) => {
  const now = Date.now();
  let rows: any[] = [];
  try {
    // @ts-ignore composite index
    rows = await db
      .query('acp_tool_calls')
      .withIndex('by_thread_tool', (q: any) => q.eq('threadId', args.threadId).eq('toolCallId', args.toolCallId))
      .collect();
  } catch {
    rows = await db
      .query('acp_tool_calls')
      .filter((q) => q.and(q.eq(q.field('threadId'), args.threadId), q.eq(q.field('toolCallId'), args.toolCallId)))
      .collect();
  }
  if (rows.length > 0) {
    const doc = rows[0]!;
    await db.patch(doc._id, {
      title: args.title,
      kind: args.kind,
      status: args.status,
      content_json: args.content_json ?? doc.content_json,
      locations_json: args.locations_json ?? doc.locations_json,
      updatedAt: now,
    } as any);
    return doc._id;
  }
  const id = await db.insert('acp_tool_calls', {
    threadId: args.threadId,
    toolCallId: args.toolCallId,
    title: args.title,
    kind: args.kind,
    status: args.status,
    content_json: args.content_json,
    locations_json: args.locations_json,
    createdAt: now,
    updatedAt: now,
  } as any);
  return id;
});

