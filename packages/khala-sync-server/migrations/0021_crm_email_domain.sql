-- KS-8.11 (#8322): CRM / email / enrichment domain — Postgres twins of the 36
-- canonical D1 tables (worker migrations 0026/0038/0041/0063/0064/0181/0193/
-- 0218/0219/0220/0296). Plan: docs/khala-sync/MIGRATION_PLAN.md §3.8 (Wave C).
--
-- The three-to-four `*_0193_new` D1 names were TRANSIENT rebuild artifacts:
-- worker migration 0193 created them, copied the canonical rows in, dropped
-- the old canonical tables, and RENAMED the artifacts back to the canonical
-- names. Only the canonical tables exist in live D1; only canonical twins are
-- created here. Verified superseded — nothing to port under the `_0193_new`
-- names.
--
-- PRIVACY (the KS-8.11 gate): these rows carry names, emails, and notes —
-- PII. The Postgres twin stores EXACTLY what D1 stores (no widening, no new
-- derived columns); every diagnostic and verification surface built on this
-- schema reports keys/hashes/counts only, never row contents.
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's byte
-- representation — TEXT ISO-8601 timestamps (sort correctly as text), 0/1
-- booleans as smallint, JSON payloads as text (NOT jsonb: row-hash
-- reconciliation compares exact bytes), REAL cost columns as double
-- precision, and money-cents columns as bigint (SQLite INTEGER is 64-bit).
-- Tightening to native types is a post-retirement cleanup, never
-- mid-migration.
--
-- IDEMPOTENCY / DEDUPE KEYS PORT EXACTLY (the issue's compliance risks):
--   - `email_campaign_sends.idempotency_key` — the campaign send dedupe key
--     (`email_campaign_send:<enrollment_id>:<step_key>`, i.e. enrollment ×
--     step). Ported as the SAME unique constraint so the send path can never
--     double-email a real person off the Postgres side.
--   - `email_campaign_enrollments.idempotency_key`,
--     `email_messages.idempotency_key`, `list_subscribers.idempotency_key`,
--     `email_provider_events (provider, provider_event_id)` (webhook replay
--     safety), `email_drafts (provider, provider_draft_id)`,
--     `crm_mcp_grants.grant_ref` / `.token_hash`,
--     `business_outreach_sends.send_receipt_ref`,
--     `business_outreach_suppressions (subject_ref, reason)`.
--
-- NO CROSS-TABLE FOREIGN KEYS (deliberate, unlike D1): dual-write mirrors and
-- the backfill land per-table and per-row; a send mirror may arrive before
-- its enrollment is backfilled. Referential integrity is verified by
-- set-membership at reconciliation, not enforced mid-migration.
--
-- Partial unique indexes NOT ported mid-migration (artanis 0011 precedent —
-- re-added at read cutover; D1 stays the enforcement authority):
--   - `crm_accounts (tenant_ref, name) WHERE archived_at IS NULL`
--   - `exa_enrichment_cache_entries (cache_key) WHERE archived_at IS NULL`
-- Exception kept: `crm_activities (source_record_type, source_record_id)`
-- partial unique IS ported — it is the INSERT OR IGNORE replay-dedupe key for
-- provider-event activity logging, rows are append-only (no deletes anywhere
-- in this domain), so a transient double is impossible from id-keyed copies.
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL QUERY PATTERNS in the owning worker
-- modules (crm-store.ts, crm-email.ts, crm-command.ts, crm-mcp-grant.ts,
-- email.ts, email-campaigns.ts, email-campaign-dispatcher.ts,
-- email-preferences.ts, email-sequence-authoring.ts, native-lists.ts,
-- business-outreach.ts, adjutant-enrichment-ledger.ts,
-- adjutant-enrichment-operations.ts, resend-webhooks.ts) — NOT blind-ported
-- from D1. Notably ADDED relative to D1: `email_deliveries (provider,
-- provider_message_id)` — the Resend webhook delivery-state upsert filters on
-- exactly that pair and D1 had no index for it.

-- ---------------------------------------------------------------------------
-- CRM core (worker migration 0218)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS crm_contacts (
  id                    text PRIMARY KEY,
  tenant_ref            text NOT NULL,
  primary_email         text NOT NULL,
  secondary_email       text,
  full_name             text,
  first_name            text,
  last_name             text,
  job_title             text,
  contact_type          text NOT NULL DEFAULT 'prospect',
  relationship_stage    text NOT NULL DEFAULT 'new',
  lifecycle_stage       text NOT NULL DEFAULT 'lead',
  account_id            text,
  portal_access_status  text NOT NULL DEFAULT 'none',
  engagement_score      integer NOT NULL DEFAULT 0,
  last_contacted_at     text,
  last_engaged_at       text,
  last_replied_at       text,
  external_source_label text,
  external_source_id    text,
  notes                 text,
  metadata_json         text NOT NULL DEFAULT '{}',
  created_at            text NOT NULL,
  updated_at            text NOT NULL,
  archived_at           text
);

-- The upsert find key (tenant, normalized email) — ports exactly.
CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_crm_contacts_tenant_email
  ON crm_contacts (tenant_ref, primary_email);

CREATE INDEX IF NOT EXISTS idx_pg_crm_contacts_tenant_listing
  ON crm_contacts (tenant_ref, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_pg_crm_contacts_account
  ON crm_contacts (account_id);

CREATE TABLE IF NOT EXISTS crm_accounts (
  id            text PRIMARY KEY,
  tenant_ref    text NOT NULL,
  name          text NOT NULL,
  domain        text,
  account_type  text NOT NULL DEFAULT 'company',
  status        text NOT NULL DEFAULT 'active',
  website_url   text,
  notes         text,
  metadata_json text NOT NULL DEFAULT '{}',
  created_at    text NOT NULL,
  updated_at    text NOT NULL,
  archived_at   text
);

-- D1's unique (tenant_ref, name) WHERE archived_at IS NULL is deliberately a
-- plain index here (partial-unique rule above); the upsert find path reads
-- (tenant_ref, name, archived_at IS NULL).
CREATE INDEX IF NOT EXISTS idx_pg_crm_accounts_tenant_name
  ON crm_accounts (tenant_ref, name);

CREATE INDEX IF NOT EXISTS idx_pg_crm_accounts_tenant_listing
  ON crm_accounts (tenant_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS crm_contact_lists (
  id          text PRIMARY KEY,
  tenant_ref  text NOT NULL,
  slug        text NOT NULL,
  name        text NOT NULL,
  description text,
  is_system   smallint NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
  created_at  text NOT NULL,
  updated_at  text NOT NULL,
  archived_at text
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_crm_contact_lists_tenant_slug
  ON crm_contact_lists (tenant_ref, slug);

CREATE TABLE IF NOT EXISTS crm_contact_list_memberships (
  id                text PRIMARY KEY,
  tenant_ref        text NOT NULL,
  contact_id        text NOT NULL,
  list_id           text NOT NULL,
  membership_status text NOT NULL DEFAULT 'active',
  source            text NOT NULL DEFAULT 'manual',
  created_at        text NOT NULL,
  updated_at        text NOT NULL
);

-- The membership upsert conflict key — ports exactly.
CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_crm_contact_list_memberships_pair
  ON crm_contact_list_memberships (contact_id, list_id);

CREATE TABLE IF NOT EXISTS crm_activities (
  id                 text PRIMARY KEY,
  tenant_ref         text NOT NULL,
  contact_id         text NOT NULL,
  account_id         text,
  activity_type      text NOT NULL,
  subject            text,
  summary            text,
  occurred_at        text NOT NULL,
  actor_ref          text,
  source_system      text NOT NULL DEFAULT 'crm',
  source_record_type text,
  source_record_id   text,
  metadata_json      text NOT NULL DEFAULT '{}',
  created_at         text NOT NULL,
  updated_at         text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_crm_activities_contact
  ON crm_activities (tenant_ref, contact_id, occurred_at DESC);

-- INSERT OR IGNORE replay-dedupe key for provider-event activity logging
-- (append-only table — the partial-unique exception documented above).
CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_crm_activities_source
  ON crm_activities (source_record_type, source_record_id)
  WHERE source_record_type IS NOT NULL AND source_record_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS crm_engagement_snapshots (
  id                     text PRIMARY KEY,
  tenant_ref             text NOT NULL,
  contact_id             text NOT NULL,
  last_email_sent_at     text,
  last_email_opened_at   text,
  last_email_clicked_at  text,
  last_email_replied_at  text,
  email_sent_count_30d   integer NOT NULL DEFAULT 0,
  email_open_count_30d   integer NOT NULL DEFAULT 0,
  email_click_count_30d  integer NOT NULL DEFAULT 0,
  engagement_score       integer NOT NULL DEFAULT 0,
  snapshot_metadata_json text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  updated_at             text NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_crm_engagement_snapshots_contact
  ON crm_engagement_snapshots (contact_id);

CREATE TABLE IF NOT EXISTS crm_opportunities (
  id                     text PRIMARY KEY,
  tenant_ref             text NOT NULL,
  account_id             text,
  name                   text NOT NULL,
  round_name             text,
  stage                  text NOT NULL DEFAULT 'sourcing',
  status                 text NOT NULL DEFAULT 'open',
  target_amount_cents    bigint,
  expected_amount_cents  bigint,
  conviction_probability integer,
  target_close_date      text,
  summary                text,
  metadata_json          text NOT NULL DEFAULT '{}',
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  archived_at            text
);

CREATE INDEX IF NOT EXISTS idx_pg_crm_opportunities_tenant_listing
  ON crm_opportunities (tenant_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS crm_opportunity_contact_roles (
  id             text PRIMARY KEY,
  tenant_ref     text NOT NULL,
  opportunity_id text NOT NULL,
  contact_id     text NOT NULL,
  role_type      text NOT NULL DEFAULT 'participant',
  status         text NOT NULL DEFAULT 'active',
  notes          text,
  created_at     text NOT NULL,
  updated_at     text NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_crm_opportunity_contact_roles_pair
  ON crm_opportunity_contact_roles (opportunity_id, contact_id);

CREATE TABLE IF NOT EXISTS crm_source_import_runs (
  id             text PRIMARY KEY,
  tenant_ref     text NOT NULL,
  source_label   text NOT NULL,
  status         text NOT NULL DEFAULT 'completed',
  total_rows     integer NOT NULL DEFAULT 0,
  imported_rows  integer NOT NULL DEFAULT 0,
  updated_rows   integer NOT NULL DEFAULT 0,
  duplicate_rows integer NOT NULL DEFAULT 0,
  failed_rows    integer NOT NULL DEFAULT 0,
  error_summary  text,
  metadata_json  text NOT NULL DEFAULT '{}',
  created_at     text NOT NULL,
  updated_at     text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_crm_source_import_runs_tenant
  ON crm_source_import_runs (tenant_ref, created_at DESC);

-- ---------------------------------------------------------------------------
-- CRM email templates / send ledger / commands / MCP grants (0219, 0218, 0220)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS crm_email_templates (
  id                     text PRIMARY KEY,
  tenant_ref             text NOT NULL,
  slug                   text NOT NULL,
  name                   text NOT NULL,
  subject_template       text NOT NULL,
  body_markdown_template text NOT NULL,
  status                 text NOT NULL DEFAULT 'active',
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  archived_at            text
);

-- The template upsert conflict key — ports exactly.
CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_crm_email_templates_tenant_slug
  ON crm_email_templates (tenant_ref, slug);

CREATE TABLE IF NOT EXISTS crm_email_messages (
  id                  text PRIMARY KEY,
  tenant_ref          text NOT NULL,
  contact_id          text NOT NULL,
  template_id         text,
  channel             text NOT NULL,
  from_email          text,
  to_email            text NOT NULL,
  subject             text NOT NULL,
  body_markdown       text NOT NULL,
  body_html           text,
  status              text NOT NULL DEFAULT 'draft',
  send_reason         text,
  provider_message_id text,
  provider_draft_id   text,
  error_message       text,
  sent_at             text,
  created_at          text NOT NULL,
  updated_at          text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_crm_email_messages_contact
  ON crm_email_messages (contact_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_pg_crm_email_messages_tenant
  ON crm_email_messages (tenant_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS crm_contact_commands (
  id              text PRIMARY KEY,
  tenant_ref      text NOT NULL,
  contact_id      text,
  command_kind    text NOT NULL,
  status          text NOT NULL DEFAULT 'proposed',
  proposed_by_ref text,
  approval_state  text NOT NULL DEFAULT 'pending_approval',
  payload_json    text NOT NULL DEFAULT '{}',
  result_json     text NOT NULL DEFAULT '{}',
  created_at      text NOT NULL,
  updated_at      text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_crm_contact_commands_tenant
  ON crm_contact_commands (tenant_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS crm_mcp_grants (
  id                     text PRIMARY KEY,
  grant_ref              text NOT NULL UNIQUE,
  token_hash             text NOT NULL UNIQUE,
  tenant_ref             text NOT NULL,
  authority_classes_json text NOT NULL DEFAULT '[]',
  label                  text,
  status                 text NOT NULL DEFAULT 'active',
  created_at             text NOT NULL,
  expires_at             text
);

CREATE INDEX IF NOT EXISTS idx_pg_crm_mcp_grants_tenant
  ON crm_mcp_grants (tenant_ref, created_at DESC);

-- ---------------------------------------------------------------------------
-- Email ledger (worker migration 0026 as rebuilt by 0193 — canonical names)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS email_templates (
  id                      text PRIMARY KEY,
  kind                    text NOT NULL,
  slug                    text NOT NULL UNIQUE,
  name                    text NOT NULL,
  subject_template        text NOT NULL,
  text_template           text NOT NULL,
  html_template           text NOT NULL,
  variable_schema_version text NOT NULL,
  status                  text NOT NULL,
  created_at              text NOT NULL,
  updated_at              text NOT NULL
);

CREATE TABLE IF NOT EXISTS email_messages (
  id                    text PRIMARY KEY,
  kind                  text NOT NULL,
  actor_user_id         text,
  target_user_id        text,
  to_email              text NOT NULL,
  from_email            text NOT NULL,
  reply_to_email        text,
  subject               text NOT NULL,
  text_body             text NOT NULL,
  html_body             text NOT NULL,
  template_id           text,
  template_slug         text NOT NULL,
  template_context_json text NOT NULL DEFAULT '{}',
  status                text NOT NULL,
  provider              text,
  provider_message_id   text,
  provider_draft_id     text,
  provider_thread_id    text,
  idempotency_key       text NOT NULL UNIQUE,
  source_authority_ref  text NOT NULL,
  action_submission_id  text,
  metadata_json         text NOT NULL DEFAULT '{}',
  error_name            text,
  error_message         text,
  created_at            text NOT NULL,
  updated_at            text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_email_messages_kind_status
  ON email_messages (kind, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_pg_email_messages_target_user
  ON email_messages (target_user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS email_deliveries (
  id                            text PRIMARY KEY,
  message_id                    text NOT NULL,
  provider                      text NOT NULL,
  provider_message_id           text,
  provider_thread_id            text,
  provider_request_id           text,
  provider_idempotency_key      text NOT NULL,
  status                        text NOT NULL,
  error_name                    text,
  error_message                 text,
  provider_payload_summary_json text NOT NULL DEFAULT '{}',
  attempted_at                  text NOT NULL,
  completed_at                  text,
  created_at                    text NOT NULL,
  updated_at                    text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_email_deliveries_message
  ON email_deliveries (message_id, attempted_at DESC);

-- Re-derived (not in D1): the Resend webhook delivery-state update filters
-- WHERE provider = 'resend' AND provider_message_id = ?.
CREATE INDEX IF NOT EXISTS idx_pg_email_deliveries_provider_message
  ON email_deliveries (provider, provider_message_id);

CREATE TABLE IF NOT EXISTS email_drafts (
  id                  text PRIMARY KEY,
  message_id          text NOT NULL,
  provider            text NOT NULL,
  provider_draft_id   text NOT NULL,
  provider_message_id text,
  provider_thread_id  text,
  status              text NOT NULL,
  provenance_json     text NOT NULL DEFAULT '{}',
  created_at          text NOT NULL,
  updated_at          text NOT NULL,
  sent_at             text
);

-- The draft upsert conflict key — ports exactly.
CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_email_drafts_provider_draft
  ON email_drafts (provider, provider_draft_id);

CREATE INDEX IF NOT EXISTS idx_pg_email_drafts_message
  ON email_drafts (message_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS email_provider_events (
  id                   text PRIMARY KEY,
  provider             text NOT NULL,
  provider_event_id    text NOT NULL,
  event_type           text NOT NULL,
  email                text,
  email_message_id     text,
  provider_message_id  text,
  occurred_at          text,
  payload_summary_json text NOT NULL DEFAULT '{}',
  source_authority_ref text NOT NULL,
  created_at           text NOT NULL
);

-- Webhook replay-safety key — ports exactly.
CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_email_provider_events_provider_event
  ON email_provider_events (provider, provider_event_id);

CREATE INDEX IF NOT EXISTS idx_pg_email_provider_events_type_created
  ON email_provider_events (provider, event_type, created_at DESC);

-- ---------------------------------------------------------------------------
-- Campaigns, preferences, suppression (worker migrations 0063/0064)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS email_campaigns (
  id                   text PRIMARY KEY,
  slug                 text NOT NULL UNIQUE,
  name                 text NOT NULL,
  audience             text NOT NULL,
  status               text NOT NULL,
  source_authority_ref text NOT NULL,
  metadata_json        text NOT NULL DEFAULT '{}',
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  archived_at          text
);

CREATE INDEX IF NOT EXISTS idx_pg_email_campaigns_status_updated
  ON email_campaigns (status, updated_at DESC);

CREATE TABLE IF NOT EXISTS email_campaign_steps (
  id             text PRIMARY KEY,
  campaign_id    text NOT NULL,
  step_key       text NOT NULL,
  name           text NOT NULL,
  delay_seconds  integer NOT NULL CHECK (delay_seconds >= 0),
  template_slug  text NOT NULL,
  lifecycle_kind text,
  status         text NOT NULL,
  metadata_json  text NOT NULL DEFAULT '{}',
  created_at     text NOT NULL,
  updated_at     text NOT NULL,
  archived_at    text
);

-- The step upsert conflict key — ports exactly.
CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_email_campaign_steps_campaign_key
  ON email_campaign_steps (campaign_id, step_key);

CREATE INDEX IF NOT EXISTS idx_pg_email_campaign_steps_campaign_delay
  ON email_campaign_steps (campaign_id, delay_seconds ASC);

CREATE TABLE IF NOT EXISTS email_campaign_enrollments (
  id                   text PRIMARY KEY,
  campaign_id          text NOT NULL,
  user_id              text,
  email                text NOT NULL,
  status               text NOT NULL,
  idempotency_key      text NOT NULL UNIQUE,
  source_authority_ref text NOT NULL,
  metadata_json        text NOT NULL DEFAULT '{}',
  enrolled_at          text NOT NULL,
  completed_at         text,
  canceled_at          text,
  created_at           text NOT NULL,
  updated_at           text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_email_campaign_enrollments_campaign_status
  ON email_campaign_enrollments (campaign_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_pg_email_campaign_enrollments_email
  ON email_campaign_enrollments (email, updated_at DESC);

CREATE TABLE IF NOT EXISTS email_campaign_sends (
  id                   text PRIMARY KEY,
  campaign_id          text NOT NULL,
  step_id              text NOT NULL,
  enrollment_id        text NOT NULL,
  user_id              text,
  email                text NOT NULL,
  due_at               text NOT NULL,
  status               text NOT NULL,
  -- The enrollment × step send-dedupe key. Ports exactly (see header).
  idempotency_key      text NOT NULL UNIQUE,
  source_authority_ref text NOT NULL,
  email_message_id     text,
  provider_event_id    text,
  error_name           text,
  error_message        text,
  metadata_json        text NOT NULL DEFAULT '{}',
  claimed_at           text,
  sent_at              text,
  skipped_at           text,
  failed_at            text,
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  attempt_count        integer NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
  next_attempt_at      text
);

-- The dispatcher's due query: status = 'scheduled' AND due_at <= now AND
-- (next_attempt_at IS NULL OR next_attempt_at <= now) ORDER BY due_at.
CREATE INDEX IF NOT EXISTS idx_pg_email_campaign_sends_dispatch_due
  ON email_campaign_sends (status, due_at ASC, next_attempt_at ASC);

CREATE INDEX IF NOT EXISTS idx_pg_email_campaign_sends_enrollment
  ON email_campaign_sends (enrollment_id, due_at ASC);

CREATE TABLE IF NOT EXISTS email_preferences (
  id                    text PRIMARY KEY,
  user_id               text,
  email                 text NOT NULL UNIQUE,
  marketing_opt_in      smallint NOT NULL DEFAULT 1 CHECK (marketing_opt_in IN (0, 1)),
  drip_opt_in           smallint NOT NULL DEFAULT 1 CHECK (drip_opt_in IN (0, 1)),
  transactional_opt_in  smallint NOT NULL DEFAULT 1 CHECK (transactional_opt_in IN (0, 1)),
  source_authority_ref  text NOT NULL,
  updated_by_user_id    text,
  created_at            text NOT NULL,
  updated_at            text NOT NULL
);

-- COMPLIANCE GATE: the send path's suppression read is
--   WHERE email = ? AND active = 1 AND archived_at IS NULL
--     AND scope IN (?, 'all')
-- (email-campaigns.ts isEmailSuppressed; email-preferences.ts
-- readSuppression). This index serves it exactly.
CREATE TABLE IF NOT EXISTS email_suppression_entries (
  id                   text PRIMARY KEY,
  email                text NOT NULL,
  reason               text NOT NULL,
  scope                text NOT NULL,
  active               smallint NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
  source_authority_ref text NOT NULL,
  provider_event_id    text,
  note                 text,
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  archived_at          text
);

CREATE INDEX IF NOT EXISTS idx_pg_email_suppression_entries_email_active
  ON email_suppression_entries (email, active, updated_at DESC)
  WHERE archived_at IS NULL;

-- ---------------------------------------------------------------------------
-- Native subscriber lists (worker migration 0181)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS subscriber_lists (
  id                   text PRIMARY KEY,
  owner_user_id        text,
  team_id              text,
  slug                 text NOT NULL UNIQUE,
  name                 text NOT NULL,
  status               text NOT NULL,
  source_authority_ref text NOT NULL,
  metadata_json        text NOT NULL DEFAULT '{}',
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  archived_at          text
);

CREATE INDEX IF NOT EXISTS idx_pg_subscriber_lists_status_updated
  ON subscriber_lists (status, updated_at DESC);

CREATE TABLE IF NOT EXISTS list_subscribers (
  id              text PRIMARY KEY,
  list_id         text NOT NULL,
  email           text NOT NULL,
  status          text NOT NULL,
  source_ref      text NOT NULL,
  idempotency_key text NOT NULL UNIQUE,
  metadata_json   text NOT NULL DEFAULT '{}',
  subscribed_at   text NOT NULL,
  unsubscribed_at text,
  bounced_at      text,
  created_at      text NOT NULL,
  updated_at      text NOT NULL
);

-- One subscriber row per (list, email) — ports exactly.
CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_list_subscribers_list_email
  ON list_subscribers (list_id, email);

CREATE INDEX IF NOT EXISTS idx_pg_list_subscribers_list_status
  ON list_subscribers (list_id, status, updated_at DESC);

-- ---------------------------------------------------------------------------
-- Business outreach (worker migration 0296)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS business_outreach_template_approvals (
  approval_receipt_ref text PRIMARY KEY,
  template_version_ref text NOT NULL,
  approved_by_ref      text NOT NULL,
  source_ref           text NOT NULL,
  created_at           text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_business_outreach_template_approvals_template
  ON business_outreach_template_approvals (template_version_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS business_outreach_suppressions (
  suppression_ref text PRIMARY KEY,
  subject_ref     text NOT NULL,
  reason          text NOT NULL,
  source_ref      text NOT NULL,
  created_at      text NOT NULL
);

-- The suppression replay-dedupe key — ports exactly.
CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_business_outreach_suppressions_subject_reason
  ON business_outreach_suppressions (subject_ref, reason);

CREATE TABLE IF NOT EXISTS business_outreach_drafts (
  draft_ref            text PRIMARY KEY,
  pipeline_ref         text NOT NULL,
  subject_ref          text NOT NULL,
  template_version_ref text NOT NULL,
  segment_ref          text NOT NULL,
  audit_report_ref     text NOT NULL,
  finding_refs_json    text NOT NULL DEFAULT '[]',
  body_text            text NOT NULL,
  claim_lint_refs_json text NOT NULL DEFAULT '[]',
  source_ref           text NOT NULL,
  state                text NOT NULL DEFAULT 'draft',
  created_at           text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_business_outreach_drafts_pipeline
  ON business_outreach_drafts (pipeline_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS business_outreach_sends (
  send_ref             text PRIMARY KEY,
  pipeline_ref         text NOT NULL,
  draft_ref            text NOT NULL,
  subject_ref          text NOT NULL,
  template_version_ref text NOT NULL,
  mailbox_ref          text NOT NULL,
  channel              text NOT NULL,
  source_ref           text NOT NULL,
  approval_receipt_ref text NOT NULL,
  send_receipt_ref     text NOT NULL UNIQUE,
  sent_at              text NOT NULL,
  created_at           text NOT NULL
);

-- The daily mailbox send-cap count: WHERE mailbox_ref = ? AND
-- substr(sent_at, 1, 10) = ? — served by (mailbox_ref, sent_at).
CREATE INDEX IF NOT EXISTS idx_pg_business_outreach_sends_mailbox_day
  ON business_outreach_sends (mailbox_ref, sent_at);

CREATE INDEX IF NOT EXISTS idx_pg_business_outreach_sends_pipeline
  ON business_outreach_sends (pipeline_ref, created_at DESC);

-- ---------------------------------------------------------------------------
-- Exa enrichment (worker migrations 0038/0041)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS exa_enrichment_runs (
  id                    text PRIMARY KEY,
  assignment_id         text NOT NULL,
  software_order_id     text,
  site_id               text,
  plan_id               text NOT NULL,
  subject               text NOT NULL,
  status                text NOT NULL,
  request_budget        integer NOT NULL DEFAULT 0,
  request_count         integer NOT NULL DEFAULT 0,
  cache_hit_count       integer NOT NULL DEFAULT 0,
  source_count          integer NOT NULL DEFAULT 0,
  approved_source_count integer NOT NULL DEFAULT 0,
  cost_dollars          double precision,
  error_code            text,
  error_summary         text,
  started_at            text,
  completed_at          text,
  created_at            text NOT NULL,
  updated_at            text NOT NULL,
  archived_at           text
);

CREATE INDEX IF NOT EXISTS idx_pg_exa_enrichment_runs_assignment
  ON exa_enrichment_runs (assignment_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_pg_exa_enrichment_runs_status
  ON exa_enrichment_runs (status, updated_at DESC);

CREATE TABLE IF NOT EXISTS exa_enrichment_queries (
  id                      text PRIMARY KEY,
  run_id                  text NOT NULL,
  assignment_id           text NOT NULL,
  query_hash              text NOT NULL,
  query_text              text NOT NULL,
  source_category         text NOT NULL,
  search_type             text NOT NULL,
  freshness_max_age_hours integer NOT NULL,
  status                  text NOT NULL,
  result_count            integer NOT NULL DEFAULT 0,
  latency_ms              integer,
  cost_dollars            double precision,
  error_code              text,
  error_summary           text,
  created_at              text NOT NULL,
  updated_at              text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_exa_enrichment_queries_run
  ON exa_enrichment_queries (run_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_pg_exa_enrichment_queries_hash
  ON exa_enrichment_queries (query_hash, created_at DESC);

CREATE TABLE IF NOT EXISTS exa_enrichment_sources (
  id                 text PRIMARY KEY,
  run_id             text NOT NULL,
  query_id           text,
  assignment_id      text NOT NULL,
  software_order_id  text,
  site_id            text,
  source_category    text NOT NULL,
  review_status      text NOT NULL,
  title              text NOT NULL,
  url                text NOT NULL,
  domain             text NOT NULL,
  published_date     text,
  highlight_text     text,
  selected_text_hash text,
  exa_request_id     text,
  search_type        text,
  public_safe        smallint NOT NULL DEFAULT 0 CHECK (public_safe IN (0, 1)),
  rejected_reason    text,
  approved_at        text,
  rejected_at        text,
  created_at         text NOT NULL,
  updated_at         text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_exa_enrichment_sources_run
  ON exa_enrichment_sources (run_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_pg_exa_enrichment_sources_assignment
  ON exa_enrichment_sources (assignment_id, public_safe, review_status, created_at DESC);

CREATE TABLE IF NOT EXISTS exa_enrichment_budget_events (
  id            text PRIMARY KEY,
  assignment_id text NOT NULL,
  run_id        text,
  day_key       text NOT NULL,
  request_units integer NOT NULL,
  reason        text NOT NULL,
  created_at    text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_exa_enrichment_budget_assignment_day
  ON exa_enrichment_budget_events (assignment_id, day_key, created_at DESC);

CREATE TABLE IF NOT EXISTS exa_enrichment_cache_entries (
  id                      text PRIMARY KEY,
  cache_key               text NOT NULL,
  source_category         text NOT NULL,
  search_type             text NOT NULL,
  freshness_max_age_hours integer NOT NULL,
  results_json            text NOT NULL,
  result_count            integer NOT NULL DEFAULT 0,
  cost_dollars            double precision,
  created_at              text NOT NULL,
  expires_at              text NOT NULL,
  archived_at             text
);

-- D1's unique (cache_key) WHERE archived_at IS NULL is deliberately a plain
-- index here (partial-unique rule above; cache rows are archived-then-
-- reinserted so the live twin can transiently hold both during convergence).
CREATE INDEX IF NOT EXISTS idx_pg_exa_enrichment_cache_fresh
  ON exa_enrichment_cache_entries (cache_key, expires_at DESC);

CREATE TABLE IF NOT EXISTS exa_enrichment_metric_events (
  id                text PRIMARY KEY,
  assignment_id     text NOT NULL,
  run_id            text,
  query_id          text,
  event_name        text NOT NULL,
  status            text NOT NULL,
  error_code        text,
  search_type       text,
  source_category   text,
  result_count      integer,
  source_card_count integer,
  latency_ms        integer,
  cost_dollars      double precision,
  cache_status      text,
  created_at        text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_exa_enrichment_metric_assignment
  ON exa_enrichment_metric_events (assignment_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_pg_exa_enrichment_metric_event
  ON exa_enrichment_metric_events (event_name, created_at DESC);
