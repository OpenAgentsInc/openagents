/**
 * KS-8.15 (#8326): training domain backfill + verification core.
 *
 * Copies D1 rows into the Cloud SQL twins using the same shared registry
 * as the Worker mirror, then verifies the KS-8.15 acceptance shape:
 *  - exact per-table row counts,
 *  - newest-N full-row sha256 hashes (byte-exact receipt round-trip —
 *    training receipts feed public claims),
 *  - window/lease chain equality: per-window ordered
 *    `training_window_events` chain fingerprints and per-window lease-set
 *    fingerprints,
 *  - verification-event chain contiguity: per-challenge ordered
 *    `training_verification_events` chain fingerprints,
 *  - state tallies for challenges and trace contributions (the
 *    Verified/Rejected/pending/paired totals behind public projections).
 *
 * (Gym leaderboard recomputation equality belongs to the gym/evals
 * remainder lane — leaderboard snapshots are gym tables.)
 */

import { createHash } from "node:crypto"
import {
  TRAINING_DOMAIN_TABLE_SPECS,
  TRAINING_DOMAIN_TABLES,
  normalizeTrainingDomainValue,
  requireTrainingDomainUnsafe,
  upsertTrainingDomainRows,
  type TrainingDomainRow,
  type TrainingDomainTable,
} from "./training-domain-tables.js"
import type { SyncSql } from "./sql.js"

export {
  TRAINING_DOMAIN_TABLE_SPECS,
  TRAINING_DOMAIN_TABLES,
  upsertTrainingDomainRows,
  type TrainingDomainRow,
  type TrainingDomainTable,
}

export type TrainingDomainSourceRow = TrainingDomainRow

export type TrainingNewestRowHash = Readonly<{ key: string; hash: string }>

export const trainingDomainRowKey = (
  table: TrainingDomainTable,
  row: TrainingDomainSourceRow,
): string =>
  TRAINING_DOMAIN_TABLE_SPECS[table].keyColumns
    .map((column) => String(row[column] ?? "<null>"))
    .join(":")

export const trainingDomainRowHash = (
  table: TrainingDomainTable,
  row: TrainingDomainSourceRow,
): string => {
  const hash = createHash("sha256")
  for (const column of TRAINING_DOMAIN_TABLE_SPECS[table].columns) {
    const value = normalizeTrainingDomainValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("\x1f")
  }
  return hash.digest("hex")
}

export const trainingDomainNewestOrderSql = (
  table: TrainingDomainTable,
): string => {
  const spec = TRAINING_DOMAIN_TABLE_SPECS[table]
  return `${spec.orderColumn} DESC, ${spec.keyColumns.join(" DESC, ")} DESC`
}

export const trainingDomainNewestHashesFromRows = (
  table: TrainingDomainTable,
  rows: ReadonlyArray<TrainingDomainSourceRow>,
): ReadonlyArray<TrainingNewestRowHash> =>
  rows.map((row) => ({
    hash: trainingDomainRowHash(table, row),
    key: trainingDomainRowKey(table, row),
  }))

export const postgresTrainingDomainRowCount = async (
  sql: SyncSql,
  table: TrainingDomainTable,
): Promise<number> => {
  const unsafe = requireTrainingDomainUnsafe(sql)
  const rows = await unsafe(`SELECT COUNT(*) AS total_rows FROM ${table}`, [])
  return Number(rows[0]?.["total_rows"] ?? 0)
}

