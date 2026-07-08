import {
  decodeKhalaFleetIntent,
  type KhalaFleetIntent,
  type KhalaFleetIntentKind,
} from "@openagentsinc/khala-fleet-intents"
import type { SqlTag } from "./sql.js"

/**
 * Fleet steering-intent reader (MH-6 #8585) — the CONSUMPTION seam for the
 * durable typed intents the fleet steering mutators record in
 * `khala_sync_fleet_steering_intents` (migration 0050).
 *
 * This is how the desktop/daemon FleetRun authority OBSERVES mobile-dispatched
 * steering: poll with the last-seen `seq` as `afterSeq`, apply the requested
 * behavior locally (pause the run, resume the worker whose approval was
 * allowed, deliver the steer body to the in-flight turn), and persist the new
 * watermark. Authority stays desktop/daemon-side; the mutators only make the
 * intent durable and project the desired post-image.
 *
 * Mirrors `readPendingRuntimeControlIntents` (./runtime-intents.ts) and
 * `readPendingFleetIntents` (./fleet-intents.ts): a single bounded SELECT,
 * transaction-mode safe (SPEC §4), no session state, resumable by `seq`.
 */

export const DEFAULT_FLEET_STEERING_INTENTS_LIMIT = 100
export const MAX_FLEET_STEERING_INTENTS_LIMIT = 500

export interface FleetSteeringIntentRow {
  /** Monotonic watermark — the poller's resume cursor. */
  readonly seq: number
  readonly intentId: string
  readonly scope: string
  readonly runRef: string
  readonly kind: KhalaFleetIntentKind
  /** Denormalized run-control action (pause/resume/drain/stop), else null. */
  readonly action: string | null
  /** Denormalized approval ref for approval_decision, else null. */
  readonly approvalRef: string | null
  /** Denormalized decision (allow/deny) for approval_decision, else null. */
  readonly decision: string | null
  readonly surface: string
  readonly requestedByUserId: string
  readonly idempotencyKey: string
  /** The full typed intent value (authoritative). */
  readonly intent: KhalaFleetIntent
  readonly mutationRef: string
  readonly createdAt: string
}

export interface ReadPendingFleetSteeringIntentsInput {
  /** Only intents with `seq > afterSeq` (the poller's watermark). Default 0. */
  readonly afterSeq?: number
  /** Restrict to one fleet scope (`scope.fleet_run.<runRef>`); omit for all. */
  readonly scope?: string
  /** Page size; clamped to `MAX_FLEET_STEERING_INTENTS_LIMIT`. */
  readonly limit?: number
}

interface RawFleetSteeringIntentRow {
  readonly seq: number | string | bigint
  readonly intent_id: string
  readonly scope: string
  readonly run_ref: string
  readonly kind: string
  readonly action: string | null
  readonly approval_ref: string | null
  readonly decision: string | null
  readonly surface: string
  readonly requested_by_user_id: string
  readonly idempotency_key: string
  readonly intent_json: unknown
  readonly mutation_ref: string
  readonly created_at: string
}

const rowToIntent = (
  row: RawFleetSteeringIntentRow,
): FleetSteeringIntentRow => ({
  action: row.action,
  approvalRef: row.approval_ref,
  createdAt: row.created_at,
  decision: row.decision,
  idempotencyKey: row.idempotency_key,
  // jsonb comes back as a string over some drivers (Bun's native SQL), an
  // already-parsed object over others (postgres.js) — same rule as the
  // runtime control-intent reader in ./runtime-intents.ts.
  intent: decodeKhalaFleetIntent(
    typeof row.intent_json === "string"
      ? (JSON.parse(row.intent_json) as unknown)
      : row.intent_json,
  ),
  intentId: row.intent_id,
  kind: row.kind as KhalaFleetIntentKind,
  mutationRef: row.mutation_ref,
  requestedByUserId: row.requested_by_user_id,
  runRef: row.run_ref,
  scope: row.scope,
  seq: Number(row.seq),
  surface: row.surface,
})

/**
 * Read fleet steering intents recorded after `afterSeq`, oldest first,
 * optionally restricted to one fleet scope. "Pending" is from the CALLER'S
 * point of view: the table has no consumed flag by design (an immutable
 * request log attributable to its mutations); each consumer tracks its own
 * watermark and treats everything past it as pending.
 */
export const readPendingFleetSteeringIntents = async (
  sql: SqlTag,
  input: ReadPendingFleetSteeringIntentsInput = {},
): Promise<ReadonlyArray<FleetSteeringIntentRow>> => {
  const afterSeq = Math.max(0, Math.floor(input.afterSeq ?? 0))
  const limit = Math.min(
    Math.max(1, Math.floor(input.limit ?? DEFAULT_FLEET_STEERING_INTENTS_LIMIT)),
    MAX_FLEET_STEERING_INTENTS_LIMIT,
  )
  const rows: Array<RawFleetSteeringIntentRow> =
    input.scope === undefined
      ? await sql`
          SELECT seq, intent_id, scope, run_ref, kind, action, approval_ref,
                 decision, surface, requested_by_user_id, idempotency_key,
                 intent_json, mutation_ref, created_at
          FROM khala_sync_fleet_steering_intents
          WHERE seq > ${afterSeq}
          ORDER BY seq ASC
          LIMIT ${limit}
        `
      : await sql`
          SELECT seq, intent_id, scope, run_ref, kind, action, approval_ref,
                 decision, surface, requested_by_user_id, idempotency_key,
                 intent_json, mutation_ref, created_at
          FROM khala_sync_fleet_steering_intents
          WHERE seq > ${afterSeq} AND scope = ${input.scope}
          ORDER BY seq ASC
          LIMIT ${limit}
        `
  return rows.map(rowToIntent)
}
