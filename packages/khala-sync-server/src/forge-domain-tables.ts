import type { SyncSql, SyncTransactionSql } from "./sql.js"

/**
 * KS-8.16 (#8327): Forge (git intake + coordination) domain.
 *
 * Shared table metadata and Postgres converge/upsert helpers for ALL
 * SIXTEEN `forge_*` tables moving from D1 to Cloud SQL (khala-sync
 * migration `0021_forge_domain.sql`, mirroring worker migrations
 * 0251/0252/0253/0254/0255/0256/0259/0260/0284). This file is imported by
 * both the Worker dual-write mirror
 * (`apps/openagents.com/workers/api/src/forge-domain-store.ts`) and the
 * backfill/verify CLI (`scripts/backfill-forge.ts`) — one registry, so
 * mirror and backfill can never fight: both write identical converge
 * upserts keyed on the table's (composite) primary key.
 *
 * SECRETS (SPEC invariant 9): `forge_git_access_tokens` carries token
 * HASHES/prefixes only — raw tokens are never stored on either engine,
 * and the twin is column-for-column with D1 (no widening). The custody
 * columns are declared in `custodyColumns` so callers keep them out of
 * diagnostics and human-facing output: only row KEYS (tenant_ref,
 * token_ref) and sha256 row hashes may be printed for that table.
 *
 * Runtime-neutral on purpose: no Node built-ins (the Worker imports this).
 */

export type ForgeDomainTable =
  | "forge_coordination_issues"
  | "forge_coordination_prs"
  | "forge_coordination_status"
  | "forge_dispatch_leases"
  | "forge_merge_queue_ledger"
  | "forge_git_packfile_archives"
  | "forge_tenants"
  | "forge_git_access_tokens"
  | "forge_git_access_token_scopes"
  | "forge_verification_receipts"
  | "forge_promotion_decisions"
  | "forge_git_receive_pack_intakes"
  | "forge_git_refs"
  | "forge_git_objects"
  | "forge_git_ref_locks"
  | "forge_github_mirror_receipts"

export type ForgeDomainTableSpec = Readonly<{
  columns: ReadonlyArray<string>
  /**
   * The table's (composite) PRIMARY KEY — the converge key used by the
   * backfill, the mirror read-back, and ON CONFLICT.
   */
  keyColumns: ReadonlyArray<string>
  /** Newest-first ordering column for hash verification. */
  orderColumn: string
  /**
   * Secret-adjacent custody columns (token hashes/prefixes). Values from
   * these columns must never appear in diagnostics, logs, or
   * backfill/verify output — keys and sha256 row hashes only.
   */
  custodyColumns?: ReadonlyArray<string>
}>

export type ForgeDomainRow = Readonly<Record<string, unknown>>

/** Backfill/verify sweep order (parents-before-children is cosmetic —
 * there are no FKs on the Postgres side — but keeps output readable). */
export const FORGE_DOMAIN_TABLES: ReadonlyArray<ForgeDomainTable> = [
  "forge_tenants",
  "forge_git_access_tokens",
  "forge_git_access_token_scopes",
  "forge_coordination_issues",
  "forge_coordination_prs",
  "forge_coordination_status",
  "forge_dispatch_leases",
  "forge_merge_queue_ledger",
  "forge_git_packfile_archives",
  "forge_git_receive_pack_intakes",
  "forge_git_refs",
  "forge_git_objects",
  "forge_git_ref_locks",
  "forge_verification_receipts",
  "forge_promotion_decisions",
  "forge_github_mirror_receipts",
]

export const FORGE_DOMAIN_TABLE_SPECS: Readonly<
  Record<ForgeDomainTable, ForgeDomainTableSpec>
