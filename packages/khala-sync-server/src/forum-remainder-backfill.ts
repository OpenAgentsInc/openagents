/**
 * KS-8.10 remainder (#8338): forum remainder backfill + verification core —
 * D1 → Postgres.
 *
 * Testable core behind `scripts/backfill-forum-remainder.ts`, following the
 * parent content lane (`forum-content-backfill.ts`, #8321). Takes raw D1
 * rows (snake_case objects, exactly as `wrangler d1 execute --json`
 * returns them) and converges them into the Postgres twins from migration
 * `0026_forum_remainder.sql` via the SHARED registry in
 * `./forum-remainder-tables.ts` (the same `upsertForumRemainderRows` the
 * Worker's dual-write mirror uses — backfill and mirror can never fight
 * because they write identical converge upserts keyed on the PK).
 *
 * Verification (`verify*`) matches the KS-8.10 acceptance specifics for the
 * remainder family:
 *
 *   - exact row counts per table;
 *   - domain scalar tallies (private-message/thread counts, work-request
 *     state tallies, offer/quote sums, notification-read tallies) — byte
 *     lengths and counts only, never subjects or message content;
 *   - TRUST RECOMPUTE-AND-COMPARE: `forum_trust_edges` grouped edge tallies
 *     (per target_actor / forum / trust_kind: count + weight sum) computed
 *     identically on both stores and compared — the "trust recomputation
 *     equality" acceptance at the storage layer (the recompute authority is
 *     D1; we verify the mirror reproduces the same inputs and derived
 *     aggregates);
 *   - WORK-REQUEST SET-MEMBERSHIP referential checks: every lifecycle
 *     child's `work_request_id` (and acceptance/result `offer_id`) resolves
 *     to a parent row WITHIN each store (no cross-store joins), and the
 *     distinct referenced-id sets — including the cross-domain refs into
 *     KS-8.1 assignments / KS-8.8 tips (escrow_id, reserve_receipt_ref,
 *     quote_ref, receipt_ref) — are byte-identical across stores;
 *   - newest-N full row hashes per table.
 *
 * PRIVACY: output references row KEYS and sha256 hashes only — never
 * message subjects, participant lists, or message content.
 */

import { createHash } from "node:crypto"
import {
  FORUM_REMAINDER_TABLE_COLUMNS,
  FORUM_REMAINDER_TABLE_PK,
  FORUM_REMAINDER_TABLES,
  isForumRemainderTable,
  upsertForumRemainderRows,
  type ForumRemainderRow,
  type ForumRemainderTable,
} from "./forum-remainder-tables.js"
import {
  normalizeForumContentValue,
  requireForumContentUnsafe,
} from "./forum-content-tables.js"
import type { SyncSql } from "./sql.js"

export {
  FORUM_REMAINDER_TABLE_COLUMNS,
  FORUM_REMAINDER_TABLE_PK,
  FORUM_REMAINDER_TABLES,
  isForumRemainderTable,
  upsertForumRemainderRows,
  type ForumRemainderRow,
  type ForumRemainderTable,
}

export type D1RemainderRow = ForumRemainderRow

/** Newest-first ordering column per table (for the hash sample). */
export const FORUM_REMAINDER_TABLE_ORDER: Readonly<
  Record<ForumRemainderTable, string>
> = {
  forum_acl_grants: "created_at",
  forum_actor_forum_trust: "updated_at",
  forum_notification_reads: "updated_at",
  forum_private_message_threads: "updated_at",
  forum_private_messages: "created_at",
  forum_score_snapshots: "created_at",
  forum_trust_edges: "created_at",
  forum_work_request_acceptances: "created_at",
  forum_work_request_lifecycle_posts: "created_at",
  forum_work_request_offers: "updated_at",
  forum_work_request_relay_links: "created_at",
  forum_work_request_results: "created_at",
  forum_work_requests: "updated_at",
}

// ---------------------------------------------------------------------------
// Row hashes
// ---------------------------------------------------------------------------

/**
 * Canonical row hash: the registry-order column values joined with unit
 * separators, sha256'd. Normalization matches `upsertForumRemainderRows`,
 * so the SAME D1 export row and its Postgres twin hash identically
 * (bigint counters come back as strings from postgres.js; `String()`
 * canonicalizes both sides).
 */
