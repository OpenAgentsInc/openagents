/**
 * KS-8.6 (#8317): Artanis supervision domain backfill core — D1 → Postgres.
 *
 * Testable core behind `scripts/backfill-artanis.ts`, following the
 * KS-8.1 pylon lane (`pylon-backfill.ts`). Takes raw D1 rows (snake_case
 * objects, exactly as `wrangler d1 execute --json` returns them) and
 * upserts them into the Postgres twins from migration
 * `0011_artanis_domain.sql` with `ON CONFLICT ... DO NOTHING` — so the
 * backfill NEVER fights the live dual-write mirror (MIGRATION_PLAN §1.2:
 * rows the mirror already converged are fresher than any snapshot page;
 * rows the mirror never touched are filled here). Running a batch twice is
 * a no-op by construction (idempotency test: `artanis-backfill.test.ts`).
 *
 * Verification (`verify*`): exact row counts, per-status/state tallies,
 * and newest-N row-hash comparison over a canonical column serialization.
 * Nothing "close enough": exact or explain.
 *
 * Unlike the pylon lane, D1 and Postgres table names are IDENTICAL for
 * this domain (all twenty `artanis_*` tables keep their names).
 */

import { createHash } from "node:crypto"
import type { SyncSql } from "./sql.js"

// ---------------------------------------------------------------------------
// Table registry (column lists mirror migration 0010 exactly, which mirrors
// the live D1 schema: worker migrations 0119/0120/0161/0163/0164/0165/0169/
// 0213/0215/0245/0248/0249/0256)
// ---------------------------------------------------------------------------

export type ArtanisBackfillTable =
  | "artanis_runtime_snapshots"
  | "artanis_loop_records"
  | "artanis_loop_ticks"
  | "artanis_approval_gates"
  | "artanis_health_snapshots"
  | "artanis_work_routing_proposals"
  | "artanis_forum_publication_intents"
  | "artanis_nexus_pylon_adapter_dispatches"
  | "artanis_responder_state"
  | "artanis_responder_actions"
  | "artanis_responder_ticks"
  | "artanis_admin_tick_decisions"
  | "artanis_closeout_verdicts"
  | "artanis_fleet_overseer_decisions"
  | "artanis_standing_spend_grants"
  | "artanis_spend_decisions"
  | "artanis_labor_unattended_receipts"
  | "artanis_owner_memory"
  | "artanis_threads"
  | "artanis_messages"

/** The eight uniform persistence-ledger tables share one column set. */
const LEDGER_COLUMNS = [
  "id",
  "agent_id",
  "record_ref",
  "idempotency_key",
  "state",
  "active",
  "source_kind",
  "scope_ref",
  "parent_ref",
  "record_json",
  "public_projection_json",
  "content_hash",
  "closeout_json",
  "created_at",
  "updated_at",
  "closed_at",
] as const

export type ArtanisTableSpec = Readonly<{
  /** Column list in canonical (migration) order. */
  columns: ReadonlyArray<string>
  /** Conflict target for the DO NOTHING upsert (the table's natural key). */
  conflictKey: string
  /** Column newest-N verification orders by (text ISO timestamps sort). */
  orderColumn: string
  /** Column the per-status tally groups by during verification. */
  statusColumn: string
}>

const ledgerSpec: ArtanisTableSpec = {
  columns: LEDGER_COLUMNS,
  conflictKey: "record_ref",
  orderColumn: "updated_at",
  statusColumn: "state",
}

export const ARTANIS_TABLE_SPECS: Readonly<
  Record<ArtanisBackfillTable, ArtanisTableSpec>
