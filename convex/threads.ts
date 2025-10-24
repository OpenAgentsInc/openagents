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
  return id;
});
