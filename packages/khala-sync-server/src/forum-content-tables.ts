/**
 * KS-8.10 (#8321): forum content domain — the SHARED table registry and
 * Postgres converge-upsert core for the thirteen forum content tables in
 * khala-sync migration `0014_forum_content.sql`.
 *
 * ONE source of truth for column lists, primary keys, and conflict
 * semantics, consumed by BOTH sides of the migration machinery:
 *
 *   - the Worker's dual-write mirror
 *     (`apps/openagents.com/workers/api/src/forum/forum-content-store.ts`)
 *     — the KS-8.5 lane duplicated its column registry between the store
 *     and the backfill; this lane shares it instead, so a schema change
 *     cannot silently drift the two.
 *   - the backfill + verify core (`./forum-content-backfill.ts`) and CLI
 *     (`scripts/backfill-forum-content.ts`).
 *
 * This module is imported by Worker code, so it uses NO node built-ins
 * (hashing lives in forum-content-backfill.ts, which only the CLI and
 * tests load).
 *
 * CONFLICT SEMANTICS: every table converges on its PRIMARY KEY
 * (`ON CONFLICT (pk) DO UPDATE SET …` = the D1 snapshot value). Unlike the
 * agent-runtime event ledgers there is no insert-if-absent table here: D1
 * never re-issues an id for these tables (the INSERT OR IGNORE paths —
 * follows/watches/bookmarks/context links — dedupe on secondary uniques,
 * and the mirror only ever replays rows that EXIST in D1, so a converge on
 * the PK reproduces exactly the D1 row set). Secondary uniques exist on
 * the Postgres twins for parity; a violation there surfaces as a logged
 * dual-write failure, which IS the drift signal.
 */

import type { SyncSql } from "./sql.js"

export type ForumContentTable =
  | "forum_boards"
  | "forum_categories"
  | "forum_forums"
  | "forum_topics"
  | "forum_posts"
  | "forum_post_bodies"
  | "forum_post_revisions"
  | "forum_actor_follows"
  | "forum_watches"
  | "forum_bookmarks"
  | "forum_reports"
  | "forum_moderation_events"
  | "forum_context_links"

/**
 * Structure tables first, then topics/posts/bodies, then the satellites,
 * so backfill pages always land parents before children (no FKs, but the
 * verify joins expect it).
 */
export const FORUM_CONTENT_TABLES: ReadonlyArray<ForumContentTable> = [
  "forum_boards",
  "forum_categories",
  "forum_forums",
  "forum_topics",
  "forum_posts",
  "forum_post_bodies",
  "forum_post_revisions",
  "forum_actor_follows",
  "forum_watches",
  "forum_bookmarks",
  "forum_reports",
  "forum_moderation_events",
  "forum_context_links",
]

/**
 * Column lists in D1 PHYSICAL order (`SELECT *` order — SQLite ALTER ADD
 * COLUMN appends, so `discoverability` trails forum_categories /
 * forum_forums and `idempotency_key` trails forum_moderation_events).
 * Row hashes iterate this order on both stores; keep it stable.
 */
export const FORUM_CONTENT_TABLE_COLUMNS: Readonly<
  Record<ForumContentTable, ReadonlyArray<string>>
