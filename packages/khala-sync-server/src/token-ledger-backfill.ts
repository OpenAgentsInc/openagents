/**
 * KS-8.2 (#8308): token ledger backfill core — D1 → Postgres.
 *
 * Testable core behind `scripts/backfill-token-ledger.ts`, following the
 * KS-8.1 template (`pylon-backfill.ts`). Takes raw D1 rows (snake_case
 * objects, exactly as `wrangler d1 execute --json` returns them) and
 * copies them into the Postgres twins from migration
 * `0008_token_usage_ledger.sql`:
 *
 *   - `token_usage_events` — `ON CONFLICT DO NOTHING` (bare: covers both
 *     the id primary key and the idempotency_key unique). The dual-write
 *     mirror writes byte-identical rows, so backfill and mirror never
 *     fight; a re-run inserts zero rows (idempotency contract). Pages are
 *     batched into ONE multi-row INSERT (this table is the big one —
 *     hundreds of thousands of rows).
 *
 *   - the three `public_khala_tokens_served_*` rollups and
 *     `token_usage_leaderboard_preferences` — CONVERGE upserts
 *     (`ON CONFLICT ... DO UPDATE SET` = the D1 value). Rollups are
 *     cumulative counters both sides increment for the same ledger
 *     events, so "converge to the D1 snapshot" is correct and the tiny
 *     write race against a concurrent mirror increment (today's row
 *     only) is exactly what the runbook's second catch-up sweep +
 *     `--verify` exist to close.
 *
 * Verification (`verify*`): the 2026-06-29 after-action reconciliation
 * culture, with the domain-specific exactness the KS-8.2 acceptance
 * demands — exact row counts, exact `SUM(total_tokens)`, the exact public
 * tokens-served SUM (input+output with total_tokens fallback), per-
 * provider row/token tallies, and newest-N row-hash comparison. Nothing
 * "close enough": exact or explain.
 */

import { createHash } from "node:crypto"
import type { SyncSql } from "./sql.js"

// ---------------------------------------------------------------------------
// Table shapes (column lists mirror migration 0008 exactly)
// ---------------------------------------------------------------------------

export type TokenLedgerBackfillTable =
  | "token_usage_events"
  | "public_khala_tokens_served_daily_rollups"
  | "public_khala_tokens_served_model_daily_rollups"
  | "public_khala_tokens_served_channel_daily_rollups"
  | "token_usage_leaderboard_preferences"

export const TOKEN_LEDGER_TABLES: ReadonlyArray<TokenLedgerBackfillTable> = [
  "token_usage_events",
  "public_khala_tokens_served_daily_rollups",
  "public_khala_tokens_served_model_daily_rollups",
  "public_khala_tokens_served_channel_daily_rollups",
  "token_usage_leaderboard_preferences",
]

const EVENT_COLUMNS = [
  "id",
  "idempotency_key",
  "observed_at",
  "ingested_at",
  "producer_system",
  "source_route",
  "role_ref",
  "actor_user_id",
  "actor_team_id",
  "account_ref",
  "anonymized_source_ref",
  "run_ref",
  "session_ref",
  "task_ref",
  "repository_ref",
  "provider",
  "model",
  "backend_profile",
  "input_tokens",
  "output_tokens",
  "reasoning_tokens",
  "cache_read_tokens",
  "cache_write_5m_tokens",
  "cache_write_1h_tokens",
  "total_tokens",
  "usage_truth",
  "cost_amount",
  "currency",
  "demand_channel",
  "demand_kind",
  "demand_source",
  "demand_client",
  "leaderboard_eligible",
  "privacy_opt_out",
  "safe_metadata_json",
] as const

const DAILY_ROLLUP_COLUMNS = [
  "timezone",
  "day",
  "tokens_served",
  "usage_events",
  "updated_at",
] as const

const MODEL_ROLLUP_COLUMNS = [
  "day",
  "provider",
  "model",
  "tokens_served",
  "usage_events",
  "updated_at",
] as const

const CHANNEL_ROLLUP_COLUMNS = [
  "day",
  "demand_channel",
  "tokens_served",
  "usage_events",
  "updated_at",
] as const

const PREFERENCE_COLUMNS = [
  "subject_kind",
  "subject_ref",
  "leaderboard_participation",
  "leaderboard_visibility",
  "updated_at",
  "updated_by_user_id",
] as const

