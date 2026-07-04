/**
 * KS-8.1 (#8307): pylon assignments/dispatch backfill core — D1 → Postgres.
 *
 * Testable core behind `scripts/backfill-pylon.ts`. Takes raw D1 rows
 * (snake_case objects, exactly as `wrangler d1 execute --json` returns
 * them) and upserts them into the Postgres twins from migration
 * `0005_pylon_dispatch.sql` with `ON CONFLICT ... DO NOTHING` — so the
 * backfill NEVER fights the live dual-write mirror (MIGRATION_PLAN §1.2:
 * rows the mirror already converged are fresher than any snapshot page;
 * rows the mirror never touched are filled here). Running a batch twice is
 * a no-op by construction (idempotency test: `pylon-backfill.test.ts`).
 *
 * Verification (`verify*`): the 2026-06-29 after-action reconciliation
 * culture — exact row counts, per-status/state tallies, and newest-N
 * row-hash comparison over a canonical column serialization. Nothing
 * "close enough": exact or explain.
 */

import { createHash } from "node:crypto"
import type { SyncSql } from "./sql.js"

// ---------------------------------------------------------------------------
// Table shapes (column lists mirror migration 0005 exactly)
// ---------------------------------------------------------------------------

export type PylonBackfillTable =
  | "pylon_registrations"
  | "pylon_assignments"
  | "pylon_assignment_events"

/** D1 source table for each Postgres target. */
export const D1_SOURCE_TABLES: Readonly<Record<PylonBackfillTable, string>> = {
  pylon_assignment_events: "pylon_api_events",
  pylon_assignments: "pylon_api_assignments",
  pylon_registrations: "pylon_api_registrations",
}

const REGISTRATION_COLUMNS = [
  "id",
  "pylon_ref",
  "owner_agent_user_id",
  "owner_agent_credential_id",
  "owner_agent_token_prefix",
  "display_name",
  "status",
  "resource_mode",
  "capability_refs_json",
  "client_version",
  "client_protocol_version",
  "wallet_ref",
  "wallet_ready",
  "latest_heartbeat_at",
  "latest_heartbeat_status",
  "latest_resource_mode",
  "latest_health_refs_json",
  "latest_load_refs_json",
  "latest_capacity_refs_json",
  "provider_nostr_pubkey",
  "provider_nostr_npub",
  "provider_market_relay_refs_json",
  "provider_nip90_lane_refs_json",
  "public_projection_json",
  "created_at",
  "updated_at",
  "archived_at",
] as const

const ASSIGNMENT_COLUMNS = [
  "id",
  "assignment_ref",
  "pylon_ref",
  "owner_agent_user_id",
  "idempotency_key_hash",
  "job_kind",
  "state",
  "payment_mode",
  "lease_expires_at",
  "task_refs_json",
  "acceptance_criteria_refs_json",
  "result_expectation_refs_json",
  "artifact_refs_json",
  "proof_refs_json",
  "accepted_work_refs_json",
  "rejection_refs_json",
  "closeout_refs_json",
  "coding_assignment_json",
  "public_projection_json",
  "created_at",
  "updated_at",
  "archived_at",
] as const

const EVENT_COLUMNS = [
  "id",
  "event_ref",
  "pylon_ref",
  "owner_agent_user_id",
  "idempotency_key_hash",
  "event_kind",
  "assignment_ref",
  "status",
  "event_body_json",
  "public_projection_json",
  "created_at",
  "archived_at",
] as const

export const TABLE_COLUMNS: Readonly<
  Record<PylonBackfillTable, ReadonlyArray<string>>
> = {
  pylon_assignment_events: EVENT_COLUMNS,
  pylon_assignments: ASSIGNMENT_COLUMNS,
  pylon_registrations: REGISTRATION_COLUMNS,
}

/** Conflict target for the DO NOTHING upsert (the table's natural key). */
export const TABLE_CONFLICT_KEY: Readonly<Record<PylonBackfillTable, string>> =
  {
    pylon_assignment_events: "event_ref",
    pylon_assignments: "assignment_ref",
    pylon_registrations: "pylon_ref",
  }

/** Column the per-status tally groups by during verification. */
export const TABLE_STATUS_COLUMN: Readonly<
  Record<PylonBackfillTable, string>
> = {
  pylon_assignment_events: "event_kind",
  pylon_assignments: "state",
  pylon_registrations: "status",
}

