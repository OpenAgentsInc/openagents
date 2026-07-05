/**
 * KS-8.10 remainder (#8338): forum remainder domain — the SHARED table
 * registry and Postgres converge-upsert core for the THIRTEEN remainder
 * forum tables that finish the KS-8.10 family behind the parent content
 * lane (#8321, `forum-content-tables.ts`, migration `0014_forum_content.sql`).
 *
 * Tables (khala-sync migration `0026_forum_remainder.sql`):
 *   - private content: `forum_private_message_threads`,
 *     `forum_private_messages` (bodies are refs; diagnostics stay keys-only);
 *   - `forum_acl_grants`;
 *   - trust (derived): `forum_trust_edges`, `forum_actor_forum_trust`;
 *   - `forum_score_snapshots` (derived);
 *   - `forum_notification_reads`;
 *   - the work-request lifecycle family (6): `forum_work_requests`,
 *     `forum_work_request_relay_links`, `forum_work_request_offers`,
 *     `forum_work_request_lifecycle_posts`, `forum_work_request_acceptances`,
 *     `forum_work_request_results`.
 *
 * ONE source of truth for column lists, primary keys, and conflict
 * semantics, consumed by BOTH sides of the migration machinery:
 *
 *   - the Worker's dual-write mirror
 *     (`apps/openagents.com/workers/api/src/forum/forum-remainder-store.ts`)
 *     — composed into `forumContentDatabaseForEnv` so the SAME forum write
 *     call sites cover the remainder tables with no new wiring;
 *   - the backfill + verify core (`./forum-remainder-backfill.ts`) and CLI
 *     (`scripts/backfill-forum-remainder.ts`).
 *
 * This module is imported by Worker code, so it uses NO node built-ins
 * (hashing lives in forum-remainder-backfill.ts, which only the CLI and
 * tests load). Value normalization and the `unsafe(text, params)` widener
 * are reused from the parent content registry so the two lanes normalize
 * D1 bytes identically.
 *
 * CONFLICT SEMANTICS: every table converges on its PRIMARY KEY
 * (`ON CONFLICT (pk) DO UPDATE SET …` = the D1 snapshot value). D1 never
 * re-issues an id for these tables, and the mirror only ever replays rows
 * that EXIST in D1, so a converge on the PK reproduces exactly the D1 row
 * set. Secondary uniques exist on the Postgres twins for parity; a
 * violation there surfaces as a logged dual-write failure — the drift
 * signal. The trust and score-snapshot tables are DERIVED (recomputed from
 * events in D1); this lane mirrors the D1 snapshot and VERIFIES equality
 * against D1 rather than re-running the recompute on Postgres.
 */

import {
  normalizeForumContentValue,
  requireForumContentUnsafe,
  type ForumContentRow,
} from "./forum-content-tables.js"
import type { SyncSql } from "./sql.js"

export type ForumRemainderTable =
  | "forum_private_message_threads"
  | "forum_private_messages"
  | "forum_acl_grants"
  | "forum_trust_edges"
  | "forum_actor_forum_trust"
  | "forum_score_snapshots"
  | "forum_notification_reads"
  | "forum_work_requests"
  | "forum_work_request_relay_links"
  | "forum_work_request_offers"
  | "forum_work_request_lifecycle_posts"
  | "forum_work_request_acceptances"
  | "forum_work_request_results"

/**
 * Dependency order: private-message threads before messages; trust edges
 * before the aggregated actor_forum_trust; work_requests before every
 * lifecycle child (offers before acceptances/results, which reference an
 * offer id). No FKs on the Postgres twins, but backfill pages land parents
 * before children and the set-membership verify expects this order.
 */
export const FORUM_REMAINDER_TABLES: ReadonlyArray<ForumRemainderTable> = [
  "forum_private_message_threads",
  "forum_private_messages",
  "forum_acl_grants",
  "forum_trust_edges",
  "forum_actor_forum_trust",
  "forum_score_snapshots",
  "forum_notification_reads",
  "forum_work_requests",
  "forum_work_request_relay_links",
  "forum_work_request_offers",
  "forum_work_request_lifecycle_posts",
  "forum_work_request_acceptances",
  "forum_work_request_results",
]

/**
 * Column lists in D1 PHYSICAL order (`SELECT *` order — SQLite ALTER ADD
 * COLUMN appends, so `provider_pubkey` trails forum_work_request_offers,
 * added by worker migration 0179). Row hashes iterate this order on both
 * stores; keep it stable.
 */
export const FORUM_REMAINDER_TABLE_COLUMNS: Readonly<
  Record<ForumRemainderTable, ReadonlyArray<string>>