export const TOKEN_LEDGER_TABLE_COLUMNS: Readonly<
  Record<TokenLedgerBackfillTable, ReadonlyArray<string>>
> = {
  public_khala_tokens_served_channel_daily_rollups: CHANNEL_ROLLUP_COLUMNS,
  public_khala_tokens_served_daily_rollups: DAILY_ROLLUP_COLUMNS,
  public_khala_tokens_served_model_daily_rollups: MODEL_ROLLUP_COLUMNS,
  token_usage_events: EVENT_COLUMNS,
  token_usage_leaderboard_preferences: PREFERENCE_COLUMNS,
}

/**
 * Conflict handling per table: the event ledger NEVER overwrites (mirror
 * rows win / identical bytes); rollups + preferences CONVERGE to the D1
 * snapshot value (cumulative counters / last-writer state).
 */
const TABLE_CONFLICT: Readonly<
  Record<
    TokenLedgerBackfillTable,
    Readonly<{ keyColumns: ReadonlyArray<string>; mode: "nothing" | "converge" }>
  >
> = {
  public_khala_tokens_served_channel_daily_rollups: {
    keyColumns: ["day", "demand_channel"],
    mode: "converge",
  },
  public_khala_tokens_served_daily_rollups: {
    keyColumns: ["timezone", "day"],
    mode: "converge",
  },
  public_khala_tokens_served_model_daily_rollups: {
    keyColumns: ["day", "provider", "model"],
    mode: "converge",
  },
  token_usage_events: { keyColumns: [], mode: "nothing" },
  token_usage_leaderboard_preferences: {
    keyColumns: ["subject_kind", "subject_ref"],
    mode: "converge",
  },
}

/** Natural key used for newest-N hash comparison output. */
export const TOKEN_LEDGER_TABLE_KEY: Readonly<
  Record<TokenLedgerBackfillTable, ReadonlyArray<string>>
> = {
  public_khala_tokens_served_channel_daily_rollups: ["day", "demand_channel"],
  public_khala_tokens_served_daily_rollups: ["timezone", "day"],
  public_khala_tokens_served_model_daily_rollups: ["day", "provider", "model"],
  token_usage_events: ["id"],
  token_usage_leaderboard_preferences: ["subject_kind", "subject_ref"],
}

/** Newest-first ordering column per table (for the hash sample). */
export const TOKEN_LEDGER_TABLE_ORDER: Readonly<
  Record<TokenLedgerBackfillTable, string>
> = {
  public_khala_tokens_served_channel_daily_rollups: "day",
  public_khala_tokens_served_daily_rollups: "day",
  public_khala_tokens_served_model_daily_rollups: "day",
  token_usage_events: "observed_at",
  token_usage_leaderboard_preferences: "updated_at",
}

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
 * it locally (same note as pylon-backfill: DIRECT connections only, never
 * Hyperdrive).
 */
type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "token ledger backfill requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/**
 * Upsert one page of D1 rows into `table`. Events: one multi-row
 * `INSERT ... ON CONFLICT DO NOTHING` per page (the big table). Rollups /
 * preferences: converge upserts. Returns how many rows were actually
 * inserted fresh (0 on an events re-run — the idempotency contract;
 * converge tables report page size since DO UPDATE returns every row).
 */
