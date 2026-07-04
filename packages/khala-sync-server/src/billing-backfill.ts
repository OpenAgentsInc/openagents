/**
 * KS-8.7 (#8318): billing / Stripe / pay-ins backfill core — D1 → Postgres.
 *
 * Testable core behind `scripts/backfill-billing.ts`, following the KS-8.1
 * (`pylon-backfill.ts`) and KS-8.2 (`token-ledger-backfill.ts`) templates.
 * Takes raw D1 rows (snake_case objects, exactly as `wrangler d1 execute
 * --json` returns them) and copies them into the Postgres twins from
 * migration `0015_billing_pay_ins.sql`.
 *
 * WRITE MODE: every table CONVERGE-upserts to the authoritative D1 snapshot
 * (`ON CONFLICT (pk) DO UPDATE SET col = EXCLUDED.col`). This domain's rows
 * are copies of rows D1 has already accepted, so converging can never alter
 * an amount or re-make an idempotency decision — and converge (not DO
 * NOTHING) is required because webhook status, checkout fulfillment,
 * pay-in state machines, and auto-top-up policies are UPDATEd in place on
 * D1: a backfill sweep must bring a stale mirror row forward to the D1
 * value. Re-running a sweep against an already-converged mirror is a no-op
 * (idempotency contract; regression-tested).
 *
 * VERIFY (`--verify`): the 2026-06-29 after-action reconciliation
 * methodology with the §3.4 money exactness this domain demands:
 *   - exact row counts per table;
 *   - billing_ledger_entries: exact SUM(amount_cents) overall, exact
 *     per-(currency, source) row/sum tallies, and EXACT PER-USER BALANCE
 *     EQUALITY (the full GROUP BY user_id balance map compared entry by
 *     entry — balance = SUM(ledger) must reconcile to the cent per account
 *     before any read cutover);
 *   - pay_ins: exact SUM(cost_msat) per (pay_in_type, state);
 *   - pay_in_legs: exact SUM(amount_msat) per (direction, kind);
 *   - stripe_webhook_events: exact event-id SET equality (order-insensitive
 *     digest over every event_id — the webhook dedupe gate must hold the
 *     identical key set on both sides) plus per-processing_status counts;
 *   - buyer_payment_receipts / _credit_debits / _challenges: exact
 *     SUM(amount minor units) per (asset, status);
 *   - khala_code_paid_plan_payment_intents: exact SUM(amount_cents) /
 *     SUM(amount_sats) per (rail, status);
 *   - newest-N row-hash comparison for every table.
 * Nothing "close enough": exact or explain.
 */

import { createHash } from "node:crypto"
import {
  BILLING_DOMAIN_TABLE_SPECS,
  BILLING_DOMAIN_TABLES,
  normalizeBillingValue,
  type BillingDomainTable,
} from "./billing-domain-tables.js"
import type { SyncSql } from "./sql.js"

export {
  BILLING_DOMAIN_TABLE_SPECS,
  BILLING_DOMAIN_TABLES,
  type BillingDomainTable,
}

export type D1SourceRow = Readonly<Record<string, unknown>>

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)`; the
 * structural `SyncSql` seam deliberately does not, so this module widens it
 * locally (same note as pylon-backfill: DIRECT connections only, never
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
      "billing backfill requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/** The converge upsert statement text for one table (shared with tests). */
export const billingConvergeUpsertSql = (
  table: BillingDomainTable,
  rowCount: number,
): string => {
  const spec = BILLING_DOMAIN_TABLE_SPECS[table]
  const columns = spec.columns
  const tuples: Array<string> = []
  for (let row = 0; row < rowCount; row++) {
    const placeholders = columns.map(
      (_, index) => `$${row * columns.length + index + 1}`,
    )
    tuples.push(`(${placeholders.join(", ")})`)
  }
  const setClauses = columns
    .filter((column) => !spec.keyColumns.includes(column))
    .map((column) => `${column} = EXCLUDED.${column}`)
    .join(", ")
  return `INSERT INTO ${table} (${columns.join(", ")}) VALUES ${tuples.join(", ")} ON CONFLICT (${spec.keyColumns.join(", ")}) DO UPDATE SET ${setClauses}`
}

/**
 * Converge one page of D1 rows into `table` as ONE multi-row statement.
 * Returns the page size (every row is inserted-or-converged; a re-run
 * against identical mirror rows is a byte-level no-op).
 */
export const upsertBillingRows = async (
  sql: SyncSql,
  table: BillingDomainTable,
  rows: ReadonlyArray<D1SourceRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireUnsafe(sql)
  const columns = BILLING_DOMAIN_TABLE_SPECS[table].columns
  const params: Array<unknown> = []
  for (const row of rows) {
    for (const column of columns) {
      params.push(normalizeBillingValue(row[column]))
    }
  }
  await unsafe(billingConvergeUpsertSql(table, rows.length), params)
  return rows.length
}

// ---------------------------------------------------------------------------
// Row hashes (newest-N comparison)
// ---------------------------------------------------------------------------

/**
 * Canonical row hash: the migration-order column values joined with unit
 * separators, sha256'd. Normalization matches `upsertBillingRows`, so the
 * SAME D1 export row and its Postgres twin hash identically.
 */
export const billingRowHash = (
  table: BillingDomainTable,
  row: D1SourceRow,
): string => {
  const columns = BILLING_DOMAIN_TABLE_SPECS[table].columns
  const hash = createHash("sha256")
  for (const column of columns) {
    const value = normalizeBillingValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ key: string; hash: string }>

export const billingRowKey = (
  table: BillingDomainTable,
  row: D1SourceRow,
): string =>
  BILLING_DOMAIN_TABLE_SPECS[table].keyColumns
    .map((column) => String(row[column] ?? "<null>"))
    .join(":")

export const billingNewestHashesFromRows = (
  table: BillingDomainTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> =>
  rows.map((row) => ({
    hash: billingRowHash(table, row),
    key: billingRowKey(table, row),
  }))

/** The deterministic newest-first ORDER BY both sides use for the sample. */
export const billingNewestOrderSql = (table: BillingDomainTable): string => {
  const spec = BILLING_DOMAIN_TABLE_SPECS[table]
  return `${spec.orderColumn} DESC, ${spec.keyColumns.join(" DESC, ")} DESC`
}

export const postgresBillingNewestHashes = async (
  sql: SyncSql,
  table: BillingDomainTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const unsafe = requireUnsafe(sql)
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${billingNewestOrderSql(table)} LIMIT $1`,
    [limit],
  )
  return billingNewestHashesFromRows(table, rows)
}

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

