import { mutationGeneric, queryGeneric } from "convex/server";

export const enqueue = mutationGeneric(async ({ db }, args: { threadDocId: string; text: string; role?: string; projectId?: string }) => {
  const now = Date.now();
  // Write user message immediately so UI updates
  await db.insert("messages", {
    threadId: args.threadDocId,
    role: args.role || 'user',
    kind: 'message',
    text: args.text,
    ts: now,
    createdAt: now,
  } as any);
  // Enqueue a run for the bridge runner
  const id = await db.insert("runs", {
    threadDocId: args.threadDocId,
    projectId: args.projectId || undefined,
    text: args.text,
    role: args.role || 'user',
    status: 'pending',
    createdAt: now,
    updatedAt: now,
  } as any);
  return id;
});

export const lease = mutationGeneric(async ({ db }) => {
  // Take the oldest pending run and mark as processing
  const rows = await db
    .query("runs")
    .filter((q) => q.eq(q.field("status"), 'pending'))
    .order("asc")
    .collect();
  if (!Array.isArray(rows) || rows.length === 0) return null;
  const row = rows[0] as any;
  await db.patch(row._id, { status: 'processing', updatedAt: Date.now() } as any);
  return row;
});

export const complete = mutationGeneric(async ({ db }, args: { id: string }) => {
  try {
    // @ts-ignore tolerate raw id
    await db.patch(args.id as any, { status: 'done', updatedAt: Date.now() } as any);
  } catch {}
  return true;
});

export const fail = mutationGeneric(async ({ db }, args: { id: string; error?: string }) => {
  try {
    // @ts-ignore tolerate raw id
    await db.patch(args.id as any, { status: 'error', error: args.error || '', updatedAt: Date.now() } as any);
  } catch {}
  return true;
});

