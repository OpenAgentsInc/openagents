/**
 * KS-8.10 (#8321): forum content backfill + verification core —
 * D1 → Postgres.
 *
 * Testable core behind `scripts/backfill-forum-content.ts`, following the
 * KS-8.2/KS-8.5 templates (`token-ledger-backfill.ts`,
 * `agent-runtime-backfill.ts`). Takes raw D1 rows (snake_case objects,
 * exactly as `wrangler d1 execute --json` returns them) and converges them
 * into the Postgres twins from migration `0014_forum_content.sql` via the
 * SHARED registry in `./forum-content-tables.ts` (the same
 * `upsertForumContentRows` the Worker's dual-write mirror uses — backfill
 * and mirror can never fight because they write identical converge
 * upserts keyed on the PK).
 *
 * Verification (`verify*`): the 2026-06-29 after-action reconciliation
 * culture with the KS-8.10 acceptance specifics —
 *
 *   - exact row counts per table;
 *   - domain scalar tallies (forum topic/post counter sums, per-state
 *     post/topic tallies, report status tallies, body byte totals);
 *   - PER-TOPIC POST-CHAIN comparison (count / distinct / min / max over
 *     `post_number` per topic — a thread page is exactly this chain, so
 *     chain equality across stores is the "public thread pages
 *     shadow-compared" evidence at the storage layer);
 *   - PER-THREAD SPOT HASHES over sampled topics: sha256 over the ordered
 *     (post_number, post id, state, sha256(body_text)) chain — the
 *     post-body content-checksum acceptance (bodies are the long pole);
 *   - newest-N full row hashes per table.
 *
 * Output references row KEYS and sha256 hashes only — never body text.
 */

import { createHash } from "node:crypto"
import {
  FORUM_CONTENT_TABLE_COLUMNS,
  FORUM_CONTENT_TABLE_PK,
  FORUM_CONTENT_TABLES,
  normalizeForumContentValue,
  requireForumContentUnsafe,
  upsertForumContentRows,
  type ForumContentRow,
  type ForumContentTable,
} from "./forum-content-tables.js"
import type { SyncSql } from "./sql.js"

export {
  FORUM_CONTENT_TABLE_COLUMNS,
  FORUM_CONTENT_TABLE_PK,
  FORUM_CONTENT_TABLES,
  upsertForumContentRows,
  type ForumContentRow,
  type ForumContentTable,
}

export type D1SourceRow = ForumContentRow

/** Newest-first ordering column per table (for the hash sample). */
export const FORUM_CONTENT_TABLE_ORDER: Readonly<
  Record<ForumContentTable, string>
> = {
  forum_actor_follows: "created_at",
  forum_boards: "updated_at",
  forum_bookmarks: "created_at",
  forum_categories: "updated_at",
  forum_context_links: "created_at",
  forum_forums: "updated_at",
  forum_moderation_events: "created_at",
  forum_post_bodies: "updated_at",
  forum_post_revisions: "created_at",
  forum_posts: "updated_at",
  forum_reports: "updated_at",
  forum_topics: "updated_at",
  forum_watches: "created_at",
}

// ---------------------------------------------------------------------------
// Row hashes
// ---------------------------------------------------------------------------

/**
 * Canonical row hash: the registry-order column values joined with unit
 * separators, sha256'd. Normalization matches `upsertForumContentRows`,
 * so the SAME D1 export row and its Postgres twin hash identically
 * (bigint counters come back as strings from postgres.js; `String()`
 * canonicalizes both sides).
 */
