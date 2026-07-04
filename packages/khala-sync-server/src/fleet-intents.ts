import {
  decodeFleetIntentRow,
  type FleetIntentRow,
} from "@openagentsinc/khala-sync"
import type { SqlTag } from "./sql.js"

export {
  decodeFleetIntentRow,
  encodeFleetIntentRow,
  FleetIntentKind,
  FleetIntentRow,
} from "@openagentsinc/khala-sync"

/**
 * Fleet intent reader (KS-3.2, #8292) — the CONSUMPTION seam for the
 * durable operator intents the fleet mutators record in
 * `khala_sync_fleet_intents` (migrations 0004/0005).
 *
 * HONEST V1 CONTRACT: the fleet mutators make an operator intent durable
 * and project the desired post-image; they do not change dispatch behavior
 * by themselves. This reader is how an enforcement loop (the Pylon
 * supervisor, via the admin-guarded Worker route
 * `GET /api/internal/khala-sync/fleet-intents`) OBSERVES those intents:
 * poll with the last-seen intent `id` as `afterId`, apply the requested
 * behavior locally, and persist the new watermark. Wiring the supervisor
 * loop itself is the follow-up enforcement lane tracked on epic #8282.
 *
 * The reader is a single bounded SELECT — transaction-mode safe (SPEC §4),
 * no session state, usable over Hyperdrive and over a direct connection.
 */

export const DEFAULT_FLEET_INTENTS_LIMIT = 100
export const MAX_FLEET_INTENTS_LIMIT = 500

export interface ReadPendingFleetIntentsInput {
  /** Only intents with `id > afterId` (the poller's watermark). Default 0. */
  readonly afterId?: number
  /** Restrict to one fleet scope (`scope.fleet_run.<runId>`); omit for all. */
  readonly scope?: string
  /** Page size; clamped to `MAX_FLEET_INTENTS_LIMIT`. */
  readonly limit?: number
}

const toIso = (raw: unknown): string =>
  raw instanceof Date ? raw.toISOString() : new Date(String(raw)).toISOString()

interface RawIntentRow {
  readonly id: number | string | bigint
  readonly scope: string
  readonly run_id: string
  readonly intent: string
  readonly desired_slots: number | string | null
  readonly worker_id: string | null
  readonly flag_ref: string | null
  readonly requested_by_user_id: string
  readonly mutation_ref: string
  readonly created_at: Date | string
}

const rowToIntent = (row: RawIntentRow): FleetIntentRow =>
  decodeFleetIntentRow({
    createdAt: toIso(row.created_at),
    desiredSlots: row.desired_slots === null ? null : Number(row.desired_slots),
    flagRef: row.flag_ref,
    id: Number(row.id),
    intent: row.intent,
    mutationRef: row.mutation_ref,
    requestedByUserId: row.requested_by_user_id,
    runId: row.run_id,
    scope: row.scope,
    workerId: row.worker_id,
  })

/**
 * Read operator intents recorded after `afterId`, oldest first, optionally
 * restricted to one fleet scope. "Pending" is from the CALLER'S point of
 * view: the table has no consumed flag by design (intents are an immutable
 * request log attributable to their mutations); each consumer tracks its
 * own watermark and treats everything past it as pending.
 */
export const readPendingFleetIntents = async (
  sql: SqlTag,
  input: ReadPendingFleetIntentsInput = {},
): Promise<ReadonlyArray<FleetIntentRow>> => {
  const afterId = Math.max(0, Math.floor(input.afterId ?? 0))
  const limit = Math.min(
    Math.max(1, Math.floor(input.limit ?? DEFAULT_FLEET_INTENTS_LIMIT)),
    MAX_FLEET_INTENTS_LIMIT,
  )
  const rows: Array<RawIntentRow> =
    input.scope === undefined
      ? await sql`
          SELECT id, scope, run_id, intent, desired_slots, worker_id,
                 flag_ref, requested_by_user_id, mutation_ref, created_at
          FROM khala_sync_fleet_intents
          WHERE id > ${afterId}
          ORDER BY id ASC
          LIMIT ${limit}
        `
      : await sql`
          SELECT id, scope, run_id, intent, desired_slots, worker_id,
                 flag_ref, requested_by_user_id, mutation_ref, created_at
          FROM khala_sync_fleet_intents
          WHERE id > ${afterId} AND scope = ${input.scope}
          ORDER BY id ASC
          LIMIT ${limit}
        `
  return rows.map(rowToIntent)
}
