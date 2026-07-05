/**
 * KS-8.18 (#8329): Identity and auth core backfill + verification core —
 * D1 → Postgres. The LAST and most sensitive KS-8 domain.
 *
 * Testable core behind `scripts/backfill-identity-auth.ts`, following the
 * KS-8.16 template (`forge-backfill.ts`). Takes raw D1 rows (snake_case
 * objects, exactly as `wrangler d1 execute --json` returns them) and
 * converges them into the Postgres twins from migration
 * `0028_identity_auth_domain.sql` via the SHARED registry in
 * `./identity-auth-domain-tables.ts` (the same `upsertIdentityAuthRows`
 * the Worker's dual-write mirror uses — backfill and mirror can never
 * fight because they write identical converge upserts keyed on the
 * composite PK).
 *
 * Verification (`verify*`), matching the §3.15 acceptance at the storage
 * layer:
 *   - exact row counts per table (identity SET EQUALITY: users +
 *     auth_identities counts and newest-row hashes);
 *   - domain scalar tallies (status/health/kind counts, active-grant and
 *     active-lease tallies) — every tally is CUSTODY-SAFE by construction
 *     (no tally selects a ciphertext / value_json / user_code / state
 *     column value);
 *   - newest-N full row hashes per table.
 *
 * SECRETS (SPEC invariant 9): output references row KEYS (ids / refs /
 * owner_user_id) and sha256 hashes only. Custody columns
 * (token ciphertext + IVs + key ids, openauth value_json, device
 * user_code, OAuth state nonce) participate in row hashes — a hash, never
 * the value — and are NEVER selected into a tally or printed. Raw tokens
 * exist on neither engine.
 */

import { createHash } from "node:crypto"
import {
  IDENTITY_AUTH_DOMAIN_TABLE_SPECS,
  IDENTITY_AUTH_DOMAIN_TABLES,
  isIdentityAuthDomainTable,
  normalizeIdentityAuthValue,
  requireIdentityAuthUnsafe,
  upsertIdentityAuthRows,
  type IdentityAuthDomainRow,
  type IdentityAuthDomainTable,
} from "./identity-auth-domain-tables.js"
import type { SyncSql } from "./sql.js"

export {
  IDENTITY_AUTH_DOMAIN_TABLE_SPECS,
  IDENTITY_AUTH_DOMAIN_TABLES,
  isIdentityAuthDomainTable,
  upsertIdentityAuthRows,
  type IdentityAuthDomainRow,
  type IdentityAuthDomainTable,
}

export type D1IdentityAuthSourceRow = IdentityAuthDomainRow

// ---------------------------------------------------------------------------
// Row hashes
// ---------------------------------------------------------------------------

/**
 * Canonical row hash: the registry-order column values joined with unit
 * separators, sha256'd. Normalization matches `upsertIdentityAuthRows`,
 * so the SAME D1 export row and its Postgres twin hash identically
 * (bigint counters come back as strings from postgres.js; `String()`
 * canonicalizes both sides). Custody columns are hashed as bytes but the
 * hash — never the value — is what surfaces.
 */