export const upsertTokenLedgerRows = async (
  sql: SyncSql,
  table: TokenLedgerBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireUnsafe(sql)
  const columns = TOKEN_LEDGER_TABLE_COLUMNS[table]
  const conflict = TABLE_CONFLICT[table]

  if (conflict.mode === "nothing") {
    // One page = one statement: rows.length × columns placeholders.
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

  const setClauses = columns
    .filter((column) => !conflict.keyColumns.includes(column))
    .map((column) => `${column} = EXCLUDED.${column}`)
    .join(", ")
  let touched = 0
  for (const row of rows) {
    const values = columns.map((column) => normalizeValue(row[column]))
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${columns.join(", ")}) VALUES (${placeholders}) ON CONFLICT (${conflict.keyColumns.join(", ")}) DO UPDATE SET ${setClauses} RETURNING 1 AS touched`,
      values as Array<unknown>,
    )
    touched += result.length
  }
  return touched
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

/** The public tokens-served expression (input+output, total fallback). */
export const PUBLIC_TOKENS_SERVED_SQL = `CASE
  WHEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0) > 0
    THEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
  ELSE COALESCE(total_tokens, 0)
END`

export type TokenLedgerEventsTally = Readonly<{
  totalRows: number
  sumTotalTokens: number
  sumPublicTokensServed: number
  byProvider: Readonly<
    Record<string, Readonly<{ rows: number; totalTokens: number }>>
  >
}>

/** The exact-events tally on the Postgres side. */
export const postgresTokenLedgerEventsTally = async (
  sql: SyncSql,
): Promise<TokenLedgerEventsTally> => {
  const unsafe = requireUnsafe(sql)
  const totals = await unsafe(
    `SELECT COUNT(*) AS total_rows,
            COALESCE(SUM(total_tokens), 0) AS sum_total_tokens,
            COALESCE(SUM(${PUBLIC_TOKENS_SERVED_SQL}), 0) AS sum_public
       FROM token_usage_events`,
    [],
  )
  const providers = await unsafe(
    `SELECT COALESCE(provider, '<null>') AS provider_key,
            COUNT(*) AS row_count,
            COALESCE(SUM(total_tokens), 0) AS total_tokens
       FROM token_usage_events
      GROUP BY COALESCE(provider, '<null>')
      ORDER BY provider_key`,
    [],
  )
  return tallyFromRows(totals[0], providers)
}

/** The same tally shape over D1 export rows (fetched by the CLI). */
export const tallyFromRows = (
  totalsRow: Record<string, unknown> | undefined,
  providerRows: ReadonlyArray<Record<string, unknown>>,
): TokenLedgerEventsTally => {
  const byProvider: Record<string, { rows: number; totalTokens: number }> = {}
  for (const row of providerRows) {
    byProvider[String(row["provider_key"] ?? "<null>")] = {
      rows: Number(row["row_count"] ?? 0),
      totalTokens: Number(row["total_tokens"] ?? 0),
    }
  }
  return {
    byProvider,
    sumPublicTokensServed: Number(totalsRow?.["sum_public"] ?? 0),
    sumTotalTokens: Number(totalsRow?.["sum_total_tokens"] ?? 0),
    totalRows: Number(totalsRow?.["total_rows"] ?? 0),
  }
}

export type TokenLedgerRollupTally = Readonly<{
  totalRows: number
  sumTokensServed: number
  sumUsageEvents: number
}>

export const postgresTokenLedgerRollupTally = async (
  sql: SyncSql,
  table: TokenLedgerBackfillTable,
): Promise<TokenLedgerRollupTally> => {
  const unsafe = requireUnsafe(sql)
  const rows = await unsafe(
    `SELECT COUNT(*) AS total_rows,
            COALESCE(SUM(tokens_served), 0) AS sum_tokens_served,
            COALESCE(SUM(usage_events), 0) AS sum_usage_events
       FROM ${table}`,
    [],
  )
  return {
    sumTokensServed: Number(rows[0]?.["sum_tokens_served"] ?? 0),
    sumUsageEvents: Number(rows[0]?.["sum_usage_events"] ?? 0),
    totalRows: Number(rows[0]?.["total_rows"] ?? 0),
  }
}

/** Plain row count (leaderboard preferences verify path). */
export const postgresTokenLedgerRowCount = async (
  sql: SyncSql,
  table: TokenLedgerBackfillTable,
): Promise<number> => {
  const unsafe = requireUnsafe(sql)
  const rows = await unsafe(`SELECT COUNT(*) AS total_rows FROM ${table}`, [])
  return Number(rows[0]?.["total_rows"] ?? 0)
}

/**
 * Canonical row hash: the migration-order column values joined with unit
 * separators, sha256'd. Normalization matches `upsertTokenLedgerRows`, so
 * the SAME D1 export row and its Postgres twin hash identically.
 */
export const tokenLedgerRowHash = (
  table: TokenLedgerBackfillTable,
  row: D1SourceRow,
): string => {
  const columns = TOKEN_LEDGER_TABLE_COLUMNS[table]
  const hash = createHash("sha256")
  for (const column of columns) {
    const value = normalizeValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ key: string; hash: string }>

const rowKey = (
  table: TokenLedgerBackfillTable,
  row: D1SourceRow,
): string =>
  TOKEN_LEDGER_TABLE_KEY[table]
    .map((column) => String(row[column] ?? "<null>"))
    .join(":")

export const postgresTokenLedgerNewestHashes = async (
  sql: SyncSql,
  table: TokenLedgerBackfillTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const unsafe = requireUnsafe(sql)
  const orderColumn = TOKEN_LEDGER_TABLE_ORDER[table]
  const keyColumns = TOKEN_LEDGER_TABLE_KEY[table]
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${orderColumn} DESC, ${keyColumns.join(" DESC, ")} DESC LIMIT $1`,
    [limit],
  )
  return rows.map((row) => ({
    hash: tokenLedgerRowHash(table, row),
    key: rowKey(table, row),
  }))
}