export const forumContentRowHash = (
  table: ForumContentTable,
  row: D1SourceRow,
): string => {
  const columns = FORUM_CONTENT_TABLE_COLUMNS[table]
  const hash = createHash("sha256")
  for (const column of columns) {
    const value = normalizeForumContentValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ key: string; hash: string }>

const rowKey = (table: ForumContentTable, row: D1SourceRow): string =>
  String(row[FORUM_CONTENT_TABLE_PK[table]] ?? "<null>")

export const d1ForumContentNewestHashes = (
  table: ForumContentTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> =>
  rows.map((row) => ({
    hash: forumContentRowHash(table, row),
    key: rowKey(table, row),
  }))

export const postgresForumContentNewestHashes = async (
  sql: SyncSql,
  table: ForumContentTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const unsafe = requireForumContentUnsafe(sql)
  const orderColumn = FORUM_CONTENT_TABLE_ORDER[table]
  const pk = FORUM_CONTENT_TABLE_PK[table]
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${orderColumn} DESC, ${pk} DESC LIMIT $1`,
    [limit],
  )
  return rows.map((row) => ({
    hash: forumContentRowHash(table, row),
    key: rowKey(table, row),
  }))
}

// ---------------------------------------------------------------------------
// Counts and scalar tallies
// ---------------------------------------------------------------------------

export const postgresForumContentRowCount = async (
  sql: SyncSql,
  table: ForumContentTable,
): Promise<number> => {
  const unsafe = requireForumContentUnsafe(sql)
  const rows = await unsafe(`SELECT COUNT(*) AS total_rows FROM ${table}`, [])
  return Number(rows[0]?.["total_rows"] ?? 0)
}

/**
 * Domain scalar tallies per table (compared exactly across stores). The
 * SQL text is portable and runs verbatim on D1 AND Postgres so both sides
 * compute the same numbers over the same rows. Body tallies measure byte
 * LENGTHS only — never content.
 */
export const FORUM_CONTENT_SCALAR_TALLIES: Readonly<
  Record<
    ForumContentTable,
    ReadonlyArray<Readonly<{ metric: string; sql: string }>>
  >
> = {
  forum_actor_follows: [
    {
      metric: "active_follows",
      sql: `SELECT COUNT(*) AS value FROM forum_actor_follows WHERE archived_at IS NULL`,
    },
    {
      metric: "distinct_followers",
      sql: `SELECT COUNT(DISTINCT actor_ref) AS value FROM forum_actor_follows`,
    },
  ],
  forum_boards: [
    {
      metric: "active_boards",
      sql: `SELECT COUNT(*) AS value FROM forum_boards WHERE archived_at IS NULL`,
    },
  ],
  forum_bookmarks: [
    {
      metric: "active_bookmarks",
      sql: `SELECT COUNT(*) AS value FROM forum_bookmarks WHERE archived_at IS NULL`,
    },
  ],
  forum_categories: [
    {
      metric: "sum_order_index",
      sql: `SELECT COALESCE(SUM(order_index), 0) AS value FROM forum_categories`,
    },
  ],
  forum_context_links: [
    {
      metric: "topic_links",
      sql: `SELECT COUNT(*) AS value FROM forum_context_links WHERE target_kind = 'topic'`,
    },
    {
      metric: "post_links",
      sql: `SELECT COUNT(*) AS value FROM forum_context_links WHERE target_kind = 'post'`,
    },
  ],
  forum_forums: [
    {
      metric: "sum_topic_count",
      sql: `SELECT COALESCE(SUM(topic_count), 0) AS value FROM forum_forums`,
    },
    {
      metric: "sum_post_count",
      sql: `SELECT COALESCE(SUM(post_count), 0) AS value FROM forum_forums`,
    },
    {
      metric: "locked_forums",
      sql: `SELECT COUNT(*) AS value FROM forum_forums WHERE locked = 1`,
    },
  ],
  forum_moderation_events: [
    {
      metric: "events_with_report",
      sql: `SELECT COUNT(*) AS value FROM forum_moderation_events WHERE report_id IS NOT NULL`,
    },
  ],
  forum_post_bodies: [
    {
      metric: "sum_body_length",
      sql: `SELECT COALESCE(SUM(LENGTH(body_text)), 0) AS value FROM forum_post_bodies`,
    },
    {
      metric: "archived_bodies",
      sql: `SELECT COUNT(*) AS value FROM forum_post_bodies WHERE archived_at IS NOT NULL`,
    },
  ],
  forum_post_revisions: [
    {
      metric: "edit_revisions",
      sql: `SELECT COUNT(*) AS value FROM forum_post_revisions WHERE action_kind = 'edit'`,
    },
    {
      metric: "tombstone_revisions",
      sql: `SELECT COUNT(*) AS value FROM forum_post_revisions WHERE action_kind = 'tombstone'`,
    },
  ],
  forum_posts: [
    {
      metric: "sum_post_number",
      sql: `SELECT COALESCE(SUM(post_number), 0) AS value FROM forum_posts`,
    },
    {
      metric: "visible_posts",
      sql: `SELECT COUNT(*) AS value FROM forum_posts WHERE state = 'visible'`,
    },
    {
      metric: "tombstoned_posts",
      sql: `SELECT COUNT(*) AS value FROM forum_posts WHERE state = 'tombstoned'`,
    },
  ],
  forum_reports: [
    {
      metric: "open_reports",
      sql: `SELECT COUNT(*) AS value FROM forum_reports WHERE status = 'open'`,
    },
    {
      metric: "resolved_reports",
      sql: `SELECT COUNT(*) AS value FROM forum_reports WHERE status = 'resolved'`,
    },
  ],
  forum_topics: [
    {
      metric: "sum_post_count",
      sql: `SELECT COALESCE(SUM(post_count), 0) AS value FROM forum_topics`,
    },
    {
      metric: "open_topics",
      sql: `SELECT COUNT(*) AS value FROM forum_topics WHERE state = 'open'`,
    },
  ],
  forum_watches: [
    {
      metric: "active_watches",
      sql: `SELECT COUNT(*) AS value FROM forum_watches WHERE archived_at IS NULL`,
    },
  ],
}

export const postgresForumContentScalar = async (
  sql: SyncSql,
  tallySql: string,
): Promise<number> => {
  const unsafe = requireForumContentUnsafe(sql)
  const rows = await unsafe(tallySql, [])
  return Number(rows[0]?.["value"] ?? 0)
}

// ---------------------------------------------------------------------------
// Per-topic post chains (the thread-page shape)
// ---------------------------------------------------------------------------

/**
 * Per-topic post-chain shape: for every topic, the post count, min/max
 * post_number, and distinct-post_number count. A thread page IS this
 * chain, so full chain-map equality across stores is the storage-layer
 * "public thread pages shadow-compared" evidence. Contiguity
 * (count === distinct === max - min + 1) can legitimately be BROKEN on
 * both sides (tombstones keep their post_number) — the comparison is
 * store-vs-store equality, not absolute contiguity.
 */
export type PostChainRow = Readonly<{
  topicId: string
  posts: number
  distinctNumbers: number
  minNumber: number
  maxNumber: number
}>

export type PostChainTally = Readonly<{
  chains: ReadonlyArray<PostChainRow>
  totalPosts: number
}>

/** The SQL producing per-topic chain rows (same text runs on D1 + PG). */
export const postChainSql = (): string =>
  `SELECT topic_id,
       COUNT(*) AS posts,
       COUNT(DISTINCT post_number) AS distinct_numbers,
       MIN(post_number) AS min_number,
       MAX(post_number) AS max_number
  FROM forum_posts
 GROUP BY topic_id
 ORDER BY topic_id`

export const postChainTallyFromRows = (
  rows: ReadonlyArray<Record<string, unknown>>,
): PostChainTally => {
  const chains = rows.map((row) => ({
    distinctNumbers: Number(row["distinct_numbers"] ?? 0),
    maxNumber: Number(row["max_number"] ?? 0),
    minNumber: Number(row["min_number"] ?? 0),
    posts: Number(row["posts"] ?? 0),
    topicId: String(row["topic_id"] ?? ""),
  }))
  return {
    chains,
    totalPosts: chains.reduce((sum, chain) => sum + chain.posts, 0),
  }
}

export const postgresPostChainTally = async (
  sql: SyncSql,
): Promise<PostChainTally> => {
  const unsafe = requireForumContentUnsafe(sql)
  return postChainTallyFromRows(await unsafe(postChainSql(), []))
}

export type PostChainMismatch = Readonly<{
  topicId: string
  d1: PostChainRow | undefined
  postgres: PostChainRow | undefined
}>

export const comparePostChains = (
  d1: PostChainTally,
  postgres: PostChainTally,
): ReadonlyArray<PostChainMismatch> => {
  const postgresByTopic = new Map(
    postgres.chains.map((chain) => [chain.topicId, chain]),
  )
  const mismatches: Array<PostChainMismatch> = []
  const seen = new Set<string>()
  for (const chain of d1.chains) {
    seen.add(chain.topicId)
    const twin = postgresByTopic.get(chain.topicId)
    if (
      twin === undefined ||
      twin.posts !== chain.posts ||
      twin.distinctNumbers !== chain.distinctNumbers ||
      twin.minNumber !== chain.minNumber ||
      twin.maxNumber !== chain.maxNumber
    ) {
      mismatches.push({ d1: chain, postgres: twin, topicId: chain.topicId })
    }
  }
  for (const chain of postgres.chains) {
    if (!seen.has(chain.topicId)) {
      mismatches.push({ d1: undefined, postgres: chain, topicId: chain.topicId })
    }
  }
  return mismatches
}

// ---------------------------------------------------------------------------
// Per-thread spot hashes (post-body content checksums)
// ---------------------------------------------------------------------------

/**
 * The per-thread spot-hash query: the ordered (post_number, id, state,
 * body) chain for one topic. Portable text — runs verbatim on D1 (with
 * `?`) and Postgres (with `$1`); the CLI/store substitute the placeholder
 * style. Tombstoned posts have archived bodies (LEFT JOIN keeps them with
 * a NULL body), so the chain hash covers redaction state too.
 */
export const THREAD_SPOT_HASH_SQL_D1 = `SELECT p.id, p.post_number, p.state, b.body_text
  FROM forum_posts p
  LEFT JOIN forum_post_bodies b ON b.post_id = p.id
 WHERE p.topic_id = ?
 ORDER BY p.post_number ASC, p.id ASC`

export const THREAD_SPOT_HASH_SQL_PG = THREAD_SPOT_HASH_SQL_D1.replace(
  "?",
  "$1",
)

/**
 * sha256 over the ordered (post_number, post id, state, sha256(body))
 * chain of one thread. Hashing the BODY HASH (not the body) keeps verify
 * output safe to paste while still detecting any single-byte body drift.
 */
export const threadSpotHashFromRows = (
  rows: ReadonlyArray<Record<string, unknown>>,
): string => {
  const chain = createHash("sha256")
  for (const row of rows) {
    const body = row["body_text"]
    const bodyHash =
      body === null || body === undefined
        ? "<null>"
        : createHash("sha256").update(String(body)).digest("hex")
    chain.update(
      `${String(row["post_number"] ?? "")}:${String(row["id"] ?? "")}:${String(
        row["state"] ?? "",
      )}:${bodyHash}`,
    )
    chain.update("")
  }
  return chain.digest("hex")
}

export const postgresThreadSpotHash = async (
  sql: SyncSql,
  topicId: string,
): Promise<string> => {
  const unsafe = requireForumContentUnsafe(sql)
  return threadSpotHashFromRows(
    await unsafe(THREAD_SPOT_HASH_SQL_PG, [topicId]),
  )
}

export type ThreadSpotHashMismatch = Readonly<{
  topicId: string
  d1Hash: string
  postgresHash: string
}>

// ---------------------------------------------------------------------------
// Verify report
// ---------------------------------------------------------------------------

export type ForumContentVerifyReport = Readonly<{
  table: ForumContentTable
  countsMatch: boolean
  d1Total: number
  postgresTotal: number
  scalarMismatches: ReadonlyArray<{
    metric: string
    d1: number
    postgres: number
  }>
  chainMismatches: ReadonlyArray<PostChainMismatch>
  newestHashMismatches: ReadonlyArray<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }>
}>

export const compareNewestHashes = (
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): ForumContentVerifyReport["newestHashMismatches"] => {
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

export const buildForumContentVerifyReport = (
  input: Readonly<{
    table: ForumContentTable
    d1Total: number
    postgresTotal: number
    scalars: ReadonlyArray<{ metric: string; d1: number; postgres: number }>
    d1Chains?: PostChainTally | undefined
    postgresChains?: PostChainTally | undefined
    d1Newest: ReadonlyArray<NewestRowHash>
    postgresNewest: ReadonlyArray<NewestRowHash>
  }>,
): ForumContentVerifyReport => ({
  chainMismatches:
    input.d1Chains === undefined || input.postgresChains === undefined
      ? []
      : comparePostChains(input.d1Chains, input.postgresChains),
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

export const forumContentVerifyReportClean = (
  report: ForumContentVerifyReport,
): boolean =>
  report.countsMatch &&
  report.scalarMismatches.length === 0 &&
  report.chainMismatches.length === 0 &&
  report.newestHashMismatches.length === 0
