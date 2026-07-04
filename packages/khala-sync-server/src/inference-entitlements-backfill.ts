/**
 * KS-8.9 (#8320): inference entitlements backfill core — D1 → Postgres.
 *
 * Testable core behind `scripts/backfill-inference-entitlements.ts`,
 * following the KS-8.1/KS-8.2 templates (`pylon-backfill.ts`,
 * `token-ledger-backfill.ts`) generalized for a WIDE domain: a table
 * registry drives one page/upsert/verify engine across all 29 tables from
 * migration `0013_inference_entitlements.sql`.
 *
 * Conflict handling per table:
 *   - EVENT tables (usage events, quota events, receipts, splits, sources)
 *     use `ON CONFLICT DO NOTHING` — the dual-write mirror writes
 *     byte-identical rows, so backfill and mirror never fight and a re-run
 *     inserts zero rows (the idempotency contract).
 *   - STATE tables (tallies, grants, entitlement markers, jobs, counters,
 *     challenge/receipt/entitlement chains with mutable status/archived_at)
 *     CONVERGE (`ON CONFLICT ... DO UPDATE SET` = the D1 snapshot value) —
 *     D1 stays the write authority through the whole dual-write window, so
 *     "converge to D1" is correct, and the today's-row converge race
 *     against a concurrent mirror increment is exactly what the runbook's
 *     second catch-up sweep + `--verify` close.
 *
 * Verification (the §3.6 acceptance):
 *   - exact row counts per table,
 *   - per-group tallies on a bounded domain column per table (plan/scope/
 *     state/status/kind — the "per-plan" evidence),
 *   - newest-N row-hash comparison,
 *   - TALLY = SUM(EVENTS) per key for the three enforcement tally families
 *     (`inference_free_tier_usage` / `inference_free_usage_tally` /
 *     `inference_earned_allowance`) on the POSTGRES side — a lost
 *     increment is a free-tier leak, a doubled one a false denial, so
 *     nothing "close enough": exact or explain.
 */

import { createHash } from "node:crypto"
import type { SyncSql } from "./sql.js"

// ---------------------------------------------------------------------------
// Table registry (column lists mirror migration 0013 exactly)
// ---------------------------------------------------------------------------

export type EntitlementsTableSpec = Readonly<{
  columns: ReadonlyArray<string>
  /** "nothing" (event ledger) or converge keys (state tables). */
  conflict:
    | Readonly<{ mode: "nothing" }>
    | Readonly<{ mode: "converge"; keyColumns: ReadonlyArray<string> }>
  /** Natural key used for newest-N hash comparison output. */
  keyColumns: ReadonlyArray<string>
  /** Newest-first ordering column (for the hash sample). */
  orderColumn: string
  /** Bounded-cardinality column for per-group ("per-plan") tallies. */
  groupColumn?: string
}>

export const INFERENCE_ENTITLEMENTS_TABLES: Readonly<
  Record<string, EntitlementsTableSpec>