> = {
  artanis_admin_tick_decisions: {
    columns: ["id", "state", "action_json", "assignment_ref", "created_at"],
    conflictKey: "id",
    orderColumn: "created_at",
    statusColumn: "state",
  },
  artanis_approval_gates: ledgerSpec,
  artanis_closeout_verdicts: {
    columns: [
      "id",
      "assignment_ref",
      "outcome",
      "claimed_trace_digest_prefix",
      "accept_state",
      "detail",
      "created_at",
    ],
    conflictKey: "assignment_ref",
    orderColumn: "created_at",
    statusColumn: "outcome",
  },
  artanis_fleet_overseer_decisions: {
    columns: [
      "id",
      "state",
      "action_json",
      "context_json",
      "approval_gate_ref",
      "health_snapshot_ref",
      "created_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    statusColumn: "state",
  },
  artanis_forum_publication_intents: ledgerSpec,
  artanis_health_snapshots: ledgerSpec,
  artanis_labor_unattended_receipts: {
    columns: ["receipt_ref", "serialized_json", "terminal_state", "created_at"],
    conflictKey: "receipt_ref",
    orderColumn: "created_at",
    statusColumn: "terminal_state",
  },
  artanis_loop_records: ledgerSpec,
  artanis_loop_ticks: ledgerSpec,
  artanis_messages: {
    columns: [
      "message_ref",
      "thread_ref",
      "caller_id",
      "author_id",
      "author_kind",
      "body",
      "metadata_json",
      "created_at",
    ],
    conflictKey: "message_ref",
    orderColumn: "created_at",
    statusColumn: "author_kind",
  },
  artanis_nexus_pylon_adapter_dispatches: ledgerSpec,
  artanis_owner_memory: {
    columns: [
      "memory_ref",
      "owner_id",
      "kind",
      "role",
      "note_category",
      "body",
      "created_at",
    ],
    conflictKey: "memory_ref",
    orderColumn: "created_at",
    statusColumn: "kind",
  },
  artanis_responder_actions: {
    columns: [
      "id",
      "topic_id",
      "first_post_id",
      "question_class",
      "state",
      "proposal_json",
      "reply_post_id",
      "asked_at",
      "replied_at",
      "created_at",
      "updated_at",
      "tip_receipt_ref",
      "tip_pay_in_id",
      "tip_ladder_rung",
      "tip_ladder_reason",
      "asker_actor_ref",
      "asker_provenance",
    ],
    conflictKey: "topic_id",
    orderColumn: "updated_at",
    statusColumn: "state",
  },
  artanis_responder_state: {
    columns: [
      "id",
      "scan_cursor_iso",
      "responses_today",
      "responses_day",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "responses_day",
  },
  artanis_responder_ticks: {
    columns: [
      "tick_ref",
      "scheduled_at",
      "scan_state",
      "scan_scanned",
      "scan_proposed",
      "scan_blocked",
      "scan_skipped",
      "scan_skipped_reason",
      "compose_state",
      "compose_considered",
      "compose_responded",
      "compose_blocked",
      "compose_tipped",
      "compose_skipped_reason",
      "created_at",
      "updated_at",
    ],
    conflictKey: "scheduled_at",
    orderColumn: "scheduled_at",
    statusColumn: "scan_state",
  },
  artanis_runtime_snapshots: ledgerSpec,
  artanis_spend_decisions: {
    columns: [
      "id",
      "grant_ref",
      "state",
      "intended_amount_sat",
      "paid_amount_sat",
      "destination_source_ref",
      "recipient_ref",
      "rationale",
      "payment_ref",
      "policy_applied",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "state",
  },
  artanis_standing_spend_grants: {
    columns: [
      "grant_ref",
      "per_payout_cap_sat",
      "per_day_cap_sat",
      "authority_ref",
      "active",
      "created_at",
      "revoked_at",
    ],
    conflictKey: "grant_ref",
    orderColumn: "created_at",
    statusColumn: "active",
  },
  artanis_threads: {
    columns: [
      "thread_ref",
      "caller_id",
      "caller_kind",
      "subject_agent_ref",
      "subject_agent_kind",
      "title",
      "status",
      "source_ref",
      "metadata_json",
      "last_message_at",
      "created_at",
      "updated_at",
    ],
    conflictKey: "thread_ref",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  artanis_work_routing_proposals: ledgerSpec,
}

export const ARTANIS_BACKFILL_TABLES = Object.keys(
  ARTANIS_TABLE_SPECS,
) as ReadonlyArray<ArtanisBackfillTable>

export type D1SourceRow = Readonly<Record<string, unknown>>

const normalizeValue = (value: unknown): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)` for
 * dynamic-text parameterized statements; the structural `SyncSql` seam
 * deliberately does not, so this module widens it locally. The backfill
 * runs over DIRECT connections (never Hyperdrive).
 */
type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "artanis backfill requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/**
 * Upsert one page of D1 rows into `table`. `ON CONFLICT (natural key) DO
 * NOTHING`: rows the dual-write mirror already owns win. Returns how many
 * rows were actually inserted (0 on a re-run — the idempotency contract).
 */
export const upsertArtanisRows = async (
  sql: SyncSql,
  table: ArtanisBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireUnsafe(sql)
  const spec = ARTANIS_TABLE_SPECS[table]
  let inserted = 0
  for (const row of rows) {
    const values = spec.columns.map((column) => normalizeValue(row[column]))
    const columnsSql = spec.columns.join(", ")
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${columnsSql}) VALUES (${placeholders}) ON CONFLICT (${spec.conflictKey}) DO NOTHING RETURNING ${spec.conflictKey}`,
      values as Array<unknown>,
    )
    inserted += result.length
  }
  return inserted
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

export type ArtanisVerifyTally = Readonly<{
  total: number
  byStatus: Readonly<Record<string, number>>
}>

/** Count + per-status tally over the Postgres side of one table. */
export const postgresArtanisTally = async (
  sql: SyncSql,
  table: ArtanisBackfillTable,
): Promise<ArtanisVerifyTally> => {
  const statusColumn = ARTANIS_TABLE_SPECS[table].statusColumn
  const rows = (await requireUnsafe(sql)(
    `SELECT ${statusColumn}::text AS status_value, count(*) AS row_count FROM ${table} GROUP BY ${statusColumn} ORDER BY ${statusColumn}`,
    [],
  )) as Array<{ status_value: string | null; row_count: unknown }>
  const byStatus: Record<string, number> = {}
  let total = 0
  for (const row of rows) {
    const count = Number(row.row_count)
    byStatus[row.status_value ?? "<null>"] = count
    total += count
  }
  return { byStatus, total }
}

/**
 * Canonical row hash: the migration-order column values joined with unit
 * separators, sha256'd. Column normalization matches `upsertArtanisRows`,
 * so the SAME D1 export row and its Postgres twin hash identically.
 */
export const artanisRowHash = (
  table: ArtanisBackfillTable,
  row: D1SourceRow,
): string => {
  const hash = createHash("sha256")
  for (const column of ARTANIS_TABLE_SPECS[table].columns) {
    const value = normalizeValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ key: string; hash: string }>

/**
 * Newest-N row hashes on the Postgres side, keyed by the table's natural
 * key, newest-first by the table's order column.
 */
export const postgresArtanisNewestRowHashes = async (
  sql: SyncSql,
  table: ArtanisBackfillTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const spec = ARTANIS_TABLE_SPECS[table]
  const rows = await requireUnsafe(sql)(
    `SELECT * FROM ${table} ORDER BY ${spec.orderColumn} DESC, ${spec.conflictKey} DESC LIMIT $1`,
    [limit],
  )
  return rows.map((row) => ({
    hash: artanisRowHash(table, row),
    key: String(row[spec.conflictKey]),
  }))
}

/** Same newest-N hashing over D1 export rows (already fetched by the CLI). */
export const d1ArtanisNewestRowHashes = (
  table: ArtanisBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> => {
  const keyColumn = ARTANIS_TABLE_SPECS[table].conflictKey
  return rows.map((row) => ({
    hash: artanisRowHash(table, row),
    key: String(row[keyColumn]),
  }))
}

export type ArtanisVerifyTableReport = Readonly<{
  table: ArtanisBackfillTable
  d1Total: number
  postgresTotal: number
  countsMatch: boolean
  statusMismatches: ReadonlyArray<{
    status: string
    d1: number
    postgres: number
  }>
  newestHashMismatches: ReadonlyArray<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }>
}>

export const compareArtanisTallies = (
  table: ArtanisBackfillTable,
  d1: ArtanisVerifyTally,
  postgres: ArtanisVerifyTally,
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): ArtanisVerifyTableReport => {
  const statuses = new Set([
    ...Object.keys(d1.byStatus),
    ...Object.keys(postgres.byStatus),
  ])
  const statusMismatches: Array<{
    status: string
    d1: number
    postgres: number
  }> = []
  for (const status of [...statuses].sort()) {
    const d1Count = d1.byStatus[status] ?? 0
    const postgresCount = postgres.byStatus[status] ?? 0
    if (d1Count !== postgresCount) {
      statusMismatches.push({ d1: d1Count, postgres: postgresCount, status })
    }
  }

  const postgresByKey = new Map(
    postgresNewest.map((entry) => [entry.key, entry.hash]),
  )
  const newestHashMismatches: Array<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }> = []
  for (const entry of d1Newest) {
    const postgresHash = postgresByKey.get(entry.key)
    if (postgresHash !== entry.hash) {
      newestHashMismatches.push({
        d1Hash: entry.hash,
        key: entry.key,
        postgresHash,
      })
    }
  }

  return {
    countsMatch: d1.total === postgres.total,
    d1Total: d1.total,
    newestHashMismatches,
    postgresTotal: postgres.total,
    statusMismatches,
    table,
  }
}
