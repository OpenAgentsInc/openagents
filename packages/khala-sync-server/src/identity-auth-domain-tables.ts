import type { SyncSql, SyncTransactionSql } from "./sql.js"

/**
 * KS-8.18 (#8329): Identity and auth core domain — the LAST and most
 * sensitive KS-8 domain.
 *
 * Shared table metadata and Postgres converge/upsert helpers for the
 * SEVENTEEN canonical identity/auth tables moving from D1 to Cloud SQL
 * (khala-sync migration `0027_identity_auth_domain.sql`, mirroring worker
 * migrations 0002/0003/0004/0009/0011/0044-0050/0173/0234/0237/0283). This
 * file is imported by both the Worker dual-write mirror
 * (`apps/openagents.com/workers/api/src/identity-auth-domain-store.ts`) and
 * the backfill/verify CLI (`scripts/backfill-identity-auth.ts`) — one
 * registry, so mirror and backfill can never fight: both write identical
 * converge upserts keyed on the table's (composite) primary key.
 *
 * SCOPE — canonical tables only. The provider_account rebuild history left
 * transient artifact tables (`provider_accounts_0173_new`,
 * `provider_account_*_0173_data`, `provider_account_connection_attempts_0237_data`)
 * that are DROPped inside the same worker migration that creates them.
 * They are deliberately NOT twinned here.
 *
 * SECRETS (SPEC invariant 9 — this is the domain the invariant was written
 * for). Several tables carry secret-bearing material. The Postgres twin
 * stores EXACTLY what D1 stores, with NO widening and the same at-rest
 * encryption posture as today:
 *   - `provider_account_token_custody` holds ENCRYPTED token material
 *     (refresh/access/id-token ciphertext + IVs + key ids). Raw tokens
 *     live on NEITHER engine — only ciphertext keyed by KMS key id.
 *   - `openauth_storage.value_json` is the OpenAuth session/refresh payload.
 *   - `provider_account_connection_attempts.user_code` is a device-code
 *     one-time challenge; `github_write_connection_attempts.state` is the
 *     OAuth CSRF / owner-claim challenge nonce.
 * Every such column is declared in `custodyColumns`. Callers keep these
 * OUT of diagnostics, logs, and backfill/verify output: only row KEYS
 * (ids / refs / owner_user_id) and sha256 row hashes may ever be printed
 * for those tables. Custody values participate in row hashes (a hash,
 * never the value) and are NEVER selected into a scalar tally.
 *
 * Runtime-neutral on purpose: no Node built-ins (the Worker imports this).
 */

export type IdentityAuthDomainTable =
  | "users"
  | "auth_identities"
  | "openauth_storage"
  | "openauth_agent_links"
  | "github_write_connections"
  | "github_write_connection_attempts"
  | "github_write_auth_grants"
  | "provider_accounts"
  | "provider_account_connection_attempts"
  | "provider_account_auth_grants"
  | "provider_account_events"
  | "provider_account_sanity_checks"
  | "provider_account_parallel_probe_receipts"
  | "provider_account_leases"
  | "provider_account_failover_receipts"
  | "provider_account_token_custody"
  | "provider_account_token_custody_audit"

export type IdentityAuthDomainTableSpec = Readonly<{
  columns: ReadonlyArray<string>
  /**
   * The table's (composite) PRIMARY KEY — the converge key used by the
   * backfill, the mirror read-back, and ON CONFLICT.
   */
  keyColumns: ReadonlyArray<string>
  /** Newest-first ordering column for hash verification. */
  orderColumn: string
  /**
   * Secret-bearing custody columns (encrypted token ciphertext/IVs/key
   * ids, OpenAuth session payloads, device/OAuth challenge nonces). Values
   * from these columns must NEVER appear in diagnostics, logs, or
   * backfill/verify output — keys and sha256 row hashes only.
   */
  custodyColumns?: ReadonlyArray<string>
}>

export type IdentityAuthDomainRow = Readonly<Record<string, unknown>>

/** Backfill/verify sweep order (parents-before-children is cosmetic —
 * there are no FKs on the Postgres side — but keeps output readable). */
export const IDENTITY_AUTH_DOMAIN_TABLES: ReadonlyArray<IdentityAuthDomainTable> =
  [
    "users",
    "auth_identities",
    "openauth_storage",
    "openauth_agent_links",
    "github_write_connections",
    "github_write_connection_attempts",
    "github_write_auth_grants",
    "provider_accounts",
    "provider_account_connection_attempts",
    "provider_account_auth_grants",
    "provider_account_events",
    "provider_account_sanity_checks",
    "provider_account_parallel_probe_receipts",
    "provider_account_leases",
    "provider_account_failover_receipts",
    "provider_account_token_custody",
    "provider_account_token_custody_audit",
  ]

export const IDENTITY_AUTH_DOMAIN_TABLE_SPECS: Readonly<
  Record<IdentityAuthDomainTable, IdentityAuthDomainTableSpec>
