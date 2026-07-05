import {
  decodeRuntimeControlIntentRow,
  type RuntimeControlIntentRow,
} from "@openagentsinc/khala-sync"
import type { SqlTag } from "./sql.js"

export {
  decodeRuntimeControlIntentRow,
  encodeRuntimeControlIntentRow,
  RuntimeControlIntentRow,
} from "@openagentsinc/khala-sync"

/**
 * Runtime control-intent reader (#8388) — the CONSUMPTION seam for the
 * durable `khala_sync_runtime_control_intents` rows the runtime.* mutators
 * record (`packages/khala-sync-server/src/runtime-mutators.ts`, migration
 * 0029; the resumable `seq` watermark column is migration 0032).
 *
 * HONEST V1 CONTRACT: `runtime.startTurn` and friends make an operator/
 * client intent durable; nothing consumes it by itself (see
 * `docs/khala-code/2026-07-04-mobile-tailnet-handshake.md`). This reader is
 * how the Pylon-side dispatch consumer (#8388,
 * `apps/pylon/src/orchestration/runtime-intent-enforcement.ts`) OBSERVES
 * those intents: poll with the last-seen `seq` as `afterSeq`, dispatch the
 * requested behavior (start/interrupt/steer a real local Codex/Claude
 * turn), and persist the new watermark.
 *
 * Mirrors `readPendingFleetIntents` (./fleet-intents.ts) precisely: a
 * single bounded SELECT, transaction-mode safe (SPEC §4), no session state.
 */

export const DEFAULT_RUNTIME_INTENTS_LIMIT = 100
export const MAX_RUNTIME_INTENTS_LIMIT = 500

export interface ReadPendingRuntimeControlIntentsInput {
  /** Only intents with `seq > afterSeq` (the poller's watermark). Default 0. */
  readonly afterSeq?: number
  /** Restrict to one owner's intents; omit for all owners. */
  readonly ownerUserId?: string
  /** Page size; clamped to `MAX_RUNTIME_INTENTS_LIMIT`. */
  readonly limit?: number
}

interface RawRuntimeControlIntentRow {
  readonly seq: number | string | bigint
  readonly intent_id: string
  readonly thread_id: string
  readonly turn_id: string | null
  readonly owner_user_id: string
  readonly kind: string
  readonly status: string
  readonly intent_json: unknown
  readonly created_at: Date | string
  readonly updated_at: Date | string
}

const toIso = (raw: unknown): string =>
  raw instanceof Date ? raw.toISOString() : new Date(String(raw)).toISOString()

const rowToIntent = (
  row: RawRuntimeControlIntentRow,
): RuntimeControlIntentRow =>
  decodeRuntimeControlIntentRow({
    createdAt: toIso(row.created_at),
    // jsonb comes back as a string over some drivers (Bun's native SQL),
    // an already-parsed object over others (postgres.js) — same rule as
    // the changelog post-image reader in ./read-service.ts.
    intent:
      typeof row.intent_json === "string"
        ? JSON.parse(row.intent_json)
        : row.intent_json,
    intentId: row.intent_id,
    kind: row.kind,
    ownerUserId: row.owner_user_id,
    seq: Number(row.seq),
    status: row.status,
    threadId: row.thread_id,
    turnId: row.turn_id,
    updatedAt: toIso(row.updated_at),
  })

/**
 * Read runtime control intents recorded after `afterSeq`, oldest first,
 * optionally restricted to one owner. "Pending" is from the CALLER'S point
 * of view: the table has no consumed flag by design (an immutable request
 * log attributable to its mutations); each consumer tracks its own
 * watermark and treats everything past it as pending.
 */
export const readPendingRuntimeControlIntents = async (
  sql: SqlTag,
  input: ReadPendingRuntimeControlIntentsInput = {},
): Promise<ReadonlyArray<RuntimeControlIntentRow>> => {
  const afterSeq = Math.max(0, Math.floor(input.afterSeq ?? 0))
  const limit = Math.min(
    Math.max(1, Math.floor(input.limit ?? DEFAULT_RUNTIME_INTENTS_LIMIT)),
    MAX_RUNTIME_INTENTS_LIMIT,
  )
  const rows: Array<RawRuntimeControlIntentRow> =
    input.ownerUserId === undefined
      ? await sql`
          SELECT seq, intent_id, thread_id, turn_id, owner_user_id, kind,
                 status, intent_json, created_at, updated_at
          FROM khala_sync_runtime_control_intents
          WHERE seq > ${afterSeq}
          ORDER BY seq ASC
          LIMIT ${limit}
        `
      : await sql`
          SELECT seq, intent_id, thread_id, turn_id, owner_user_id, kind,
                 status, intent_json, created_at, updated_at
          FROM khala_sync_runtime_control_intents
          WHERE seq > ${afterSeq} AND owner_user_id = ${input.ownerUserId}
          ORDER BY seq ASC
          LIMIT ${limit}
        `
  return rows.map(rowToIntent)
}

// ---------------------------------------------------------------------------
// chat_message reader (#8388) — resolves a `turn.start` intent's `bodyRef`
// (the `chat_message.<messageId>` convention) into the real prompt text.
// ---------------------------------------------------------------------------

export type RuntimeChatMessageRow = {
  readonly messageId: string
  readonly threadId: string
  readonly authorUserId: string
  readonly body: string
  readonly createdAt: string
  readonly updatedAt: string
  readonly deletedAt: string | null
}

interface RawChatMessageRow {
  readonly message_id: string
  readonly thread_id: string
  readonly author_user_id: string
  readonly body: string
  readonly created_at: string
  readonly updated_at: string
  readonly deleted_at: string | null
}

/**
 * Read one `chat_message` row by id, optionally scoped to a thread (the
 * caller already knows the thread from the `turn.start` intent, so this
 * doubles as a cross-thread confusion guard). Returns `null` when the
 * message does not exist (or belongs to a different thread) — the caller
 * treats that as a real error condition (surfaced as a failed turn), never
 * a silent skip.
 */
export const readChatMessageById = async (
  sql: SqlTag,
  input: { readonly messageId: string; readonly threadId?: string },
): Promise<RuntimeChatMessageRow | null> => {
  const rows: Array<RawChatMessageRow> =
    input.threadId === undefined
      ? await sql`
          SELECT message_id, thread_id, author_user_id, body, created_at,
                 updated_at, deleted_at
          FROM khala_sync_chat_messages
          WHERE message_id = ${input.messageId}
        `
      : await sql`
          SELECT message_id, thread_id, author_user_id, body, created_at,
                 updated_at, deleted_at
          FROM khala_sync_chat_messages
          WHERE message_id = ${input.messageId} AND thread_id = ${input.threadId}
        `
  const row = rows[0]
  if (row === undefined) return null
  return {
    authorUserId: row.author_user_id,
    body: row.body,
    createdAt: row.created_at,
    deletedAt: row.deleted_at,
    messageId: row.message_id,
    threadId: row.thread_id,
    updatedAt: row.updated_at,
  }
}
