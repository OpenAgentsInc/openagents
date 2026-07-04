-- KS-8.12 (#8323): Sites domain CORE — Postgres twins of the FIFTEEN
-- content/builder tables the Sites product's version-chain and deployment
-- state machine live on: `site_projects`, `site_versions`,
-- `site_deployments`, `site_deployment_attempts`, `site_access_grants`,
-- `site_events`, `site_builder_sessions`, `site_builder_messages`,
-- `site_builder_events`, `site_builder_phase_runs`,
-- `site_builder_file_snapshots`, `site_builder_previews`,
-- `site_builder_artifacts`, `site_builder_repair_attempts`,
-- `site_builder_saved_versions` (worker migrations 0032/0038/0082/0083/
-- 0084/0085).
-- Plan: docs/khala-sync/MIGRATION_PLAN.md §3.9 (universal porting rules in
-- §1); templates: 0014_forum_content.sql (KS-8.10 — the freshest),
-- 0010_agent_runtime.sql (KS-8.5), 0011_artanis_domain.sql (KS-8.6).
--
-- SCOPE NOTE: the KS-8.12 issue names ~51 tables. This migration covers
-- the CORE the dual-write mirror lands with (the #8316/#8321 precedent:
-- ship the core fully, file the exact remainder). The REMAINDER — content
-- satellites (`site_build_validations`, `site_revision_feedback`,
-- `site_compatibility_checks`, `site_provisioning_plans`,
-- `site_storage_bindings`, `site_source_exports`, referral
-- sources/policy), `site_environment_values` (may carry secrets — SPEC
-- invariant 9: secret material NEVER rides the sync path; same handling
-- as credentials), the site COMMERCE/payment tables (`site_commerce_*`,
-- `site_mdk_*`, `site_payment_catalog_items`,
-- `site_referral_payout_ledger_entries` — money discipline: D1 authority
-- referencing the KS-8.7/KS-8.8 rails by ID, no forked rails),
-- `targeted_site_*` (15, incl. `targeted_site_campaign_metric_events`,
-- an Analytics Engine candidate), `tenant_custom_hostnames`, and the
-- legacy `deployments`/`deployment_events` pair — moves in the filed
-- follow-up remainder lane (#8357).
--
-- LARGE-PAYLOAD SPLIT CONFIRMED (the issue's file-snapshot risk): builder
-- file snapshot BODIES already live in R2 (`artifact_ref`); the D1 row
-- carries metadata plus `preview_text` capped at 4000 chars by
-- `assertSafeText` in sites-builder-sessions.ts. Site version source
-- archives / build logs / worker modules are likewise R2 keys
-- (`*_r2_key`). Postgres rows stay bounded — no payload re-homing needed.
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's byte
-- representation — TEXT ISO-8601 timestamps, JSON payload columns as text
-- (NOT jsonb: row-hash reconciliation compares exact bytes). Counters and
-- sequences are bigint; stores cast reads with Number(). Tightening to
-- native types is a post-retirement cleanup, never mid-migration.
--
-- IDEMPOTENCY KEYS PORT EXACTLY (MIGRATION_PLAN §1): every
-- `site_builder_*` table's UNIQUE(idempotency_key) ports verbatim — these
-- are the write-dedupe keys the D1 SELECT-before-INSERT +
-- INSERT-OR-IGNORE paths key on. Natural-key uniques port verbatim:
-- builder messages/events/phase runs (session_id, sequence), file
-- snapshots (session_id, path, sequence), repair attempts (session_id,
-- attempt_number). SQLite treats NULLs as distinct inside composite
-- uniques; Postgres default (NULLS DISTINCT) matches.
--
-- DELIBERATELY NOT PORTED MID-MIGRATION (the KS-8.6 artanis rationale):
--   * `site_projects` partial uniques on (slug) / (software_order_id)
--     WHERE archived_at IS NULL, and
--   * `site_deployments` partial unique on (site_id) WHERE
--     status = 'active' (the one-active-deployment invariant).
--   D1 stays the enforcement authority for the whole dual-write window;
--   the mirror replays PER-STATEMENT read-back snapshots, so a
--   rollback+activate pair can transiently present two 'active' rows or a
--   re-used slug between mirror ops. Porting those uniques would make the
--   mirror reject exactly those replays and CREATE drift. They return at
--   read cutover, when Postgres becomes the arbiter.
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL QUERY PATTERNS (the KS-8.2 rule) and
-- each is justified inline; D1 artifacts not backed by a live read are
-- dropped until the read cutover re-derives them.
--
-- NO FOREIGN KEYS (dual-write mirrors and the backfill land per-row;
-- integrity is verified by reconciliation — same as 0005/0008/0010/0014).

CREATE TABLE IF NOT EXISTS site_projects (
  id                          text NOT NULL PRIMARY KEY,
  software_order_id           text,
  owner_user_id               text NOT NULL,
  team_id                     text,
  project_id                  text,
  slug                        text NOT NULL,
  title                       text NOT NULL,
  prompt                      text NOT NULL,
  status                      text NOT NULL,
  access_mode                 text NOT NULL,
  visibility                  text NOT NULL,
  source_repository_provider  text,
  source_repository_owner    text,
  source_repository_name     text,
  source_repository_ref      text,
  active_version_id           text,
  active_deployment_id        text,
  created_at                  text NOT NULL,
  updated_at                  text NOT NULL,
  archived_at                 text
);

-- readProjectBySlug + site serving lookups resolve active projects by
-- slug; non-unique here (see header — the partial unique returns at read
-- cutover).
CREATE INDEX IF NOT EXISTS site_projects_slug_idx
  ON site_projects (slug);

-- Owner site-library listings (site-library.ts listSites: owner_user_id
-- filtered, updated_at DESC).
CREATE INDEX IF NOT EXISTS site_projects_owner_updated_idx
  ON site_projects (owner_user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS site_versions (
  id                          text NOT NULL PRIMARY KEY,
  site_id                     text NOT NULL,
  source_kind                 text NOT NULL,
  source_commit_sha           text,
  source_archive_r2_key       text,
  artifact_manifest_r2_key    text,
  build_log_r2_key            text,
  build_status                text NOT NULL,
  build_command               text,
  worker_module_r2_key        text,
  static_assets_manifest_json text NOT NULL DEFAULT '{}',
  d1_binding_name             text,
  r2_binding_name             text,
  metadata_json               text NOT NULL DEFAULT '{}',
  created_by_user_id          text,
  created_by_run_id           text,
  created_at                  text NOT NULL,
  saved_at                    text,
  rejected_at                 text
);

-- The per-project version chain (listVersions/readLatest: site_id,
-- created_at DESC) — the KS-8.12 acceptance object.
CREATE INDEX IF NOT EXISTS site_versions_site_created_idx
  ON site_versions (site_id, created_at DESC);

CREATE TABLE IF NOT EXISTS site_deployments (
  id                       text NOT NULL PRIMARY KEY,
  site_id                  text NOT NULL,
  version_id               text NOT NULL,
  slug                     text NOT NULL,
  url                      text NOT NULL,
  runtime_kind             text NOT NULL,
  runtime_script_name      text,
  dispatch_namespace       text,
  status                   text NOT NULL,
  deployed_by_user_id      text,
  external_deployment_id   text,
  started_at               text,
  activated_at             text,
  failed_at                text,
  disabled_at              text,
  rolled_back_at           text,
  created_at               text NOT NULL,
  updated_at               text NOT NULL
);

-- Deployment state machine reads and the secondary-key mirror
-- (`WHERE site_id = ?` rollback/disable transitions) scan by site + status.
CREATE INDEX IF NOT EXISTS site_deployments_site_status_idx
  ON site_deployments (site_id, status);

CREATE TABLE IF NOT EXISTS site_deployment_attempts (
  id                      text NOT NULL PRIMARY KEY,
  site_id                 text NOT NULL,
  version_id              text NOT NULL,
  deployment_id           text,
  runtime_kind            text NOT NULL,
  runtime_script_name     text,
  dispatch_namespace      text,
  external_deployment_id  text,
  status                  text NOT NULL,
  upload_receipt_ref      text,
  health_status           text NOT NULL,
  health_url              text,
  health_ref              text,
  rollback_ref            text,
  observability_ref       text,
  metadata_json           text NOT NULL DEFAULT '{}',
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  archived_at             text
);

-- Attempt history is read per site newest-first (worker migration 0085's
-- live read).
CREATE INDEX IF NOT EXISTS site_deployment_attempts_site_created_idx
  ON site_deployment_attempts (site_id, created_at DESC);

CREATE TABLE IF NOT EXISTS site_access_grants (
  id              text NOT NULL PRIMARY KEY,
  site_id         text NOT NULL,
  principal_kind  text NOT NULL,
  principal_ref   text NOT NULL,
  role            text NOT NULL,
  created_at      text NOT NULL,
  revoked_at      text
);

-- Access checks list active grants per site.
CREATE INDEX IF NOT EXISTS site_access_grants_site_idx
  ON site_access_grants (site_id);

CREATE TABLE IF NOT EXISTS site_events (
  id                text NOT NULL PRIMARY KEY,
  site_id           text NOT NULL,
  version_id        text,
  deployment_id     text,
  type              text NOT NULL,
  summary           text NOT NULL,
  actor_user_id     text,
  actor_run_id      text,
  payload_json      text,
  created_at        text NOT NULL,
  -- Appended by worker migration 0038 (SQLite ALTER appends; D1 physical
  -- column order kept for SELECT * row-hash parity).
  email_message_id  text
);

-- Site timeline reads (listEvents: site_id, created_at DESC).
CREATE INDEX IF NOT EXISTS site_events_site_created_idx
  ON site_events (site_id, created_at DESC);

CREATE TABLE IF NOT EXISTS site_builder_sessions (
  id                      text NOT NULL PRIMARY KEY,
  idempotency_key         text NOT NULL,
  site_id                 text,
  order_id                text,
  workroom_id             text,
  owner_user_id           text NOT NULL,
  customer_user_id        text,
  created_by_actor_ref    text NOT NULL,
  status                  text NOT NULL,
  prompt_summary          text NOT NULL,
  source_site_version_id  text,
  source_revision_id      text,
  active_preview_id       text,
  active_artifact_id      text,
  metadata_json           text NOT NULL DEFAULT '{}',
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  archived_at             text,
  CONSTRAINT site_builder_sessions_idempotency_key_key UNIQUE (idempotency_key)
);

-- Session listings by site (site-library + builder surfaces read
-- site_id, created_at DESC; the secondary-key mirror also reads by
-- site_id).
CREATE INDEX IF NOT EXISTS site_builder_sessions_site_created_idx
  ON site_builder_sessions (site_id, created_at DESC);

CREATE TABLE IF NOT EXISTS site_builder_messages (
  id               text NOT NULL PRIMARY KEY,
  idempotency_key  text NOT NULL,
  session_id       text NOT NULL,
  sequence         bigint NOT NULL,
  actor_kind       text NOT NULL,
  visibility       text NOT NULL,
  body             text NOT NULL,
  metadata_json    text NOT NULL DEFAULT '{}',
  created_at       text NOT NULL,
  archived_at      text,
  CONSTRAINT site_builder_messages_idempotency_key_key UNIQUE (idempotency_key),
  CONSTRAINT site_builder_messages_session_sequence_key UNIQUE (session_id, sequence)
);

CREATE TABLE IF NOT EXISTS site_builder_events (
  id               text NOT NULL PRIMARY KEY,
  idempotency_key  text NOT NULL,
  session_id       text NOT NULL,
  sequence         bigint NOT NULL,
  event_kind       text NOT NULL,
  phase_kind       text,
  visibility       text NOT NULL,
  status           text NOT NULL,
  title            text NOT NULL,
  summary          text NOT NULL,
  source_ref       text,
  payload_json     text NOT NULL DEFAULT '{}',
  created_at       text NOT NULL,
  archived_at      text,
  CONSTRAINT site_builder_events_idempotency_key_key UNIQUE (idempotency_key),
  CONSTRAINT site_builder_events_session_sequence_key UNIQUE (session_id, sequence)
);

CREATE TABLE IF NOT EXISTS site_builder_phase_runs (
  id               text NOT NULL PRIMARY KEY,
  idempotency_key  text NOT NULL,
  session_id       text NOT NULL,
  sequence         bigint NOT NULL,
  phase_kind       text NOT NULL,
  status           text NOT NULL,
  title            text NOT NULL,
  summary          text NOT NULL,
  started_at       text,
  completed_at     text,
  metadata_json    text NOT NULL DEFAULT '{}',
  created_at       text NOT NULL,
  archived_at      text,
  CONSTRAINT site_builder_phase_runs_idempotency_key_key UNIQUE (idempotency_key),
  CONSTRAINT site_builder_phase_runs_session_sequence_key UNIQUE (session_id, sequence)
);

CREATE TABLE IF NOT EXISTS site_builder_file_snapshots (
  id               text NOT NULL PRIMARY KEY,
  idempotency_key  text NOT NULL,
  session_id       text NOT NULL,
  path             text NOT NULL,
  sequence         bigint NOT NULL,
  language         text,
  content_hash     text NOT NULL,
  byte_size        bigint NOT NULL,
  source_ref       text,
  artifact_ref     text,
  preview_text     text,
  visibility       text NOT NULL,
  metadata_json    text NOT NULL DEFAULT '{}',
  created_at       text NOT NULL,
  updated_at       text NOT NULL,
  archived_at      text,
  CONSTRAINT site_builder_file_snapshots_idempotency_key_key UNIQUE (idempotency_key),
  CONSTRAINT site_builder_file_snapshots_session_path_sequence_key
    UNIQUE (session_id, path, sequence)
);

CREATE TABLE IF NOT EXISTS site_builder_previews (
  id               text NOT NULL PRIMARY KEY,
  idempotency_key  text NOT NULL,
  session_id       text NOT NULL,
  preview_kind     text NOT NULL,
  status           text NOT NULL,
  preview_url      text,
  version_ref      text,
  artifact_ref     text,
  health_ref       text,
  metadata_json    text NOT NULL DEFAULT '{}',
  created_at       text NOT NULL,
  updated_at       text NOT NULL,
  archived_at      text,
  CONSTRAINT site_builder_previews_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS site_builder_artifacts (
  id               text NOT NULL PRIMARY KEY,
  idempotency_key  text NOT NULL,
  session_id       text NOT NULL,
  artifact_kind    text NOT NULL,
  artifact_ref     text NOT NULL,
  content_hash     text,
  byte_size        bigint,
  manifest_ref     text,
  metadata_json    text NOT NULL DEFAULT '{}',
  created_at       text NOT NULL,
  archived_at      text,
  CONSTRAINT site_builder_artifacts_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS site_builder_repair_attempts (
  id                text NOT NULL PRIMARY KEY,
  idempotency_key   text NOT NULL,
  session_id        text NOT NULL,
  preview_id        text,
  phase_kind        text,
  attempt_number    bigint NOT NULL,
  retry_budget      bigint NOT NULL,
  status            text NOT NULL,
  failure_kind      text NOT NULL,
  redacted_summary  text NOT NULL,
  stop_reason       text,
  metadata_json     text NOT NULL DEFAULT '{}',
  created_at        text NOT NULL,
  completed_at      text,
  archived_at       text,
  CONSTRAINT site_builder_repair_attempts_idempotency_key_key UNIQUE (idempotency_key),
  CONSTRAINT site_builder_repair_attempts_session_attempt_key
    UNIQUE (session_id, attempt_number)
);

CREATE TABLE IF NOT EXISTS site_builder_saved_versions (
  id                  text NOT NULL PRIMARY KEY,
  idempotency_key     text NOT NULL,
  session_id          text NOT NULL,
  site_id             text NOT NULL,
  site_version_id     text NOT NULL,
  preview_id          text,
  artifact_ref        text,
  build_receipt_ref   text,
  source_hash         text,
  notes               text,
  site_metadata_json  text NOT NULL DEFAULT '{}',
  created_at          text NOT NULL,
  archived_at         text,
  CONSTRAINT site_builder_saved_versions_idempotency_key_key UNIQUE (idempotency_key)
);

-- Saved-version listings per builder session (newest-first).
CREATE INDEX IF NOT EXISTS site_builder_saved_versions_session_created_idx
  ON site_builder_saved_versions (session_id, created_at DESC);