export const postgresTrainingDomainNewestHashes = async (
  sql: SyncSql,
  table: TrainingDomainTable,
  limit: number,
): Promise<ReadonlyArray<TrainingNewestRowHash>> => {
  const unsafe = requireTrainingDomainUnsafe(sql)
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${trainingDomainNewestOrderSql(table)} LIMIT $1`,
    [limit],
  )
  return trainingDomainNewestHashesFromRows(table, rows)
}

export const compareTrainingNewestHashes = (
  d1Newest: ReadonlyArray<TrainingNewestRowHash>,
  postgresNewest: ReadonlyArray<TrainingNewestRowHash>,
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
// Chain fingerprints (window events, verification events, lease sets)
// ---------------------------------------------------------------------------

export type TrainingChainFingerprint = Readonly<{
  groupKey: string
  count: number
  firstCreatedAt: string | null
  lastCreatedAt: string | null
  digest: string
}>

export type TrainingChainTable =
  | "training_window_events"
  | "training_verification_events"

const chainParentColumn = (table: TrainingChainTable): string =>
  table === "training_window_events" ? "window_ref" : "challenge_ref"

/**
 * Per-parent ordered event-chain fingerprints: id + state_from→state_to +
 * created_at per link, ordered (created_at, id). Equal digests on both
 * stores prove the chain is contiguous AND transition-identical — the
 * KS-8.15 "verification-event chains contiguous" / "window chain
 * equality" acceptance.
 */
export const trainingChainFingerprintsFromRows = (
  table: TrainingChainTable,
  rows: ReadonlyArray<TrainingDomainSourceRow>,
): ReadonlyArray<TrainingChainFingerprint> => {
  const parentColumn = chainParentColumn(table)
  const groups = new Map<string, Array<TrainingDomainSourceRow>>()
  for (const row of rows) {
    const key = String(row[parentColumn] ?? "")
    groups.set(key, [...(groups.get(key) ?? []), row])
  }

  return [...groups.entries()]
    .map(([groupKey, groupRows]) => {
      const ordered = [...groupRows].sort((left, right) => {
        const leftCreated = String(left["created_at"] ?? "")
        const rightCreated = String(right["created_at"] ?? "")
        if (leftCreated !== rightCreated) {
          return leftCreated.localeCompare(rightCreated)
        }
        return String(left["id"] ?? "").localeCompare(String(right["id"] ?? ""))
      })
      const hash = createHash("sha256")
      for (const row of ordered) {
        hash.update(String(row["id"] ?? ""))
        hash.update("\x1f")
        hash.update(String(row["state_from"] ?? ""))
        hash.update("\x1f")
        hash.update(String(row["state_to"] ?? ""))
        hash.update("\x1f")
        hash.update(String(row["created_at"] ?? ""))
        hash.update("\x1e")
      }
      return {
        count: ordered.length,
        digest: hash.digest("hex"),
        firstCreatedAt:
          ordered.length === 0 ? null : String(ordered[0]?.["created_at"] ?? ""),
        groupKey,
        lastCreatedAt:
          ordered.length === 0
            ? null
            : String(ordered[ordered.length - 1]?.["created_at"] ?? ""),
      }
    })
    .sort((left, right) => left.groupKey.localeCompare(right.groupKey))
}

export const postgresTrainingChainFingerprints = async (
  sql: SyncSql,
  table: TrainingChainTable,
): Promise<ReadonlyArray<TrainingChainFingerprint>> => {
  const unsafe = requireTrainingDomainUnsafe(sql)
  const parentColumn = chainParentColumn(table)
  const rows = await unsafe(
    `SELECT id, ${parentColumn}, state_from, state_to, created_at
       FROM ${table}
      ORDER BY ${parentColumn} ASC, created_at ASC, id ASC`,
    [],
  )
  return trainingChainFingerprintsFromRows(table, rows)
}

export const compareTrainingChainFingerprints = (
  d1Chains: ReadonlyArray<TrainingChainFingerprint>,
  postgresChains: ReadonlyArray<TrainingChainFingerprint>,
): ReadonlyArray<
  Readonly<{
    groupKey: string
    d1: TrainingChainFingerprint | undefined
    postgres: TrainingChainFingerprint | undefined
  }>
> => {
  const postgresByKey = new Map(
    postgresChains.map((chain) => [chain.groupKey, chain]),
  )
  const d1ByKey = new Map(d1Chains.map((chain) => [chain.groupKey, chain]))
  const keys = new Set([...d1ByKey.keys(), ...postgresByKey.keys()])
  return [...keys].sort().flatMap((groupKey) => {
    const d1 = d1ByKey.get(groupKey)
    const postgres = postgresByKey.get(groupKey)
    return d1?.digest === postgres?.digest &&
      d1?.count === postgres?.count &&
      d1?.firstCreatedAt === postgres?.firstCreatedAt &&
      d1?.lastCreatedAt === postgres?.lastCreatedAt
      ? []
      : [{ d1, groupKey, postgres }]
  })
}

/**
 * Per-window lease-SET fingerprint (lease_ref + pylon_ref + state +
 * claimed_at + lease_expires_at, sorted by lease_ref). Equal sets on
 * both stores are the "window/lease chain equality" half that guards the
 * double-lease = double-payout risk.
 */
export type TrainingLeaseSetFingerprint = Readonly<{
  count: number
  digest: string
}>

export const trainingLeaseSetFingerprintFromRows = (
  rows: ReadonlyArray<TrainingDomainSourceRow>,
): TrainingLeaseSetFingerprint => {
  const keys = rows
    .map((row) =>
      [
        row["window_ref"],
        row["lease_ref"],
        row["pylon_ref"],
        row["state"],
        row["claimed_at"],
        row["lease_expires_at"],
        row["archived_at"] ?? "",
      ]
        .map((value) => String(value ?? ""))
        .join("\x1f"),
    )
    .sort()
  const hash = createHash("sha256")
  for (const key of keys) {
    hash.update(key)
    hash.update("\x1e")
  }
  return { count: keys.length, digest: hash.digest("hex") }
}

export const postgresTrainingLeaseSetFingerprint = async (
  sql: SyncSql,
): Promise<TrainingLeaseSetFingerprint> => {
  const unsafe = requireTrainingDomainUnsafe(sql)
  const rows = await unsafe(
    `SELECT window_ref, lease_ref, pylon_ref, state, claimed_at,
            lease_expires_at, archived_at
       FROM training_window_leases
      ORDER BY window_ref ASC, lease_ref ASC`,
    [],
  )
  return trainingLeaseSetFingerprintFromRows(rows)
}

// ---------------------------------------------------------------------------
// State tallies (public-projection totals)
// ---------------------------------------------------------------------------

export type TrainingStateTally = ReadonlyArray<
  Readonly<{ state: string; total: number }>
>

export const trainingStateTallyFromRows = (
  rows: ReadonlyArray<TrainingDomainSourceRow>,
): TrainingStateTally => {
  const totals = new Map<string, number>()
  for (const row of rows) {
    const state = String(row["state"] ?? "")
    totals.set(state, (totals.get(state) ?? 0) + 1)
  }
  return [...totals.entries()]
    .map(([state, total]) => ({ state, total }))
    .sort((left, right) => left.state.localeCompare(right.state))
}

export type TrainingTallyTable =
  | "training_verification_challenges"
  | "training_trace_contributions"

export const postgresTrainingStateTally = async (
  sql: SyncSql,
  table: TrainingTallyTable,
): Promise<TrainingStateTally> => {
  const unsafe = requireTrainingDomainUnsafe(sql)
  const rows = await unsafe(
    `SELECT state, COUNT(*) AS total FROM ${table} GROUP BY state ORDER BY state ASC`,
    [],
  )
  return rows.map((row) => ({
    state: String(row["state"] ?? ""),
    total: Number(row["total"] ?? 0),
  }))
}

export const compareTrainingStateTallies = (
  d1Tally: TrainingStateTally,
  postgresTally: TrainingStateTally,
): boolean =>
  JSON.stringify(d1Tally) === JSON.stringify(postgresTally)

// ---------------------------------------------------------------------------
// Verify report
// ---------------------------------------------------------------------------

export type TrainingDomainVerifyReport = Readonly<{
  countMismatches: ReadonlyArray<
    Readonly<{ table: TrainingDomainTable; d1: number; postgres: number }>
  >
  newestHashMismatches: ReadonlyArray<
    Readonly<{
      table: TrainingDomainTable
      mismatches: ReadonlyArray<
        Readonly<{
          key: string
          d1Hash: string | undefined
          postgresHash: string | undefined
        }>
      >
    }>
  >
  chainMismatches: ReadonlyArray<
    Readonly<{
      table: TrainingChainTable
      mismatches: ReturnType<typeof compareTrainingChainFingerprints>
    }>
  >
  leaseSetMismatch:
    | undefined
    | Readonly<{
        d1: TrainingLeaseSetFingerprint
        postgres: TrainingLeaseSetFingerprint
      }>
  stateTallyMismatches: ReadonlyArray<
    Readonly<{
      table: TrainingTallyTable
      d1: TrainingStateTally
      postgres: TrainingStateTally
    }>
  >
}>

export const trainingDomainVerifyReportOk = (
  report: TrainingDomainVerifyReport,
): boolean =>
  report.countMismatches.length === 0 &&
  report.newestHashMismatches.length === 0 &&
  report.chainMismatches.length === 0 &&
  report.leaseSetMismatch === undefined &&
  report.stateTallyMismatches.length === 0
