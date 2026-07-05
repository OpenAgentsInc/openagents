/**
 * KS-8.15 remainder (#8355): gym / mullet / blueprint / replay-clip /
 * mirrorcode eval domain backfill + verification core.
 *
 * Copies D1 rows into the Cloud SQL twins using the same shared registry as
 * the Worker mirror, then verifies the KS-8.15 acceptance shape for this lane:
 *  - exact per-table row counts,
 *  - newest-N full-row sha256 hashes (byte-exact round-trip — the gym /
 *    ladder / mirrorcode rows feed PUBLIC projections). This is also the
 *    "leaderboard recomputation equality" acceptance for the derived
 *    snapshot tables (gym_ladder_leaderboard_snapshots /
 *    gym_run_progress_snapshots): the D1 write path already built the
 *    public-safe projection, the backfill copies those bytes verbatim, and
 *    the verifier proves equality by row-hash — Postgres NEVER recomputes a
 *    leaderboard.
 *  - state tallies for the tables with a lifecycle column (the
 *    Verified/queued/succeeded totals behind public projections).
 */

import { createHash } from "node:crypto"
import {
  GYM_EVALS_DOMAIN_TABLE_SPECS,
  GYM_EVALS_DOMAIN_TABLES,
  normalizeGymEvalsDomainValue,
  requireGymEvalsDomainUnsafe,
  upsertGymEvalsDomainRows,
  type GymEvalsDomainRow,
  type GymEvalsDomainTable,
} from "./gym-evals-domain-tables.js"
import type { SyncSql } from "./sql.js"

export {
  GYM_EVALS_DOMAIN_TABLE_SPECS,
  GYM_EVALS_DOMAIN_TABLES,
  upsertGymEvalsDomainRows,
  type GymEvalsDomainRow,
  type GymEvalsDomainTable,
}

export type GymEvalsDomainSourceRow = GymEvalsDomainRow

export type GymEvalsNewestRowHash = Readonly<{ key: string; hash: string }>

export const gymEvalsDomainRowKey = (
  table: GymEvalsDomainTable,
  row: GymEvalsDomainSourceRow,
): string =>
  GYM_EVALS_DOMAIN_TABLE_SPECS[table].keyColumns
    .map((column) => String(row[column] ?? "<null>"))
    .join(":")

export const gymEvalsDomainRowHash = (
  table: GymEvalsDomainTable,
  row: GymEvalsDomainSourceRow,
): string => {
  const hash = createHash("sha256")
  for (const column of GYM_EVALS_DOMAIN_TABLE_SPECS[table].columns) {
    const value = normalizeGymEvalsDomainValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("\x1f")
  }
  return hash.digest("hex")
}

export const gymEvalsDomainNewestOrderSql = (
  table: GymEvalsDomainTable,
): string => {
  const spec = GYM_EVALS_DOMAIN_TABLE_SPECS[table]
  return `${spec.orderColumn} DESC, ${spec.keyColumns.join(" DESC, ")} DESC`
}

export const gymEvalsDomainNewestHashesFromRows = (
  table: GymEvalsDomainTable,
  rows: ReadonlyArray<GymEvalsDomainSourceRow>,
): ReadonlyArray<GymEvalsNewestRowHash> =>
  rows.map((row) => ({
    hash: gymEvalsDomainRowHash(table, row),
    key: gymEvalsDomainRowKey(table, row),
  }))

export const postgresGymEvalsDomainRowCount = async (
  sql: SyncSql,
  table: GymEvalsDomainTable,
): Promise<number> => {
  const unsafe = requireGymEvalsDomainUnsafe(sql)
  const rows = await unsafe(`SELECT COUNT(*) AS total_rows FROM ${table}`, [])
  return Number(rows[0]?.["total_rows"] ?? 0)
}

