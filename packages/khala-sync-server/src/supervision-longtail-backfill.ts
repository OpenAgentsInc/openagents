/**
 * KS-8.17 (#8328): supervision long-tail (Adjutant / Omni / Autopilot / ops)
 * backfill + verification core — D1 → Postgres.
 *
 * Testable core behind `scripts/backfill-supervision-longtail.ts`, following
 * the KS-8.16 template (`forge-backfill.ts`). Takes raw D1 rows (snake_case
 * objects, exactly as `wrangler d1 execute --json` returns them) and
 * converges them into the Postgres twins from migration
 * `0024_supervision_longtail.sql` via the SHARED registry in
 * `./supervision-longtail-domain-tables.ts` (the same
 * `upsertSupervisionLongtailRows` the Worker's dual-write mirror uses —
 * backfill and mirror can never fight because they write identical converge
 * upserts keyed on the composite PK).
 *
 * Verification (`verify*`), matching the §3.14 acceptance:
 *   - exact row counts per table;
 *   - domain scalar tallies (per-state/severity counts, token/credit/cost
 *     sums) run verbatim on BOTH engines;
 *   - IDEMPOTENCY KEY SET EQUALITY for `omni_idempotency_keys` (the pure
 *     idempotency table — the acceptance keys on key-set equality);
 *   - PUBLIC PROOF-BUNDLE DIGESTS per (workroom) for
 *     `omni_public_proof_bundles` — the shadow-compared public projection
 *     surface (sha256 over the ordered public-safe projection fields);
 *   - newest-N full row hashes per table.
 *
 * SECRETS (SPEC invariant 9): output references row KEYS and sha256 hashes
 * only. Custody columns (`custodyColumns` in the registry: transcript/
 * metadata/entries/result/receipt JSON) participate in the row hash — a hash,
 * never the value — and are NEVER selected into a tally or printed.
 */

import { createHash } from "node:crypto"
import {
  SUPERVISION_LONGTAIL_TABLE_SPECS,
  SUPERVISION_LONGTAIL_TABLES,
  isSupervisionLongtailTable,
  normalizeSupervisionLongtailValue,
  requireSupervisionLongtailUnsafe,
  upsertSupervisionLongtailRows,
  type SupervisionLongtailRow,
  type SupervisionLongtailTable,
} from "./supervision-longtail-domain-tables.js"
import type { SyncSql } from "./sql.js"

export {
  SUPERVISION_LONGTAIL_TABLE_SPECS,
  SUPERVISION_LONGTAIL_TABLES,
  isSupervisionLongtailTable,
  upsertSupervisionLongtailRows,
  type SupervisionLongtailRow,
  type SupervisionLongtailTable,
}

export type D1SupervisionLongtailSourceRow = SupervisionLongtailRow

// ---------------------------------------------------------------------------
// Row hashes
// ---------------------------------------------------------------------------

/**
 * Canonical row hash: the registry-order column values joined with unit
 * separators, sha256'd. Normalization matches `upsertSupervisionLongtailRows`
 * so the SAME D1 export row and its Postgres twin hash identically (bigint
 * counters come back as strings from postgres.js; `String()` canonicalizes
 * both sides).
 */
