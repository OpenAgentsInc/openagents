/**
 * KS-8.12 (#8323): Sites domain CORE — the SHARED table registry and
 * Postgres converge-upsert core for the fifteen sites content/builder
 * tables in khala-sync migration `0020_sites_core.sql`.
 *
 * ONE source of truth for column lists, primary keys, secondary mirror
 * keys, and conflict semantics, consumed by BOTH sides of the migration
 * machinery (the KS-8.10 sharing rule — a schema change cannot silently
 * drift the two):
 *
 *   - the Worker's dual-write mirror
 *     (`apps/openagents.com/workers/api/src/sites-content-store.ts`)
 *   - the backfill + verify core (`./sites-content-backfill.ts`) and CLI
 *     (`scripts/backfill-sites-content.ts`).
 *
 * This module is imported by Worker code, so it uses NO node built-ins
 * (hashing lives in sites-content-backfill.ts, which only the CLI and
 * tests load).
 *
 * CONFLICT SEMANTICS: every table converges on its PRIMARY KEY
 * (`ON CONFLICT (pk) DO UPDATE SET …` = the D1 snapshot value). D1 never
 * re-issues an id for these tables (the builder INSERT OR IGNORE paths
 * dedupe on UNIQUE(idempotency_key), and the mirror only ever replays
 * rows that EXIST in D1, so a converge on the PK reproduces exactly the
 * D1 row set). Secondary uniques exist on the Postgres twins for parity;
 * a violation there surfaces as a logged dual-write failure, which IS the
 * drift signal.
 *
 * SECONDARY MIRROR KEYS: unlike the forum domain, several sites writes
 * are keyed by a PARENT id, not the row PK — deployment state
 * transitions (`UPDATE site_deployments … WHERE site_id = ?`) and
 * library archival (`UPDATE site_builder_sessions … WHERE site_id = ?`).
 * `SITES_CONTENT_TABLE_MIRROR_KEYS` registers, per table, the columns a
 * read-back mirror may key on; the fan-out per key is bounded (a site's
 * deployments / sessions, a session's satellites).
 */

import type { SyncSql } from "./sql.js"

export type SitesContentTable =
  | "site_projects"
  | "site_versions"
  | "site_deployments"
  | "site_deployment_attempts"
  | "site_access_grants"
  | "site_events"
  | "site_builder_sessions"
  | "site_builder_messages"
  | "site_builder_events"
  | "site_builder_phase_runs"
  | "site_builder_file_snapshots"
  | "site_builder_previews"
  | "site_builder_artifacts"
  | "site_builder_repair_attempts"
  | "site_builder_saved_versions"

/**
 * Projects first, then versions/deployments (chain parents), then the
 * satellites, then builder sessions before their satellites, so backfill
 * pages always land parents before children (no FKs, but the verify
 * joins expect it).
 */
export const SITES_CONTENT_TABLES: ReadonlyArray<SitesContentTable> = [
  "site_projects",
  "site_versions",
  "site_deployments",
  "site_deployment_attempts",
  "site_access_grants",
  "site_events",
  "site_builder_sessions",
  "site_builder_messages",
  "site_builder_events",
  "site_builder_phase_runs",
  "site_builder_file_snapshots",
  "site_builder_previews",
  "site_builder_artifacts",
  "site_builder_repair_attempts",
  "site_builder_saved_versions",
]

/**
 * Column lists in D1 PHYSICAL order (`SELECT *` order — SQLite ALTER ADD
 * COLUMN appends, so `email_message_id` trails site_events). Row hashes
 * iterate this order on both stores; keep it stable.
 */
export const SITES_CONTENT_TABLE_COLUMNS: Readonly<
  Record<SitesContentTable, ReadonlyArray<string>>