> = {
  /* eslint-disable sort-keys */
  inference_free_usage_tally: {
    columns: [
      "owner_key", "identity_kind", "cumulative_free_usd_micros",
      "free_request_count", "created_at", "updated_at",
    ],
    conflict: { mode: "converge", keyColumns: ["owner_key"] },
    keyColumns: ["owner_key"],
    orderColumn: "updated_at",
    groupColumn: "identity_kind",
  },
  inference_free_usage_events: {
    columns: [
      "request_id", "owner_key", "account_ref", "served_model",
      "free_usd_micros", "created_at",
    ],
    conflict: { mode: "nothing" },
    keyColumns: ["request_id"],
    orderColumn: "created_at",
    groupColumn: "served_model",
  },
  inference_premium_allowlist: {
    columns: [
      "owner_key", "scope", "granted_by", "note", "created_at", "updated_at",
    ],
    conflict: { mode: "converge", keyColumns: ["owner_key"] },
    keyColumns: ["owner_key"],
    orderColumn: "updated_at",
    groupColumn: "scope",
  },
  inference_earned_allowance: {
    columns: [
      "owner_key", "earned_free_usd_micros", "accrual_count", "created_at",
      "updated_at",
    ],
    conflict: { mode: "converge", keyColumns: ["owner_key"] },
    keyColumns: ["owner_key"],
    orderColumn: "updated_at",
  },
  inference_earned_allowance_events: {
    columns: [
      "accrual_event_ref", "owner_key", "accrual_kind", "earned_usd_micros",
      "created_at",
    ],
    conflict: { mode: "nothing" },
    keyColumns: ["accrual_event_ref"],
    orderColumn: "created_at",
    groupColumn: "accrual_kind",
  },
  inference_batch_jobs: {
    columns: [
      "job_id", "account_ref", "status", "charge_receipt_ref",
      "dataset_size", "processed_items", "failed_items", "results_r2_key",
      "created_at", "updated_at", "enqueued_at", "started_at",
    ],
    conflict: { mode: "converge", keyColumns: ["job_id"] },
    keyColumns: ["job_id"],
    orderColumn: "created_at",
    groupColumn: "status",
  },
  inference_operator_exemption: {
    columns: [
      "owner_key", "scope", "granted_by", "note", "created_at", "updated_at",
    ],
    conflict: { mode: "converge", keyColumns: ["owner_key"] },
    keyColumns: ["owner_key"],
    orderColumn: "updated_at",
    groupColumn: "scope",
  },
  inference_free_tier_keys: {
    columns: [
      "account_ref", "scope", "mint_source", "note", "created_at",
      "updated_at",
    ],
    conflict: { mode: "converge", keyColumns: ["account_ref"] },
    keyColumns: ["account_ref"],
    orderColumn: "updated_at",
    groupColumn: "scope",
  },
  inference_free_tier_usage: {
    columns: [
      "account_ref", "usage_day", "free_request_count", "free_total_tokens",
      "created_at", "updated_at",
    ],
    conflict: { mode: "converge", keyColumns: ["account_ref", "usage_day"] },
    keyColumns: ["account_ref", "usage_day"],
    orderColumn: "usage_day",
    groupColumn: "usage_day",
  },
  inference_free_tier_usage_events: {
    columns: [
      "request_id", "account_ref", "usage_day", "served_model",
      "total_tokens", "created_at",
    ],
    conflict: { mode: "nothing" },
    keyColumns: ["request_id"],
    orderColumn: "created_at",
    groupColumn: "usage_day",
  },
  inference_free_key_mints: {
    columns: [
      "ip_hash", "mint_day", "mint_count", "created_at", "updated_at",
    ],
    conflict: { mode: "converge", keyColumns: ["ip_hash", "mint_day"] },
    keyColumns: ["ip_hash", "mint_day"],
    orderColumn: "mint_day",
    groupColumn: "mint_day",
  },
  inference_privacy_entitlements: {
    columns: [
      "account_ref", "privacy_tier", "note", "created_at", "updated_at",
    ],
    conflict: { mode: "converge", keyColumns: ["account_ref"] },
    keyColumns: ["account_ref"],
    orderColumn: "updated_at",
    groupColumn: "privacy_tier",
  },
  inference_privacy_entitlement_receipts: {
    columns: [
      "receipt_ref", "entitlement_ref", "account_ref", "purchase_ref",
      "idempotency_key", "privacy_tier", "capture_excluded", "reason_ref",
      "created_at", "updated_at",
    ],
    conflict: { mode: "nothing" },
    keyColumns: ["receipt_ref"],
    orderColumn: "created_at",
    groupColumn: "privacy_tier",
  },
  inference_confidential_compute_execution_receipts: {
    columns: [
      "receipt_ref", "execution_ref", "account_ref", "request_ref",
      "idempotency_key", "capture_excluded", "reason_ref", "created_at",
      "updated_at",
    ],
    conflict: { mode: "nothing" },
    keyColumns: ["receipt_ref"],
    orderColumn: "created_at",
  },
  inference_referral_margin_splits: {
    columns: [
      "id", "request_id", "account_ref", "referred_user_id",
      "referrer_user_id", "referral_attribution_id", "referral_source_id",
      "referral_invite_id", "payout_ref", "qualifying_event_ref",
      "charge_receipt_ref", "funding_kind", "adapter_id", "requested_model",
      "served_model", "served_by_contributor", "serving_node_count",
      "charge_usd", "cost_usd", "margin_usd", "margin_sats",
      "openagents_usd", "openagents_sats", "serving_node_usd",
      "serving_node_sats", "referrer_usd", "referrer_sats", "created_at",
      "archived_at",
    ],
    conflict: { mode: "nothing" },
    keyColumns: ["id"],
    orderColumn: "created_at",
    groupColumn: "funding_kind",
  },
  builtin_compute_agent_quota_events: {
    columns: [
      "id", "actor_user_id", "grant_ref", "provider", "budget_class",
      "session_units", "session_budget_seconds", "token_ceiling",
      "created_at",
    ],
    conflict: { mode: "nothing" },
    keyColumns: ["id"],
    orderColumn: "created_at",
    groupColumn: "budget_class",
  },
  orange_check_entitlements: {
    columns: [
      "id", "agent_user_id", "actor_ref", "state", "receipt_ref",
      "action_ref", "paid_amount_cents", "created_at", "updated_at",
    ],
    conflict: { mode: "converge", keyColumns: ["id"] },
    keyColumns: ["id"],
    orderColumn: "updated_at",
    groupColumn: "state",
  },
  agent_rate_limit_challenges: {
    columns: [
      "id", "idempotency_key_hash", "actor_ref", "owner_user_id",
      "route_key", "method", "path", "submission_idempotency_key_hash",
      "client_fingerprint_hash", "request_body_digest", "price_asset",
      "price_denomination", "price_value", "spend_cap_asset",
      "spend_cap_denomination", "spend_cap_value", "entitlement_kind",
      "expires_at", "public_projection_json", "created_at", "archived_at",
    ],
    conflict: { mode: "converge", keyColumns: ["id"] },
    keyColumns: ["id"],
    orderColumn: "created_at",
    groupColumn: "entitlement_kind",
  },
  agent_rate_limit_receipts: {
    columns: [
      "id", "receipt_ref", "challenge_id", "actor_ref", "owner_user_id",
      "route_key", "amount_asset", "amount_denomination", "amount_value",
      "entitlement_ref", "redacted_payment_ref", "public_projection_json",
      "created_at", "archived_at",
    ],
    conflict: { mode: "converge", keyColumns: ["id"] },
    keyColumns: ["id"],
    orderColumn: "created_at",
    groupColumn: "amount_asset",
  },
  agent_rate_limit_entitlements: {
    columns: [
      "id", "entitlement_ref", "challenge_id", "receipt_ref", "actor_ref",
      "owner_user_id", "route_key", "method", "path",
      "submission_idempotency_key_hash", "client_fingerprint_hash",
      "request_body_digest", "entitlement_kind", "status", "expires_at",
      "created_at", "consumed_at", "archived_at",
    ],
    conflict: { mode: "converge", keyColumns: ["id"] },
    keyColumns: ["id"],
    orderColumn: "created_at",
    groupColumn: "status",
  },
  agent_rate_limit_redemptions: {
    columns: [
      "id", "idempotency_key_hash", "challenge_id", "actor_ref",
      "proof_ref", "entitlement_ref", "receipt_ref", "replayed",
      "public_projection_json", "created_at", "archived_at",
    ],
    conflict: { mode: "converge", keyColumns: ["id"] },
    keyColumns: ["id"],
    orderColumn: "created_at",
    groupColumn: "replayed",
  },
  agent_search_requests: {
    columns: [
      "id", "receipt_ref", "actor_ref", "agent_user_id", "credential_id",
      "token_prefix", "idempotency_key_hash", "request_body_digest",
      "query_hash", "query_text", "mode", "provider",
      "provider_request_id", "status", "cache_status", "charge_state",
      "product_id", "entitlement_ref", "provider_cost_dollars",
      "public_projection_json", "created_at", "completed_at", "archived_at",
    ],
    conflict: { mode: "converge", keyColumns: ["id"] },
    keyColumns: ["id"],
    orderColumn: "created_at",
    groupColumn: "charge_state",
  },
  agent_search_sources: {
    columns: [
      "id", "search_request_id", "source_ref", "title", "url", "domain",
      "published_date", "score", "highlight_text", "selected_text_hash",
      "public_safe", "created_at",
    ],
    conflict: { mode: "nothing" },
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  agent_search_quota_events: {
    columns: [
      "id", "actor_ref", "credential_id", "event_kind", "mode", "units",
      "product_id", "entitlement_ref", "created_at",
    ],
    conflict: { mode: "nothing" },
    keyColumns: ["id"],
    orderColumn: "created_at",
    groupColumn: "event_kind",
  },
  agent_search_cache_entries: {
    columns: [
      "id", "cache_key", "mode", "provider", "results_json",
      "result_count", "cost_dollars", "created_at", "expires_at",
      "archived_at",
    ],
    conflict: { mode: "converge", keyColumns: ["id"] },
    keyColumns: ["id"],
    orderColumn: "created_at",
    groupColumn: "mode",
  },
  agent_search_payment_challenges: {
    columns: [
      "id", "idempotency_key_hash", "actor_ref", "agent_user_id",
      "credential_id", "token_prefix", "method", "path", "mode",
      "request_body_digest", "product_id", "price_asset",
      "price_denomination", "price_value", "spend_cap_asset",
      "spend_cap_denomination", "spend_cap_value", "expires_at",
      "public_projection_json", "created_at", "archived_at",
    ],
    conflict: { mode: "converge", keyColumns: ["id"] },
    keyColumns: ["id"],
    orderColumn: "created_at",
    groupColumn: "product_id",
  },
  agent_search_payment_receipts: {
    columns: [
      "id", "receipt_ref", "challenge_id", "actor_ref", "agent_user_id",
      "credential_id", "product_id", "amount_asset", "amount_denomination",
      "amount_value", "entitlement_ref", "redacted_payment_ref",
      "public_projection_json", "created_at", "archived_at",
    ],
    conflict: { mode: "converge", keyColumns: ["id"] },
    keyColumns: ["id"],
    orderColumn: "created_at",
    groupColumn: "product_id",
  },
  agent_search_entitlements: {
    columns: [
      "id", "entitlement_ref", "challenge_id", "receipt_ref", "actor_ref",
      "agent_user_id", "credential_id", "product_id", "scope_ref",
      "method", "path", "mode", "request_body_digest", "status",
      "expires_at", "created_at", "consumed_at", "archived_at",
    ],
    conflict: { mode: "converge", keyColumns: ["id"] },
    keyColumns: ["id"],
    orderColumn: "created_at",
    groupColumn: "status",
  },
  agent_search_payment_redemptions: {
    columns: [
      "id", "idempotency_key_hash", "challenge_id", "actor_ref",
      "credential_id", "proof_ref", "entitlement_ref", "receipt_ref",
      "public_projection_json", "created_at", "archived_at",
    ],
    conflict: { mode: "converge", keyColumns: ["id"] },
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  /* eslint-enable sort-keys */
}

export type InferenceEntitlementsTable = keyof typeof INFERENCE_ENTITLEMENTS_TABLES &
  string

export const INFERENCE_ENTITLEMENTS_TABLE_NAMES: ReadonlyArray<string> =
  Object.keys(INFERENCE_ENTITLEMENTS_TABLES)

export type D1SourceRow = Readonly<Record<string, unknown>>

const normalizeValue = (value: unknown): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "bigint") return Number(value)
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)`; the
 * structural `SyncSql` seam deliberately does not, so this module widens
 * it locally (DIRECT connections only, never Hyperdrive).
 */
type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "inference entitlements backfill requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

export const requireEntitlementsTable = (
  table: string,
): EntitlementsTableSpec => {
  const spec = INFERENCE_ENTITLEMENTS_TABLES[table]
  if (spec === undefined) {
    throw new Error(`unknown inference entitlements table: ${table}`)
  }
  return spec
}

/**
 * Upsert one page of D1 rows into `table`. Event tables: one multi-row
 * `INSERT ... ON CONFLICT DO NOTHING` per page. State tables: converge
 * upserts to the D1 snapshot. `agent_search_cache_entries` additionally
 * archives conflicting ACTIVE twins first so the one-active-entry-per-key
 * partial unique index stays satisfied mid-sweep. Returns how many rows
 * were actually inserted fresh (0 on an events re-run — the idempotency
 * contract; converge tables report page size since DO UPDATE touches
 * every row).
 */
export const upsertEntitlementsRows = async (
  sql: SyncSql,
  table: string,
  rows: ReadonlyArray<D1SourceRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireUnsafe(sql)
  const spec = requireEntitlementsTable(table)
  const columns = spec.columns

  if (spec.conflict.mode === "nothing") {
    const params: Array<unknown> = []
    const tuples = rows.map((row) => {
      const placeholders = columns.map((column) => {
        params.push(normalizeValue(row[column]))
        return `$${params.length}`
      })
      return `(${placeholders.join(", ")})`
    })
    const result = await unsafe(
      `INSERT INTO ${table} (${columns.join(", ")}) VALUES ${tuples.join(", ")} ON CONFLICT DO NOTHING RETURNING 1 AS inserted`,
      params,
    )
    return result.length
  }

  const keyColumns = spec.conflict.keyColumns
  const setClauses = columns
    .filter((column) => !keyColumns.includes(column))
    .map((column) => `${column} = EXCLUDED.${column}`)
    .join(", ")
  let touched = 0
  for (const row of rows) {
    if (table === "agent_search_cache_entries" && row["archived_at"] == null) {
      // Keep the one-active-entry-per-key partial unique index satisfied:
      // archive any OTHER active twin for this cache_key before converging
      // this (D1-authoritative) active row.
      await unsafe(
        `UPDATE agent_search_cache_entries
            SET archived_at = $1
          WHERE cache_key = $2 AND archived_at IS NULL AND id <> $3`,
        [
          normalizeValue(row["created_at"]),
          normalizeValue(row["cache_key"]),
          normalizeValue(row["id"]),
        ],
      )
    }
    const values = columns.map((column) => normalizeValue(row[column]))
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${columns.join(", ")}) VALUES (${placeholders}) ON CONFLICT (${keyColumns.join(", ")}) DO UPDATE SET ${setClauses} RETURNING 1 AS touched`,
      values as Array<unknown>,
    )
    touched += result.length
  }
  return touched
}