> = {
  forum_acl_grants: [
    "id",
    "actor_ref",
    "forum_id",
    "permission",
    "scope_ref",
    "granted_by_actor_ref",
    "created_at",
    "revoked_at",
  ],
  forum_actor_forum_trust: [
    "id",
    "actor_ref",
    "forum_id",
    "trust_score",
    "reward_count",
    "report_count",
    "moderator_adjustment_count",
    "score_ref",
    "updated_at",
    "archived_at",
  ],
  forum_notification_reads: [
    "id",
    "actor_ref",
    "notification_id",
    "idempotency_key",
    "read_at",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  forum_private_message_threads: [
    "id",
    "subject",
    "slug",
    "created_by_actor_ref",
    "participant_refs_json",
    "latest_message_id",
    "message_count",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  forum_private_messages: [
    "id",
    "thread_id",
    "sender_actor_ref",
    "recipient_actor_ref",
    "content_ref",
    "public_projection_json",
    "created_at",
    "archived_at",
  ],
  forum_score_snapshots: [
    "id",
    "target_kind",
    "target_id",
    "positive_bitcoin_sats",
    "boost_bitcoin_sats",
    "down_signal_bitcoin_sats",
    "reply_count",
    "net_investment_sats",
    "score_ref",
    "rebuilt_from_event_ref",
    "public_projection_json",
    "created_at",
    "archived_at",
  ],
  forum_trust_edges: [
    "id",
    "source_actor_ref",
    "target_actor_ref",
    "forum_id",
    "trust_kind",
    "weight",
    "event_ref",
    "created_at",
    "archived_at",
  ],
  forum_work_request_acceptances: [
    "id",
    "idempotency_key",
    "work_request_id",
    "offer_id",
    "quote_ref",
    "requester_actor_ref",
    "provider_actor_ref",
    "amount_msats",
    "escrow_id",
    "reserve_receipt_ref",
    "acceptance_event_ref",
    "public_projection_json",
    "created_at",
    "archived_at",
  ],
  forum_work_request_lifecycle_posts: [
    "id",
    "work_request_id",
    "topic_id",
    "post_id",
    "idempotency_key",
    "lifecycle_kind",
    "receipt_ref",
    "state_after",
    "created_at",
    "archived_at",
  ],
  forum_work_request_offers: [
    "id",
    "work_request_id",
    "quote_ref",
    "provider_actor_ref",
    "amount_sats",
    "amount_msats",
    "capability_refs_json",
    "relay_event_ref",
    "state",
    "public_projection_json",
    "created_at",
    "updated_at",
    "archived_at",
    // Appended by worker migration 0179 (SQLite ALTER appends).
    "provider_pubkey",
  ],
  forum_work_request_relay_links: [
    "id",
    "work_request_id",
    "topic_id",
    "job_event_id",
    "job_event_kind",
    "relay_url",
    "relay_ref",
    "bridge_actor_ref",
    "event_json",
    "created_at",
    "archived_at",
  ],
  forum_work_request_results: [
    "id",
    "work_request_id",
    "offer_id",
    "quote_ref",
    "provider_actor_ref",
    "result_event_ref",
    "verification_command_ref",
    "artifact_refs_json",
    "closeout_ref",
    "public_projection_json",
    "created_at",
    "archived_at",
  ],
  forum_work_requests: [
    "id",
    "idempotency_key",
    "topic_id",
    "first_post_id",
    "requester_actor_ref",
    "title",
    "objective_ref",
    "verification_command_ref",
    "repository_refs_json",
    "required_capability_refs_json",
    "budget_sats",
    "budget_msats",
    "deadline_ref",
    "relay_url",
    "job_event_id",
    "job_event_kind",
    "job_result_kind",
    "state",
    "quote_count",
    "public_projection_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
}

/**
 * Primary-key column per table — the converge-upsert arbiter AND the
 * read-back mirror key. Every scoped D1 write keys these tables by this
 * column, and D1 never replaces a row's id, so the PK is the stable
 * arbiter. All thirteen use `id`.
 */
export const FORUM_REMAINDER_TABLE_PK: Readonly<
  Record<ForumRemainderTable, string>
> = {
  forum_acl_grants: "id",
  forum_actor_forum_trust: "id",
  forum_notification_reads: "id",
  forum_private_message_threads: "id",
  forum_private_messages: "id",
  forum_score_snapshots: "id",
  forum_trust_edges: "id",
  forum_work_request_acceptances: "id",
  forum_work_request_lifecycle_posts: "id",
  forum_work_request_offers: "id",
  forum_work_request_relay_links: "id",
  forum_work_request_results: "id",
  forum_work_requests: "id",
}

export const isForumRemainderTable = (
  value: string,
): value is ForumRemainderTable =>
  Object.prototype.hasOwnProperty.call(FORUM_REMAINDER_TABLE_PK, value)

export type ForumRemainderRow = ForumContentRow

/**
 * Converge-upsert one page of D1 rows into `table` on Postgres: the row
 * becomes exactly the D1 snapshot (`ON CONFLICT (pk) DO UPDATE SET` every
 * non-key column). Idempotent — a re-run with the same rows converges to
 * the same state. Returns how many rows were touched. Byte-identical
 * recipe to `upsertForumContentRows`; the two lanes never fight because
 * they write the same converge upsert keyed on the PK.
 */
export const upsertForumRemainderRows = async (
  sql: SyncSql,
  table: ForumRemainderTable,
  rows: ReadonlyArray<ForumRemainderRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireForumContentUnsafe(sql)
  const columns = FORUM_REMAINDER_TABLE_COLUMNS[table]
  const pk = FORUM_REMAINDER_TABLE_PK[table]
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