export const identityAuthRowHash = (
  table: IdentityAuthDomainTable,
  row: D1IdentityAuthSourceRow,
): string => {
  const columns = IDENTITY_AUTH_DOMAIN_TABLE_SPECS[table].columns
  const hash = createHash("sha256")
  for (const column of columns) {
    const value = normalizeIdentityAuthValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type IdentityAuthNewestRowHash = Readonly<{ key: string; hash: string }>

/** Public-safe row key: the composite PK values joined with '/'. PK
 * columns are never custody-bearing, so keys are always safe to print. */
export const identityAuthRowKey = (
  table: IdentityAuthDomainTable,
  row: D1IdentityAuthSourceRow,
): string =>
  IDENTITY_AUTH_DOMAIN_TABLE_SPECS[table].keyColumns
    .map((column) => String(row[column] ?? "<null>"))
    .join("/")

export const d1IdentityAuthNewestHashes = (
  table: IdentityAuthDomainTable,
  rows: ReadonlyArray<D1IdentityAuthSourceRow>,
): ReadonlyArray<IdentityAuthNewestRowHash> =>
  rows.map((row) => ({
    hash: identityAuthRowHash(table, row),
    key: identityAuthRowKey(table, row),
  }))

/** Newest-first ORDER BY clause for a table (order column, then PK). */
export const identityAuthNewestOrderSql = (
  table: IdentityAuthDomainTable,
): string => {
  const spec = IDENTITY_AUTH_DOMAIN_TABLE_SPECS[table]
  const keys = spec.keyColumns.map((column) => `${column} DESC`).join(", ")
  return `${spec.orderColumn} DESC, ${keys}`
}

export const postgresIdentityAuthNewestHashes = async (
  sql: SyncSql,
  table: IdentityAuthDomainTable,
  limit: number,
): Promise<ReadonlyArray<IdentityAuthNewestRowHash>> => {
  const unsafe = requireIdentityAuthUnsafe(sql)
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${identityAuthNewestOrderSql(table)} LIMIT $1`,
    [limit],
  )
  return rows.map((row) => ({
    hash: identityAuthRowHash(table, row),
    key: identityAuthRowKey(table, row),
  }))
}

// ---------------------------------------------------------------------------
// Counts and scalar tallies
// ---------------------------------------------------------------------------

export const postgresIdentityAuthRowCount = async (
  sql: SyncSql,
  table: IdentityAuthDomainTable,
): Promise<number> => {
  const unsafe = requireIdentityAuthUnsafe(sql)
  const rows = await unsafe(`SELECT COUNT(*) AS total_rows FROM ${table}`, [])
  return Number(rows[0]?.["total_rows"] ?? 0)
}

/**
 * Domain scalar tallies per table (compared exactly across stores). The
 * SQL text is portable and runs verbatim on D1 AND Postgres so both sides
 * compute the same numbers over the same rows. NO tally selects a custody
 * column value — only counts over status/health/kind/classification.
 */
export const IDENTITY_AUTH_SCALAR_TALLIES: Readonly<
  Record<
    IdentityAuthDomainTable,
    ReadonlyArray<Readonly<{ metric: string; sql: string }>>
  >
> = {
  users: [
    {
      metric: "active_users",
      sql: `SELECT COUNT(*) AS value FROM users WHERE status = 'active'`,
    },
    {
      metric: "agent_users",
      sql: `SELECT COUNT(*) AS value FROM users WHERE kind = 'agent'`,
    },
    {
      metric: "deleted_users",
      sql: `SELECT COUNT(*) AS value FROM users WHERE deleted_at IS NOT NULL`,
    },
  ],
  auth_identities: [
    {
      metric: "distinct_providers",
      sql: `SELECT COUNT(DISTINCT provider) AS value FROM auth_identities`,
    },
    {
      metric: "distinct_users",
      sql: `SELECT COUNT(DISTINCT user_id) AS value FROM auth_identities`,
    },
    {
      metric: "deleted_identities",
      sql: `SELECT COUNT(*) AS value FROM auth_identities WHERE deleted_at IS NOT NULL`,
    },
  ],
  openauth_storage: [
    // value_json is CUSTODY — never selected; count live vs expired rows.
    {
      metric: "with_expiry",
      sql: `SELECT COUNT(*) AS value FROM openauth_storage WHERE expires_at IS NOT NULL`,
    },
  ],
  openauth_agent_links: [
    {
      metric: "active_links",
      sql: `SELECT COUNT(*) AS value FROM openauth_agent_links WHERE status = 'active'`,
    },
    {
      metric: "distinct_owners",
      sql: `SELECT COUNT(DISTINCT openauth_user_id) AS value FROM openauth_agent_links`,
    },
  ],
  github_write_connections: [
    {
      metric: "connected",
      sql: `SELECT COUNT(*) AS value FROM github_write_connections WHERE status = 'connected'`,
    },
    {
      metric: "healthy",
      sql: `SELECT COUNT(*) AS value FROM github_write_connections WHERE health = 'healthy'`,
    },
    {
      metric: "distinct_users",
      sql: `SELECT COUNT(DISTINCT user_id) AS value FROM github_write_connections`,
    },
  ],
  github_write_connection_attempts: [
    // `state` is CUSTODY — never selected; count lifecycle states only.
    {
      metric: "pending",
      sql: `SELECT COUNT(*) AS value FROM github_write_connection_attempts WHERE status = 'pending'`,
    },
    {
      metric: "connected",
      sql: `SELECT COUNT(*) AS value FROM github_write_connection_attempts WHERE status = 'connected'`,
    },
  ],
  github_write_auth_grants: [
    {
      metric: "issued_grants",
      sql: `SELECT COUNT(*) AS value FROM github_write_auth_grants WHERE status = 'issued'`,
    },
    {
      metric: "used_grants",
      sql: `SELECT COUNT(*) AS value FROM github_write_auth_grants WHERE status = 'used'`,
    },
    {
      metric: "revoked_grants",
      sql: `SELECT COUNT(*) AS value FROM github_write_auth_grants WHERE status = 'revoked'`,
    },
  ],
  provider_accounts: [
    {
      metric: "connected_accounts",
      sql: `SELECT COUNT(*) AS value FROM provider_accounts WHERE status = 'connected'`,
    },
    {
      metric: "healthy_accounts",
      sql: `SELECT COUNT(*) AS value FROM provider_accounts WHERE health = 'healthy'`,
    },
    {
      metric: "sum_lease_limit",
      sql: `SELECT COALESCE(SUM(lease_limit), 0) AS value FROM provider_accounts`,
    },
  ],
  provider_account_connection_attempts: [
    // user_code is CUSTODY — never selected; lifecycle counts only.
    {
      metric: "pending",
      sql: `SELECT COUNT(*) AS value FROM provider_account_connection_attempts WHERE status = 'pending'`,
    },
    {
      metric: "connected",
      sql: `SELECT COUNT(*) AS value FROM provider_account_connection_attempts WHERE status = 'connected'`,
    },
  ],
  provider_account_auth_grants: [
    {
      metric: "issued_grants",
      sql: `SELECT COUNT(*) AS value FROM provider_account_auth_grants WHERE status = 'issued'`,
    },
    {
      metric: "used_grants",
      sql: `SELECT COUNT(*) AS value FROM provider_account_auth_grants WHERE status = 'used'`,
    },
    {
      metric: "distinct_accounts",
      sql: `SELECT COUNT(DISTINCT provider_account_id) AS value FROM provider_account_auth_grants`,
    },
  ],
  provider_account_events: [
    {
      metric: "distinct_kinds",
      sql: `SELECT COUNT(DISTINCT kind) AS value FROM provider_account_events`,
    },
    {
      metric: "distinct_users",
      sql: `SELECT COUNT(DISTINCT user_id) AS value FROM provider_account_events`,
    },
  ],
  provider_account_sanity_checks: [
    {
      metric: "healthy_checks",
      sql: `SELECT COUNT(*) AS value FROM provider_account_sanity_checks WHERE classification = 'healthy'`,
    },
    {
      metric: "distinct_accounts",
      sql: `SELECT COUNT(DISTINCT provider_account_id) AS value FROM provider_account_sanity_checks`,
    },
  ],
  provider_account_parallel_probe_receipts: [
    {
      metric: "passed_receipts",
      sql: `SELECT COUNT(*) AS value FROM provider_account_parallel_probe_receipts WHERE terminal_status = 'passed'`,
    },
    {
      metric: "failed_receipts",
      sql: `SELECT COUNT(*) AS value FROM provider_account_parallel_probe_receipts WHERE terminal_status = 'failed'`,
    },
  ],
  provider_account_leases: [
    {
      metric: "active_leases",
      sql: `SELECT COUNT(*) AS value FROM provider_account_leases WHERE status = 'active'`,
    },
    {
      metric: "succeeded_leases",
      sql: `SELECT COUNT(*) AS value FROM provider_account_leases WHERE status = 'succeeded'`,
    },
    {
      metric: "distinct_accounts",
      sql: `SELECT COUNT(DISTINCT provider_account_id) AS value FROM provider_account_leases`,
    },
  ],
  provider_account_failover_receipts: [
    {
      metric: "retrying",
      sql: `SELECT COUNT(*) AS value FROM provider_account_failover_receipts WHERE outcome = 'retrying'`,
    },
    {
      metric: "blocked",
      sql: `SELECT COUNT(*) AS value FROM provider_account_failover_receipts WHERE outcome = 'blocked'`,
    },
    {
      metric: "sum_attempts",
      sql: `SELECT COALESCE(SUM(attempt_number), 0) AS value FROM provider_account_failover_receipts`,
    },
  ],
  provider_account_token_custody: [
    // Custody-safe tallies only: never selects ciphertext/IV/key columns.
    {
      metric: "distinct_owners",
      sql: `SELECT COUNT(DISTINCT owner_user_id) AS value FROM provider_account_token_custody`,
    },
    {
      // Custody-safe: counts rows with an account id — never references a
      // ciphertext / IV / key-id column.
      metric: "with_account_id",
      sql: `SELECT COUNT(*) AS value FROM provider_account_token_custody WHERE account_id IS NOT NULL`,
    },
  ],
  provider_account_token_custody_audit: [
    {
      metric: "succeeded_events",
      sql: `SELECT COUNT(*) AS value FROM provider_account_token_custody_audit WHERE status = 'succeeded'`,
    },
    {
      metric: "failed_events",
      sql: `SELECT COUNT(*) AS value FROM provider_account_token_custody_audit WHERE status = 'failed'`,
    },
  ],
}

export const postgresIdentityAuthScalar = async (
  sql: SyncSql,
  tallySql: string,
): Promise<number> => {
  const unsafe = requireIdentityAuthUnsafe(sql)
  const rows = await unsafe(tallySql, [])
  return Number(rows[0]?.["value"] ?? 0)
}

// ---------------------------------------------------------------------------
// Verify report
// ---------------------------------------------------------------------------

export type IdentityAuthScalarMismatch = Readonly<{
  metric: string
  d1: number
  postgres: number
}>

export type IdentityAuthNewestHashMismatch = Readonly<{
  key: string
  d1Hash: string | undefined
  postgresHash: string | undefined
}>

export type IdentityAuthVerifyReport = Readonly<{
  table: IdentityAuthDomainTable
  d1Total: number
  postgresTotal: number
  countsMatch: boolean
  scalarMismatches: ReadonlyArray<IdentityAuthScalarMismatch>
  newestHashMismatches: ReadonlyArray<IdentityAuthNewestHashMismatch>
}>

export const buildIdentityAuthVerifyReport = (input: {
  table: IdentityAuthDomainTable
  d1Total: number
  postgresTotal: number
  scalars: ReadonlyArray<{ metric: string; d1: number; postgres: number }>
  d1Newest: ReadonlyArray<IdentityAuthNewestRowHash>
  postgresNewest: ReadonlyArray<IdentityAuthNewestRowHash>
}): IdentityAuthVerifyReport => {
  const scalarMismatches = input.scalars.filter(
    (scalar) => scalar.d1 !== scalar.postgres,
  )
  const postgresByKey = new Map(
    input.postgresNewest.map((entry) => [entry.key, entry.hash]),
  )
  const newestHashMismatches: Array<IdentityAuthNewestHashMismatch> = []
  for (const entry of input.d1Newest) {
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
    countsMatch: input.d1Total === input.postgresTotal,
    d1Total: input.d1Total,
    newestHashMismatches,
    postgresTotal: input.postgresTotal,
    scalarMismatches,
    table: input.table,
  }
}

export const identityAuthVerifyReportClean = (
  report: IdentityAuthVerifyReport,
): boolean =>
  report.countsMatch &&
  report.scalarMismatches.length === 0 &&
  report.newestHashMismatches.length === 0