// ---------------------------------------------------------------------------
// Verification — counts, per-group tallies, newest-N hashes
// ---------------------------------------------------------------------------

export type GroupTally = Readonly<Record<string, number>>

export type EntitlementsTableTally = Readonly<{
  totalRows: number
  byGroup: GroupTally
}>

export const postgresEntitlementsTableTally = async (
  sql: SyncSql,
  table: string,
): Promise<EntitlementsTableTally> => {
  const unsafe = requireUnsafe(sql)
  const spec = requireEntitlementsTable(table)
  const totals = await unsafe(
    `SELECT COUNT(*) AS total_rows FROM ${table}`,
    [],
  )
  const byGroup: Record<string, number> = {}
  if (spec.groupColumn !== undefined) {
    const rows = await unsafe(
      `SELECT COALESCE(CAST(${spec.groupColumn} AS text), '<null>') AS group_key,
              COUNT(*) AS row_count
         FROM ${table}
        GROUP BY COALESCE(CAST(${spec.groupColumn} AS text), '<null>')
        ORDER BY group_key`,
      [],
    )
    for (const row of rows) {
      byGroup[String(row["group_key"] ?? "<null>")] = Number(
        row["row_count"] ?? 0,
      )
    }
  }
  return { byGroup, totalRows: Number(totals[0]?.["total_rows"] ?? 0) }
}