> = {
  forum_actor_follows: [
    "id",
    "actor_ref",
    "target_actor_ref",
    "idempotency_key",
    "created_at",
    "archived_at",
  ],
  forum_boards: [
    "id",
    "slug",
    "title",
    "description_ref",
    "visibility",
    "public_projection_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  forum_bookmarks: [
    "id",
    "actor_ref",
    "topic_id",
    "post_id",
    "bookmark_kind",
    "idempotency_key",
    "created_at",
    "archived_at",
  ],
  forum_categories: [
    "id",
    "board_id",
    "slug",
    "title",
    "description_ref",
    "order_index",
    "created_at",
    "updated_at",
    "archived_at",
    "discoverability",
  ],
  forum_context_links: [
    "id",
    "target_kind",
    "target_id",
    "forum_id",
    "topic_id",
    "post_id",
    "context_kind",
    "context_id",
    "context_slug",
    "context_title",
    "public_url",
    "source_ref",
    "public_projection_json",
    "created_at",
    "archived_at",
  ],
  forum_forums: [
    "id",
    "board_id",
    "category_id",
    "slug",
    "title",
    "description_ref",
    "visibility",
    "locked",
    "topic_count",
    "post_count",
    "latest_topic_id",
    "latest_post_id",
    "public_projection_json",
    "created_at",
    "updated_at",
    "archived_at",
    "discoverability",
  ],
  forum_moderation_events: [
    "id",
    "moderator_actor_ref",
    "action_kind",
    "target_kind",
    "target_id",
    "reason_ref",
    "report_id",
    "public_projection_json",
    "created_at",
    "archived_at",
    "idempotency_key",
  ],
  forum_post_bodies: [
    "post_id",
    "content_kind",
    "body_text",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  forum_post_revisions: [
    "id",
    "idempotency_key",
    "post_id",
    "actor_ref",
    "action_kind",
    "previous_body_text",
    "next_body_text",
    "previous_state",
    "next_state",
    "reason_ref",
    "public_projection_json",
    "created_at",
    "archived_at",
  ],
  forum_posts: [
    "id",
    "idempotency_key",
    "topic_id",
    "forum_id",
    "actor_ref",
    "actor_json",
    "content_ref",
    "parent_post_id",
    "quote_post_id",
    "post_number",
    "state",
    "revision_ref",
    "public_projection_json",
    "receipt_refs_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  forum_reports: [
    "id",
    "idempotency_key",
    "reporter_actor_ref",
    "target_kind",
    "target_id",
    "reason_ref",
    "status",
    "public_projection_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  forum_topics: [
    "id",
    "idempotency_key",
    "forum_id",
    "actor_ref",
    "actor_json",
    "slug",
    "title",
    "first_post_id",
    "latest_post_id",
    "post_count",
    "pin_state",
    "state",
    "score_ref",
    "public_projection_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  forum_watches: [
    "id",
    "actor_ref",
    "forum_id",
    "topic_id",
    "watch_kind",
    "idempotency_key",
    "created_at",
    "archived_at",
  ],
}

/**
 * Primary-key column per table — the converge-upsert arbiter AND the
 * read-back mirror key. Every D1 write in the forum repository keys these
 * tables by this column (INSERT first bind / UPDATE `WHERE <pk> = ?`), and
 * D1 never replaces a row's id, so the PK is the stable arbiter.
 */
export const FORUM_CONTENT_TABLE_PK: Readonly<
  Record<ForumContentTable, string>
> = {
  forum_actor_follows: "id",
  forum_boards: "id",
  forum_bookmarks: "id",
  forum_categories: "id",
  forum_context_links: "id",
  forum_forums: "id",
  forum_moderation_events: "id",
  forum_post_bodies: "post_id",
  forum_post_revisions: "id",
  forum_posts: "id",
  forum_reports: "id",
  forum_topics: "id",
  forum_watches: "id",
}

export const isForumContentTable = (
  value: string,
): value is ForumContentTable =>
  Object.prototype.hasOwnProperty.call(FORUM_CONTENT_TABLE_PK, value)

export type ForumContentRow = Readonly<Record<string, unknown>>

/**
 * D1 → Postgres value normalization: D1/SQLite hands back TEXT / INTEGER /
 * REAL / NULL; booleans arrive as 0/1 already. Keep bytes identical so
 * row-hash reconciliation compares equal.
 */
export const normalizeForumContentValue = (
  value: unknown,
): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "bigint") return Number(value)
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)`; the
 * structural `SyncSql` seam deliberately does not, so this module widens
 * it locally (same note as the pylon/token/agent-runtime backfills). Both
 * runtime drivers here run with `prepare: false`, so `unsafe` uses the
 * unnamed statement and is transaction-pooler/Hyperdrive-safe.
 */
export type ForumContentUnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

export const requireForumContentUnsafe = (
  sql: SyncSql,
): ForumContentUnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: ForumContentUnsafeQuery })
    .unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "forum content store requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/**
 * Converge-upsert one page of D1 rows into `table` on Postgres: the row
 * becomes exactly the D1 snapshot (`ON CONFLICT (pk) DO UPDATE SET` every
 * non-key column). Idempotent — a re-run with the same rows converges to
 * the same state. Returns how many rows were touched.
 */
export const upsertForumContentRows = async (
  sql: SyncSql,
  table: ForumContentTable,
  rows: ReadonlyArray<ForumContentRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireForumContentUnsafe(sql)
  const columns = FORUM_CONTENT_TABLE_COLUMNS[table]
  const pk = FORUM_CONTENT_TABLE_PK[table]
  const setClauses = columns
    .filter((column) => column !== pk)
    .map((column) => `${column} = EXCLUDED.${column}`)
    .join(", ")

  let touched = 0
  for (const row of rows) {
    const values = columns.map((column) =>
      normalizeForumContentValue(row[column]),
    )
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${columns.join(", ")}) VALUES (${placeholders}) ON CONFLICT (${pk}) DO UPDATE SET ${setClauses} RETURNING 1 AS touched`,
      values as Array<unknown>,
    )
    touched += result.length
  }
  return touched
}