// ---------------------------------------------------------------------------
// Money reconciliation: grouped exact tallies
// ---------------------------------------------------------------------------

/**
 * A grouped exact tally: group key → { rows, sums per metric }. Both sides
 * produce the same shape from the same GROUP BY; comparison is exact map
 * equality (missing group on either side is a mismatch).
 */
export type GroupedTally = Readonly<
  Record<string, Readonly<{ rows: number; sums: ReadonlyArray<number> }>>
>

export type GroupedTallySpec = Readonly<{
  /** Human label for reports, e.g. 'per-(currency, source) amount_cents'. */
  label: string
  groupColumns: ReadonlyArray<string>
  sumColumns: ReadonlyArray<string>
}>

/**
 * Money-reconciliation specs per table. billing_ledger_entries carries TWO
 * grouped tallies: the per-(currency, source) sum AND the per-user balance
 * map (the §3.4 acceptance: balance = SUM(ledger) to the cent per account).
 */
export const BILLING_GROUPED_TALLIES: Readonly<
  Partial<Record<BillingDomainTable, ReadonlyArray<GroupedTallySpec>>>
> = {
  billing_auto_top_up_events: [
    {
      groupColumns: ["currency", "status"],
      label: "per-(currency, status) amount_cents",
      sumColumns: ["amount_cents"],
    },
  ],
  billing_ledger_entries: [
    {
      groupColumns: ["currency", "source"],
      label: "per-(currency, source) amount_cents",
      sumColumns: ["amount_cents"],
    },
    {
      groupColumns: ["user_id"],
      label: "PER-USER BALANCE (SUM(amount_cents) to the cent)",
      sumColumns: ["amount_cents"],
    },
  ],
  buyer_payment_challenges: [
    {
      groupColumns: ["price_asset", "status"],
      label: "per-(price_asset, status) price_amount_minor_units",
      sumColumns: ["price_amount_minor_units"],
    },
  ],
  buyer_payment_credit_debits: [
    {
      groupColumns: ["amount_asset", "status"],
      label: "per-(amount_asset, status) amount_minor_units",
      sumColumns: ["amount_minor_units"],
    },
  ],
  buyer_payment_receipts: [
    {
      groupColumns: ["amount_asset", "status"],
      label: "per-(amount_asset, status) amount_minor_units",
      sumColumns: ["amount_minor_units"],
    },
  ],
  khala_code_paid_plan_payment_intents: [
    {
      groupColumns: ["rail", "status"],
      label: "per-(rail, status) amount_cents + amount_sats",
      sumColumns: ["amount_cents", "amount_sats"],
    },
  ],
  pay_in_legs: [
    {
      groupColumns: ["direction", "kind"],
      label: "per-(direction, kind) amount_msat",
      sumColumns: ["amount_msat"],
    },
  ],
  pay_ins: [
    {
      groupColumns: ["pay_in_type", "state"],
      label: "per-(pay_in_type, state) cost_msat",
      sumColumns: ["cost_msat"],
    },
  ],
  stripe_checkout_sessions: [
    {
      groupColumns: ["currency", "fulfillment_status"],
      label: "per-(currency, fulfillment_status) amount_cents",
      sumColumns: ["amount_cents"],
    },
  ],
  stripe_webhook_events: [
    {
      groupColumns: ["processing_status"],
      label: "per-processing_status event counts",
      sumColumns: [],
    },
  ],
}