export const forumRemainderRowHash = (
  table: ForumRemainderTable,
  row: D1RemainderRow,
): string => {
  const columns = FORUM_REMAINDER_TABLE_COLUMNS[table]
  const hash = createHash("sha256")
  for (const column of columns) {
    const value = normalizeForumContentValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ key: string; hash: string }>

const rowKey = (table: ForumRemainderTable, row: D1RemainderRow): string =>
  String(row[FORUM_REMAINDER_TABLE_PK[table]] ?? "<null>")

export const d1ForumRemainderNewestHashes = (
  table: ForumRemainderTable,
  rows: ReadonlyArray<D1RemainderRow>,
): ReadonlyArray<NewestRowHash> =>
  rows.map((row) => ({
    hash: forumRemainderRowHash(table, row),
    key: rowKey(table, row),
  }))

export const postgresForumRemainderNewestHashes = async (
  sql: SyncSql,
  table: ForumRemainderTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const unsafe = requireForumContentUnsafe(sql)
  const orderColumn = FORUM_REMAINDER_TABLE_ORDER[table]
  const pk = FORUM_REMAINDER_TABLE_PK[table]
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${orderColumn} DESC, ${pk} DESC LIMIT $1`,
    [limit],
  )
  return rows.map((row) => ({
    hash: forumRemainderRowHash(table, row),
    key: rowKey(table, row),
  }))
}

// ---------------------------------------------------------------------------
// Counts and scalar tallies
// ---------------------------------------------------------------------------

export const postgresForumRemainderRowCount = async (
  sql: SyncSql,
  table: ForumRemainderTable,
): Promise<number> => {
  const unsafe = requireForumContentUnsafe(sql)
  const rows = await unsafe(`SELECT COUNT(*) AS total_rows FROM ${table}`, [])
  return Number(rows[0]?.["total_rows"] ?? 0)
}

/**
 * Domain scalar tallies per table (compared exactly across stores). The
 * SQL text is portable and runs verbatim on D1 AND Postgres so both sides
 * compute the same numbers over the same rows. No column selects any
 * subject, participant, or message content — counts, sums, and byte
 * lengths only.
 */
export const FORUM_REMAINDER_SCALAR_TALLIES: Readonly<
  Record<
    ForumRemainderTable,
    ReadonlyArray<Readonly<{ metric: string; sql: string }>>
  >
> = {
  forum_acl_grants: [
    {
      metric: "active_grants",
      sql: `SELECT COUNT(*) AS value FROM forum_acl_grants WHERE revoked_at IS NULL`,
    },
    {
      metric: "distinct_grantees",
      sql: `SELECT COUNT(DISTINCT actor_ref) AS value FROM forum_acl_grants`,
    },
  ],
  forum_actor_forum_trust: [
    {
      metric: "sum_trust_score",
      sql: `SELECT COALESCE(SUM(trust_score), 0) AS value FROM forum_actor_forum_trust`,
    },
    {
      metric: "sum_reward_count",
      sql: `SELECT COALESCE(SUM(reward_count), 0) AS value FROM forum_actor_forum_trust`,
    },
    {
      metric: "sum_report_count",
      sql: `SELECT COALESCE(SUM(report_count), 0) AS value FROM forum_actor_forum_trust`,
    },
  ],
  forum_notification_reads: [
    {
      metric: "active_reads",
      sql: `SELECT COUNT(*) AS value FROM forum_notification_reads WHERE archived_at IS NULL`,
    },
    {
      metric: "distinct_actors",
      sql: `SELECT COUNT(DISTINCT actor_ref) AS value FROM forum_notification_reads`,
    },
  ],
  forum_private_message_threads: [
    {
      metric: "active_threads",
      sql: `SELECT COUNT(*) AS value FROM forum_private_message_threads WHERE archived_at IS NULL`,
    },
    {
      metric: "sum_message_count",
      sql: `SELECT COALESCE(SUM(message_count), 0) AS value FROM forum_private_message_threads`,
    },
  ],
  forum_private_messages: [
    {
      metric: "active_messages",
      sql: `SELECT COUNT(*) AS value FROM forum_private_messages WHERE archived_at IS NULL`,
    },
    {
      metric: "distinct_threads",
      sql: `SELECT COUNT(DISTINCT thread_id) AS value FROM forum_private_messages`,
    },
  ],
  forum_score_snapshots: [
    {
      metric: "sum_positive_sats",
      sql: `SELECT COALESCE(SUM(positive_bitcoin_sats), 0) AS value FROM forum_score_snapshots`,
    },
    {
      metric: "sum_reply_count",
      sql: `SELECT COALESCE(SUM(reply_count), 0) AS value FROM forum_score_snapshots`,
    },
  ],
  forum_trust_edges: [
    {
      metric: "active_edges",
      sql: `SELECT COUNT(*) AS value FROM forum_trust_edges WHERE archived_at IS NULL`,
    },
    {
      metric: "sum_weight",
      sql: `SELECT COALESCE(SUM(weight), 0) AS value FROM forum_trust_edges`,
    },
  ],
  forum_work_request_acceptances: [
    {
      metric: "acceptances",
      sql: `SELECT COUNT(*) AS value FROM forum_work_request_acceptances`,
    },
    {
      metric: "sum_amount_msats",
      sql: `SELECT COALESCE(SUM(amount_msats), 0) AS value FROM forum_work_request_acceptances`,
    },
  ],
  forum_work_request_lifecycle_posts: [
    {
      metric: "lifecycle_posts",
      sql: `SELECT COUNT(*) AS value FROM forum_work_request_lifecycle_posts`,
    },
  ],
  forum_work_request_offers: [
    {
      metric: "offered",
      sql: `SELECT COUNT(*) AS value FROM forum_work_request_offers WHERE state = 'offered'`,
    },
    {
      metric: "accepted",
      sql: `SELECT COUNT(*) AS value FROM forum_work_request_offers WHERE state = 'accepted'`,
    },
    {
      metric: "sum_amount_msats",
      sql: `SELECT COALESCE(SUM(amount_msats), 0) AS value FROM forum_work_request_offers`,
    },
  ],
  forum_work_request_relay_links: [
    {
      metric: "relay_links",
      sql: `SELECT COUNT(*) AS value FROM forum_work_request_relay_links`,
    },
  ],
  forum_work_request_results: [
    {
      metric: "results",
      sql: `SELECT COUNT(*) AS value FROM forum_work_request_results`,
    },
  ],
  forum_work_requests: [
    {
      metric: "open",
      sql: `SELECT COUNT(*) AS value FROM forum_work_requests WHERE state = 'open'`,
    },
    {
      metric: "settled",
      sql: `SELECT COUNT(*) AS value FROM forum_work_requests WHERE state = 'settled'`,
    },
    {
      metric: "sum_quote_count",
      sql: `SELECT COALESCE(SUM(quote_count), 0) AS value FROM forum_work_requests`,
    },
  ],
}

export const postgresForumRemainderScalar = async (
  sql: SyncSql,
  tallySql: string,
): Promise<number> => {
  const unsafe = requireForumContentUnsafe(sql)
  const rows = await unsafe(tallySql, [])
  return Number(rows[0]?.["value"] ?? 0)
}

// ---------------------------------------------------------------------------
// Trust recompute-and-compare (derived tables)
// ---------------------------------------------------------------------------

/**
 * The per-(target_actor, forum, kind) trust-edge aggregate. A thread's
 * trust score is recomputed in D1 from exactly these grouped edges; store
 * vs store equality of the aggregate is the "trust recomputation equality"
 * evidence at the storage layer. Portable text — runs verbatim on D1 and
 * Postgres. `COALESCE(forum_id, '')` folds the SQLite-distinct NULL forum
 * into a stable group key on both engines.
 */
export const trustEdgeRecomputeSql = (): string =>
  `SELECT target_actor_ref,
       COALESCE(forum_id, '') AS forum_key,
       trust_kind,
       COUNT(*) AS edge_count,
       COALESCE(SUM(weight), 0) AS weight_sum
  FROM forum_trust_edges
 WHERE archived_at IS NULL
 GROUP BY target_actor_ref, COALESCE(forum_id, ''), trust_kind
 ORDER BY target_actor_ref, forum_key, trust_kind`

export type TrustEdgeAggregateRow = Readonly<{
  targetActorRef: string
  forumKey: string
  trustKind: string
  edgeCount: number
  weightSum: number
}>

export const trustEdgeAggregateFromRows = (
  rows: ReadonlyArray<Record<string, unknown>>,
): ReadonlyArray<TrustEdgeAggregateRow> =>
  rows.map((row) => ({
    edgeCount: Number(row["edge_count"] ?? 0),
    forumKey: String(row["forum_key"] ?? ""),
    targetActorRef: String(row["target_actor_ref"] ?? ""),
    trustKind: String(row["trust_kind"] ?? ""),
    weightSum: Number(row["weight_sum"] ?? 0),
  }))

export const postgresTrustEdgeAggregate = async (
  sql: SyncSql,
): Promise<ReadonlyArray<TrustEdgeAggregateRow>> => {
  const unsafe = requireForumContentUnsafe(sql)
  return trustEdgeAggregateFromRows(await unsafe(trustEdgeRecomputeSql(), []))
}

const trustAggregateKey = (row: TrustEdgeAggregateRow): string =>
  `${row.targetActorRef}${row.forumKey}${row.trustKind}`

export type TrustRecomputeMismatch = Readonly<{
  key: string
  d1: TrustEdgeAggregateRow | undefined
  postgres: TrustEdgeAggregateRow | undefined
}>

export const compareTrustEdgeAggregates = (
  d1: ReadonlyArray<TrustEdgeAggregateRow>,
  postgres: ReadonlyArray<TrustEdgeAggregateRow>,
): ReadonlyArray<TrustRecomputeMismatch> => {
  const pgByKey = new Map(postgres.map((row) => [trustAggregateKey(row), row]))
  const mismatches: Array<TrustRecomputeMismatch> = []
  const seen = new Set<string>()
  for (const row of d1) {
    const key = trustAggregateKey(row)
    seen.add(key)
    const twin = pgByKey.get(key)
    if (
      twin === undefined ||
      twin.edgeCount !== row.edgeCount ||
      twin.weightSum !== row.weightSum
    ) {
      mismatches.push({ d1: row, key, postgres: twin })
    }
  }
  for (const row of postgres) {
    const key = trustAggregateKey(row)
    if (!seen.has(key)) {
      mismatches.push({ d1: undefined, key, postgres: row })
    }
  }
  return mismatches
}

// ---------------------------------------------------------------------------
// Work-request set-membership referential checks
// ---------------------------------------------------------------------------

/**
 * A referential check that resolves entirely WITHIN one store (never a
 * cross-store join): the count of child rows whose foreign id has no parent
 * row. Portable text runs on both D1 and Postgres; the two orphan counts
 * are compared (both must be 0). The lifecycle family couples to KS-8.1
 * assignments and KS-8.8 tips only BY ID — those live in other stores, so
 * cross-domain refs are verified by set equality (below), not by joining.
 */
export type ReferentialCheck = Readonly<{
  name: string
  sql: string
}>

export const FORUM_WORK_REQUEST_REFERENTIAL_CHECKS: ReadonlyArray<ReferentialCheck> =
  [
    {
      name: "relay_links.work_request_id -> work_requests.id",
      sql: `SELECT COUNT(*) AS value
              FROM forum_work_request_relay_links c
              LEFT JOIN forum_work_requests p ON p.id = c.work_request_id
             WHERE p.id IS NULL`,
    },
    {
      name: "offers.work_request_id -> work_requests.id",
      sql: `SELECT COUNT(*) AS value
              FROM forum_work_request_offers c
              LEFT JOIN forum_work_requests p ON p.id = c.work_request_id
             WHERE p.id IS NULL`,
    },
    {
      name: "lifecycle_posts.work_request_id -> work_requests.id",
      sql: `SELECT COUNT(*) AS value
              FROM forum_work_request_lifecycle_posts c
              LEFT JOIN forum_work_requests p ON p.id = c.work_request_id
             WHERE p.id IS NULL`,
    },
    {
      name: "acceptances.work_request_id -> work_requests.id",
      sql: `SELECT COUNT(*) AS value
              FROM forum_work_request_acceptances c
              LEFT JOIN forum_work_requests p ON p.id = c.work_request_id
             WHERE p.id IS NULL`,
    },
    {
      name: "acceptances.offer_id -> offers.id",
      sql: `SELECT COUNT(*) AS value
              FROM forum_work_request_acceptances c
              LEFT JOIN forum_work_request_offers p ON p.id = c.offer_id
             WHERE p.id IS NULL`,
    },
    {
      name: "results.work_request_id -> work_requests.id",
      sql: `SELECT COUNT(*) AS value
              FROM forum_work_request_results c
              LEFT JOIN forum_work_requests p ON p.id = c.work_request_id
             WHERE p.id IS NULL`,
    },
    {
      name: "results.offer_id -> offers.id",
      sql: `SELECT COUNT(*) AS value
              FROM forum_work_request_results c
              LEFT JOIN forum_work_request_offers p ON p.id = c.offer_id
             WHERE p.id IS NULL`,
    },
  ]

/**
 * Cross-domain reference SETS captured from the work-request lifecycle
 * that point at KS-8.1 assignments / KS-8.8 tips / escrow ledgers by id.
 * The distinct value set is digested per store and compared for byte
 * equality — the mirror must preserve the EXACT reference set without
 * joining across stores. Each entry is a portable SELECT of one ref column.
 */
export const FORUM_WORK_REQUEST_CROSS_DOMAIN_REF_SETS: ReadonlyArray<
  Readonly<{ name: string; sql: string }>
> = [
  {
    name: "acceptances.escrow_id",
    sql: `SELECT DISTINCT escrow_id AS ref FROM forum_work_request_acceptances ORDER BY escrow_id`,
  },
  {
    name: "acceptances.reserve_receipt_ref",
    sql: `SELECT DISTINCT reserve_receipt_ref AS ref FROM forum_work_request_acceptances ORDER BY reserve_receipt_ref`,
  },
  {
    name: "offers.quote_ref",
    sql: `SELECT DISTINCT quote_ref AS ref FROM forum_work_request_offers ORDER BY quote_ref`,
  },
  {
    name: "lifecycle_posts.receipt_ref",
    sql: `SELECT DISTINCT receipt_ref AS ref FROM forum_work_request_lifecycle_posts ORDER BY receipt_ref`,
  },
]

/** sha256 over a sorted distinct ref set — safe to print (opaque ids). */
export const refSetDigest = (
  rows: ReadonlyArray<Record<string, unknown>>,
): string => {
  const values = rows
    .map((row) => String(row["ref"] ?? ""))
    .sort((a, b) => (a < b ? -1 : a > b ? 1 : 0))
  const hash = createHash("sha256")
  for (const value of values) {
    hash.update(value)
    hash.update("")
  }
  return `${values.length}:${hash.digest("hex")}`
}

export const postgresScalarValue = async (
  sql: SyncSql,
  text: string,
): Promise<number> => {
  const unsafe = requireForumContentUnsafe(sql)
  const rows = await unsafe(text, [])
  return Number(rows[0]?.["value"] ?? 0)
}

export const postgresRefSetDigest = async (
  sql: SyncSql,
  text: string,
): Promise<string> => {
  const unsafe = requireForumContentUnsafe(sql)
  return refSetDigest(await unsafe(text, []))
}

// ---------------------------------------------------------------------------
// Verify report (per table)
// ---------------------------------------------------------------------------

export type ForumRemainderVerifyReport = Readonly<{
  table: ForumRemainderTable
  countsMatch: boolean
  d1Total: number
  postgresTotal: number
  scalarMismatches: ReadonlyArray<{
    metric: string
    d1: number
    postgres: number
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
): ForumRemainderVerifyReport["newestHashMismatches"] => {
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

export const buildForumRemainderVerifyReport = (
  input: Readonly<{
    table: ForumRemainderTable
    d1Total: number
    postgresTotal: number
    scalars: ReadonlyArray<{ metric: string; d1: number; postgres: number }>
    d1Newest: ReadonlyArray<NewestRowHash>
    postgresNewest: ReadonlyArray<NewestRowHash>
  }>,
): ForumRemainderVerifyReport => ({
  countsMatch: input.d1Total === input.postgresTotal,
  d1Total: input.d1Total,
  newestHashMismatches: compareNewestHashes(
    input.d1Newest,
    input.postgresNewest,
  ),
  postgresTotal: input.postgresTotal,
  scalarMismatches: input.scalars.filter(
    (scalar) => scalar.d1 !== scalar.postgres,
  ),
  table: input.table,
})

export const forumRemainderVerifyReportClean = (
  report: ForumRemainderVerifyReport,
): boolean =>
  report.countsMatch &&
  report.scalarMismatches.length === 0 &&
  report.newestHashMismatches.length === 0
