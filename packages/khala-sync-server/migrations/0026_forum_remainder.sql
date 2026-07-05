-- KS-8.10 remainder (#8338): forum remainder domain — Postgres twins of the
-- THIRTEEN remainder forum tables that finish the KS-8.10 family after the
-- content core (#8321, migration 0014_forum_content.sql):
--   * private content: `forum_private_message_threads`,
--     `forum_private_messages` (worker migration 0101);
--   * `forum_acl_grants` (0101);
--   * trust (DERIVED): `forum_trust_edges`, `forum_actor_forum_trust` (0101);
--   * `forum_score_snapshots` (DERIVED, 0101);
--   * `forum_notification_reads` (0113);
--   * work-request lifecycle (6): `forum_work_requests`,
--     `forum_work_request_relay_links`,
--     `forum_work_request_lifecycle_posts` (0166),
--     `forum_work_request_offers`, `forum_work_request_acceptances` (0168),
--     `forum_work_request_results` + the `provider_pubkey` ALTER on offers
--     (0179).
-- Plan: docs/khala-sync/MIGRATION_PLAN.md §3.7 (universal porting rules in
-- §1); template: 0014_forum_content.sql (the parent content lane).
--
-- MIGRATION-NUMBER NOTE: renumbered 0024 -> 0026 on rebase after sibling
-- KS-8 lanes landed 0024_supervision_longtail.sql and
-- 0025_sites_remainder.sql. The KS-8 waves collide on numbers; the file is
-- order-only with no cross-references, so renumbering is safe.
--
-- PRIVACY: `forum_private_message_threads` / `forum_private_messages` are
-- sensitive. The Postgres twin stores EXACTLY what D1 stores (message
-- bodies live behind `content_ref`, participants behind
-- `participant_refs_json`); migration diagnostics reference row KEYS and
-- sha256 hashes ONLY — never subjects, participant lists, or message
-- content.
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's byte
-- representation — TEXT ISO-8601 timestamps, JSON payload/array columns as
-- text (NOT jsonb: row-hash reconciliation compares exact bytes). Counters
-- and sats amounts are bigint; stores cast reads with Number(). Tightening
-- to native types is a post-retirement cleanup, never mid-migration.
--
-- DERIVED TABLES (recompute-and-compare): `forum_trust_edges`,
-- `forum_actor_forum_trust`, and `forum_score_snapshots` are recomputed
-- from events in D1. This lane mirrors the D1 snapshot and VERIFIES the
-- twin against D1 (counts, aggregate tallies, and a portable cross-table
-- recompute of actor_forum_trust from trust_edges that runs identically on
-- both stores) rather than re-running the recompute on Postgres.
--
-- IDEMPOTENCY / NATURAL KEYS PORT EXACTLY (MIGRATION_PLAN §1):
--   * `forum_work_requests`: UNIQUE(idempotency_key), UNIQUE(topic_id),
--     UNIQUE(job_event_id).
--   * `forum_work_request_relay_links`: UNIQUE(work_request_id),
--     UNIQUE(topic_id), UNIQUE(job_event_id).
--   * `forum_work_request_offers`: UNIQUE(quote_ref).
--   * `forum_work_request_lifecycle_posts`: UNIQUE(post_id),
--     UNIQUE(idempotency_key).
--   * `forum_work_request_acceptances`: UNIQUE(idempotency_key),
--     UNIQUE(work_request_id), UNIQUE(escrow_id), UNIQUE(reserve_receipt_ref).
--   * `forum_work_request_results`: UNIQUE(quote_ref).
--   * `forum_acl_grants`: UNIQUE(actor_ref, forum_id, permission, scope_ref)
--     — SQLite treats NULL forum_id as distinct inside a composite unique;
--     Postgres default (NULLS DISTINCT) matches.
--   * `forum_actor_forum_trust`: UNIQUE(actor_ref, forum_id).
--   * `forum_notification_reads`: D1's PARTIAL uniques
--     (actor_ref, notification_id) and (actor_ref, idempotency_key), both
--     WHERE archived_at IS NULL, port as the same partial indexes.
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL QUERY PATTERNS (the KS-8.2 rule):
-- kept indexes serve live reads — private-message thread pages by
-- (thread_id, created_at); notification read lookups by actor; the
-- work-request board by (state, created_at) and by requester; per-request
-- offer/lifecycle/result listings. D1 artifacts not backed by a live read
-- on the Postgres side are deferred until the read cutover re-derives them.
--
-- NO FOREIGN KEYS (dual-write mirrors and the backfill land per-row;
-- integrity is verified by reconciliation set-membership — same as
-- 0005/0008/0010/0014).

CREATE TABLE IF NOT EXISTS forum_private_message_threads (
  id                      text NOT NULL PRIMARY KEY,
  subject                 text NOT NULL,
  slug                    text NOT NULL,
  created_by_actor_ref    text NOT NULL,
  participant_refs_json   text NOT NULL DEFAULT '[]',
  latest_message_id       text,
  message_count           bigint NOT NULL DEFAULT 0,
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  archived_at             text
);

CREATE TABLE IF NOT EXISTS forum_private_messages (
  id                      text NOT NULL PRIMARY KEY,
  thread_id               text NOT NULL,
  sender_actor_ref        text NOT NULL,
  recipient_actor_ref     text NOT NULL,
  content_ref             text NOT NULL,
  public_projection_json  text NOT NULL DEFAULT '{}',
  created_at              text NOT NULL,
  archived_at             text
);

CREATE INDEX IF NOT EXISTS idx_forum_private_messages_thread_created
  ON forum_private_messages (thread_id, created_at);

CREATE TABLE IF NOT EXISTS forum_acl_grants (
  id                    text NOT NULL PRIMARY KEY,
  actor_ref             text NOT NULL,
  forum_id              text,
  permission            text NOT NULL,
  scope_ref             text NOT NULL,
  granted_by_actor_ref  text NOT NULL,
  created_at            text NOT NULL,
  revoked_at            text,
  CONSTRAINT forum_acl_grants_natural_key
    UNIQUE (actor_ref, forum_id, permission, scope_ref)
);

CREATE TABLE IF NOT EXISTS forum_trust_edges (
  id                 text NOT NULL PRIMARY KEY,
  source_actor_ref   text NOT NULL,
  target_actor_ref   text NOT NULL,
  forum_id           text,
  trust_kind         text NOT NULL,
  weight             bigint NOT NULL DEFAULT 0,
  event_ref          text NOT NULL,
  created_at         text NOT NULL,
  archived_at        text
);

CREATE INDEX IF NOT EXISTS idx_forum_trust_edges_actor_forum
  ON forum_trust_edges (target_actor_ref, forum_id, trust_kind);

CREATE TABLE IF NOT EXISTS forum_actor_forum_trust (
  id                          text NOT NULL PRIMARY KEY,
  actor_ref                   text NOT NULL,
  forum_id                    text NOT NULL,
  trust_score                 bigint NOT NULL DEFAULT 0,
  reward_count                bigint NOT NULL DEFAULT 0,
  report_count                bigint NOT NULL DEFAULT 0,
  moderator_adjustment_count  bigint NOT NULL DEFAULT 0,
  score_ref                   text NOT NULL,
  updated_at                  text NOT NULL,
  archived_at                 text,
  CONSTRAINT forum_actor_forum_trust_actor_forum_key
    UNIQUE (actor_ref, forum_id)
);

CREATE TABLE IF NOT EXISTS forum_score_snapshots (
  id                        text NOT NULL PRIMARY KEY,
  target_kind               text NOT NULL,
  target_id                 text NOT NULL,
  positive_bitcoin_sats     bigint NOT NULL DEFAULT 0,
  boost_bitcoin_sats        bigint NOT NULL DEFAULT 0,
  down_signal_bitcoin_sats  bigint NOT NULL DEFAULT 0,
  reply_count               bigint NOT NULL DEFAULT 0,
  net_investment_sats       bigint NOT NULL DEFAULT 0,
  score_ref                 text NOT NULL,
  rebuilt_from_event_ref    text NOT NULL,
  public_projection_json    text NOT NULL DEFAULT '{}',
  created_at                text NOT NULL,
  archived_at               text
);

CREATE INDEX IF NOT EXISTS idx_forum_score_snapshots_target
  ON forum_score_snapshots (target_kind, target_id, created_at);

CREATE TABLE IF NOT EXISTS forum_notification_reads (
  id               text NOT NULL PRIMARY KEY,
  actor_ref        text NOT NULL,
  notification_id  text NOT NULL,
  idempotency_key  text NOT NULL,
  read_at          text NOT NULL,
  created_at       text NOT NULL,
  updated_at       text NOT NULL,
  archived_at      text
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forum_notification_reads_actor_notification
  ON forum_notification_reads (actor_ref, notification_id)
  WHERE archived_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_forum_notification_reads_actor_idempotency
  ON forum_notification_reads (actor_ref, idempotency_key)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS forum_work_requests (
  id                             text NOT NULL PRIMARY KEY,
  idempotency_key                text NOT NULL,
  topic_id                       text NOT NULL,
  first_post_id                  text NOT NULL,
  requester_actor_ref            text NOT NULL,
  title                          text NOT NULL,
  objective_ref                  text NOT NULL,
  verification_command_ref       text NOT NULL,
  repository_refs_json           text NOT NULL DEFAULT '[]',
  required_capability_refs_json  text NOT NULL DEFAULT '[]',
  budget_sats                    bigint NOT NULL,
  budget_msats                   bigint NOT NULL,
  deadline_ref                   text NOT NULL,
  relay_url                      text NOT NULL,
  job_event_id                   text NOT NULL,
  job_event_kind                 bigint NOT NULL,
  job_result_kind                bigint NOT NULL,
  state                          text NOT NULL DEFAULT 'open',
  quote_count                    bigint NOT NULL DEFAULT 0,
  public_projection_json         text NOT NULL DEFAULT '{}',
  created_at                     text NOT NULL,
  updated_at                     text NOT NULL,
  archived_at                    text,
  CONSTRAINT forum_work_requests_idempotency_key_key UNIQUE (idempotency_key),
  CONSTRAINT forum_work_requests_topic_id_key UNIQUE (topic_id),
  CONSTRAINT forum_work_requests_job_event_id_key UNIQUE (job_event_id)
);

CREATE INDEX IF NOT EXISTS idx_forum_work_requests_state_created
  ON forum_work_requests (state, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_work_requests_actor_created
  ON forum_work_requests (requester_actor_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS forum_work_request_relay_links (
  id                text NOT NULL PRIMARY KEY,
  work_request_id   text NOT NULL,
  topic_id          text NOT NULL,
  job_event_id      text NOT NULL,
  job_event_kind    bigint NOT NULL,
  relay_url         text NOT NULL,
  relay_ref         text NOT NULL,
  bridge_actor_ref  text NOT NULL,
  event_json        text NOT NULL,
  created_at        text NOT NULL,
  archived_at       text,
  CONSTRAINT forum_work_request_relay_links_work_request_id_key
    UNIQUE (work_request_id),
  CONSTRAINT forum_work_request_relay_links_topic_id_key UNIQUE (topic_id),
  CONSTRAINT forum_work_request_relay_links_job_event_id_key
    UNIQUE (job_event_id)
);

CREATE TABLE IF NOT EXISTS forum_work_request_offers (
  id                      text NOT NULL PRIMARY KEY,
  work_request_id         text NOT NULL,
  quote_ref               text NOT NULL,
  provider_actor_ref      text NOT NULL,
  amount_sats             bigint NOT NULL,
  amount_msats            bigint NOT NULL,
  capability_refs_json    text NOT NULL DEFAULT '[]',
  relay_event_ref         text,
  state                   text NOT NULL DEFAULT 'offered',
  public_projection_json  text NOT NULL DEFAULT '{}',
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  archived_at             text,
  -- Appended by worker migration 0179 (SQLite ALTER appends).
  provider_pubkey         text,
  CONSTRAINT forum_work_request_offers_quote_ref_key UNIQUE (quote_ref)
);

CREATE INDEX IF NOT EXISTS idx_forum_work_request_offers_request
  ON forum_work_request_offers (work_request_id, state, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS forum_work_request_lifecycle_posts (
  id               text NOT NULL PRIMARY KEY,
  work_request_id  text NOT NULL,
  topic_id         text NOT NULL,
  post_id          text NOT NULL,
  idempotency_key  text NOT NULL,
  lifecycle_kind   text NOT NULL,
  receipt_ref      text NOT NULL,
  state_after      text NOT NULL,
  created_at       text NOT NULL,
  archived_at      text,
  CONSTRAINT forum_work_request_lifecycle_posts_post_id_key UNIQUE (post_id),
  CONSTRAINT forum_work_request_lifecycle_posts_idempotency_key_key
    UNIQUE (idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_forum_work_request_lifecycle_posts_request
  ON forum_work_request_lifecycle_posts (work_request_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS forum_work_request_acceptances (
  id                      text NOT NULL PRIMARY KEY,
  idempotency_key         text NOT NULL,
  work_request_id         text NOT NULL,
  offer_id                text NOT NULL,
  quote_ref               text NOT NULL,
  requester_actor_ref     text NOT NULL,
  provider_actor_ref      text NOT NULL,
  amount_msats            bigint NOT NULL,
  escrow_id               text NOT NULL,
  reserve_receipt_ref     text NOT NULL,
  acceptance_event_ref    text NOT NULL,
  public_projection_json  text NOT NULL DEFAULT '{}',
  created_at              text NOT NULL,
  archived_at             text,
  CONSTRAINT forum_work_request_acceptances_idempotency_key_key
    UNIQUE (idempotency_key),
  CONSTRAINT forum_work_request_acceptances_work_request_id_key
    UNIQUE (work_request_id),
  CONSTRAINT forum_work_request_acceptances_escrow_id_key UNIQUE (escrow_id),
  CONSTRAINT forum_work_request_acceptances_reserve_receipt_ref_key
    UNIQUE (reserve_receipt_ref)
);

CREATE TABLE IF NOT EXISTS forum_work_request_results (
  id                        text NOT NULL PRIMARY KEY,
  work_request_id           text NOT NULL,
  offer_id                  text NOT NULL,
  quote_ref                 text NOT NULL,
  provider_actor_ref        text NOT NULL,
  result_event_ref          text NOT NULL,
  verification_command_ref  text NOT NULL,
  artifact_refs_json        text NOT NULL DEFAULT '[]',
  closeout_ref              text,
  public_projection_json    text NOT NULL DEFAULT '{}',
  created_at                text NOT NULL,
  archived_at               text,
  CONSTRAINT forum_work_request_results_quote_ref_key UNIQUE (quote_ref)
);

CREATE INDEX IF NOT EXISTS idx_forum_work_request_results_request
  ON forum_work_request_results (work_request_id, created_at DESC)
  WHERE archived_at IS NULL;
