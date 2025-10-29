import { queryGeneric, mutationGeneric } from "convex/server";
import type { ToolCallContent, ToolCallLocation, ToolKind, ToolCallStatus } from '@agentclientprotocol/sdk'

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
  kind: ToolKind;
  status: ToolCallStatus;
  // Accept either pre-parsed arrays or JSON strings from upstream
  content?: ToolCallContent[];
  content_json?: string;
  locations?: ToolCallLocation[];
  locations_json?: string;
  // Typed, flattened vectors as an alternative to JSON/object arrays
  content_texts?: string[];
  content_diff_paths?: string[];
  content_diff_new_texts?: string[];
  content_diff_old_texts?: (string | null)[];
  content_terminal_ids?: string[];
  locations_paths?: string[];
  locations_lines?: (number | null)[];
}) => {
  const now = Date.now();
  const parseJsonArray = (s?: string): any[] | undefined => {
    if (!s || typeof s !== 'string') return undefined as any;
    try { const v = JSON.parse(s); return Array.isArray(v) ? v : undefined as any } catch { return undefined as any }
  };
  const typedContent: ToolCallContent[] | undefined = (() => {
    if (Array.isArray(args.content)) return args.content as ToolCallContent[];
    if (Array.isArray(args.content_texts) && args.content_texts.length) {
      return (args.content_texts as string[]).map((text) => ({ type: 'content', content: { type: 'text', text } })) as ToolCallContent[];
    }
    // Merge diff vectors into content entries if present
    if (Array.isArray(args.content_diff_paths) && Array.isArray(args.content_diff_new_texts) && args.content_diff_paths.length === args.content_diff_new_texts.length) {
      const diffs = (args.content_diff_paths as string[]).map((path, i) => ({ type: 'diff', path, newText: (args.content_diff_new_texts as string[])[i], oldText: Array.isArray(args.content_diff_old_texts) ? (args.content_diff_old_texts as (string | null)[])[i] ?? undefined : undefined } as any));
      const base: ToolCallContent[] = [] as any;
      base.push(...diffs);
      return base;
    }
    // Terminal content embedding
    if (Array.isArray(args.content_terminal_ids) && args.content_terminal_ids.length) {
      const t = (args.content_terminal_ids as string[]).map((terminalId) => ({ type: 'terminal', terminalId } as any));
      return t as unknown as ToolCallContent[];
    }
    return parseJsonArray(args.content_json) as unknown as ToolCallContent[] | undefined;
  })();
  const typedLocs: ToolCallLocation[] | undefined = (() => {
    if (Array.isArray(args.locations)) return args.locations as ToolCallLocation[];
    const paths = Array.isArray(args.locations_paths) ? args.locations_paths : [];
    const lines = Array.isArray(args.locations_lines) ? args.locations_lines : [];
    if (paths.length && (lines.length === 0 || lines.length === paths.length)) {
      return paths.map((path, i) => ({ path, line: typeof lines[i] === 'number' ? Number(lines[i]) : undefined }));
    }
    return parseJsonArray(args.locations_json) as unknown as ToolCallLocation[] | undefined;
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
