/**
 * KS-8.4 (#8315): Pylon control-plane remainder backfill core.
 *
 * This is the testable core behind `scripts/backfill-pylon-control-plane.ts`.
 * It copies raw D1 rows into the Postgres twins from migration
 * `0009_pylon_control_plane_remainder.sql` using the same migration culture as
 * KS-8.1/KS-8.2: bounded pages, natural-key `ON CONFLICT DO NOTHING`, exact row
 * counts, per-domain tallies, and newest-N row hashes. It is intentionally a
 * direct-connection tool and is never part of the Worker request path.
 */

import { createHash } from "node:crypto"
import type { SyncSql } from "./sql.js"

export type PylonControlPlaneBackfillTable =
  | "fleet_alerts"
  | "pylon_agent_runner_status_events"
  | "pylon_capacity_funnel_snapshots"
  | "pylon_codex_raw_event_chunks"
  | "pylon_codex_raw_events"
  | "pylon_marketplace_assignments"
  | "pylon_marketplace_job_intakes"
  | "pylon_marketplace_triage_actions"
  | "pylon_provider_job_lifecycle"
  | "pylon_quarantines"
  | "pylon_spark_payout_targets"
  | "runner_sessions"

export const PYLON_CONTROL_PLANE_TABLES = [
  "pylon_quarantines",
  "pylon_marketplace_job_intakes",
  "pylon_marketplace_assignments",
  "pylon_marketplace_triage_actions",
  "pylon_provider_job_lifecycle",
  "pylon_agent_runner_status_events",
  "pylon_capacity_funnel_snapshots",
  "pylon_spark_payout_targets",
  "pylon_codex_raw_events",
  "pylon_codex_raw_event_chunks",
  "runner_sessions",
  "fleet_alerts",
] as const satisfies ReadonlyArray<PylonControlPlaneBackfillTable>

export type D1SourceRow = Readonly<Record<string, unknown>>

export const D1_SOURCE_TABLES: Readonly<
  Record<PylonControlPlaneBackfillTable, string>
> = {
  fleet_alerts: "fleet_alerts",
  pylon_agent_runner_status_events: "pylon_agent_runner_status_events",
  pylon_capacity_funnel_snapshots: "pylon_capacity_funnel_snapshots",
  pylon_codex_raw_event_chunks: "pylon_codex_raw_event_chunks",
  pylon_codex_raw_events: "pylon_codex_raw_events",
  pylon_marketplace_assignments: "pylon_marketplace_assignments",
  pylon_marketplace_job_intakes: "pylon_marketplace_job_intakes",
  pylon_marketplace_triage_actions: "pylon_marketplace_triage_actions",
  pylon_provider_job_lifecycle: "pylon_provider_job_lifecycle",
  pylon_quarantines: "pylon_api_quarantines",
  pylon_spark_payout_targets: "pylon_spark_payout_targets",
  runner_sessions: "runner_sessions",
}

const TABLE_COLUMNS = {
  fleet_alerts: [
    "id",
    "alert_ref",
    "detected_at",
    "classification",
    "reason_ref",
    "burn_tokens_window",
    "window_minutes",
    "stall_threshold_tokens",
    "active_assignments",
    "queued_assignments",
    "recovery_actions_json",
    "recovered_lease_count",
    "created_at",
  ],
  pylon_agent_runner_status_events: [
    "event_ref",
    "owner_agent_user_id",
    "runner_ref",
    "runner_kind",
    "pylon_ref",
    "assignment_ref",
    "state",
    "state_started_at",
    "updated_at",
    "retention_state",
    "event_json",
    "created_at",
    "retained_at",
    "archived_at",
  ],
  pylon_capacity_funnel_snapshots: [
    "id",
    "bucket_kind",
    "bucket_start_at",
    "snapshot_at",
    "total_count",
    "aggregate_json",
    "public_projection_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  pylon_codex_raw_event_chunks: [
    "chunk_ref",
    "assignment_ref",
    "lease_ref",
    "pylon_ref",
    "owner_user_id",
    "run_ref",
    "session_ref",
    "workspace_ref",
    "turn_index",
    "chunk_index",
    "event_count",
    "byte_length",
    "content_digest",
    "r2_key",
    "observed_at",
    "created_at",
    "updated_at",
    "demand_kind",
    "demand_source",
  ],
  pylon_codex_raw_events: [
    "raw_event_ref",
    "assignment_ref",
    "lease_ref",
    "pylon_ref",
    "owner_user_id",
    "run_ref",
    "session_ref",
    "workspace_ref",
    "turn_index",
    "event_count",
    "byte_length",
    "content_digest",
    "r2_key",
    "observed_at",
    "created_at",
    "updated_at",
    "demand_kind",
    "demand_source",
  ],
  pylon_marketplace_assignments: [
    "id",
    "assignment_ref",
    "intake_ref",
    "job_ref",
    "idempotency_key",
    "request_hash",
    "state",
    "payout_state",
    "record_json",
    "created_at",
    "updated_at",
  ],
  pylon_marketplace_job_intakes: [
    "id",
    "intake_ref",
    "job_ref",
    "idempotency_key",
    "request_hash",
    "state",
    "source",
    "job_kind",
    "privacy_class",
    "record_json",
    "created_at",
    "updated_at",
  ],
  pylon_marketplace_triage_actions: [
    "id",
    "target_intake_ref",
    "idempotency_key",
    "request_hash",
    "outcome",
    "response_json",
    "created_at",
  ],
  pylon_provider_job_lifecycle: [
    "id",
    "pylon_ref",
    "assignment_ref",
    "owner_agent_user_id",
    "job_kind",
    "stage",
    "task_refs_json",
    "artifact_refs_json",
    "proof_refs_json",
    "closeout_refs_json",
    "accepted_work_refs_json",
    "public_projection_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  pylon_quarantines: [
    "id",
    "quarantine_ref",
    "pylon_ref",
    "owner_agent_user_id",
    "state",
    "reason_refs_json",
    "action_refs_json",
    "source_refs_json",
    "expires_at",
    "released_at",
    "public_projection_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  pylon_spark_payout_targets: [
    "pylon_ref",
    "owner_agent_user_id",
    "payout_target_ref",
    "raw_spark_address",
    "created_at",
    "updated_at",
  ],
  runner_sessions: [
    "id",
    "runner_id",
    "lane",
    "backend",
    "status",
    "team_id",
    "thread_id",
    "workroom_id",
    "provider_account_ref",
    "active_auth_grant_ref",
    "opencode_server_url",
    "opencode_server_auth_ref",
    "created_at",
    "updated_at",
    "started_at",
    "completed_at",
    "failed_at",
  ],
} as const satisfies Readonly<
  Record<PylonControlPlaneBackfillTable, ReadonlyArray<string>>
