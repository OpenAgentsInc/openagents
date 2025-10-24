import { mutationGeneric } from "convex/server";

export const clearAll = mutationGeneric(async ({ db }) => {
  // Delete messages first to avoid dangling references
  const msgs = await db.query("messages").collect();
  for (const m of msgs) {
    // @ts-ignore _id present at runtime
    await db.delete(m._id);
  }
  const threads = await db.query("threads").collect();
  for (const t of threads) {
    // @ts-ignore _id present at runtime
    await db.delete(t._id);
  }
  return { clearedMessages: msgs.length, clearedThreads: threads.length };
});