/** SQL for a grouped tally — valid on BOTH SQLite/D1 and Postgres. */
export const groupedTallySql = (
  table: BillingDomainTable,
  spec: GroupedTallySpec,
): string => {
  const keyExpr = spec.groupColumns
    .map((column) => `COALESCE(${column}, '<null>')`)
    .join(" || ':' || ")
  const sums = spec.sumColumns
    .map(
      (column, index) => `COALESCE(SUM(${column}), 0) AS sum_${index}`,
    )
    .join(", ")
  return `SELECT ${keyExpr} AS group_key, COUNT(*) AS row_count${
    sums.length > 0 ? `, ${sums}` : ""
  } FROM ${table} GROUP BY ${spec.groupColumns.join(", ")}`
}

export const groupedTallyFromRows = (
  spec: GroupedTallySpec,
  rows: ReadonlyArray<Record<string, unknown>>,
): GroupedTally => {
  const tally: Record<string, { rows: number; sums: Array<number> }> = {}
  for (const row of rows) {
    tally[String(row["group_key"] ?? "<null>")] = {
      rows: Number(row["row_count"] ?? 0),
      sums: spec.sumColumns.map((_, index) => Number(row[`sum_${index}`] ?? 0)),
    }
  }
  return tally
}

export const postgresGroupedTally = async (
  sql: SyncSql,
  table: BillingDomainTable,
  spec: GroupedTallySpec,
): Promise<GroupedTally> => {
  const unsafe = requireUnsafe(sql)
  return groupedTallyFromRows(spec, await unsafe(groupedTallySql(table, spec), []))
}

export type GroupedTallyMismatch = Readonly<{
  groupKey: string
  d1Rows: number
  postgresRows: number
  d1Sums: ReadonlyArray<number>
  postgresSums: ReadonlyArray<number>
}>

export const compareGroupedTallies = (
  d1: GroupedTally,
  postgres: GroupedTally,
): ReadonlyArray<GroupedTallyMismatch> => {
  const keys = new Set([...Object.keys(d1), ...Object.keys(postgres)])
  const mismatches: Array<GroupedTallyMismatch> = []
  for (const key of [...keys].sort()) {
    const left = d1[key] ?? { rows: 0, sums: [] }
    const right = postgres[key] ?? { rows: 0, sums: [] }
    const sumsEqual =
      left.sums.length === right.sums.length &&
      left.sums.every((value, index) => value === right.sums[index])
    if (left.rows !== right.rows || !sumsEqual) {
      mismatches.push({
        d1Rows: left.rows,
        d1Sums: left.sums,
        groupKey: key,
        postgresRows: right.rows,
        postgresSums: right.sums,
      })
    }
  }
  return mismatches
}

// ---------------------------------------------------------------------------
// Key-set digest (webhook event-id set equality)
// ---------------------------------------------------------------------------

/**
 * Order-insensitive digest over a table's ENTIRE natural-key set: sha256
 * over the sorted keys. Used for `stripe_webhook_events` (the dedupe gate:
 * the event-id SET must be identical) and any table where set equality —
 * not just counts — is the acceptance.
 */
export const keySetDigestFromKeys = (
  keys: ReadonlyArray<string>,
): Readonly<{ count: number; digest: string }> => {
  const hash = createHash("sha256")
  for (const key of [...keys].sort()) {
    hash.update(key)
    hash.update("")
  }
  return { count: keys.length, digest: hash.digest("hex") }
}

export const postgresKeySetDigest = async (
  sql: SyncSql,
  table: BillingDomainTable,
): Promise<Readonly<{ count: number; digest: string }>> => {
  const unsafe = requireUnsafe(sql)
  const keyColumns = BILLING_DOMAIN_TABLE_SPECS[table].keyColumns
  const keyExpr = keyColumns
    .map((column) => `COALESCE(${column}, '<null>')`)
    .join(" || ':' || ")
  const rows = await unsafe(
    `SELECT ${keyExpr} AS row_key FROM ${table} ORDER BY row_key ASC`,
    [],
  )
  return keySetDigestFromKeys(rows.map((row) => String(row["row_key"] ?? "")))
}

// ---------------------------------------------------------------------------
// Counts + report assembly
// ---------------------------------------------------------------------------

export const postgresBillingRowCount = async (
  sql: SyncSql,
  table: BillingDomainTable,
): Promise<number> => {
  const unsafe = requireUnsafe(sql)
  const rows = await unsafe(`SELECT COUNT(*) AS total_rows FROM ${table}`, [])
  return Number(rows[0]?.["total_rows"] ?? 0)
}

export type BillingVerifyReport = Readonly<{
  table: BillingDomainTable
  countsMatch: boolean
  d1Total: number
  postgresTotal: number
  groupedMismatches: ReadonlyArray<
    Readonly<{ label: string; mismatches: ReadonlyArray<GroupedTallyMismatch> }>
  >
  keySetMatch: boolean | undefined
  newestHashMismatches: ReadonlyArray<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }>
}>

export const billingVerifyReportOk = (report: BillingVerifyReport): boolean =>
  report.countsMatch &&
  report.keySetMatch !== false &&
  report.newestHashMismatches.length === 0 &&
  report.groupedMismatches.every((group) => group.mismatches.length === 0)