>

export const TABLE_COLUMNS_FOR_TEST = TABLE_COLUMNS

export const TABLE_CONFLICT_KEY: Readonly<
  Record<PylonControlPlaneBackfillTable, string>
> = {
  fleet_alerts: "alert_ref",
  pylon_agent_runner_status_events: "event_ref",
  pylon_capacity_funnel_snapshots: "bucket_kind, bucket_start_at",
  pylon_codex_raw_event_chunks: "chunk_ref",
  pylon_codex_raw_events: "raw_event_ref",
  pylon_marketplace_assignments: "assignment_ref",
  pylon_marketplace_job_intakes: "intake_ref",
  pylon_marketplace_triage_actions: "idempotency_key",
  pylon_provider_job_lifecycle: "assignment_ref",
  pylon_quarantines: "quarantine_ref",
  pylon_spark_payout_targets: "pylon_ref",
  runner_sessions: "id",
}

export const TABLE_ORDER_COLUMN: Readonly<
  Record<PylonControlPlaneBackfillTable, string>
> = {
  fleet_alerts: "detected_at",
  pylon_agent_runner_status_events: "updated_at",
  pylon_capacity_funnel_snapshots: "updated_at",
  pylon_codex_raw_event_chunks: "observed_at",
  pylon_codex_raw_events: "observed_at",
  pylon_marketplace_assignments: "updated_at",
  pylon_marketplace_job_intakes: "updated_at",
  pylon_marketplace_triage_actions: "created_at",
  pylon_provider_job_lifecycle: "updated_at",
  pylon_quarantines: "updated_at",
  pylon_spark_payout_targets: "updated_at",
  runner_sessions: "updated_at",
}

export const TABLE_TALLY_COLUMN: Readonly<
  Record<PylonControlPlaneBackfillTable, string>
> = {
  fleet_alerts: "classification",
  pylon_agent_runner_status_events: "state",
  pylon_capacity_funnel_snapshots: "bucket_kind",
  pylon_codex_raw_event_chunks: "demand_kind",
  pylon_codex_raw_events: "demand_kind",
  pylon_marketplace_assignments: "state",
  pylon_marketplace_job_intakes: "state",
  pylon_marketplace_triage_actions: "outcome",
  pylon_provider_job_lifecycle: "stage",
  pylon_quarantines: "state",
  pylon_spark_payout_targets: "owner_agent_user_id",
  runner_sessions: "status",
}

const normalizeValue = (value: unknown): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "boolean") return value ? 1 : 0
  if (typeof value === "number") return value
  return String(value)
}

type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "pylon control-plane backfill requires unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

