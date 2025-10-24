import { query, mutation } from "convex/server";

export const list = query(async ({ db }) => {
  return await db.query("threads").order("desc").collect();
});

export const createDemo = mutation(async ({ db }) => {
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

