/**
 * KS-8.13 (#8324): Khala Code product-state backfill + verification core.
 *
 * Copies D1 rows into the Cloud SQL twins using the same shared registry as
 * the Worker mirror, then verifies the product-state acceptance shape:
 * exact table counts, newest-N row hashes, active membership set equality,
 * and ordered message-chain fingerprints for team chat and thread messages.
 */

import { createHash } from "node:crypto"
import {
  KHALA_CODE_PRODUCT_STATE_TABLE_SPECS,
  KHALA_CODE_PRODUCT_STATE_TABLES,
  normalizeKhalaCodeProductStateValue,
  requireKhalaCodeProductStateUnsafe,
  upsertKhalaCodeProductStateRows,
  type KhalaCodeProductStateRow,
  type KhalaCodeProductStateTable,
} from "./khala-code-product-state-tables.js"
import type { SyncSql } from "./sql.js"

export {
  KHALA_CODE_PRODUCT_STATE_TABLE_SPECS,
  KHALA_CODE_PRODUCT_STATE_TABLES,
  upsertKhalaCodeProductStateRows,
  type KhalaCodeProductStateRow,
  type KhalaCodeProductStateTable,
}

export type D1SourceRow = KhalaCodeProductStateRow

export type NewestRowHash = Readonly<{ key: string; hash: string }>

export const khalaCodeProductStateRowKey = (
  table: KhalaCodeProductStateTable,
  row: D1SourceRow,
): string =>
  KHALA_CODE_PRODUCT_STATE_TABLE_SPECS[table].keyColumns
    .map((column) => String(row[column] ?? "<null>"))
    .join(":")

export const khalaCodeProductStateRowHash = (
  table: KhalaCodeProductStateTable,
  row: D1SourceRow,
): string => {
  const hash = createHash("sha256")
  for (const column of KHALA_CODE_PRODUCT_STATE_TABLE_SPECS[table].columns) {
    const value = normalizeKhalaCodeProductStateValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("\x1f")
  }
  return hash.digest("hex")
}

export const khalaCodeProductStateNewestOrderSql = (
  table: KhalaCodeProductStateTable,
): string => {
  const spec = KHALA_CODE_PRODUCT_STATE_TABLE_SPECS[table]
  return `${spec.orderColumn} DESC, ${spec.keyColumns.join(" DESC, ")} DESC`
}

