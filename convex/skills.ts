import { queryGeneric, mutationGeneric } from "convex/server";

/**
 * Skills: merged personal/registry/project-scoped view, mirrored from the filesystem.
 * The bridge calls upsert/remove functions on startup and on file changes.
 */

type Scope = 'user' | 'registry' | 'project';

type SkillDoc = {
  skillId: string;
  name: string;
  description: string;
  license?: string | null;
  allowed_tools?: string[] | null;
  metadata?: any;
  source: Scope;
  projectId?: string | null;
  path?: string | null;
  createdAt: number;
  updatedAt: number;
};

export const listAll = queryGeneric(async ({ db }) => {
  const rows = await db.query("skills").collect();
  return (rows as SkillDoc[]).sort((a, b) => a.name.localeCompare(b.name));
});

export const listByScope = queryGeneric(async ({ db }, args: { source?: Scope; projectId?: string }) => {
  const { source, projectId } = args || {} as any;
  let q = db.query("skills");
  if (source === 'project' && projectId) {
    try {
      // @ts-ignore withIndex
      const rows = await db
        .query("skills")
        .withIndex('by_project', (q: any) => q.eq('projectId', projectId))
        .collect();
      return (rows as SkillDoc[]).sort((a, b) => a.name.localeCompare(b.name));
    } catch {}
  }
  const rows = await q.collect();
  const filtered = (rows as SkillDoc[]).filter((s) => {
    if (source && s.source !== source) return false;
    if (source === 'project' && projectId) return s.projectId === projectId;
    return true;
  });
  return filtered.sort((a, b) => a.name.localeCompare(b.name));
});

/**
 * Upsert one skill document based on (skillId, source, projectId?) unique key.
 */
export const upsertFromFs = mutationGeneric(async ({ db }, s: SkillDoc) => {
  const key = { skillId: s.skillId, source: s.source, projectId: s.projectId || null } as const;
  let existing: any[] = [];
  try {
    // @ts-ignore withIndex on composite key
    existing = await db
      .query("skills")
      .withIndex('by_skill_source_project', (q: any) => q.eq('skillId', key.skillId).eq('source', key.source).eq('projectId', key.projectId))
      .collect();
  } catch {
    existing = await db
      .query("skills")
      .filter((q) =>
        q.and(
          q.eq(q.field('skillId'), key.skillId),
          q.and(q.eq(q.field('source'), key.source), q.eq(q.field('projectId'), key.projectId))
        )
      )
      .collect();
  }
  if (existing.length > 0) {
    const doc = existing[0]!;
    await db.patch(doc._id, {
      name: s.name,
      description: s.description,
      license: s.license ?? undefined,
      allowed_tools: s.allowed_tools ?? undefined,
      metadata: s.metadata ?? undefined,
      path: s.path ?? undefined,
      updatedAt: s.updatedAt,
    } as any);
    return doc._id;
  }
  const id = await db.insert("skills", {
    skillId: s.skillId,
    name: s.name,
    description: s.description,
    license: s.license ?? undefined,
    allowed_tools: s.allowed_tools ?? undefined,
    metadata: s.metadata ?? undefined,
    source: s.source,
    projectId: s.projectId || undefined,
    path: s.path || undefined,
    createdAt: s.createdAt,
    updatedAt: s.updatedAt,
  } as any);
  return id;
});

/**
 * Bulk upsert from filesystem scans. Intended for startup and watcher batches.
 */
export const bulkUpsertFromFs = mutationGeneric(async ({ db }, args: { items: SkillDoc[] }) => {
  const items = Array.isArray(args.items) ? args.items : [];
  let last: any = null;
  for (const s of items) {
    // eslint-disable-next-line no-await-in-loop
    last = await upsertFromFs({ db } as any, s as any);
  }
  return last;
});

/** Remove a mirrored skill when its source file/folder is deleted. */
export const removeByScope = mutationGeneric(async ({ db }, args: { skillId: string; source: Scope; projectId?: string | null }) => {
  const projectId = args.projectId || null;
  let rows: any[] = [];
  try {
    // @ts-ignore withIndex
    rows = await db
      .query("skills")
      .withIndex('by_skill_source_project', (q: any) => q.eq('skillId', args.skillId).eq('source', args.source).eq('projectId', projectId))
      .collect();
  } catch {
    rows = await db
      .query("skills")
      .filter((q) =>
        q.and(
          q.eq(q.field('skillId'), args.skillId),
          q.and(q.eq(q.field('source'), args.source), q.eq(q.field('projectId'), projectId))
        )
      )
      .collect();
  }
  for (const row of rows) {
    try { await db.delete(row._id); } catch {}
  }
  return true;
});