export const postgresGymEvalsDomainNewestHashes = async (
  sql: SyncSql,
  table: GymEvalsDomainTable,
  limit: number,
): Promise<ReadonlyArray<GymEvalsNewestRowHash>> => {
  const unsafe = requireGymEvalsDomainUnsafe(sql)
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${gymEvalsDomainNewestOrderSql(table)} LIMIT $1`,
    [limit],
  )
  return gymEvalsDomainNewestHashesFromRows(table, rows)
}

export const compareGymEvalsNewestHashes = (
  d1Newest: ReadonlyArray<GymEvalsNewestRowHash>,
  postgresNewest: ReadonlyArray<GymEvalsNewestRowHash>,
): ReadonlyArray<
  Readonly<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }>
> => {
  const postgresByKey = new Map(
    postgresNewest.map((entry) => [entry.key, entry.hash]),
  )
  const mismatches: Array<
    Readonly<{
      key: string
      d1Hash: string | undefined
      postgresHash: string | undefined
    }>
  > = []
  for (const entry of d1Newest) {
    const postgresHash = postgresByKey.get(entry.key)
    if (postgresHash !== entry.hash) {
      mismatches.push({ d1Hash: entry.hash, key: entry.key, postgresHash })
    }
  }
  return mismatches
}

// ---------------------------------------------------------------------------
// State tallies (public-projection totals for lifecycle-bearing tables)
// ---------------------------------------------------------------------------

export type GymEvalsStateTally = ReadonlyArray<
  Readonly<{ state: string; total: number }>
>

/**
 * The tables whose lifecycle column feeds a public tally. The column differs:
 * most use `status`; the agentcl eval runs use `state`.
 */
export const GYM_EVALS_TALLY_TABLES: Readonly<
  Partial<Record<GymEvalsDomainTable, string>>
> = {
  gym_agentcl_eval_runs: "state",
  gym_mutalisk_khala_delegation_jobs: "latest_stage",
  mullet_simulation_runs: "status",
  blueprint_action_submissions: "status",
  blueprint_probe_contributions: "status",
  replay_clip_jobs: "status",
  mirrorcode_runs: "status",
}

export type GymEvalsTallyTable = keyof typeof GYM_EVALS_TALLY_TABLES

export const gymEvalsStateTallyFromRows = (
  column: string,
  rows: ReadonlyArray<GymEvalsDomainSourceRow>,
): GymEvalsStateTally => {
  const totals = new Map<string, number>()
  for (const row of rows) {
    const state = String(row[column] ?? "")
    totals.set(state, (totals.get(state) ?? 0) + 1)
  }
  return [...totals.entries()]
    .map(([state, total]) => ({ state, total }))
    .sort((left, right) => left.state.localeCompare(right.state))
}

export const postgresGymEvalsStateTally = async (
  sql: SyncSql,
  table: GymEvalsDomainTable,
  column: string,
): Promise<GymEvalsStateTally> => {
  const unsafe = requireGymEvalsDomainUnsafe(sql)
  const rows = await unsafe(
    `SELECT ${column} AS state, COUNT(*) AS total FROM ${table} GROUP BY ${column} ORDER BY ${column} ASC`,
    [],
  )
  return rows.map((row) => ({
    state: String(row["state"] ?? ""),
    total: Number(row["total"] ?? 0),
  }))
}

export const compareGymEvalsStateTallies = (
  d1Tally: GymEvalsStateTally,
  postgresTally: GymEvalsStateTally,
): boolean => JSON.stringify(d1Tally) === JSON.stringify(postgresTally)

// ---------------------------------------------------------------------------
// Verify report
// ---------------------------------------------------------------------------

export type GymEvalsDomainVerifyReport = Readonly<{
  countMismatches: ReadonlyArray<
    Readonly<{ table: GymEvalsDomainTable; d1: number; postgres: number }>
  >
  newestHashMismatches: ReadonlyArray<
    Readonly<{
      table: GymEvalsDomainTable
      mismatches: ReadonlyArray<
        Readonly<{
          key: string
          d1Hash: string | undefined
          postgresHash: string | undefined
        }>
      >
    }>
  >
  stateTallyMismatches: ReadonlyArray<
    Readonly<{
      table: GymEvalsDomainTable
      d1: GymEvalsStateTally
      postgres: GymEvalsStateTally
    }>
  >
}>

export const gymEvalsDomainVerifyReportOk = (
  report: GymEvalsDomainVerifyReport,
): boolean =>
  report.countMismatches.length === 0 &&
  report.newestHashMismatches.length === 0 &&
  report.stateTallyMismatches.length === 0
