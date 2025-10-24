import { queryGeneric, mutationGeneric } from "convex/server";
import type { GenericId } from "convex/values";

export const list = queryGeneric(async ({ db }) => {
  return await db.query("threads").order("desc").collect();
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
