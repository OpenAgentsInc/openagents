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
  // Typed, flattened vectors as an alternative to JSON/object arrays
  content_texts?: string[];
  locations_paths?: string[];
  locations_lines?: (number | null)[];
}) => {
  const now = Date.now();
  const parseJsonArray = (s?: string): any[] | undefined => {
    if (!s || typeof s !== 'string') return undefined as any;
    try { const v = JSON.parse(s); return Array.isArray(v) ? v : undefined as any } catch { return undefined as any }
  };
  const typedContent = (() => {
    if (Array.isArray(args.content)) return args.content as any[];
    if (Array.isArray(args.content_texts) && args.content_texts.length) {
      return (args.content_texts as string[]).map((text) => ({ type: 'content', content: { type: 'text', text } }));
    }
    return parseJsonArray(args.content_json);
  })();
  const typedLocs = (() => {
    if (Array.isArray(args.locations)) return args.locations as any[];
    const paths = Array.isArray(args.locations_paths) ? args.locations_paths : [];
    const lines = Array.isArray(args.locations_lines) ? args.locations_lines : [];
    if (paths.length && (lines.length === 0 || lines.length === paths.length)) {
      return paths.map((path, i) => ({ path, line: typeof lines[i] === 'number' ? Number(lines[i]) : undefined }));
    }
    return parseJsonArray(args.locations_json);
  })();
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
