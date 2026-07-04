-- KS-8.10 (#8321): forum content + trust domain — Postgres twins of the
-- THIRTEEN core forum content tables: `forum_boards`, `forum_categories`,
-- `forum_forums`, `forum_topics`, `forum_posts`, `forum_post_bodies`,
-- `forum_post_revisions`, `forum_actor_follows`, `forum_watches`,
-- `forum_bookmarks`, `forum_reports`, `forum_moderation_events`,
-- `forum_context_links` (worker migrations 0101/0102/0103/0105/0110/0111/
-- 0112).
-- Plan: docs/khala-sync/MIGRATION_PLAN.md §3.7 (universal porting rules in
-- §1); templates: 0005_pylon_dispatch.sql (KS-8.1),
-- 0008_token_usage_ledger.sql (KS-8.2), 0010_agent_runtime.sql (KS-8.5).
--
-- SCOPE NOTE: the KS-8.10 issue names ~26 tables. This migration covers
-- the content core the dual-write mirror lands with. The REMAINDER —
-- private messages (2), `forum_acl_grants`, trust
-- (`forum_actor_forum_trust`, `forum_trust_edges` — recompute-and-compare
-- per the plan), `forum_score_snapshots` (derived), notification reads,
-- and `forum_work_request_*` (6, cross-referencing KS-8.1 assignments and
-- KS-8.8 tips by id) — moves in the filed follow-up remainder lane. The
-- forum MONEY tables (`forum_money_actions`, `forum_payment_events`,
-- `forum_receipts`, L402, direct tips, tip recipient wallets, settlement
-- claims) belong to KS-8.8 and are deliberately ABSENT here.
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's byte
-- representation — TEXT ISO-8601 timestamps, 0/1 booleans as smallint,
-- JSON payload columns as text (NOT jsonb: row-hash reconciliation
-- compares exact bytes). Counters are bigint; stores cast reads with
-- Number(). Tightening to native types is a post-retirement cleanup,
-- never mid-migration.
--
-- IDEMPOTENCY KEYS PORT EXACTLY (MIGRATION_PLAN §1):
--   * `forum_topics` / `forum_posts` / `forum_reports` /
--     `forum_post_revisions` / `forum_actor_follows` / `forum_watches` /
--     `forum_bookmarks`: UNIQUE(idempotency_key) ports verbatim — these
--     are the write-dedupe keys the D1 SELECT-before-INSERT and
--     INSERT-OR-IGNORE paths key on.
--   * `forum_moderation_events`: D1's PARTIAL unique
--     (idempotency_key WHERE idempotency_key IS NOT NULL AND archived_at
--     IS NULL) ports as the same partial index.
--   * natural-key uniques port verbatim: topics (forum_id, slug), posts
--     (topic_id, post_number), follows (actor_ref, target_actor_ref),
--     watches (actor_ref, watch_kind, forum_id, topic_id), bookmarks
--     (actor_ref, bookmark_kind, topic_id, post_id), context links
--     (target_kind, target_id, context_kind, context_id), categories
--     (board_id, slug), forums (category_id, slug), boards (slug).
--     SQLite treats NULLs as distinct inside composite uniques; Postgres
--     default (NULLS DISTINCT) matches.
--
-- PUBLIC-SAFETY: forum content is a public projection surface —
-- `public_projection_json` columns carry the already-validated
-- ForumPublicProjection envelope verbatim. Post bodies are forum-visible
-- content (not secrets), but migration diagnostics still reference row
-- KEYS and sha256 hashes only, never body text.
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL QUERY PATTERNS (the KS-8.2 rule):
-- kept indexes serve the live reads (board index, forum topic listings
-- bumped by updated_at, thread pages by post_number, report/moderation
-- queues, follow lookups, context-link panels). D1 artifacts not backed
-- by a live read on the Postgres side are dropped until the read cutover
-- re-derives them.
--
-- NO FOREIGN KEYS (dual-write mirrors and the backfill land per-row;
-- integrity is verified by reconciliation — same as 0005/0008/0010).

CREATE TABLE IF NOT EXISTS forum_boards (
  id                      text NOT NULL PRIMARY KEY,
  slug                    text NOT NULL,
  title                   text NOT NULL,
  description_ref         text,
  visibility              text NOT NULL DEFAULT 'public',
  public_projection_json  text NOT NULL DEFAULT '{}',
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  archived_at             text,
  CONSTRAINT forum_boards_slug_key UNIQUE (slug)
);

CREATE TABLE IF NOT EXISTS forum_categories (
  id               text NOT NULL PRIMARY KEY,
  board_id         text NOT NULL,
  slug             text NOT NULL,
  title            text NOT NULL,
  description_ref  text,
  order_index      bigint NOT NULL DEFAULT 0,
  created_at       text NOT NULL,
  updated_at       text NOT NULL,
  archived_at      text,
  -- Appended by worker migration 0102 (SQLite ALTER appends; D1 physical
  -- column order kept for SELECT * row-hash parity).
  discoverability  text NOT NULL DEFAULT 'listed',
  CONSTRAINT forum_categories_board_slug_key UNIQUE (board_id, slug)
);

CREATE TABLE IF NOT EXISTS forum_forums (
  id                      text NOT NULL PRIMARY KEY,
  board_id                text NOT NULL,
  category_id             text NOT NULL,
  slug                    text NOT NULL,
  title                   text NOT NULL,
  description_ref         text,
  visibility              text NOT NULL DEFAULT 'public',
  locked                  smallint NOT NULL DEFAULT 0,
  topic_count             bigint NOT NULL DEFAULT 0,
  post_count              bigint NOT NULL DEFAULT 0,
  latest_topic_id         text,
  latest_post_id          text,
  public_projection_json  text NOT NULL DEFAULT '{}',
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  archived_at             text,
  -- Appended by worker migration 0102.
  discoverability         text NOT NULL DEFAULT 'listed',
  CONSTRAINT forum_forums_category_slug_key UNIQUE (category_id, slug)
);

CREATE TABLE IF NOT EXISTS forum_topics (
  id                      text NOT NULL PRIMARY KEY,
  idempotency_key         text NOT NULL,
  forum_id                text NOT NULL,
  actor_ref               text NOT NULL,
  actor_json              text NOT NULL DEFAULT '{}',
  slug                    text NOT NULL,
  title                   text NOT NULL,
  first_post_id           text NOT NULL,
  latest_post_id          text NOT NULL,
  post_count              bigint NOT NULL DEFAULT 1,
  pin_state               text NOT NULL DEFAULT 'normal',
  state                   text NOT NULL DEFAULT 'open',
  score_ref               text,
  public_projection_json  text NOT NULL DEFAULT '{}',
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  archived_at             text,
  CONSTRAINT forum_topics_idempotency_key_key UNIQUE (idempotency_key),
  CONSTRAINT forum_topics_forum_slug_key UNIQUE (forum_id, slug)
);

CREATE TABLE IF NOT EXISTS forum_posts (
  id                      text NOT NULL PRIMARY KEY,
  idempotency_key         text NOT NULL,
  topic_id                text NOT NULL,
  forum_id                text NOT NULL,
  actor_ref               text NOT NULL,
  actor_json              text NOT NULL DEFAULT '{}',
  content_ref             text NOT NULL,
  parent_post_id          text,
  quote_post_id           text,
  post_number             bigint NOT NULL,
  state                   text NOT NULL DEFAULT 'visible',
  revision_ref            text,
  public_projection_json  text NOT NULL DEFAULT '{}',
  receipt_refs_json       text NOT NULL DEFAULT '[]',
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  archived_at             text,
  CONSTRAINT forum_posts_idempotency_key_key UNIQUE (idempotency_key),
  CONSTRAINT forum_posts_topic_number_key UNIQUE (topic_id, post_number)
);

CREATE TABLE IF NOT EXISTS forum_post_bodies (
  post_id       text NOT NULL PRIMARY KEY,
  content_kind  text NOT NULL DEFAULT 'plain_text',
  body_text     text NOT NULL,
  created_at    text NOT NULL,
  updated_at    text NOT NULL,
  archived_at   text
);

CREATE TABLE IF NOT EXISTS forum_post_revisions (
  id                      text NOT NULL PRIMARY KEY,
  idempotency_key         text NOT NULL,
  post_id                 text NOT NULL,
  actor_ref               text NOT NULL,
  action_kind             text NOT NULL,
  previous_body_text      text,
  next_body_text          text,
  previous_state          text NOT NULL,
  next_state              text NOT NULL,
  reason_ref              text,
  public_projection_json  text NOT NULL DEFAULT '{}',
  created_at              text NOT NULL,
  archived_at             text,
  CONSTRAINT forum_post_revisions_idempotency_key_key
    UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS forum_actor_follows (
  id                text NOT NULL PRIMARY KEY,
  actor_ref         text NOT NULL,
  target_actor_ref  text NOT NULL,
  idempotency_key   text NOT NULL,
  created_at        text NOT NULL,
  archived_at       text,
  CONSTRAINT forum_actor_follows_idempotency_key_key
    UNIQUE (idempotency_key),
  CONSTRAINT forum_actor_follows_pair_key
    UNIQUE (actor_ref, target_actor_ref)
);

CREATE TABLE IF NOT EXISTS forum_watches (
  id               text NOT NULL PRIMARY KEY,
  actor_ref        text NOT NULL,
  forum_id         text,
  topic_id         text,
  watch_kind       text NOT NULL,
  idempotency_key  text NOT NULL,
  created_at       text NOT NULL,
  archived_at      text,
  CONSTRAINT forum_watches_idempotency_key_key UNIQUE (idempotency_key),
  CONSTRAINT forum_watches_target_key
    UNIQUE (actor_ref, watch_kind, forum_id, topic_id)
);

CREATE TABLE IF NOT EXISTS forum_bookmarks (
  id               text NOT NULL PRIMARY KEY,
  actor_ref        text NOT NULL,
  topic_id         text,
  post_id          text,
  bookmark_kind    text NOT NULL,
  idempotency_key  text NOT NULL,
  created_at       text NOT NULL,
  archived_at      text,
  CONSTRAINT forum_bookmarks_idempotency_key_key UNIQUE (idempotency_key),
  CONSTRAINT forum_bookmarks_target_key
    UNIQUE (actor_ref, bookmark_kind, topic_id, post_id)
);

CREATE TABLE IF NOT EXISTS forum_reports (
  id                      text NOT NULL PRIMARY KEY,
  idempotency_key         text NOT NULL,
  reporter_actor_ref      text NOT NULL,
  target_kind             text NOT NULL,
  target_id               text NOT NULL,
  reason_ref              text NOT NULL,
  status                  text NOT NULL DEFAULT 'open',
  public_projection_json  text NOT NULL DEFAULT '{}',
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  archived_at             text,
  CONSTRAINT forum_reports_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS forum_moderation_events (
  id                      text NOT NULL PRIMARY KEY,
  moderator_actor_ref     text NOT NULL,
  action_kind             text NOT NULL,
  target_kind             text NOT NULL,
  target_id               text NOT NULL,
  reason_ref              text NOT NULL,
  report_id               text,
  public_projection_json  text NOT NULL DEFAULT '{}',
  created_at              text NOT NULL,
  archived_at             text,
  -- Appended by worker migration 0112.
  idempotency_key         text
);

-- The D1 partial unique from worker migration 0112, ported verbatim.
CREATE UNIQUE INDEX IF NOT EXISTS idx_forum_moderation_events_idempotency
  ON forum_moderation_events (idempotency_key)
  WHERE idempotency_key IS NOT NULL AND archived_at IS NULL;

CREATE TABLE IF NOT EXISTS forum_context_links (
  id                      text NOT NULL PRIMARY KEY,
  target_kind             text NOT NULL,
  target_id               text NOT NULL,
  forum_id                text NOT NULL,
  topic_id                text,
  post_id                 text,
  context_kind            text NOT NULL,
  context_id              text NOT NULL,
  context_slug            text,
  context_title           text,
  public_url              text,
  source_ref              text,
  public_projection_json  text NOT NULL DEFAULT '{}',
  created_at              text NOT NULL,
  archived_at             text,
  CONSTRAINT forum_context_links_target_context_key
    UNIQUE (target_kind, target_id, context_kind, context_id)
);

-- Live-read indexes (re-derived): board index (categories by board order,
-- forums by category), forum topic listings (pin + bump order), thread
-- pages (posts by post_number), body joins ride the post_id PK, report /
-- moderation queues, follower lookups both directions, context panels.

CREATE INDEX IF NOT EXISTS idx_forum_categories_board_order
  ON forum_categories (board_id, order_index, title)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_forums_category_order
  ON forum_forums (category_id, title)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_topics_forum_bump
  ON forum_topics (forum_id, pin_state, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_posts_topic_number
  ON forum_posts (topic_id, post_number)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_post_revisions_post_created
  ON forum_post_revisions (post_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_actor_follows_actor_active
  ON forum_actor_follows (actor_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_actor_follows_target_active
  ON forum_actor_follows (target_actor_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_reports_target
  ON forum_reports (target_kind, target_id, status, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_moderation_events_target
  ON forum_moderation_events (target_kind, target_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_context_links_context
  ON forum_context_links (context_kind, context_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_context_links_topic
  ON forum_context_links (topic_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_context_links_post
  ON forum_context_links (post_id, created_at DESC)
  WHERE archived_at IS NULL;