export const upsertPylonControlPlaneRows = async (
  sql: SyncSql,
  table: PylonControlPlaneBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireUnsafe(sql)
  const columns = TABLE_COLUMNS[table]
  const conflictKey = TABLE_CONFLICT_KEY[table]
  let inserted = 0
  for (const row of rows) {
    const values = columns.map((column) => normalizeValue(row[column]))
    const columnsSql = columns.join(", ")
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${columnsSql}) VALUES (${placeholders}) ON CONFLICT (${conflictKey}) DO NOTHING RETURNING ${columns[0]}`,
      values as Array<unknown>,
    )
    inserted += result.length
  }
  return inserted
}

export const pylonControlPlaneRowHash = (
  table: PylonControlPlaneBackfillTable,
  row: D1SourceRow,
): string => {
  const hash = createHash("sha256")
  for (const column of TABLE_COLUMNS[table]) {
    const value = normalizeValue(row[column])
    hash.update(value === null ? "\\u0000" : String(value))
    hash.update("\\u001f")
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ hash: string; key: string }>

const keyForRow = (
  table: PylonControlPlaneBackfillTable,
  row: D1SourceRow,
): string => {
  const key = TABLE_CONFLICT_KEY[table]
  return key
    .split(",")
    .map((column) => String(normalizeValue(row[column.trim()])))
    .join(":")
}

export const d1PylonControlPlaneNewestHashes = (
  table: PylonControlPlaneBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> =>
  rows.map((row) => ({
    hash: pylonControlPlaneRowHash(table, row),
    key: keyForRow(table, row),
  }))

export const postgresPylonControlPlaneNewestHashes = async (
  sql: SyncSql,
  table: PylonControlPlaneBackfillTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const unsafe = requireUnsafe(sql)
  const orderColumn = TABLE_ORDER_COLUMN[table]
  const keyColumns = TABLE_CONFLICT_KEY[table]
    .split(",")
    .map((column) => column.trim())
  const rows = await unsafe(
    `SELECT ${TABLE_COLUMNS[table].join(", ")} FROM ${table} ORDER BY ${orderColumn} DESC, ${keyColumns.join(" DESC, ")} DESC LIMIT $1`,
    [limit],
  )
  return d1PylonControlPlaneNewestHashes(table, rows)
}

export type PylonControlPlaneTally = Readonly<{
  byStatus: Readonly<Record<string, number>>
  total: number
}>

export const tallyFromRows = (
  table: PylonControlPlaneBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): PylonControlPlaneTally => {
  const column = TABLE_TALLY_COLUMN[table]
  const byStatus: Record<string, number> = {}
  for (const row of rows) {
    const key = String(normalizeValue(row[column]) ?? "<null>")
    byStatus[key] = (byStatus[key] ?? 0) + 1
  }
  return { byStatus, total: rows.length }
}

export const postgresPylonControlPlaneTally = async (
  sql: SyncSql,
  table: PylonControlPlaneBackfillTable,
): Promise<PylonControlPlaneTally> => {
  const column = TABLE_TALLY_COLUMN[table]
  const rows = (await requireUnsafe(sql)(
    `SELECT ${column} AS status_value, count(*) AS row_count FROM ${table} GROUP BY ${column} ORDER BY ${column}`,
    [],
  )) as Array<{ row_count: unknown; status_value: string | null }>
  const byStatus: Record<string, number> = {}
  let total = 0
  for (const row of rows) {
    const count = Number(row.row_count)
    byStatus[row.status_value ?? "<null>"] = count
    total += count
  }
  return { byStatus, total }
}

export type NewestHashMismatch = Readonly<{
  d1Hash: string | undefined
  key: string
  postgresHash: string | undefined
}>

export type PylonControlPlaneVerifyReport = Readonly<{
  countsMatch: boolean
  d1Total: number
  newestHashMismatches: ReadonlyArray<NewestHashMismatch>
  postgresTotal: number
  statusMismatches: ReadonlyArray<{
    d1: number
    postgres: number
    status: string
  }>
  table: PylonControlPlaneBackfillTable
}>

export const compareNewestHashes = (
  d1: ReadonlyArray<NewestRowHash>,
  postgres: ReadonlyArray<NewestRowHash>,
): ReadonlyArray<NewestHashMismatch> => {
  const keys = new Set([...d1.map((row) => row.key), ...postgres.map((row) => row.key)])
  const d1ByKey = new Map(d1.map((row) => [row.key, row.hash]))
  const pgByKey = new Map(postgres.map((row) => [row.key, row.hash]))
  return [...keys]
    .sort()
    .flatMap((key) => {
      const d1Hash = d1ByKey.get(key)
      const postgresHash = pgByKey.get(key)
      return d1Hash === postgresHash
        ? []
        : [{ d1Hash, key, postgresHash }]
    })
}

export const comparePylonControlPlaneTallies = (
  table: PylonControlPlaneBackfillTable,
  d1: PylonControlPlaneTally,
  postgres: PylonControlPlaneTally,
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): PylonControlPlaneVerifyReport => {
  const statuses = new Set([
    ...Object.keys(d1.byStatus),
    ...Object.keys(postgres.byStatus),
  ])
  const statusMismatches = [...statuses]
    .sort()
    .flatMap((status) => {
      const d1Count = d1.byStatus[status] ?? 0
      const postgresCount = postgres.byStatus[status] ?? 0
      return d1Count === postgresCount
        ? []
        : [{ d1: d1Count, postgres: postgresCount, status }]
    })
  return {
    countsMatch: d1.total === postgres.total,
    d1Total: d1.total,
    newestHashMismatches: compareNewestHashes(d1Newest, postgresNewest),
    postgresTotal: postgres.total,
    statusMismatches,
    table,
  }
}