export const supervisionLongtailRowHash = (
  table: SupervisionLongtailTable,
  row: D1SupervisionLongtailSourceRow,
): string => {
  const columns = SUPERVISION_LONGTAIL_TABLE_SPECS[table].columns
  const hash = createHash("sha256")
  for (const column of columns) {
    const value = normalizeSupervisionLongtailValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type SupervisionNewestRowHash = Readonly<{ key: string; hash: string }>

/** Public-safe row key: the composite PK values joined with '/'. */
export const supervisionLongtailRowKey = (
  table: SupervisionLongtailTable,
  row: D1SupervisionLongtailSourceRow,
): string =>
  SUPERVISION_LONGTAIL_TABLE_SPECS[table].keyColumns
    .map((column) => String(row[column] ?? "<null>"))
    .join("/")

export const d1SupervisionLongtailNewestHashes = (
  table: SupervisionLongtailTable,
  rows: ReadonlyArray<D1SupervisionLongtailSourceRow>,
): ReadonlyArray<SupervisionNewestRowHash> =>
  rows.map((row) => ({
    hash: supervisionLongtailRowHash(table, row),
    key: supervisionLongtailRowKey(table, row),
  }))

/** Newest-first ORDER BY clause for a table (order column, then PK). */
export const supervisionLongtailNewestOrderSql = (
  table: SupervisionLongtailTable,
): string => {
  const spec = SUPERVISION_LONGTAIL_TABLE_SPECS[table]
  const keys = spec.keyColumns.map((column) => `${column} DESC`).join(", ")
  return `${spec.orderColumn} DESC, ${keys}`
}

export const postgresSupervisionLongtailNewestHashes = async (
  sql: SyncSql,
  table: SupervisionLongtailTable,
  limit: number,
): Promise<ReadonlyArray<SupervisionNewestRowHash>> => {
  const unsafe = requireSupervisionLongtailUnsafe(sql)
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${supervisionLongtailNewestOrderSql(table)} LIMIT $1`,
    [limit],
  )
  return rows.map((row) => ({
    hash: supervisionLongtailRowHash(table, row),
    key: supervisionLongtailRowKey(table, row),
  }))
}

// ---------------------------------------------------------------------------
// Counts and scalar tallies
// ---------------------------------------------------------------------------

export const postgresSupervisionLongtailRowCount = async (
  sql: SyncSql,
  table: SupervisionLongtailTable,
): Promise<number> => {
  const unsafe = requireSupervisionLongtailUnsafe(sql)
  const rows = await unsafe(`SELECT COUNT(*) AS total_rows FROM ${table}`, [])
  return Number(rows[0]?.["total_rows"] ?? 0)
}

/**
 * Domain scalar tallies per table (compared exactly across stores). SQL text
 * is portable and runs verbatim on D1 AND Postgres. No tally selects a
 * custody column value. Tables without a listed tally are still fully
 * verified by exact counts + newest-N row hashes.
 */
export const SUPERVISION_LONGTAIL_SCALAR_TALLIES: Readonly<
  Partial<
    Record<
      SupervisionLongtailTable,
      ReadonlyArray<Readonly<{ metric: string; sql: string }>>
    >
  >
> = {
  adjutant_assignments: [
    {
      metric: "live_assignments",
      sql: `SELECT COUNT(*) AS value FROM adjutant_assignments WHERE archived_at IS NULL AND status NOT IN ('complete', 'canceled')`,
    },
    {
      metric: "complete_assignments",
      sql: `SELECT COUNT(*) AS value FROM adjutant_assignments WHERE status = 'complete'`,
    },
  ],
  adjutant_usage_receipts: [
    {
      metric: "sum_credits_charged_cents",
      sql: `SELECT COALESCE(SUM(credits_charged_cents), 0) AS value FROM adjutant_usage_receipts`,
    },
    {
      metric: "distinct_assignments",
      sql: `SELECT COUNT(DISTINCT assignment_id) AS value FROM adjutant_usage_receipts`,
    },
  ],
  autopilot_token_usage: [
    {
      metric: "sum_total_tokens",
      sql: `SELECT COALESCE(SUM(total_tokens), 0) AS value FROM autopilot_token_usage`,
    },
    {
      metric: "distinct_runs",
      sql: `SELECT COUNT(DISTINCT run_id) AS value FROM autopilot_token_usage`,
    },
  ],
  autopilot_work_orders: [
    {
      metric: "scheduled_launches_pending",
      sql: `SELECT COUNT(*) AS value FROM autopilot_work_orders WHERE scheduled_launch_json IS NOT NULL`,
    },
    {
      metric: "distinct_owners",
      sql: `SELECT COUNT(DISTINCT owner_user_id) AS value FROM autopilot_work_orders`,
    },
  ],
  autopilot_continuation_events: [
    {
      metric: "dispatched_events",
      sql: `SELECT COUNT(*) AS value FROM autopilot_continuation_events WHERE decision = 'dispatched'`,
    },
  ],
  omni_accepted_outcome_economics: [
    {
      metric: "sum_total_cost_cents",
      sql: `SELECT COALESCE(SUM(total_cost_cents), 0) AS value FROM omni_accepted_outcome_economics`,
    },
    {
      metric: "sum_accepted_value_cents",
      sql: `SELECT COALESCE(SUM(accepted_value_cents), 0) AS value FROM omni_accepted_outcome_economics`,
    },
  ],
  omni_public_proof_bundles: [
    {
      metric: "ready_bundles",
      sql: `SELECT COUNT(*) AS value FROM omni_public_proof_bundles WHERE status = 'ready'`,
    },
  ],
  omni_workrooms: [
    {
      metric: "active_workrooms",
      sql: `SELECT COUNT(*) AS value FROM omni_workrooms WHERE status = 'active'`,
    },
  ],
  relay_health_probes: [
    {
      metric: "healthy_probes",
      sql: `SELECT COUNT(*) AS value FROM relay_health_probes WHERE status = 'healthy'`,
    },
  ],
  backend_incident_events: [
    {
      metric: "critical_incidents",
      sql: `SELECT COUNT(*) AS value FROM backend_incident_events WHERE severity = 'critical'`,
    },
    {
      metric: "sum_occurrences",
      sql: `SELECT COALESCE(SUM(occurrence_count), 0) AS value FROM backend_incident_events`,
    },
  ],
  hygiene_debt_receipts: [
    {
      metric: "payable_receipts",
      sql: `SELECT COUNT(*) AS value FROM hygiene_debt_receipts WHERE state = 'payable'`,
    },
    {
      metric: "sum_payable_sats",
      sql: `SELECT COALESCE(SUM(payable_sats), 0) AS value FROM hygiene_debt_receipts`,
    },
  ],
}

export const postgresSupervisionLongtailScalar = async (
  sql: SyncSql,
  tallySql: string,
): Promise<number> => {
  const unsafe = requireSupervisionLongtailUnsafe(sql)
  const rows = await unsafe(tallySql, [])
  return Number(rows[0]?.["value"] ?? 0)
}

// ---------------------------------------------------------------------------
// Idempotency key-set equality (omni_idempotency_keys)
// ---------------------------------------------------------------------------

/** Portable key scan — runs verbatim on D1 and Postgres. */
export const omniIdempotencyKeySql = (): string =>
  `SELECT key FROM omni_idempotency_keys ORDER BY key ASC`

export const idempotencyKeySetFromRows = (
  rows: ReadonlyArray<Record<string, unknown>>,
): ReadonlySet<string> => new Set(rows.map((row) => String(row["key"])))

export const postgresOmniIdempotencyKeySet = async (
  sql: SyncSql,
): Promise<ReadonlySet<string>> => {
  const unsafe = requireSupervisionLongtailUnsafe(sql)
  return idempotencyKeySetFromRows(await unsafe(omniIdempotencyKeySql(), []))
}

export type IdempotencyKeySetComparison = Readonly<{
  d1Count: number
  postgresCount: number
  missingInPostgres: ReadonlyArray<string>
  extraInPostgres: ReadonlyArray<string>
  equal: boolean
}>

export const compareIdempotencyKeySets = (
  d1: ReadonlySet<string>,
  postgres: ReadonlySet<string>,
): IdempotencyKeySetComparison => {
  const missingInPostgres = [...d1].filter((key) => !postgres.has(key)).sort()
  const extraInPostgres = [...postgres].filter((key) => !d1.has(key)).sort()
  return {
    d1Count: d1.size,
    equal: missingInPostgres.length === 0 && extraInPostgres.length === 0,
    extraInPostgres,
    missingInPostgres,
    postgresCount: postgres.size,
  }
}

// ---------------------------------------------------------------------------
// Public proof-bundle digests (omni_public_proof_bundles) — the shadow-
// compared public projection surface (§3.14 acceptance)
// ---------------------------------------------------------------------------

/**
 * Portable public-projection scan for the proof-bundle surface: the
 * public-safe fields a servable proof bundle advertises, deterministically
 * ordered. Runs verbatim on D1 and Postgres. No custody column
 * (`metadata_json`) is selected.
 */
export const proofBundleProjectionSql = (): string =>
  `SELECT workroom_id, id, work_kind, status, legal_sensitive,
          review_state_ref, acceptance_state_ref, economics_caveat_ref,
          privacy_caveat_ref, public_receipt_ref, no_settlement_implication,
          updated_at
   FROM omni_public_proof_bundles
   WHERE archived_at IS NULL
   ORDER BY workroom_id ASC, id ASC`

export type ProofBundleDigest = Readonly<{ bundles: number; digest: string }>

/** Per-workroom proof-bundle projection digest, keyed by workroom_id. */
export const proofBundleDigestFromRows = (
  rows: ReadonlyArray<Record<string, unknown>>,
): ReadonlyMap<string, ProofBundleDigest> => {
  const grouped = new Map<
    string,
    { bundles: number; hash: ReturnType<typeof createHash> }
  >()
  for (const row of rows) {
    const key = String(row["workroom_id"])
    let entry = grouped.get(key)
    if (entry === undefined) {
      entry = { bundles: 0, hash: createHash("sha256") }
      grouped.set(key, entry)
    }
    entry.bundles += 1
    for (const column of [
      "id",
      "work_kind",
      "status",
      "legal_sensitive",
      "review_state_ref",
      "acceptance_state_ref",
      "economics_caveat_ref",
      "privacy_caveat_ref",
      "public_receipt_ref",
      "no_settlement_implication",
      "updated_at",
    ]) {
      entry.hash.update(String(row[column] ?? " "))
      entry.hash.update("")
    }
    entry.hash.update("")
  }
  return new Map(
    [...grouped.entries()].map(([key, entry]) => [
      key,
      { bundles: entry.bundles, digest: entry.hash.digest("hex") },
    ]),
  )
}

export const postgresProofBundleDigest = async (
  sql: SyncSql,
): Promise<ReadonlyMap<string, ProofBundleDigest>> => {
  const unsafe = requireSupervisionLongtailUnsafe(sql)
  return proofBundleDigestFromRows(await unsafe(proofBundleProjectionSql(), []))
}

export type ProofBundleMismatch = Readonly<{
  workroom: string
  d1: ProofBundleDigest | undefined
  postgres: ProofBundleDigest | undefined
}>

export const compareProofBundleDigests = (
  d1: ReadonlyMap<string, ProofBundleDigest>,
  postgres: ReadonlyMap<string, ProofBundleDigest>,
): ReadonlyArray<ProofBundleMismatch> => {
  const mismatches: Array<ProofBundleMismatch> = []
  const keys = new Set([...d1.keys(), ...postgres.keys()])
  for (const key of [...keys].sort()) {
    const left = d1.get(key)
    const right = postgres.get(key)
    if (
      left === undefined ||
      right === undefined ||
      left.digest !== right.digest ||
      left.bundles !== right.bundles
    ) {
      mismatches.push({ d1: left, postgres: right, workroom: key })
    }
  }
  return mismatches
}

// ---------------------------------------------------------------------------
// Verify report
// ---------------------------------------------------------------------------

export type SupervisionScalarMismatch = Readonly<{
  metric: string
  d1: number
  postgres: number
}>

export type SupervisionNewestHashMismatch = Readonly<{
  key: string
  d1Hash: string | undefined
  postgresHash: string | undefined
}>

export type SupervisionLongtailVerifyReport = Readonly<{
  table: SupervisionLongtailTable
  d1Total: number
  postgresTotal: number
  countsMatch: boolean
  scalarMismatches: ReadonlyArray<SupervisionScalarMismatch>
  newestHashMismatches: ReadonlyArray<SupervisionNewestHashMismatch>
}>

export const buildSupervisionLongtailVerifyReport = (input: {
  table: SupervisionLongtailTable
  d1Total: number
  postgresTotal: number
  scalars: ReadonlyArray<{ metric: string; d1: number; postgres: number }>
  d1Newest: ReadonlyArray<SupervisionNewestRowHash>
  postgresNewest: ReadonlyArray<SupervisionNewestRowHash>
}): SupervisionLongtailVerifyReport => {
  const scalarMismatches = input.scalars.filter(
    (scalar) => scalar.d1 !== scalar.postgres,
  )
  const postgresByKey = new Map(
    input.postgresNewest.map((entry) => [entry.key, entry.hash]),
  )
  const newestHashMismatches: Array<SupervisionNewestHashMismatch> = []
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

export const supervisionLongtailVerifyReportClean = (
  report: SupervisionLongtailVerifyReport,
): boolean =>
  report.countsMatch &&
  report.scalarMismatches.length === 0 &&
  report.newestHashMismatches.length === 0
