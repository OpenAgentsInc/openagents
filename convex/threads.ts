import { queryGeneric, mutationGeneric } from "convex/server";
import type { GenericId } from "convex/values";

export const list = queryGeneric(async ({ db }) => {
  return await db.query("threads").order("desc").collect();
});

// Return newest threads with an aggregate count of primary chat messages.
// Filters out threads with zero messages. Limit defaults to 10.
export const listWithCounts = queryGeneric(async ({ db }, args?: { limit?: number | bigint | string }) => {
  const toNum = (v: unknown, d: number) => {
    if (typeof v === 'number') return v;
    if (typeof v === 'bigint') return Number(v);
    if (typeof v === 'string') { const n = Number(v); return Number.isFinite(n) ? n : d; }
    return d;
  };
  const limit = Math.max(1, Math.min(toNum(args?.limit, 10), 100));
  // Load recent threads (descending by updatedAt)
  const threads = await db.query('threads').order('desc').collect();
  const out: any[] = [];
  for (const row of threads) {
    // Count primary messages for this thread
    let count = 0;
    try {
      // @ts-ignore withIndex/order/filter are available on Convex query builders
      const msgs = await db
        .query('messages')
        .withIndex('by_thread_ts', (q: any) => q.eq('threadId', String((row as any)?._id || (row as any)?.threadId || '')))
        .filter((q: any) =>
          q.or(
            q.eq(q.field('kind'), 'message'),
            q.or(q.eq(q.field('role'), 'assistant'), q.eq(q.field('role'), 'user'))
          )
        )
        .collect();
      count = Array.isArray(msgs) ? msgs.length : 0;
    } catch {}
    if (count > 0) {
      out.push({ ...(row as any), messageCount: count });
      if (out.length >= limit) break;
    }
  }
  return out;
});

export const byId = queryGeneric(async ({ db }, args: { id: GenericId<"threads"> | string }) => {
  // Convex generics: allow either a typed id or raw string; db.get expects a typed id.
  // We fall back to scanning if we weren't given a typed id.
  try {
    // @ts-ignore: tolerate id shape at runtime
    const doc = await db.get(args.id as any);
    if (doc) return doc;
  } catch {}
  const rows = await db
    .query("threads")
    // @ts-ignore: compare string forms as a fallback
    .filter((q) => q.eq(q.field("_id"), (args.id as any)))
    .collect();
  return rows[0] ?? null;
});

export const upsertFromStream = mutationGeneric(async (
  { db },
  args: {
    threadId?: string; // Convex document id string (preferred key)
    resumeId?: string; // Codex CLI thread id for resume
    title?: string;
    projectId?: string;
    rolloutPath?: string;
    source?: string;
    createdAt?: number; // millis
    updatedAt?: number; // millis
  }
) => {
  const createdAt = typeof args.createdAt === 'number' ? args.createdAt : 0;
  const updatedAt = typeof args.updatedAt === 'number' ? args.updatedAt : createdAt;
  let existing: any[] = [];
  if (args.threadId) {
    existing = await db
      .query("threads")
      .filter((q) => q.eq(q.field("threadId"), args.threadId))
      .collect();
  }
  if (existing.length === 0 && args.resumeId) {
    existing = await db
      .query("threads")
      .filter((q) => q.eq(q.field("resumeId"), args.resumeId))
      .collect();
  }
  if (existing.length > 0) {
    const doc = existing[0]!;
    await db.patch(doc._id, {
      title: args.title ?? doc.title,
      projectId: args.projectId ?? doc.projectId,
      rolloutPath: args.rolloutPath ?? doc.rolloutPath,
      resumeId: args.resumeId ?? doc.resumeId,
      source: args.source ?? doc.source,
      updatedAt,
    } as any);
    return doc._id;
  }
  const id = await db.insert("threads", {
    threadId: args.threadId || (args.resumeId ? String(args.resumeId) : ''),
    title: args.title ?? "New Thread",
    projectId: args.projectId ?? "",
    rolloutPath: args.rolloutPath ?? "",
    resumeId: args.resumeId ?? "",
    source: args.source ?? "stream",
    createdAt,
    updatedAt,
  } as any);
  return id;
});

export const createDemo = mutationGeneric(async ({ db }) => {
  const now = Date.now();
  const id = await db.insert("threads", {
    title: "Demo Thread",
    rolloutPath: "",
    resumeId: "",
    projectId: "",
    source: "demo",
    createdAt: now,
    updatedAt: now,
  });
  // Best-effort: set threadId to the document id string for legacy/demo rows
  try {
    // @ts-ignore tolerate patching with stringified id
    await db.patch(id as any, { threadId: String(id as any) } as any);
  } catch {}
  return id;
});

// Create a new empty thread and return its id. The threadId field is set to the stringified document id
// so that messages can reference it consistently.
export const create = mutationGeneric(async ({ db }, args?: { title?: string; projectId?: string }) => {
  const now = Date.now();
  const id = await db.insert("threads", {
    title: args?.title || "New Thread",
    rolloutPath: "",
    resumeId: "",
    projectId: args?.projectId || "",
    source: "app",
    createdAt: now,
    updatedAt: now,
  } as any);
  try {
    // @ts-ignore
    await db.patch(id as any, { threadId: String(id as any) } as any);
  } catch {}
  return id;
});
