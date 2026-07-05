import type { SyncSql, SyncTransactionSql } from "./sql.js"

/**
 * KS-8.15 remainder (#8355): gym / mullet / blueprint / replay-clip /
 * mirrorcode eval domain — shared table metadata and Postgres
 * converge/upsert helpers for the 16 tables that move from D1 to Cloud SQL in
 * the follow-up to the training CORE (khala-sync migration
 * `0026_gym_evals_domain.sql`). Extends the training-domain-tables.ts
 * registry pattern exactly.
 *
 * Imported by BOTH the Worker mirror
 * (`apps/openagents.com/workers/api/src/gym-evals-domain-store.ts`) and the
 * backfill/verify CLI (`scripts/backfill-gym-evals.ts`), so it owns
 * column/key order once — runtime-neutral, no Node built-ins.
 *
 * Write modes (same contract as training):
 *  - `converge`: state/snapshot tables upsert ON CONFLICT (keyColumns) DO
 *    UPDATE — the last authoritative D1 row wins byte-exactly (gym / ladder /
 *    mirrorcode rows feed public projections and must round-trip byte-exact).
 *  - `insertIfAbsent`: insert-once / append-only tables (harbor archives,
 *    mullet child rows) use a bare ON CONFLICT DO NOTHING (exact-replay
 *    dedupe; never clobber the original).
 */

export type GymEvalsDomainTable =
  | "gym_harbor_full_trace_archives"
  | "gym_ladder_leaderboard_snapshots"
  | "gym_mutalisk_khala_delegation_jobs"
  | "gym_mutalisk_khala_delegation_progress"
  | "gym_mutalisk_khala_delegation_summaries"
  | "gym_run_progress_snapshots"
  | "mullet_scenarios"
  | "mullet_simulation_runs"
  | "mullet_run_hourly_results"
  | "mullet_run_candidate_modes"
  | "mullet_run_exports"
  | "blueprint_program_runs"
  | "blueprint_action_submissions"
  | "blueprint_probe_contributions"
  | "replay_clip_jobs"
  | "mirrorcode_runs"

export type GymEvalsDomainWriteMode = "converge" | "insertIfAbsent"

export type GymEvalsDomainTableSpec = Readonly<{
  columns: ReadonlyArray<string>
  /**
   * The converge arbiter — the SAME unique key the live D1 writer addresses
   * rows by (PK, the ON CONFLICT upsert target, or the composite PK). Used by
   * backfill, mirror read-back, and ON CONFLICT.
   */
  keyColumns: ReadonlyArray<string>
  /** Newest-first ordering column for hash verification. */
  orderColumn: string
  writeMode: GymEvalsDomainWriteMode
}>

export type GymEvalsDomainRow = Readonly<Record<string, unknown>>

/** Parent-first order (jobs → children → ledgers) for the backfill. */
export const GYM_EVALS_DOMAIN_TABLES: ReadonlyArray<GymEvalsDomainTable> = [
  "gym_harbor_full_trace_archives",
  "gym_ladder_leaderboard_snapshots",
  "gym_mutalisk_khala_delegation_jobs",
  "gym_mutalisk_khala_delegation_progress",
  "gym_mutalisk_khala_delegation_summaries",
  "gym_run_progress_snapshots",
  "mullet_scenarios",
  "mullet_simulation_runs",
  "mullet_run_hourly_results",
  "mullet_run_candidate_modes",
  "mullet_run_exports",
  "blueprint_program_runs",
  "blueprint_action_submissions",
  "blueprint_probe_contributions",
  "replay_clip_jobs",
  "mirrorcode_runs",
]

export const GYM_EVALS_DOMAIN_TABLE_SPECS: Readonly<
  Record<GymEvalsDomainTable, GymEvalsDomainTableSpec>