export const khalaCodeProductStateNewestHashesFromRows = (
  table: KhalaCodeProductStateTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> =>
  rows.map((row) => ({
    hash: khalaCodeProductStateRowHash(table, row),
    key: khalaCodeProductStateRowKey(table, row),
  }))

export const postgresKhalaCodeProductStateRowCount = async (
  sql: SyncSql,
  table: KhalaCodeProductStateTable,
): Promise<number> => {
  const unsafe = requireKhalaCodeProductStateUnsafe(sql)
  const rows = await unsafe(`SELECT COUNT(*) AS total_rows FROM ${table}`, [])
  return Number(rows[0]?.["total_rows"] ?? 0)
}

export const postgresKhalaCodeProductStateNewestHashes = async (
  sql: SyncSql,
  table: KhalaCodeProductStateTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const unsafe = requireKhalaCodeProductStateUnsafe(sql)
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${khalaCodeProductStateNewestOrderSql(table)} LIMIT $1`,
    [limit],
  )
  return khalaCodeProductStateNewestHashesFromRows(table, rows)
}

export const compareNewestHashes = (
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): ReadonlyArray<Readonly<{ key: string; d1Hash: string | undefined; postgresHash: string | undefined }>> => {
  const postgresByKey = new Map(
    postgresNewest.map((entry) => [entry.key, entry.hash]),
  )
  const mismatches: Array<
    Readonly<{ key: string; d1Hash: string | undefined; postgresHash: string | undefined }>
  > = []
  for (const entry of d1Newest) {
    const postgresHash = postgresByKey.get(entry.key)
    if (postgresHash !== entry.hash) {
      mismatches.push({ d1Hash: entry.hash, key: entry.key, postgresHash })
    }
  }
  return mismatches
}

export type MembershipSetFingerprint = Readonly<{
  count: number
  digest: string
}>

export const membershipSetFingerprintFromRows = (
  rows: ReadonlyArray<D1SourceRow>,
): MembershipSetFingerprint => {
  const keys = rows
    .map((row) =>
      [
        row["team_id"],
        row["user_id"],
        row["role"],
        row["status"],
        row["removed_at"] ?? "",
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

export const postgresMembershipSetFingerprint = async (
  sql: SyncSql,
): Promise<MembershipSetFingerprint> => {
  const unsafe = requireKhalaCodeProductStateUnsafe(sql)
  const rows = await unsafe(
    `SELECT team_id, user_id, role, status, removed_at
       FROM team_memberships
      ORDER BY team_id ASC, user_id ASC`,
    [],
  )
  return membershipSetFingerprintFromRows(rows)
}

export type MessageChainFingerprint = Readonly<{
  groupKey: string
  count: number
  firstCreatedAt: string | null
  lastCreatedAt: string | null
  digest: string
}>

const chainGroupKey = (
  table: "team_chat_messages" | "thread_messages",
  row: D1SourceRow,
): string =>
  table === "team_chat_messages"
    ? [
        row["team_id"],
        row["project_id"] ?? "",
        row["autopilot_thread_id"] ?? "",
      ]
        .map((value) => String(value ?? ""))
        .join("\x1f")
    : String(row["thread_id"] ?? "")

export const messageChainFingerprintsFromRows = (
  table: "team_chat_messages" | "thread_messages",
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<MessageChainFingerprint> => {
  const groups = new Map<string, Array<D1SourceRow>>()
  for (const row of rows) {
    const key = chainGroupKey(table, row)
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
        hash.update(String(row["created_at"] ?? ""))
        hash.update("\x1f")
        hash.update(String(row["version"] ?? ""))
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

export const postgresMessageChainFingerprints = async (
  sql: SyncSql,
  table: "team_chat_messages" | "thread_messages",
): Promise<ReadonlyArray<MessageChainFingerprint>> => {
  const unsafe = requireKhalaCodeProductStateUnsafe(sql)
  const rows =
    table === "team_chat_messages"
      ? await unsafe(
          `SELECT id, team_id, project_id, autopilot_thread_id, created_at
             FROM team_chat_messages
            WHERE deleted_at IS NULL AND archived_at IS NULL
            ORDER BY team_id ASC, project_id ASC, autopilot_thread_id ASC, created_at ASC, id ASC`,
          [],
        )
      : await unsafe(
          `SELECT id, thread_id, created_at, version
             FROM thread_messages
            WHERE deleted_at IS NULL AND archived_at IS NULL
            ORDER BY thread_id ASC, created_at ASC, id ASC`,
          [],
        )
  return messageChainFingerprintsFromRows(table, rows)
}

export const compareMessageChainFingerprints = (
  d1Chains: ReadonlyArray<MessageChainFingerprint>,
  postgresChains: ReadonlyArray<MessageChainFingerprint>,
): ReadonlyArray<
  Readonly<{
    groupKey: string
    d1: MessageChainFingerprint | undefined
    postgres: MessageChainFingerprint | undefined
  }>
> => {
  const postgresByKey = new Map(
    postgresChains.map((chain) => [chain.groupKey, chain]),
  )
  const d1ByKey = new Map(d1Chains.map((chain) => [chain.groupKey, chain]))
  const keys = new Set([...d1ByKey.keys(), ...postgresByKey.keys()])
  return [...keys]
    .sort()
    .flatMap((groupKey) => {
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

export type KhalaCodeProductStateVerifyReport = Readonly<{
  countMismatches: ReadonlyArray<
    Readonly<{ table: KhalaCodeProductStateTable; d1: number; postgres: number }>
  >
  newestHashMismatches: ReadonlyArray<
    Readonly<{
      table: KhalaCodeProductStateTable
      mismatches: ReadonlyArray<
        Readonly<{ key: string; d1Hash: string | undefined; postgresHash: string | undefined }>
      >
    }>
  >
  membershipSetMismatch:
    | undefined
    | Readonly<{
        d1: MembershipSetFingerprint
        postgres: MembershipSetFingerprint
      }>
  messageChainMismatches: ReadonlyArray<
    Readonly<{
      table: "team_chat_messages" | "thread_messages"
      mismatches: ReturnType<typeof compareMessageChainFingerprints>
    }>
  >
}>

export const khalaCodeProductStateVerifyReportOk = (
  report: KhalaCodeProductStateVerifyReport,
): boolean =>
  report.countMismatches.length === 0 &&
  report.newestHashMismatches.length === 0 &&
  report.membershipSetMismatch === undefined &&
  report.messageChainMismatches.length === 0