> = {
  users: {
    columns: [
      "id",
      "kind",
      "display_name",
      "primary_email",
      "avatar_url",
      "status",
      "created_at",
      "updated_at",
      "deleted_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  auth_identities: {
    columns: [
      "id",
      "user_id",
      "provider",
      "provider_subject",
      "email",
      "created_at",
      "updated_at",
      "deleted_at",
      "provider_username",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  openauth_storage: {
    columns: ["key", "value_json", "expires_at", "updated_at"],
    // value_json is the OpenAuth session/refresh payload — secret-bearing.
    custodyColumns: ["value_json"],
    keyColumns: ["key"],
    orderColumn: "updated_at",
  },
  openauth_agent_links: {
    columns: [
      "id",
      "openauth_user_id",
      "agent_user_id",
      "agent_credential_id",
      "link_kind",
      "status",
      "created_at",
      "updated_at",
      "revoked_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  github_write_connections: {
    columns: [
      "id",
      "user_id",
      "github_id",
      "github_login",
      "connection_ref",
      "secret_ref",
      "scopes_json",
      "status",
      "health",
      "connected_at",
      "disconnected_at",
      "last_status_at",
      "metadata_json",
      "created_at",
      "updated_at",
      "deleted_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  github_write_connection_attempts: {
    columns: [
      "id",
      "user_id",
      "state",
      "expected_github_id",
      "expected_github_login",
      "redirect_after",
      "scopes_json",
      "status",
      "expires_at",
      "completed_at",
      "failed_at",
      "failure_reason",
      "created_at",
      "updated_at",
    ],
    // `state` is the OAuth CSRF / owner-claim challenge nonce.
    custodyColumns: ["state"],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  github_write_auth_grants: {
    columns: [
      "id",
      "connection_id",
      "user_id",
      "runner_session_id",
      "connection_ref",
      "secret_ref",
      "grant_ref",
      "status",
      "requested_action",
      "metadata_json",
      "created_at",
      "updated_at",
      "expires_at",
      "used_at",
      "revoked_at",
      "failed_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  provider_accounts: {
    columns: [
      "id",
      "user_id",
      "team_id",
      "provider",
      "auth_mode",
      "status",
      "health",
      "provider_account_ref",
      "secret_ref",
      "account_label",
      "plan_type",
      "connected_at",
      "disconnected_at",
      "denied_at",
      "last_status_at",
      "metadata_json",
      "created_at",
      "updated_at",
      "deleted_at",
      "last_sanity_check_at",
      "last_sanity_check_result",
      "operator_priority",
      "cooldown_until",
      "low_credit_flag",
      "recent_failure_class",
      "last_selected_at",
      "operator_label",
      "lease_limit",
      "last_parallel_probe_at",
      "last_parallel_probe_result",
      "last_successful_launch_at",
      "last_failed_launch_at",
      "reauth_required_reason",
      "operator_note",
      "refill_note",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  provider_account_connection_attempts: {
    columns: [
      "id",
      "provider_account_id",
      "user_id",
      "team_id",
      "provider",
      "method",
      "source",
      "login_ref",
      "verification_url",
      "user_code",
      "status",
      "expires_at",
      "completed_at",
      "failed_at",
      "metadata_json",
      "created_at",
      "updated_at",
    ],
    // user_code is the device-code one-time challenge shown to the user.
    custodyColumns: ["user_code"],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  provider_account_auth_grants: {
    columns: [
      "id",
      "provider_account_id",
      "user_id",
      "team_id",
      "thread_id",
      "workroom_id",
      "runner_session_id",
      "provider",
      "provider_account_ref",
      "provider_secret_ref",
      "grant_ref",
      "status",
      "requested_action",
      "metadata_json",
      "created_at",
      "updated_at",
      "expires_at",
      "used_at",
      "revoked_at",
      "failed_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  provider_account_events: {
    columns: [
      "id",
      "provider_account_id",
      "auth_grant_id",
      "user_id",
      "team_id",
      "thread_id",
      "workroom_id",
      "runner_session_id",
      "kind",
      "summary",
      "source_refs_json",
      "evidence_refs_json",
      "target_ref",
      "metadata_json",
      "actor_id",
      "created_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  provider_account_sanity_checks: {
    columns: [
      "id",
      "provider_account_id",
      "user_id",
      "team_id",
      "provider",
      "provider_account_ref",
      "classification",
      "summary",
      "grant_ref",
      "created_at",
      "metadata_json",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  provider_account_parallel_probe_receipts: {
    columns: [
      "id",
      "probe_run_id",
      "probe_id",
      "lease_id",
      "provider_account_id",
      "user_id",
      "team_id",
      "provider_account_ref",
      "started_at",
      "finished_at",
      "terminal_status",
      "classification",
      "collision_class",
      "metadata_json",
    ],
    keyColumns: ["id"],
    orderColumn: "started_at",
  },
  provider_account_leases: {
    columns: [
      "id",
      "lease_ref",
      "provider_account_id",
      "user_id",
      "team_id",
      "provider",
      "provider_account_ref",
      "requested_action",
      "run_id",
      "assignment_id",
      "selected_by_policy_version",
      "selection_reason",
      "status",
      "started_at",
      "expires_at",
      "released_at",
      "terminal_outcome",
      "metadata_json",
      "order_id",
      "selected_by_actor",
      "last_touched_at",
      "failure_class",
    ],
    keyColumns: ["id"],
    orderColumn: "started_at",
  },
  provider_account_failover_receipts: {
    columns: [
      "id",
      "run_id",
      "assignment_id",
      "requested_action",
      "previous_lease_ref",
      "previous_provider_account_ref",
      "next_lease_ref",
      "next_provider_account_ref",
      "failure_class",
      "account_state_action",
      "outcome",
      "attempt_number",
      "max_attempts",
      "customer_safe_status",
      "created_at",
      "metadata_json",
      "order_id",
      "policy_version",
      "cooldown_until",
      "operator_summary",
      "customer_safe_summary",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  provider_account_token_custody: {
    columns: [
      "provider_account_ref",
      "owner_user_id",
      "provider",
      "secret_ref",
      "refresh_ciphertext_b64",
      "refresh_iv_b64",
      "refresh_key_id",
      "access_ciphertext_b64",
      "access_iv_b64",
      "access_key_id",
      "access_expires_at",
      "account_id",
      "id_token_ciphertext_b64",
      "id_token_iv_b64",
      "id_token_key_id",
      "created_at",
      "updated_at",
      "last_refreshed_at",
    ],
    // ENCRYPTED token material — the most secret-bearing table in the
    // system. Ciphertext + IVs + KMS key ids never leave a row hash.
    custodyColumns: [
      "refresh_ciphertext_b64",
      "refresh_iv_b64",
      "refresh_key_id",
      "access_ciphertext_b64",
      "access_iv_b64",
      "access_key_id",
      "id_token_ciphertext_b64",
      "id_token_iv_b64",
      "id_token_key_id",
    ],
    keyColumns: ["provider_account_ref"],
    orderColumn: "updated_at",
  },
  provider_account_token_custody_audit: {
    columns: [
      "id",
      "provider_account_ref",
      "owner_user_id",
      "provider",
      "event_kind",
      "status",
      "actor_ref",
      "source_ref",
      "error_tag",
      "error_message",
      "metadata_json",
      "created_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
}

export const isIdentityAuthDomainTable = (
  value: string,
): value is IdentityAuthDomainTable =>
  Object.prototype.hasOwnProperty.call(
    IDENTITY_AUTH_DOMAIN_TABLE_SPECS,
    value,
  )

/** True when the column holds secret-bearing custody material. */
export const isIdentityAuthCustodyColumn = (
  table: IdentityAuthDomainTable,
  column: string,
): boolean =>
  (IDENTITY_AUTH_DOMAIN_TABLE_SPECS[table].custodyColumns ?? []).includes(
    column,
  )

export const normalizeIdentityAuthValue = (
  value: unknown,
): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "bigint") return value.toString()
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

export type IdentityAuthUnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)` for
 * dynamic-text parameterized statements; the structural `SyncSql` seam
 * deliberately does not, so this module widens it locally (the same
 * discipline as the other KS-8 backfill cores). Every statement built
 * from this registry is ONE parameterized statement whose dynamic text
 * comes only from compile-time table specs — Hyperdrive transaction-mode
 * safe.
 */
export const requireIdentityAuthUnsafe = (
  sql: SyncSql,
): IdentityAuthUnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: IdentityAuthUnsafeQuery }).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "identity/auth domain store requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/**
 * Converge Postgres to the given D1 snapshot rows: full-row
 * `ON CONFLICT (composite PK) DO UPDATE` upserts. Idempotent — re-running
 * the same rows converges to the identical state; the mirror can never
 * invent a session, a grant, or a token row: it only copies what the D1
 * authority already holds.
 */
export const upsertIdentityAuthRows = async (
  sql: SyncSql | SyncTransactionSql,
  table: IdentityAuthDomainTable,
  rows: ReadonlyArray<IdentityAuthDomainRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireIdentityAuthUnsafe(sql as SyncSql)
  const spec = IDENTITY_AUTH_DOMAIN_TABLE_SPECS[table]
  const setClauses = spec.columns
    .filter((column) => !spec.keyColumns.includes(column))
    .map((column) => `${column} = EXCLUDED.${column}`)
    .join(", ")
  const updateClause =
    setClauses.length === 0 ? "DO NOTHING" : `DO UPDATE SET ${setClauses}`

  let touched = 0
  for (const row of rows) {
    const values = spec.columns.map((column) =>
      normalizeIdentityAuthValue(row[column]),
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