export type D1SourceRow = Readonly<Record<string, unknown>>

const normalizeValue = (
  table: PylonBackfillTable,
  column: string,
  value: unknown,
): string | number | null => {
  if (value === undefined || value === null) {
    // D1's payment_mode gained NOT NULL DEFAULT in migration 0256; export
    // tooling can still surface NULL on rows written before the ALTER.
    return table === "pylon_assignments" && column === "payment_mode"
      ? "unpaid_smoke"
      : null
  }
  if (typeof value === "number") return value
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

/**
 * Upsert one page of D1 rows into `table`. `ON CONFLICT (natural key) DO
 * NOTHING`: rows the dual-write mirror already owns win. Returns how many
 * rows were actually inserted (0 on a re-run — the idempotency contract).
 */
export const upsertPylonRows = async (
  sql: SyncSql,
  table: PylonBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireUnsafe(sql)
  const columns = TABLE_COLUMNS[table]
  const conflictKey = TABLE_CONFLICT_KEY[table]
  let inserted = 0
  for (const row of rows) {
    const values = columns.map((column) =>
      normalizeValue(table, column, row[column]),
    )
    // One INSERT per row keeps this driver-portable (Bun SQL + postgres.js)
    // without helper-object interpolation; backfill pages are bounded, and
    // this path never runs on the Worker request path.
    const columnsSql = columns.join(", ")
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${columnsSql}) VALUES (${placeholders}) ON CONFLICT (${conflictKey}) DO NOTHING RETURNING ${conflictKey}`,
      values as Array<unknown>,
    )
    inserted += result.length
  }
  return inserted
}

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)` for
 * dynamic-text parameterized statements; the structural `SyncSql` seam
 * deliberately does not, so this module widens it locally. The backfill
 * runs over DIRECT connections (never Hyperdrive), so this stays outside
 * the Worker's transaction-mode constraints.
 */
type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "pylon backfill requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

export type PylonVerifyTally = Readonly<{
  total: number
  byStatus: Readonly<Record<string, number>>
}>

/** Count + per-status tally over the Postgres side of one table. */
export const postgresPylonTally = async (
  sql: SyncSql,
  table: PylonBackfillTable,
): Promise<PylonVerifyTally> => {
  const statusColumn = TABLE_STATUS_COLUMN[table]
  const rows = (await requireUnsafe(sql)(
    `SELECT ${statusColumn} AS status_value, count(*) AS row_count FROM ${table} GROUP BY ${statusColumn} ORDER BY ${statusColumn}`,
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
 * separators, sha256'd. Column normalization matches `upsertPylonRows`, so
 * the SAME D1 export row and its Postgres twin hash identically.
 */
export const pylonRowHash = (
  table: PylonBackfillTable,
  row: D1SourceRow,
): string => {
  const columns = TABLE_COLUMNS[table]
  const hash = createHash("sha256")
  for (const column of columns) {
    const value = normalizeValue(table, column, row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ key: string; hash: string }>

/**
 * Newest-N row hashes on the Postgres side, keyed by the table's natural
 * key, newest-first by created_at (events) / updated_at (others).
 */
export const postgresNewestRowHashes = async (
  sql: SyncSql,
  table: PylonBackfillTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const keyColumn = TABLE_CONFLICT_KEY[table]
  const orderColumn =
    table === "pylon_assignment_events" ? "created_at" : "updated_at"
  const rows = await requireUnsafe(sql)(
    `SELECT * FROM ${table} ORDER BY ${orderColumn} DESC, ${keyColumn} DESC LIMIT $1`,
    [limit],
  )
  return rows.map((row) => ({
    hash: pylonRowHash(table, row),
    key: String(row[keyColumn]),
  }))
}

/** Same newest-N hashing over D1 export rows (already fetched by the CLI). */
export const d1NewestRowHashes = (
  table: PylonBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> => {
  const keyColumn = TABLE_CONFLICT_KEY[table]
  return rows.map((row) => ({
    hash: pylonRowHash(table, row),
    key: String(row[keyColumn]),
  }))
}

export type PylonVerifyTableReport = Readonly<{
  table: PylonBackfillTable
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

export const comparePylonTallies = (
  table: PylonBackfillTable,
  d1: PylonVerifyTally,
  postgres: PylonVerifyTally,
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): PylonVerifyTableReport => {
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
