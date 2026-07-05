/**
 * KS-8.14 (#8325): business funnel / orders / referrals backfill core —
 * D1 → Postgres.
 *
 * Testable core behind `scripts/backfill-business.ts`, following the
 * KS-8.7 (`billing-backfill.ts`) template. Takes raw D1 rows (snake_case
 * objects, exactly as `wrangler d1 execute --json` returns them) and
 * copies them into the Postgres twins from migration
 * `0022_business_funnel.sql`.
 *
 * WRITE MODE: every table CONVERGE-upserts to the authoritative D1
 * snapshot (`ON CONFLICT (pk) DO UPDATE SET col = EXCLUDED.col`). This
 * domain's rows are copies of rows D1 has already accepted, so converging
 * can never re-make a consume-once attribution decision or an idempotency
 * decision — and converge (not DO NOTHING) is required because signup
 * fulfillment, pipeline stages, promise/fulfillment-loop state, buy-mode
 * spend counters, triage records, and attribution policy_state are
 * UPDATEd in place on D1: a backfill sweep must bring a stale mirror row
 * forward to the D1 value. Re-running a sweep against an already-converged
 * mirror is a no-op (idempotency contract; regression-tested).
 *
 * VERIFY (`--verify`): the §3.11 acceptance, exact or explain:
 *   - exact row counts per table;
 *   - ATTRIBUTION SET EQUALITY: order-insensitive digests over the
 *     payout-feeding attribution tuples (uniqueness keys + attribution /
 *     source ids + policy_state) for user/order/agent/business-signup
 *     referral attributions and business affiliate attributions, plus the
 *     referral_workflow_events / qa_swarm idempotency-key sets;
 *   - PROMISE-RECEIPT HASH EQUALITY: an order-insensitive digest over
 *     EVERY promise_transition_receipts full-row hash (the public
 *     product-promises registry must be continuously servable from either
 *     store);
 *   - FUNNEL COUNTS PER COHORT: exact per-(stage, source_kind) and
 *     per-(source_ref, stage) tallies for business_funnel_events and
 *     per-(event_kind, actor_class) for viral_agent_funnel_events;
 *   - money tallies: checkout kickoff cents per user, starter-credit
 *     cents/msat per window, buy-mode msats per state, workflow-event
 *     amounts per (asset, event_kind, policy_state), order cents per
 *     status, QA-swarm committed cents per payment path, pipeline quoted
 *     cents per stage;
 *   - newest-N row-hash comparison for every table.
 * Nothing "close enough": exact or explain.
 */

import { createHash } from "node:crypto"
import {
  BUSINESS_DOMAIN_TABLE_SPECS,
  BUSINESS_DOMAIN_TABLES,
  businessConvergeUpsertSql,
  normalizeBusinessValue,
  requireBusinessUnsafe,
  upsertBusinessRows,
  type BusinessDomainTable,
} from "./business-domain-tables.js"
import type { SyncSql } from "./sql.js"

export {
  BUSINESS_DOMAIN_TABLE_SPECS,
  BUSINESS_DOMAIN_TABLES,
  businessConvergeUpsertSql,
  requireBusinessUnsafe,
  upsertBusinessRows,
  type BusinessDomainTable,
}

export type D1SourceRow = Readonly<Record<string, unknown>>

// ---------------------------------------------------------------------------
// Row hashes (newest-N comparison + the promise-receipt set digest)
// ---------------------------------------------------------------------------

/**
 * Canonical row hash: the migration-order column values joined with unit
 * separators, sha256'd. Normalization matches `upsertBusinessRows`, so the
 * SAME D1 export row and its Postgres twin hash identically.
 */
