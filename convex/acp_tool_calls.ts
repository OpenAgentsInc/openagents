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
  // Accept either pre-parsed arrays or JSON strings from upstream
  content?: any[];
  content_json?: string;
  locations?: { path: string; line?: number }[];
  locations_json?: string;
}) => {
  const now = Date.now();
  const parseJsonArray = (s?: string): any[] | undefined => {
    if (!s || typeof s !== 'string') return undefined as any;
    try { const v = JSON.parse(s); return Array.isArray(v) ? v : undefined as any } catch { return undefined as any }
  };
  const typedContent = Array.isArray(args.content) ? args.content : parseJsonArray(args.content_json);
  const typedLocs = Array.isArray(args.locations) ? args.locations : parseJsonArray(args.locations_json);
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
      content: typedContent ?? doc.content,
      locations: typedLocs ?? doc.locations,
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
    content: typedContent,
    locations: typedLocs,
    createdAt: now,
    updatedAt: now,
  } as any);
  return id;
});