/** The same tally shape over D1 export rows (fetched by the CLI). */
export const entitlementsTallyFromRows = (
  totalsRow: Record<string, unknown> | undefined,
  groupRows: ReadonlyArray<Record<string, unknown>>,
): EntitlementsTableTally => {
  const byGroup: Record<string, number> = {}
  for (const row of groupRows) {
    byGroup[String(row["group_key"] ?? "<null>")] = Number(
      row["row_count"] ?? 0,
    )
  }
  return { byGroup, totalRows: Number(totalsRow?.["total_rows"] ?? 0) }
}

/**
 * Canonical row hash: the registry-order column values joined with unit
 * separators, sha256'd. Normalization matches `upsertEntitlementsRows`,
 * so the SAME D1 export row and its Postgres twin hash identically.
 */
export const entitlementsRowHash = (
  table: string,
  row: D1SourceRow,
): string => {
  const spec = requireEntitlementsTable(table)
  const hash = createHash("sha256")
  for (const column of spec.columns) {
    const value = normalizeValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ key: string; hash: string }>

const rowKey = (table: string, row: D1SourceRow): string =>
  requireEntitlementsTable(table)
    .keyColumns.map((column) => String(row[column] ?? "<null>"))
    .join(":")

export const postgresEntitlementsNewestHashes = async (
  sql: SyncSql,
  table: string,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const unsafe = requireUnsafe(sql)
  const spec = requireEntitlementsTable(table)
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${spec.orderColumn} DESC, ${spec.keyColumns.join(" DESC, ")} DESC LIMIT $1`,
    [limit],
  )
  return rows.map((row) => ({
    hash: entitlementsRowHash(table, row),
    key: rowKey(table, row),
  }))
}

/** Same newest-N hashing over D1 export rows (already fetched by the CLI). */
export const d1EntitlementsNewestHashes = (
  table: string,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> =>
  rows.map((row) => ({
    hash: entitlementsRowHash(table, row),
    key: rowKey(table, row),
  }))

export const compareNewestHashes = (
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): ReadonlyArray<{
  key: string
  d1Hash: string | undefined
  postgresHash: string | undefined
}> => {
  const postgresByKey = new Map(
    postgresNewest.map((entry) => [entry.key, entry.hash]),
  )
  const mismatches: Array<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }> = []
  for (const entry of d1Newest) {
    const postgresHash = postgresByKey.get(entry.key)
    if (postgresHash !== entry.hash) {
      mismatches.push({ d1Hash: entry.hash, key: entry.key, postgresHash })
    }
  }
  return mismatches
}

export type EntitlementsVerifyReport = Readonly<{
  table: string
  countsMatch: boolean
  d1Total: number
  postgresTotal: number
  groupMismatches: ReadonlyArray<{
    group: string
    d1Rows: number
    postgresRows: number
  }>
  newestHashMismatches: ReadonlyArray<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }>
}>

export const compareEntitlementsTallies = (
  table: string,
  d1: EntitlementsTableTally,
  postgres: EntitlementsTableTally,
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): EntitlementsVerifyReport => {
  const groups = new Set([
    ...Object.keys(d1.byGroup),
    ...Object.keys(postgres.byGroup),
  ])
  const groupMismatches: Array<{
    group: string
    d1Rows: number
    postgresRows: number
  }> = []
  for (const group of [...groups].sort()) {
    const left = d1.byGroup[group] ?? 0
    const right = postgres.byGroup[group] ?? 0
    if (left !== right) {
      groupMismatches.push({ d1Rows: left, group, postgresRows: right })
    }
  }
  return {
    countsMatch: d1.totalRows === postgres.totalRows,
    d1Total: d1.totalRows,
    groupMismatches,
    newestHashMismatches: compareNewestHashes(d1Newest, postgresNewest),
    postgresTotal: postgres.totalRows,
    table,
  }
}

// ---------------------------------------------------------------------------
// Tally = SUM(events) enforcement invariants (the §3.6 acceptance)
// ---------------------------------------------------------------------------

export type TallyInvariantMismatch = Readonly<{
  key: string
  tallyCount: number
  eventsCount: number
  tallyAmount: number
  eventsAmount: number
}>

export type TallyInvariantReport = Readonly<{
  family: string
  mismatches: ReadonlyArray<TallyInvariantMismatch>
}>

const invariantRows = (
  rows: ReadonlyArray<Record<string, unknown>>,
): ReadonlyArray<TallyInvariantMismatch> =>
  rows.map((row) => ({
    eventsAmount: Number(row["events_amount"] ?? 0),
    eventsCount: Number(row["events_count"] ?? 0),
    key: String(row["key"] ?? "<null>"),
    tallyAmount: Number(row["tally_amount"] ?? 0),
    tallyCount: Number(row["tally_count"] ?? 0),
  }))

/**
 * Verify tally = SUM(events) PER KEY on the Postgres side for the three
 * enforcement tally families. Full outer join so an event set without a
 * tally row (or vice versa) is a mismatch too. Empty mismatch arrays are
 * the KS-8.9 acceptance evidence.
 */
export const postgresEntitlementsTallyInvariants = async (
  sql: SyncSql,
): Promise<ReadonlyArray<TallyInvariantReport>> => {
  const unsafe = requireUnsafe(sql)

  const freeTier = await unsafe(
    `SELECT COALESCE(t.key, e.key) AS key,
            COALESCE(t.tally_count, 0) AS tally_count,
            COALESCE(e.events_count, 0) AS events_count,
            COALESCE(t.tally_amount, 0) AS tally_amount,
            COALESCE(e.events_amount, 0) AS events_amount
       FROM (SELECT account_ref || ':' || usage_day AS key,
                    free_request_count AS tally_count,
                    free_total_tokens AS tally_amount
               FROM inference_free_tier_usage) t
       FULL OUTER JOIN
            (SELECT account_ref || ':' || usage_day AS key,
                    COUNT(*) AS events_count,
                    COALESCE(SUM(total_tokens), 0) AS events_amount
               FROM inference_free_tier_usage_events
              GROUP BY account_ref || ':' || usage_day) e
         ON t.key = e.key
      WHERE COALESCE(t.tally_count, 0) <> COALESCE(e.events_count, 0)
         OR COALESCE(t.tally_amount, 0) <> COALESCE(e.events_amount, 0)
      ORDER BY 1`,
    [],
  )

  const freeUsage = await unsafe(
    `SELECT COALESCE(t.key, e.key) AS key,
            COALESCE(t.tally_count, 0) AS tally_count,
            COALESCE(e.events_count, 0) AS events_count,
            COALESCE(t.tally_amount, 0) AS tally_amount,
            COALESCE(e.events_amount, 0) AS events_amount
       FROM (SELECT owner_key AS key,
                    free_request_count AS tally_count,
                    cumulative_free_usd_micros AS tally_amount
               FROM inference_free_usage_tally) t
       FULL OUTER JOIN
            (SELECT owner_key AS key,
                    COUNT(*) AS events_count,
                    COALESCE(SUM(free_usd_micros), 0) AS events_amount
               FROM inference_free_usage_events
              GROUP BY owner_key) e
         ON t.key = e.key
      WHERE COALESCE(t.tally_count, 0) <> COALESCE(e.events_count, 0)
         OR COALESCE(t.tally_amount, 0) <> COALESCE(e.events_amount, 0)
      ORDER BY 1`,
    [],
  )

  const earned = await unsafe(
    `SELECT COALESCE(t.key, e.key) AS key,
            COALESCE(t.tally_count, 0) AS tally_count,
            COALESCE(e.events_count, 0) AS events_count,
            COALESCE(t.tally_amount, 0) AS tally_amount,
            COALESCE(e.events_amount, 0) AS events_amount
       FROM (SELECT owner_key AS key,
                    accrual_count AS tally_count,
                    earned_free_usd_micros AS tally_amount
               FROM inference_earned_allowance) t
       FULL OUTER JOIN
            (SELECT owner_key AS key,
                    COUNT(*) AS events_count,
                    COALESCE(SUM(earned_usd_micros), 0) AS events_amount
               FROM inference_earned_allowance_events
              GROUP BY owner_key) e
         ON t.key = e.key
      WHERE COALESCE(t.tally_count, 0) <> COALESCE(e.events_count, 0)
         OR COALESCE(t.tally_amount, 0) <> COALESCE(e.events_amount, 0)
      ORDER BY 1`,
    [],
  )

  return [
    { family: "free_tier_usage", mismatches: invariantRows(freeTier) },
    { family: "free_usage_pool", mismatches: invariantRows(freeUsage) },
    { family: "earned_allowance", mismatches: invariantRows(earned) },
  ]
}
