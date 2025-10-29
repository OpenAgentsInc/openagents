import { queryGeneric, mutationGeneric } from "convex/server";
import type { AvailableCommand } from '@agentclientprotocol/sdk'

export const forThread = queryGeneric(async ({ db }, args: { threadId: string }) => {
  let rows: any[] = [];
  try {
    // @ts-ignore
    rows = await db
      .query('acp_state')
      .withIndex('by_thread', (q: any) => q.eq('threadId', args.threadId))
      .collect();
  } catch {
    rows = await db
      .query('acp_state')
      .filter((q) => q.eq(q.field('threadId'), args.threadId))
      .collect();
  }
  return rows[0] ?? null;
});

export const update = mutationGeneric(async ({ db }, args: { threadId: string; currentModeId?: string | null; available_commands?: AvailableCommand[] | null; available_commands_json?: string | null; available_command_names?: string[] | null; available_command_descriptions?: string[] | null }) => {
  const now = Date.now();
  const cmds: AvailableCommand[] | undefined = (() => {
    if (Array.isArray(args.available_commands)) return args.available_commands as AvailableCommand[];
    const names = Array.isArray(args.available_command_names) ? args.available_command_names : [];
    const descs = Array.isArray(args.available_command_descriptions) ? args.available_command_descriptions : [];
    if (names.length && names.length === descs.length) {
      return names.map((name, i) => ({ name, description: descs[i] }));
    }
    try { const v = JSON.parse(String(args.available_commands_json||'')); return Array.isArray(v) ? (v as AvailableCommand[]) : undefined } catch { return undefined }
  })();
  let rows: any[] = [];
  try {
    // @ts-ignore
    rows = await db
      .query('acp_state')
      .withIndex('by_thread', (q: any) => q.eq('threadId', args.threadId))
      .collect();
  } catch {
    rows = await db
      .query('acp_state')
      .filter((q) => q.eq(q.field('threadId'), args.threadId))
      .collect();
  }
  if (rows.length > 0) {
    const doc = rows[0]!;
    await db.patch(doc._id, {
      currentModeId: typeof args.currentModeId === 'string' ? args.currentModeId : doc.currentModeId,
      available_commands: Array.isArray(cmds) ? cmds : doc.available_commands,
      updatedAt: now,
    } as any);
    return doc._id;
  }
  const id = await db.insert('acp_state', {
    threadId: args.threadId,
    currentModeId: typeof args.currentModeId === 'string' ? args.currentModeId : undefined,
    available_commands: Array.isArray(cmds) ? cmds : undefined,
    createdAt: now,
    updatedAt: now,
  } as any);
  return id;
});