> = {
  forge_coordination_issues: {
    columns: [
      "tenant_ref",
      "issue_ref",
      "github_issue_number",
      "title",
      "state",
      "priority_ref",
      "source_refs_json",
      "created_at",
      "updated_at",
      "git_token_refs_json",
    ],
    keyColumns: ["tenant_ref", "issue_ref"],
    orderColumn: "updated_at",
  },
  forge_coordination_prs: {
    columns: [
      "tenant_ref",
      "pr_ref",
      "issue_ref",
      "change_ref",
      "state",
      "base_head",
      "patch_head",
      "verification_ref",
      "blocker_refs_json",
      "source_refs_json",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["tenant_ref", "pr_ref"],
    orderColumn: "updated_at",
  },
  forge_coordination_status: {
    columns: [
      "tenant_ref",
      "status_ref",
      "subject_ref",
      "nip34_kind",
      "state",
      "actor_ref",
      "source_refs_json",
      "created_at",
    ],
    keyColumns: ["tenant_ref", "status_ref"],
    orderColumn: "created_at",
  },
  forge_dispatch_leases: {
    columns: [
      "tenant_ref",
      "lease_ref",
      "work_ref",
      "owner_agent_ref",
      "state",
      "idempotency_key_hash",
      "acquired_at",
      "heartbeat_at",
      "expires_at",
      "released_at",
      "source_refs_json",
    ],
    keyColumns: ["tenant_ref", "lease_ref"],
    orderColumn: "acquired_at",
  },
  forge_git_access_token_scopes: {
    columns: ["tenant_ref", "token_ref", "scope", "created_at"],
    keyColumns: ["tenant_ref", "token_ref", "scope"],
    orderColumn: "created_at",
  },
  forge_git_access_tokens: {
    columns: [
      "tenant_ref",
      "token_ref",
      "subject_ref",
      "repository_ref",
      "token_hash",
      "token_prefix",
      "state",
      "created_at",
      "expires_at",
      "last_used_at",
      "revoked_at",
      "source_refs_json",
      "ref_restrictions_json",
    ],
    custodyColumns: ["token_hash", "token_prefix"],
    keyColumns: ["tenant_ref", "token_ref"],
    orderColumn: "created_at",
  },
  forge_git_objects: {
    columns: [
      "tenant_ref",
      "repository_ref",
      "object_id",
      "object_format",
      "packfile_ref",
      "packfile_sha256",
      "first_seen_at",
      "latest_seen_at",
      "source_refs_json",
    ],
    keyColumns: ["tenant_ref", "repository_ref", "object_id"],
    orderColumn: "latest_seen_at",
  },
  forge_git_packfile_archives: {
    columns: [
      "tenant_ref",
      "packfile_ref",
      "repository_ref",
      "change_ref",
      "receive_pack_ref",
      "artifact_r2_key",
      "packfile_sha256",
      "packfile_bytes",
      "object_format",
      "command_count",
      "capabilities_json",
      "ref_updates_json",
      "source_refs_json",
      "content_type",
      "visibility",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["tenant_ref", "packfile_ref"],
    orderColumn: "created_at",
  },
  forge_git_receive_pack_intakes: {
    columns: [
      "tenant_ref",
      "receive_pack_ref",
      "repository_ref",
      "token_ref",
      "subject_ref",
      "change_ref",
      "packfile_ref",
      "packfile_sha256",
      "packfile_bytes",
      "object_format",
      "state",
      "command_count",
      "ref_updates_json",
      "source_refs_json",
      "rejection_code",
      "rejection_reason",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["tenant_ref", "receive_pack_ref"],
    orderColumn: "created_at",
  },
  forge_git_ref_locks: {
    columns: [
      "tenant_ref",
      "lock_ref",
      "repository_ref",
      "ref_name",
      "receive_pack_ref",
      "expected_old_object_id",
      "new_object_id",
      "action",
      "state",
      "acquired_at",
      "released_at",
      "source_refs_json",
    ],
    keyColumns: ["tenant_ref", "lock_ref"],
    orderColumn: "acquired_at",
  },
  forge_git_refs: {
    columns: [
      "tenant_ref",
      "repository_ref",
      "ref_name",
      "object_id",
      "previous_object_id",
      "object_format",
      "state",
      "updated_by_change_ref",
      "updated_by_packfile_ref",
      "updated_by_receive_pack_ref",
      "source_refs_json",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["tenant_ref", "repository_ref", "ref_name"],
    orderColumn: "updated_at",
  },
  forge_github_mirror_receipts: {
    columns: [
      "tenant_ref",
      "mirror_ref",
      "promotion_ref",
      "change_ref",
      "repository_ref",
      "source_canonical_ref",
      "destination_github_repository",
      "destination_github_ref",
      "commit_id",
      "status",
      "attempt_count",
      "first_attempted_at",
      "last_attempted_at",
      "completed_at",
      "refusal_reason",
      "error_reason",
      "source_refs_json",
      "redacted",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["tenant_ref", "mirror_ref"],
    orderColumn: "updated_at",
  },
  forge_merge_queue_ledger: {
    columns: [
      "tenant_ref",
      "queue_ref",
      "base_head",
      "actual_head",
      "virtual_head",
      "state",
      "next_promotion_ref",
      "ready_json",
      "blocked_json",
      "source_refs_json",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["tenant_ref", "queue_ref"],
    orderColumn: "updated_at",
  },
  forge_promotion_decisions: {
    columns: [
      "tenant_ref",
      "promotion_ref",
      "queue_ref",
      "change_ref",
      "decision",
      "base_head",
      "candidate_head",
      "promoted_head",
      "verification_ref",
      "gate_refs_json",
      "blocker_refs_json",
      "decided_by_ref",
      "decided_at",
      "source_refs_json",
      "redacted",
      "created_at",
      "target_ref",
      "queue_position",
      "gate_results_json",
    ],
    keyColumns: ["tenant_ref", "promotion_ref"],
    orderColumn: "decided_at",
  },
  forge_tenants: {
    columns: [
      "tenant_ref",
      "display_name",
      "state",
      "created_at",
      "updated_at",
      "confidential_workspace_mode",
      "attestation_ref",
      "encrypted_knowledge_pack_ref",
      "refusal_reason",
      "retention_policy_ref",
    ],
    keyColumns: ["tenant_ref"],
    orderColumn: "updated_at",
  },
  forge_verification_receipts: {
    columns: [
      "tenant_ref",
      "verification_ref",
      "change_ref",
      "repository_ref",
      "base_ref",
      "base_head",
      "head_ref",
      "head_head",
      "packfile_ref",
      "packfile_sha256",
      "executor_identity_ref",
      "command_ref",
      "command_args_json",
      "exit_code",
      "verdict",
      "started_at",
      "completed_at",
      "artifact_refs_json",
      "log_sha256",
      "source_refs_json",
      "redacted",
      "created_at",
    ],
    keyColumns: ["tenant_ref", "verification_ref"],
    orderColumn: "completed_at",
  },
}

export const isForgeDomainTable = (
  value: string,
): value is ForgeDomainTable =>
  Object.prototype.hasOwnProperty.call(FORGE_DOMAIN_TABLE_SPECS, value)

export const normalizeForgeDomainValue = (
  value: unknown,
): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "bigint") return value.toString()
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

export type ForgeDomainUnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)` for
 * dynamic-text parameterized statements; the structural `SyncSql` seam
 * deliberately does not, so this module widens it locally (the same
 * discipline as the other KS-8 backfill cores). Every statement built
 * from this registry is ONE parameterized statement whose dynamic text
 * comes only from compile-time table specs — Hyperdrive
 * transaction-mode safe.
 */
export const requireForgeDomainUnsafe = (
  sql: SyncSql,
): ForgeDomainUnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: ForgeDomainUnsafeQuery }).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "forge domain store requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/**
 * Converge Postgres to the given D1 snapshot rows: full-row
 * `ON CONFLICT (composite PK) DO UPDATE` upserts. Idempotent — re-running
 * the same rows converges to the identical state; the mirror can never
 * invent a ref tip, a lease state, or a token row: it only copies what
 * the D1 authority already holds.
 */
export const upsertForgeDomainRows = async (
  sql: SyncSql | SyncTransactionSql,
  table: ForgeDomainTable,
  rows: ReadonlyArray<ForgeDomainRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireForgeDomainUnsafe(sql as SyncSql)
  const spec = FORGE_DOMAIN_TABLE_SPECS[table]
  const setClauses = spec.columns
    .filter((column) => !spec.keyColumns.includes(column))
    .map((column) => `${column} = EXCLUDED.${column}`)
    .join(", ")
  const updateClause =
    setClauses.length === 0 ? "DO NOTHING" : `DO UPDATE SET ${setClauses}`

  let touched = 0
  for (const row of rows) {
    const values = spec.columns.map((column) =>
      normalizeForgeDomainValue(row[column]),
    )
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${spec.columns.join(", ")}) VALUES (${placeholders}) ON CONFLICT (${spec.keyColumns.join(", ")}) ${updateClause} RETURNING 1 AS touched`,
      values as Array<unknown>,
    )
    touched += result.length
  }
  return touched
}