/** Same newest-N hashing over D1 export rows (already fetched by the CLI). */
export const d1TokenLedgerNewestHashes = (
  table: TokenLedgerBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> =>
  rows.map((row) => ({
    hash: tokenLedgerRowHash(table, row),
    key: rowKey(table, row),
  }))

export type TokenLedgerVerifyReport = Readonly<{
  table: TokenLedgerBackfillTable
  countsMatch: boolean
  d1Total: number
  postgresTotal: number
  scalarMismatches: ReadonlyArray<{
    metric: string
    d1: number
    postgres: number
  }>
  providerMismatches: ReadonlyArray<{
    provider: string
    d1Rows: number
    postgresRows: number
    d1TotalTokens: number
    postgresTotalTokens: number
  }>
  newestHashMismatches: ReadonlyArray<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }>
}>

export const compareNewestHashes = (
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): TokenLedgerVerifyReport["newestHashMismatches"] => {
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

export const compareTokenLedgerEventsTallies = (
  d1: TokenLedgerEventsTally,
  postgres: TokenLedgerEventsTally,
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): TokenLedgerVerifyReport => {
  const scalarMismatches: Array<{
    metric: string
    d1: number
    postgres: number
  }> = []
  if (d1.sumTotalTokens !== postgres.sumTotalTokens) {
    scalarMismatches.push({
      d1: d1.sumTotalTokens,
      metric: "sum_total_tokens",
      postgres: postgres.sumTotalTokens,
    })
  }
  if (d1.sumPublicTokensServed !== postgres.sumPublicTokensServed) {
    scalarMismatches.push({
      d1: d1.sumPublicTokensServed,
      metric: "sum_public_tokens_served",
      postgres: postgres.sumPublicTokensServed,
    })
  }

  const providers = new Set([
    ...Object.keys(d1.byProvider),
    ...Object.keys(postgres.byProvider),
  ])
  const providerMismatches: Array<{
    provider: string
    d1Rows: number
    postgresRows: number
    d1TotalTokens: number
    postgresTotalTokens: number
  }> = []
  for (const provider of [...providers].sort()) {
    const left = d1.byProvider[provider] ?? { rows: 0, totalTokens: 0 }
    const right = postgres.byProvider[provider] ?? { rows: 0, totalTokens: 0 }
    if (left.rows !== right.rows || left.totalTokens !== right.totalTokens) {
      providerMismatches.push({
        d1Rows: left.rows,
        d1TotalTokens: left.totalTokens,
        postgresRows: right.rows,
        postgresTotalTokens: right.totalTokens,
        provider,
      })
    }
  }

  return {
    countsMatch: d1.totalRows === postgres.totalRows,
    d1Total: d1.totalRows,
    newestHashMismatches: compareNewestHashes(d1Newest, postgresNewest),
    postgresTotal: postgres.totalRows,
    providerMismatches,
    scalarMismatches,
    table: "token_usage_events",
  }
}

export const compareTokenLedgerRollupTallies = (
  table: TokenLedgerBackfillTable,
  d1: TokenLedgerRollupTally,
  postgres: TokenLedgerRollupTally,
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): TokenLedgerVerifyReport => {
  const scalarMismatches: Array<{
    metric: string
    d1: number
    postgres: number
  }> = []
  if (d1.sumTokensServed !== postgres.sumTokensServed) {
    scalarMismatches.push({
      d1: d1.sumTokensServed,
      metric: "sum_tokens_served",
      postgres: postgres.sumTokensServed,
    })
  }
  if (d1.sumUsageEvents !== postgres.sumUsageEvents) {
    scalarMismatches.push({
      d1: d1.sumUsageEvents,
      metric: "sum_usage_events",
      postgres: postgres.sumUsageEvents,
    })
  }
  return {
    countsMatch: d1.totalRows === postgres.totalRows,
    d1Total: d1.totalRows,
    newestHashMismatches: compareNewestHashes(d1Newest, postgresNewest),
    postgresTotal: postgres.totalRows,
    providerMismatches: [],
    scalarMismatches,
    table,
  }
}
