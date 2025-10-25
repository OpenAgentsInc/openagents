import { queryGeneric, mutationGeneric } from "convex/server";

/**
 * Projects: Convex wrappers around the filesystem-backed source of truth.
 * The bridge scans ~/.openagents/projects and calls upsertFromFs() to mirror
 * changes into Convex. The app can subscribe via projects:list.
 */

type Repo = { provider?: string; remote?: string; url?: string; branch?: string };
type ProjectDoc = {
  id: string;
  name: string;
  workingDir: string;
  repo?: Repo;
  agentFile?: string;
  instructions?: string;
  createdAt: number;
  updatedAt: number;
};

export const list = queryGeneric(async ({ db }) => {
  // Return all projects sorted alphabetically by name for predictable menus.
  const rows = await db.query("projects").collect();
  return (rows as ProjectDoc[]).sort((a, b) => a.name.localeCompare(b.name));
});

export const byId = queryGeneric(async ({ db }, args: { id: string }) => {
  // Prefer index if available
  try {
    // @ts-ignore convex query builder supports withIndex
    const rows = await db
      .query("projects")
      .withIndex('by_project_id', (q: any) => q.eq('id', args.id))
      .collect();
    if (Array.isArray(rows) && rows[0]) return rows[0];
  } catch {}
  // Fallback scan
  const rows = await db
    .query("projects")
    .filter((q) => q.eq(q.field("id"), args.id))
    .collect();
  return rows[0] ?? null;
});

/**
 * Upsert from filesystem metadata.
 * Use when mirroring ~/.openagents/projects/<id>/PROJECT.md into Convex.
 */
export const upsertFromFs = mutationGeneric(async ({ db }, p: ProjectDoc) => {
  const now = typeof p.updatedAt === 'number' ? p.updatedAt : Date.now();
  // Find existing by project id
  let existing: any[] = [];
  try {
    // @ts-ignore withIndex available on newer backends
    existing = await db
      .query("projects")
      .withIndex('by_project_id', (q: any) => q.eq('id', p.id))
      .collect();
  } catch {
    existing = await db
      .query("projects")
      .filter((q) => q.eq(q.field("id"), p.id))
      .collect();
  }
  if (existing.length > 0) {
    const doc = existing[0]!;
    await db.patch(doc._id, {
      name: p.name,
      workingDir: p.workingDir,
      repo: p.repo,
      agentFile: p.agentFile,
      instructions: p.instructions,
      updatedAt: now,
    } as any);
    return doc._id;
  }
  const createdAt = typeof p.createdAt === 'number' ? p.createdAt : now;
  const id = await db.insert("projects", {
    id: p.id,
    name: p.name,
    workingDir: p.workingDir,
    repo: p.repo,
    agentFile: p.agentFile,
    instructions: p.instructions,
    createdAt,
    updatedAt: now,
  } as any);
  return id;
});

export const remove = mutationGeneric(async ({ db }, args: { id: string }) => {
  // Remove by project id when the folder/file is deleted on disk.
  let rows: any[] = [];
  try {
    // @ts-ignore
    rows = await db
      .query("projects")
      .withIndex('by_project_id', (q: any) => q.eq('id', args.id))
      .collect();
  } catch {
    rows = await db
      .query("projects")
      .filter((q) => q.eq(q.field("id"), args.id))
      .collect();
  }
  for (const row of rows) {
    try { await db.delete(row._id); } catch {}
  }
  return true;
});