> = {
  gym_harbor_full_trace_archives: {
    columns: [
      "archive_ref",
      "run_ref",
      "job_ref",
      "source_kind",
      "artifact_r2_key",
      "artifact_sha256",
      "artifact_bytes",
      "content_type",
      "capture_started_at",
      "capture_completed_at",
      "visibility",
      "contains_raw_prompts",
      "contains_raw_logs",
      "contains_private_material",
      "demand_kind",
      "demand_source",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["archive_ref"],
    orderColumn: "updated_at",
    writeMode: "insertIfAbsent",
  },
  gym_ladder_leaderboard_snapshots: {
    columns: ["ladder_ref", "ladder_json", "published_at", "created_at"],
    keyColumns: ["ladder_ref"],
    orderColumn: "published_at",
    writeMode: "converge",
  },
  gym_mutalisk_khala_delegation_jobs: {
    columns: [
      "run_ref",
      "job_ref",
      "job_json",
      "projection_json",
      "latest_stage",
      "updated_at",
      "created_at",
    ],
    keyColumns: ["run_ref"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
  gym_mutalisk_khala_delegation_progress: {
    columns: ["run_ref", "stage", "progress_json", "updated_at"],
    keyColumns: ["run_ref", "stage"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
  gym_mutalisk_khala_delegation_summaries: {
    columns: [
      "run_ref",
      "candidate_manifest_ref",
      "candidate_ref",
      "summary_json",
      "admission_json",
      "bridge_output_json",
      "metric_value_bps",
      "admission_decision",
      "ingested_at",
      "updated_at",
    ],
    keyColumns: ["run_ref"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
  gym_run_progress_snapshots: {
    columns: [
      "run_ref",
      "progress_json",
      "last_updated_at",
      "ingested_at",
      "created_at",
    ],
    keyColumns: ["run_ref"],
    orderColumn: "last_updated_at",
    writeMode: "converge",
  },
  mullet_scenarios: {
    columns: [
      "id",
      "owner_user_id",
      "owner_email",
      "schema_version",
      "name",
      "kind",
      "scenario_json",
      "source_refs_json",
      "provenance_summary_json",
      "visibility",
      "export_redaction_state",
      "created_at",
      "updated_at",
      "deleted_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
  mullet_simulation_runs: {
    columns: [
      "id",
      "scenario_id",
      "owner_user_id",
      "owner_email",
      "schema_version",
      "status",
      "run_json",
      "source_refs_json",
      "provenance_summary_json",
      "provider_settlement_state",
      "power_data_state",
      "visibility",
      "export_redaction_state",
      "created_at",
      "updated_at",
      "completed_at",
      "deleted_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
  mullet_run_hourly_results: {
    columns: [
      "id",
      "run_id",
      "scenario_id",
      "owner_user_id",
      "hour_index",
      "timestamp",
      "selected_mode",
      "reason_code",
      "energy_mwh",
      "result_json",
      "created_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
    writeMode: "insertIfAbsent",
  },
  mullet_run_candidate_modes: {
    columns: [
      "id",
      "run_id",
      "hourly_result_id",
      "scenario_id",
      "owner_user_id",
      "hour_index",
      "candidate_index",
      "timestamp",
      "mode",
      "reason_code",
      "risk_adjusted_net_usd_per_mwh",
      "clears_readiness",
      "clears_demand",
      "clears_provider_floor",
      "candidate_json",
      "created_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
    writeMode: "insertIfAbsent",
  },
  mullet_run_exports: {
    columns: [
      "id",
      "run_id",
      "scenario_id",
      "owner_user_id",
      "owner_email",
      "schema_version",
      "format",
      "export_json",
      "private_visibility",
      "redaction_status",
      "content_ref",
      "created_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
    writeMode: "insertIfAbsent",
  },
  blueprint_program_runs: {
    columns: [
      "id",
      "idempotency_key",
      "actor_ref",
      "purpose_ref",
      "program_type_id",
      "program_signature_id",
      "module_version_id",
      "input_snapshot_hash",
      "typed_output_json",
      "confidence",
      "route_ref",
      "cost_ref",
      "latency_ms",
      "evidence_refs_json",
      "receipt_refs_json",
      "authority_boundary",
      "direct_mutation_disabled",
      "no_deploy",
      "no_email",
      "no_spend",
      "no_source_mutation",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
  blueprint_action_submissions: {
    columns: [
      "id",
      "idempotency_key",
      "action_kind",
      "approval_policy_ref",
      "approval_receipt_ref",
      "approval_state",
      "approved_by_ref",
      "content_redacted",
      "context_pack_refs_json",
      "direct_execution",
      "direct_program_run_execution_allowed",
      "dry_run_receipt_ref",
      "dry_run_required",
      "evidence_refs_json",
      "execution_receipt_ref",
      "failure_ref",
      "model_confidence_bypass_disabled",
      "program_run_authority_boundary",
      "proposal_only",
      "proposed_by_program_run_id",
      "proposed_effect_ref",
      "receipt_refs_json",
      "source_authority_refs_json",
      "status",
      "summary_ref",
      "tool_refs_json",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
  blueprint_probe_contributions: {
    columns: [
      "id",
      "idempotency_key",
      "contribution_kind",
      "status",
      "review_status",
      "release_gate_ready",
      "candidate_runtime_allowed",
      "production_runtime_allowed",
      "blocker_refs_json",
      "release_gate_refs_json",
      "fixture_refs_json",
      "retained_failure_refs_json",
      "target_refs_json",
      "signature_contribution_json",
      "developer_package_contribution_json",
      "projection_json",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
  replay_clip_jobs: {
    columns: [
      "job_ref",
      "status",
      "request_json",
      "source_refs_json",
      "caveat_refs_json",
      "blocker_refs_json",
      "manifest_ref",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["job_ref"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
  mirrorcode_runs: {
    columns: [
      "run_id",
      "run_json",
      "bucket",
      "grade",
      "status",
      "started_at",
      "updated_at",
      "created_at",
    ],
    keyColumns: ["run_id"],
    orderColumn: "updated_at",
    writeMode: "converge",
  },
}

export const isGymEvalsDomainTable = (
  value: string,
): value is GymEvalsDomainTable =>
  Object.prototype.hasOwnProperty.call(GYM_EVALS_DOMAIN_TABLE_SPECS, value)

export const normalizeGymEvalsDomainValue = (
  value: unknown,
): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "bigint") return value.toString()
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

export type GymEvalsDomainUnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

export const requireGymEvalsDomainUnsafe = (
  sql: SyncSql,
): GymEvalsDomainUnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: GymEvalsDomainUnsafeQuery })
    .unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "Gym/evals domain store requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/**
 * Converge/insert-if-absent upsert into the Postgres twins. Returns how many
 * rows were touched (converge) or freshly inserted (insert-if-absent) — the
 * same counting contract as the KS-8.5/8.10/8.13/8.15 stores.
 */
export const upsertGymEvalsDomainRows = async (
  sql: SyncSql | SyncTransactionSql,
  table: GymEvalsDomainTable,
  rows: ReadonlyArray<GymEvalsDomainRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireGymEvalsDomainUnsafe(sql as SyncSql)
  const spec = GYM_EVALS_DOMAIN_TABLE_SPECS[table]

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
      normalizeGymEvalsDomainValue(row[column]),
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
