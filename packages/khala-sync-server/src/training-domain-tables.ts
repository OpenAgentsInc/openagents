import type { SyncSql, SyncTransactionSql } from "./sql.js"

/**
 * KS-8.15 (#8326): training domain CORE — shared table metadata and
 * Postgres converge/upsert helpers for the seven `training_*` tables that
 * move from D1 to Cloud SQL (khala-sync migration
 * `0019_training_domain.sql`).
 *
 * This file is imported by BOTH the Worker mirror
 * (`apps/openagents.com/workers/api/src/training-domain-store.ts`) and the
 * backfill/verify CLI (`scripts/backfill-training.ts`), so it owns
 * column/key order once — keep it runtime-neutral: no Node built-ins.
 *
 * Write modes:
 *  - `converge`: state tables upsert ON CONFLICT (ref arbiter) DO UPDATE —
 *    the last authoritative D1 row wins byte-exactly (training receipts
 *    feed public claims and must round-trip byte-exact).
 *  - `insertIfAbsent`: the two append-only event ledgers use a bare
 *    ON CONFLICT DO NOTHING (exact-replay dedupe on the id PK).
 */

export type TrainingDomainTable =
  | "training_runs"
  | "training_windows"
  | "training_window_events"
  | "training_window_leases"
  | "training_verification_challenges"
  | "training_verification_events"
  | "training_trace_contributions"

export type TrainingDomainWriteMode = "converge" | "insertIfAbsent"

export type TrainingDomainTableSpec = Readonly<{
  columns: ReadonlyArray<string>
  /**
   * The converge arbiter — the SAME unique key the live D1 UPDATEs address
   * rows by (refs, not ids, for the state tables; id PK for the ledgers).
   * Used by backfill, mirror read-back, and ON CONFLICT.
   */
  keyColumns: ReadonlyArray<string>
  /** Newest-first ordering column for hash verification. */
  orderColumn: string
  writeMode: TrainingDomainWriteMode
}>

export type TrainingDomainRow = Readonly<Record<string, unknown>>

/** Parent-first order: runs → windows → leases → the dependent ledgers. */
export const TRAINING_DOMAIN_TABLES: ReadonlyArray<TrainingDomainTable> = [
  "training_runs",
  "training_windows",
  "training_window_events",
  "training_window_leases",
  "training_verification_challenges",
  "training_verification_events",
  "training_trace_contributions",
]

export const TRAINING_DOMAIN_TABLE_SPECS: Readonly<
  Record<TrainingDomainTable, TrainingDomainTableSpec>
> = {
  training_runs: {
    columns: [
      "id",
      "training_run_ref",
      "promise_ref",
      "state",
      "max_allowed_stale",
      "seal_publication_cadence_windows",
      "seal_in_flight_at",
      "manifest_json",
      "source_refs_json",
      "receipt_refs_json",
      "public_projection_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["training_run_ref"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
  training_trace_contributions: {
    columns: [
      "id",
      "contribution_ref",
      "lease_ref",
      "window_ref",
      "training_run_ref",
      "pylon_ref",
      "workload_family",
      "assignment_ref",
      "pylon_device_ref",
      "trace_commitment_digest_ref",
      "sampled_window_ref",
      "sampled_window_start_step",
      "sampled_window_end_step",
      "worker_receipt_ref",
      "state",
      "validator_device_ref",
      "replay_digest_ref",
      "verification_challenge_ref",
      "public_projection_json",
      "submitted_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["contribution_ref"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
  training_verification_challenges: {
    columns: [
      "id",
      "challenge_ref",
      "training_run_ref",
      "window_ref",
      "contribution_ref",
      "homework_kind",
      "verification_class",
      "sampling_policy",
      "state",
      "attempt_count",
      "max_attempts",
      "lease_ref",
      "leased_to_ref",
      "lease_expires_at",
      "payload_json",
      "commitment_refs_json",
      "failure_codes_json",
      "verdict_refs_json",
      "public_projection_json",
      "created_at",
      "updated_at",
      "verified_at",
      "rejected_at",
      "timed_out_at",
      "archived_at",
    ],
    keyColumns: ["challenge_ref"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
  training_verification_events: {
    columns: [
      "id",
      "challenge_ref",
      "transition_kind",
      "state_from",
      "state_to",
      "validator_ref",
      "failure_codes_json",
      "receipt_refs_json",
      "created_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
    writeMode: "insertIfAbsent",
  },
  training_window_events: {
    columns: [
      "id",
      "window_ref",
      "transition_kind",
      "state_from",
      "state_to",
      "actor_ref",
      "receipt_ref",
      "created_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
    writeMode: "insertIfAbsent",
  },
  training_window_leases: {
    columns: [
      "id",
      "lease_ref",
      "window_ref",
      "training_run_ref",
      "pylon_ref",
      "state",
      "receipt_refs_json",
      "public_projection_json",
      "claimed_at",
      "lease_expires_at",
      "archived_at",
    ],
    keyColumns: ["lease_ref"],
    orderColumn: "claimed_at",
    writeMode: "converge",
  },
  training_windows: {
    columns: [
      "id",
      "window_ref",
      "training_run_ref",
      "state",
      "homework_kind",
      "priority",
      "dataset_refs_json",
      "source_refs_json",
      "receipt_refs_json",
      "seal_metadata_json",
      "public_projection_json",
      "planned_at",
      "activated_at",
      "sealed_at",
      "reconciled_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["window_ref"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
}

export const isTrainingDomainTable = (
  value: string,
): value is TrainingDomainTable =>
  Object.prototype.hasOwnProperty.call(TRAINING_DOMAIN_TABLE_SPECS, value)

export const normalizeTrainingDomainValue = (
  value: unknown,
): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "bigint") return value.toString()
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

export type TrainingDomainUnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

export const requireTrainingDomainUnsafe = (
  sql: SyncSql,
): TrainingDomainUnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: TrainingDomainUnsafeQuery })
    .unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "Training domain store requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/**
 * Converge/insert-if-absent upsert into the Postgres twins. Returns how
 * many rows were touched (converge) or freshly inserted (ledgers) — the
 * same counting contract as the KS-8.5/8.10/8.13 stores.
 */
export const upsertTrainingDomainRows = async (
  sql: SyncSql | SyncTransactionSql,
  table: TrainingDomainTable,
  rows: ReadonlyArray<TrainingDomainRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireTrainingDomainUnsafe(sql as SyncSql)
  const spec = TRAINING_DOMAIN_TABLE_SPECS[table]

  const conflictClause =
    spec.writeMode === "insertIfAbsent"
      ? "ON CONFLICT DO NOTHING"
      : `ON CONFLICT (${spec.keyColumns.join(", ")}) DO UPDATE SET ${spec.columns
          .filter((column) => !spec.keyColumns.includes(column))
          .map((column) => `${column} = EXCLUDED.${column}`)
          .join(", ")}`

  let touched = 0
  for (const row of rows) {
    const values = spec.columns.map((column) =>
      normalizeTrainingDomainValue(row[column]),
    )
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${spec.columns.join(", ")}) VALUES (${placeholders}) ${conflictClause} RETURNING 1 AS touched`,
      values as Array<unknown>,
    )
    touched += result.length
  }
  return touched
}
