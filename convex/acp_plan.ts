import { queryGeneric, mutationGeneric } from "convex/server";
import type { PlanEntry } from '@agentclientprotocol/sdk'

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

export const set = mutationGeneric(async ({ db }, args: { threadId: string; entries?: PlanEntry[]; entries_json?: string; entries_content?: string[]; entries_priority?: PlanEntry['priority'][]; entries_status?: PlanEntry['status'][] }) => {
  const now = Date.now();
  const entries: PlanEntry[] = (() => {
    if (Array.isArray(args.entries)) return args.entries as PlanEntry[];
    const c = Array.isArray(args.entries_content) ? args.entries_content : [];
    const p = Array.isArray(args.entries_priority) ? args.entries_priority : [];
    const s = Array.isArray(args.entries_status) ? args.entries_status : [];
    if (c.length && c.length === p.length && c.length === s.length) {
      return c.map((content, i) => ({ content, priority: p[i] as PlanEntry['priority'], status: s[i] as PlanEntry['status'] }));
    }
    try { const v = JSON.parse(String(args.entries_json||'[]')); return Array.isArray(v) ? (v as PlanEntry[]) : [] } catch { return [] }
  })();
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
    await db.patch(doc._id, { entries, updatedAt: now } as any);
    return doc._id;
  }
  const id = await db.insert('acp_plan', { threadId: args.threadId, entries, createdAt: now, updatedAt: now } as any);
  return id;
});