export const businessRowHash = (
  table: BusinessDomainTable,
  row: D1SourceRow,
): string => {
  const columns = BUSINESS_DOMAIN_TABLE_SPECS[table].columns
  const hash = createHash("sha256")
  for (const column of columns) {
    const value = normalizeBusinessValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ key: string; hash: string }>

export const businessRowKey = (
  table: BusinessDomainTable,
  row: D1SourceRow,
): string =>
  BUSINESS_DOMAIN_TABLE_SPECS[table].keyColumns
    .map((column) => String(row[column] ?? "<null>"))
    .join(":")

export const businessNewestHashesFromRows = (
  table: BusinessDomainTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> =>
  rows.map((row) => ({
    hash: businessRowHash(table, row),
    key: businessRowKey(table, row),
  }))

/** The deterministic newest-first ORDER BY both sides use for the sample. */
export const businessNewestOrderSql = (table: BusinessDomainTable): string => {
  const spec = BUSINESS_DOMAIN_TABLE_SPECS[table]
  return `${spec.orderColumn} DESC, ${spec.keyColumns.join(" DESC, ")} DESC`
}

export const postgresBusinessNewestHashes = async (
  sql: SyncSql,
  table: BusinessDomainTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const unsafe = requireBusinessUnsafe(sql)
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${businessNewestOrderSql(table)} LIMIT $1`,
    [limit],
  )
  return businessNewestHashesFromRows(table, rows)
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
// Grouped exact tallies (funnel counts per cohort + money sums)
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
  /** Human label for reports, e.g. 'per-(stage, source_kind) counts'. */
  label: string
  groupColumns: ReadonlyArray<string>
  sumColumns: ReadonlyArray<string>
}>

/**
 * Reconciliation specs per table: the §3.11 "funnel counts per cohort"
 * acceptance plus exact money sums for every amount-bearing table.
 */
export const BUSINESS_GROUPED_TALLIES: Readonly<
  Partial<Record<BusinessDomainTable, ReadonlyArray<GroupedTallySpec>>>
> = {
  business_checkout_kickoffs: [
    {
      groupColumns: ["user_id"],
      label:
        "per-user total/setup/credit cents (setup + credit = total, to the cent)",
      sumColumns: [
        "total_amount_cents",
        "setup_fee_cents",
        "credit_grant_cents",
      ],
    },
  ],
  business_commitment_ledger: [
    {
      groupColumns: ["due_state", "commitment_kind"],
      label: "per-(due_state, commitment_kind) counts",
      sumColumns: [],
    },
  ],
  business_funnel_events: [
    {
      groupColumns: ["stage", "source_kind"],
      label: "FUNNEL COUNTS per (stage, source_kind) cohort",
      sumColumns: [],
    },
    {
      groupColumns: ["source_ref", "stage"],
      label: "FUNNEL COUNTS per (source_ref, stage) cohort",
      sumColumns: [],
    },
  ],
  business_pipeline_rows: [
    {
      groupColumns: ["stage"],
      label: "per-stage quoted min/max usd cents",
      sumColumns: ["quoted_min_usd_cents", "quoted_max_usd_cents"],
    },
  ],
  business_signup_requests: [
    {
      groupColumns: ["source_ref", "fulfillment_status"],
      label: "per-(source_ref, fulfillment_status) signup counts",
      sumColumns: [],
    },
  ],
  business_starter_credit_grants: [
    {
      groupColumns: ["window_ref"],
      label: "per-window usd cents + msat + cap cents",
      sumColumns: ["amount_usd_cents", "amount_msat", "amount_cap_usd_cents"],
    },
  ],
  buy_mode_campaigns: [
    {
      groupColumns: ["state"],
      label: "per-state spent/per-job/daily cap msats",
      sumColumns: ["spent_today_msats", "per_job_cap_msats", "daily_cap_msats"],
    },
  ],
  buy_mode_jobs: [
    {
      groupColumns: ["state"],
      label: "per-state amount_msats",
      sumColumns: ["amount_msats"],
    },
  ],
  promise_transition_receipts: [
    {
      groupColumns: ["promise_id", "to_state"],
      label: "per-(promise_id, to_state) receipt counts",
      sumColumns: [],
    },
  ],
  qa_swarm_first_engagements: [
    {
      groupColumns: ["payment_path"],
      label: "per-payment_path committed_amount_cents",
      sumColumns: ["committed_amount_cents"],
    },
  ],
  referral_workflow_events: [
    {
      groupColumns: ["asset", "event_kind", "policy_state"],
      label: "per-(asset, event_kind, policy_state) amount (exact)",
      sumColumns: ["amount"],
    },
  ],
  software_orders: [
    {
      groupColumns: ["status"],
      label: "per-status free_slice/quote cents",
      sumColumns: ["free_slice_cents", "quote_cents"],
    },
  ],
  viral_agent_funnel_events: [
    {
      groupColumns: ["event_kind", "actor_class"],
      label: "FUNNEL COUNTS per (event_kind, actor_class) cohort",
      sumColumns: [],
    },
  ],
}

/** SQL for a grouped tally — valid on BOTH SQLite/D1 and Postgres. */
export const groupedTallySql = (
  table: BusinessDomainTable,
  spec: GroupedTallySpec,
): string => {
  const keyExpr = spec.groupColumns
    .map((column) => `COALESCE(${column}, '<null>')`)
    .join(" || ':' || ")
  const sums = spec.sumColumns
    .map((column, index) => `COALESCE(SUM(${column}), 0) AS sum_${index}`)
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
  table: BusinessDomainTable,
  spec: GroupedTallySpec,
): Promise<GroupedTally> => {
  const unsafe = requireBusinessUnsafe(sql)
  return groupedTallyFromRows(
    spec,
    await unsafe(groupedTallySql(table, spec), []),
  )
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
// Set digests (attribution set equality + promise-receipt hash equality)
// ---------------------------------------------------------------------------

/**
 * The columns whose order-insensitive full-table SET digest is part of the
 * §3.11 acceptance. Attribution tables digest their payout-feeding tuple
 * (uniqueness key + attribution/source ids + policy_state); idempotency
 * ledgers digest the dedupe key set; `promise_transition_receipts` digests
 * ALL columns — that IS the promise-receipt hash-equality acceptance.
 */
export const BUSINESS_SET_DIGEST_COLUMNS: Readonly<
  Partial<Record<BusinessDomainTable, ReadonlyArray<string>>>
> = {
  agent_referral_attributions: [
    "agent_user_id",
    "referral_attribution_id",
    "referral_source_id",
    "policy_state",
  ],
  business_affiliate_attributions: [
    "attribution_ref",
    "code",
    "business_signup_request_id",
    "policy_state",
  ],
  business_signup_referral_attributions: [
    "business_signup_request_id",
    "referral_attribution_id",
    "referral_source_id",
    "policy_state",
  ],
  order_referral_attributions: [
    "software_order_id",
    "user_id",
    "referral_attribution_id",
    "referral_source_id",
    "policy_state",
  ],
  promise_transition_receipts:
    BUSINESS_DOMAIN_TABLE_SPECS.promise_transition_receipts.columns,
  qa_swarm_first_engagements: ["idempotency_key", "commitment_ref"],
  referral_workflow_events: ["idempotency_key"],
  user_referral_attributions: [
    "user_id",
    "referral_attribution_id",
    "referral_source_id",
    "policy_state",
  ],
}

/**
 * Order-insensitive digest over a set of composite keys: sha256 over the
 * sorted key strings. Missing/extra/differing members on either side
 * change the digest — set equality, not just counts.
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

export const businessSetDigestKeyFromRow = (
  columns: ReadonlyArray<string>,
  row: D1SourceRow,
): string =>
  columns
    .map((column) => {
      const value = normalizeBusinessValue(row[column])
      return value === null ? "<null>" : String(value)
    })
    .join("")

/** The set-digest projection SQL — valid on BOTH SQLite/D1 and Postgres. */
export const setDigestSelectSql = (
  table: BusinessDomainTable,
  columns: ReadonlyArray<string>,
): string => `SELECT ${columns.join(", ")} FROM ${table}`

export const postgresBusinessSetDigest = async (
  sql: SyncSql,
  table: BusinessDomainTable,
  columns: ReadonlyArray<string>,
): Promise<Readonly<{ count: number; digest: string }>> => {
  const unsafe = requireBusinessUnsafe(sql)
  const rows = await unsafe(setDigestSelectSql(table, columns), [])
  return keySetDigestFromKeys(
    rows.map((row) => businessSetDigestKeyFromRow(columns, row)),
  )
}

// ---------------------------------------------------------------------------
// Counts + report assembly
// ---------------------------------------------------------------------------

export const postgresBusinessRowCount = async (
  sql: SyncSql,
  table: BusinessDomainTable,
): Promise<number> => {
  const unsafe = requireBusinessUnsafe(sql)
  const rows = await unsafe(`SELECT COUNT(*) AS total_rows FROM ${table}`, [])
  return Number(rows[0]?.["total_rows"] ?? 0)
}

export type BusinessVerifyReport = Readonly<{
  table: BusinessDomainTable
  countsMatch: boolean
  d1Total: number
  postgresTotal: number
  groupedMismatches: ReadonlyArray<
    Readonly<{
      label: string
      mismatches: ReadonlyArray<GroupedTallyMismatch>
    }>
  >
  /** undefined = table has no set-digest acceptance. */
  setDigestMatch: boolean | undefined
  newestHashMismatches: ReadonlyArray<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }>
}>

export const businessVerifyReportOk = (
  report: BusinessVerifyReport,
): boolean =>
  report.countsMatch &&
  report.setDigestMatch !== false &&
  report.newestHashMismatches.length === 0 &&
  report.groupedMismatches.every((group) => group.mismatches.length === 0)