> = {
  site_access_grants: [
    "id",
    "site_id",
    "principal_kind",
    "principal_ref",
    "role",
    "created_at",
    "revoked_at",
  ],
  site_builder_artifacts: [
    "id",
    "idempotency_key",
    "session_id",
    "artifact_kind",
    "artifact_ref",
    "content_hash",
    "byte_size",
    "manifest_ref",
    "metadata_json",
    "created_at",
    "archived_at",
  ],
  site_builder_events: [
    "id",
    "idempotency_key",
    "session_id",
    "sequence",
    "event_kind",
    "phase_kind",
    "visibility",
    "status",
    "title",
    "summary",
    "source_ref",
    "payload_json",
    "created_at",
    "archived_at",
  ],
  site_builder_file_snapshots: [
    "id",
    "idempotency_key",
    "session_id",
    "path",
    "sequence",
    "language",
    "content_hash",
    "byte_size",
    "source_ref",
    "artifact_ref",
    "preview_text",
    "visibility",
    "metadata_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_builder_messages: [
    "id",
    "idempotency_key",
    "session_id",
    "sequence",
    "actor_kind",
    "visibility",
    "body",
    "metadata_json",
    "created_at",
    "archived_at",
  ],
  site_builder_phase_runs: [
    "id",
    "idempotency_key",
    "session_id",
    "sequence",
    "phase_kind",
    "status",
    "title",
    "summary",
    "started_at",
    "completed_at",
    "metadata_json",
    "created_at",
    "archived_at",
  ],
  site_builder_previews: [
    "id",
    "idempotency_key",
    "session_id",
    "preview_kind",
    "status",
    "preview_url",
    "version_ref",
    "artifact_ref",
    "health_ref",
    "metadata_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_builder_repair_attempts: [
    "id",
    "idempotency_key",
    "session_id",
    "preview_id",
    "phase_kind",
    "attempt_number",
    "retry_budget",
    "status",
    "failure_kind",
    "redacted_summary",
    "stop_reason",
    "metadata_json",
    "created_at",
    "completed_at",
    "archived_at",
  ],
  site_builder_saved_versions: [
    "id",
    "idempotency_key",
    "session_id",
    "site_id",
    "site_version_id",
    "preview_id",
    "artifact_ref",
    "build_receipt_ref",
    "source_hash",
    "notes",
    "site_metadata_json",
    "created_at",
    "archived_at",
  ],
  site_builder_sessions: [
    "id",
    "idempotency_key",
    "site_id",
    "order_id",
    "workroom_id",
    "owner_user_id",
    "customer_user_id",
    "created_by_actor_ref",
    "status",
    "prompt_summary",
    "source_site_version_id",
    "source_revision_id",
    "active_preview_id",
    "active_artifact_id",
    "metadata_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_deployment_attempts: [
    "id",
    "site_id",
    "version_id",
    "deployment_id",
    "runtime_kind",
    "runtime_script_name",
    "dispatch_namespace",
    "external_deployment_id",
    "status",
    "upload_receipt_ref",
    "health_status",
    "health_url",
    "health_ref",
    "rollback_ref",
    "observability_ref",
    "metadata_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_deployments: [
    "id",
    "site_id",
    "version_id",
    "slug",
    "url",
    "runtime_kind",
    "runtime_script_name",
    "dispatch_namespace",
    "status",
    "deployed_by_user_id",
    "external_deployment_id",
    "started_at",
    "activated_at",
    "failed_at",
    "disabled_at",
    "rolled_back_at",
    "created_at",
    "updated_at",
  ],
  site_events: [
    "id",
    "site_id",
    "version_id",
    "deployment_id",
    "type",
    "summary",
    "actor_user_id",
    "actor_run_id",
    "payload_json",
    "created_at",
    // Appended by worker migration 0038 (SQLite ALTER appends; D1
    // physical column order kept for SELECT * row-hash parity).
    "email_message_id",
  ],
  site_projects: [
    "id",
    "software_order_id",
    "owner_user_id",
    "team_id",
    "project_id",
    "slug",
    "title",
    "prompt",
    "status",
    "access_mode",
    "visibility",
    "source_repository_provider",
    "source_repository_owner",
    "source_repository_name",
    "source_repository_ref",
    "active_version_id",
    "active_deployment_id",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_versions: [
    "id",
    "site_id",
    "source_kind",
    "source_commit_sha",
    "source_archive_r2_key",
    "artifact_manifest_r2_key",
    "build_log_r2_key",
    "build_status",
    "build_command",
    "worker_module_r2_key",
    "static_assets_manifest_json",
    "d1_binding_name",
    "r2_binding_name",
    "metadata_json",
    "created_by_user_id",
    "created_by_run_id",
    "created_at",
    "saved_at",
    "rejected_at",
  ],
}

/**
 * Primary-key column per table — the converge-upsert arbiter. Every
 * scoped table keys on `id`, and D1 never replaces a row's id, so the PK
 * is the stable arbiter.
 */
export const SITES_CONTENT_TABLE_PK: Readonly<
  Record<SitesContentTable, string>
> = {
  site_access_grants: "id",
  site_builder_artifacts: "id",
  site_builder_events: "id",
  site_builder_file_snapshots: "id",
  site_builder_messages: "id",
  site_builder_phase_runs: "id",
  site_builder_previews: "id",
  site_builder_repair_attempts: "id",
  site_builder_saved_versions: "id",
  site_builder_sessions: "id",
  site_deployment_attempts: "id",
  site_deployments: "id",
  site_events: "id",
  site_projects: "id",
  site_versions: "id",
}

/**
 * Columns (besides the PK) a read-back mirror may key on when a scoped
 * UPDATE has no PK equality in its WHERE clause — the deployment
 * rollback/disable transitions and library archival key on `site_id`;
 * builder satellites could in principle transition by `session_id`. The
 * fan-out per key value is bounded (one site's deployments/sessions, one
 * session's satellites), so a keyed read-back stays a small page.
 */
export const SITES_CONTENT_TABLE_MIRROR_KEYS: Readonly<
  Record<SitesContentTable, ReadonlyArray<string>>
> = {
  site_access_grants: ["site_id"],
  site_builder_artifacts: ["session_id"],
  site_builder_events: ["session_id"],
  site_builder_file_snapshots: ["session_id"],
  site_builder_messages: ["session_id"],
  site_builder_phase_runs: ["session_id"],
  site_builder_previews: ["session_id"],
  site_builder_repair_attempts: ["session_id"],
  site_builder_saved_versions: ["session_id", "site_id"],
  site_builder_sessions: ["site_id"],
  site_deployment_attempts: ["site_id"],
  site_deployments: ["site_id"],
  site_events: ["site_id"],
  site_projects: [],
  site_versions: ["site_id"],
}

export const isSitesContentTable = (
  value: string,
): value is SitesContentTable =>
  Object.prototype.hasOwnProperty.call(SITES_CONTENT_TABLE_PK, value)

export type SitesContentRow = Readonly<Record<string, unknown>>

/**
 * D1 → Postgres value normalization: D1/SQLite hands back TEXT / INTEGER /
 * REAL / NULL; booleans arrive as 0/1 already. Keep bytes identical so
 * row-hash reconciliation compares equal.
 */
export const normalizeSitesContentValue = (
  value: unknown,
): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "bigint") return Number(value)
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)`; the
 * structural `SyncSql` seam deliberately does not, so this module widens
 * it locally (same note as the pylon/token/agent-runtime/forum
 * backfills). Both runtime drivers here run with `prepare: false`, so
 * `unsafe` uses the unnamed statement and is transaction-pooler/
 * Hyperdrive-safe.
 */
export type SitesContentUnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

export const requireSitesContentUnsafe = (
  sql: SyncSql,
): SitesContentUnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: SitesContentUnsafeQuery })
    .unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "sites content store requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/**
 * Converge-upsert one page of D1 rows into `table` on Postgres: the row
 * becomes exactly the D1 snapshot (`ON CONFLICT (pk) DO UPDATE SET` every
 * non-key column). Idempotent — a re-run with the same rows converges to
 * the same state. Returns how many rows were touched.
 */
export const upsertSitesContentRows = async (
  sql: SyncSql,
  table: SitesContentTable,
  rows: ReadonlyArray<SitesContentRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireSitesContentUnsafe(sql)
  const columns = SITES_CONTENT_TABLE_COLUMNS[table]
  const pk = SITES_CONTENT_TABLE_PK[table]
  const setClauses = columns
    .filter((column) => column !== pk)
    .map((column) => `${column} = EXCLUDED.${column}`)
    .join(", ")

  let touched = 0
  for (const row of rows) {
    const values = columns.map((column) =>
      normalizeSitesContentValue(row[column]),
    )
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${columns.join(", ")}) VALUES (${placeholders}) ON CONFLICT (${pk}) DO UPDATE SET ${setClauses} RETURNING 1 AS touched`,
      values as Array<unknown>,
    )
    touched += result.length
  }
  return touched
}
