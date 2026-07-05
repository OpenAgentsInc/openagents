-- KS-8.14 (#8325): business funnel / orders / referrals domain — Postgres
-- twins of the 32 live D1 tables (worker migrations 0030/0043/0051/0060/
-- 0061/0062/0067/0068/0069/0070/0104/0148/0152/0191/0201/0216/0270/0271/
-- 0272/0274/0275/0276/0277/0278/0292/0294/0295/0297/0298/0299).
-- Plan: docs/khala-sync/MIGRATION_PLAN.md §3.11 (Wave D) + §1 universal
-- porting rules; templates: 0015_billing_pay_ins.sql (KS-8.7, the money
-- discipline) and 0014_forum_content.sql (KS-8.10, the mirroring-database
-- seam this lane's worker wiring uses).
--
-- DOMAIN DISCIPLINE (the §3.11 risk list):
--   * D1 stays the SOLE write authority. These tables are a best-effort
--     dual-write mirror + backfill target only; reads default to D1 and
--     any read cutover is an epic-gated ops decision (#8282).
--   * REFERRAL ATTRIBUTION UNIQUENESS KEYS FEED PAYOUTS (KS-8.8) AND PORT
--     EXACTLY: the consume-once PRIMARY KEYs
--     (`user_referral_attributions.user_id`,
--     `order_referral_attributions.software_order_id`,
--     `agent_referral_attributions.agent_user_id`,
--     `business_signup_referral_attributions.business_signup_request_id`),
--     the one-claim UNIQUEs
--     (`user_referral_attributions.referral_attribution_id`,
--     `business_affiliate_attributions.business_signup_request_id`), and
--     every idempotency UNIQUE
--     (`referral_workflow_events.idempotency_key`,
--     `qa_swarm_first_engagements.idempotency_key` / `commitment_ref`,
--     `buy_mode_campaigns.idempotency_key_hash`,
--     `buy_mode_jobs.idempotency_key_hash` / `request_event_id` /
--     `result_event_id`, the funnel `event_ref`s, the fulfillment
--     `receipt_ref`/`page_ref`s) are the same key set as D1, byte-exact
--     (TEXT -> text). The write-side dedupe decision (INSERT OR IGNORE /
--     the D1 window-cap trigger) is made ONCE, on D1, and never
--     re-evaluated against Postgres.
--   * PROMISE TRANSITION RECEIPTS back the PUBLIC product-promises
--     registry (continuously servable): the acceptance for that table is
--     full row-hash SET equality, not just counts.
--   * Amounts keep D1's exact integer representations: cents and msats
--     are bigint. `referral_workflow_events.amount` is D1 NUMERIC and
--     ports as numeric (its writers record integral credits/sats/usd
--     units; the verify sums it exactly per (asset, event_kind,
--     policy_state)).
--   * The mirror/backfill write mode is a CONVERGE upsert to the
--     authoritative D1 row bytes (`ON CONFLICT (pk) DO UPDATE`). Converge
--     (not DO NOTHING) is required because signup fulfillment status,
--     pipeline stages, promise/fulfillment loop state, buy-mode campaign
--     spend counters, triage records, and attribution policy_state are
--     legitimately UPDATEd in place on D1.
--   * FULFILLMENT ESCALATION MUST NOT DOUBLE-PAGE: the escalation pager
--     runs against exactly ONE store (D1; the `(promise_id,
--     escalation_date)` UNIQUE is its dedupe). Nothing in Postgres
--     triggers or feeds an evaluator in this lane — the
--     `business_starter_credit_grants` window-cap TRIGGER is
--     deliberately NOT ported (it is a write-authority policy gate, and
--     Postgres only ever receives rows D1 already accepted).
--
-- TYPE FIDELITY (v1, reconciliation-bearing): TEXT ISO-8601 timestamps
-- stay text (sort correctly, hash byte-exact), 0/1 booleans stay smallint
-- with CHECK, JSON stays text (row-hash reconciliation compares exact
-- bytes). Tightening to native types is a post-retirement cleanup, never
-- mid-migration.
--
-- REWRITE ARTIFACTS: `business_funnel_events_0275` (worker 0277) and
-- `business_service_promises_0275` (worker 0275) were rename-swap scratch
-- tables; both migrations renamed them back to the primary names, so no
-- `_0275` table exists live and nothing is created for them here. The
-- decommission follow-up verifies their absence.
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL POSTGRES READS (the KS-8.2 rule).
-- This lane routes ZERO reads to Postgres (KHALA_SYNC_BUSINESS_READS
-- ships d1-default with compare-mode shadow reads only; postgres serving
-- is deferred to the read-cutover follow-up because the domain read
-- surface — funnel dashboards, pipeline queues, referral feeds, order
-- lists — is wide, not one bounded scan). KEPT beyond PKs/uniques:
-- nothing. DROPPED D1 read accelerators (the decommission follow-up
-- moves the reads WITH their re-derived indexes):
-- business_funnel_events_stage_time_idx / _source_time_idx /
-- _source_ref_stage_idx, business_signup_requests_* (4),
-- idx_business_signup_referral_attributions_* (2),
-- business_signup_fulfillments_* (2), idx_business_service_promises_due /
-- _blocked, idx_business_fulfillment_motion_receipts_promise / _cadence,
-- idx_business_fulfillment_escalation_pages_promise,
-- business_checkout_kickoffs_* (2), idx_business_commitment_ledger_* (3),
-- idx_business_pipeline_rows_* (5), idx_business_starter_credit_grants_*
-- (3), idx_business_affiliate_codes_owner,
-- idx_business_affiliate_attributions_* (3), software_orders_user_active_idx
-- / _status_idx, order_triage_records_priority_idx / _classification_idx,
-- order_triage_events_* (3), order_fulfillment_artifacts_* (2),
-- order_fulfillment_feedback_* (3),
-- order_github_write_authority_receipts_* (3),
-- idx_order_referral_attributions_user,
-- idx_user_referral_attributions_source,
-- idx_agent_referral_attributions_owner, idx_referral_attributions_* (3),
-- idx_referral_invites_* (2), idx_referral_workflow_events_* (5),
-- viral_agent_funnel_events_* (3), idx_qa_swarm_first_engagements_* (2),
-- idx_promise_transition_receipts_* (2), buy_mode_campaigns_updated_idx,
-- buy_mode_jobs_campaign_updated_idx / _result_event_idx,
-- buy_mode_alerts_campaign_created_idx,
-- idx_customer_one_cohort_rows_updated_at.
-- PARTIAL UNIQUE indexes that are CONSTRAINTS (not read accelerators)
-- are ported verbatim: software_orders_agent_idempotency_idx and
-- order_triage_records_active_order_idx.
--
-- NO FOREIGN KEYS (same discipline as KS-8.1/8.2/8.7): dual-write mirrors
-- and the backfill land per-row and per-table; referential integrity is
-- verified by reconciliation, not enforced mid-migration (many D1 FKs in
-- this domain reference tables outside it — users, teams, site_projects,
-- stripe_checkout_sessions, omni_accepted_outcome_contracts, …).

-- ---------------------------------------------------------------------------
-- business signup intake + fulfillment (worker 0191/0216/0271/0297)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS business_signup_requests (
  id                      text NOT NULL PRIMARY KEY,
  business_name           text NOT NULL,
  contact_email           text NOT NULL,
  website                 text,
  phone                   text NOT NULL,
  help_with               text,
  request_slack_channel   smallint NOT NULL DEFAULT 0
    CHECK (request_slack_channel IN (0, 1)),
  slack_connect_status    text NOT NULL CHECK (
    slack_connect_status IN (
      'not_requested', 'manual_invite_pending', 'invite_sent', 'accepted',
      'declined'
    )
  ),
  source_route            text NOT NULL DEFAULT '/business',
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  referral_code           text,
  referral_attribution_id text,
  fulfillment_status      text NOT NULL DEFAULT 'pending'
    CHECK (fulfillment_status IN ('pending', 'invited', 'operator_parked')),
  fulfillment_ref         text,
  fulfillment_reason      text,
  source_ref              text NOT NULL DEFAULT 'direct',
  linked_pipeline_ref     text
);

CREATE TABLE IF NOT EXISTS business_signup_fulfillments (
  id                         text NOT NULL PRIMARY KEY,
  business_signup_request_id text NOT NULL UNIQUE,
  status                     text NOT NULL
    CHECK (status IN ('invited', 'operator_parked')),
  reason                     text,
  enrichment_ref             text NOT NULL,
  team_id                    text,
  project_id                 text,
  workspace_id               text,
  invite_id                  text,
  email_message_id           text,
  email_delivery_status      text NOT NULL CHECK (
    email_delivery_status IN (
      'accepted', 'disabled', 'failed', 'missing_config', 'not_attempted'
    )
  ),
  metadata_json              text NOT NULL DEFAULT '{}',
  created_at                 text NOT NULL,
  updated_at                 text NOT NULL
);

-- Consume-once: one referral credit per converted business signup (PK).
CREATE TABLE IF NOT EXISTS business_signup_referral_attributions (
  business_signup_request_id text NOT NULL PRIMARY KEY,
  referral_attribution_id    text NOT NULL,
  referral_source_id         text NOT NULL,
  referral_invite_id         text,
  capture_path               text NOT NULL
    CHECK (capture_path IN ('human', 'agent')),
  target                     text NOT NULL
    CHECK (target IN ('home', 'order', 'agent_claim')),
  linked_at                  text NOT NULL,
  policy_state               text NOT NULL DEFAULT 'active'
    CHECK (policy_state IN ('active', 'disputed', 'archived')),
  created_at                 text NOT NULL,
  updated_at                 text NOT NULL,
  archived_at                text
);

-- ---------------------------------------------------------------------------
-- business funnel receipts (worker 0270 -> 0277 rewrite; final 0277 shape)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS business_funnel_events (
  id          text NOT NULL PRIMARY KEY,
  event_ref   text NOT NULL UNIQUE,
  stage       text NOT NULL CHECK (
    stage IN (
      'visit', 'signup', 'intake_spec', 'payment', 'provisioned',
      'first_outcome', 'retained', 'referred_engagement'
    )
  ),
  source_kind text NOT NULL CHECK (
    source_kind IN (
      'content', 'outbound', 'ai_search', 'referral', 'direct', 'unknown'
    )
  ),
  source_ref  text,
  occurred_at text NOT NULL,
  observed_at text NOT NULL
);

-- ---------------------------------------------------------------------------
-- business fulfillment loop (worker 0274/0275/0276)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS business_service_promises (
  id                           text NOT NULL PRIMARY KEY,
  promise_ref                  text NOT NULL UNIQUE,
  accepted_outcome_contract_id text,
  workspace_ref                text NOT NULL,
  crm_state_ref                text NOT NULL,
  stakeholder_refs_json        text NOT NULL DEFAULT '[]',
  state                        text NOT NULL
    CHECK (state IN ('active', 'paused', 'blocked', 'closed')),
  cadence                      text NOT NULL
    CHECK (cadence IN ('daily', 'weekly')),
  next_motion_due_at           text,
  last_motion_receipt_ref      text,
  source_refs_json             text NOT NULL DEFAULT '[]',
  metadata_json                text NOT NULL DEFAULT '{}',
  created_at                   text NOT NULL,
  updated_at                   text NOT NULL,
  blocking_reason_ref          text,
  blocked_at                   text,
  last_escalation_page_ref     text
);

CREATE TABLE IF NOT EXISTS business_fulfillment_motion_receipts (
  id                                   text NOT NULL PRIMARY KEY,
  promise_id                           text NOT NULL,
  promise_ref                          text NOT NULL,
  motion_date                          text NOT NULL,
  receipt_ref                          text NOT NULL UNIQUE,
  agent_definition_ref                 text NOT NULL,
  crm_state_ref                        text NOT NULL,
  stakeholder_refs_json                text NOT NULL DEFAULT '[]',
  stakeholder_flag_refs_json           text NOT NULL DEFAULT '[]',
  forward_motion_ref                   text NOT NULL,
  client_comms_draft_ref               text NOT NULL,
  approval_gate_ref                    text NOT NULL,
  outbound_allowed                     smallint NOT NULL DEFAULT 0
    CHECK (outbound_allowed IN (0, 1)),
  blocker_refs_json                    text NOT NULL DEFAULT '[]',
  source_refs_json                     text NOT NULL DEFAULT '[]',
  created_at                           text NOT NULL,
  cadence                              text NOT NULL DEFAULT 'daily'
    CHECK (cadence IN ('daily', 'weekly')),
  client_comms_email_ledger_ref        text,
  customer_visible_workroom_update_ref text,
  UNIQUE (promise_id, motion_date)
);

-- The (promise_id, escalation_date) UNIQUE is the pager's dedupe key —
-- ported exactly, but the pager itself only ever evaluates against D1.
CREATE TABLE IF NOT EXISTS business_fulfillment_escalation_pages (
  id                     text NOT NULL PRIMARY KEY,
  promise_id             text NOT NULL,
  promise_ref            text NOT NULL,
  escalation_date        text NOT NULL,
  receipt_ref            text NOT NULL UNIQUE,
  page_ref               text NOT NULL UNIQUE,
  owner_notification_ref text NOT NULL,
  agent_definition_ref   text NOT NULL,
  blocking_reason_ref    text NOT NULL,
  blocked_at             text NOT NULL,
  workspace_ref          text NOT NULL,
  stakeholder_refs_json  text NOT NULL DEFAULT '[]',
  source_refs_json       text NOT NULL DEFAULT '[]',
  created_at             text NOT NULL,
  UNIQUE (promise_id, escalation_date)
);

-- ---------------------------------------------------------------------------
-- business checkout kickoffs + commitment ledger (worker 0272/0278/0294)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS business_checkout_kickoffs (
  checkout_session_id         text NOT NULL PRIMARY KEY,
  business_signup_request_id  text NOT NULL,
  user_id                     text NOT NULL,
  total_amount_cents          bigint NOT NULL CHECK (total_amount_cents >= 0),
  setup_fee_cents             bigint NOT NULL CHECK (setup_fee_cents >= 0),
  credit_grant_cents          bigint NOT NULL CHECK (credit_grant_cents >= 0),
  workspace_id                text NOT NULL,
  service_promise_contract_id text NOT NULL,
  public_receipt_ref          text NOT NULL,
  created_at                  text NOT NULL,
  updated_at                  text NOT NULL,
  CHECK (setup_fee_cents + credit_grant_cents = total_amount_cents)
);

CREATE TABLE IF NOT EXISTS business_commitment_ledger (
  id                   text NOT NULL PRIMARY KEY,
  commitment_ref       text NOT NULL UNIQUE,
  engagement_ref       text NOT NULL,
  owner_ref            text NOT NULL,
  vertical_ref         text NOT NULL,
  promised_object_ref  text NOT NULL,
  commitment_kind      text NOT NULL
    CHECK (commitment_kind IN ('deliverable', 'send')),
  due_state            text NOT NULL
    CHECK (due_state IN ('due', 'blocked', 'shipped', 'parked')),
  due_at               text NOT NULL,
  shipped_at           text,
  weekly_review_ref    text NOT NULL,
  source_refs_json     text NOT NULL DEFAULT '[]',
  blocker_refs_json    text NOT NULL DEFAULT '[]',
  evidence_refs_json   text NOT NULL DEFAULT '[]',
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  pipeline_ref         text
);

-- ---------------------------------------------------------------------------
-- business pipeline + starter credits + affiliates (worker 0294-0299)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS business_pipeline_rows (
  pipeline_ref                 text NOT NULL PRIMARY KEY,
  vertical                     text NOT NULL,
  source_ref                   text NOT NULL,
  stage                        text NOT NULL CHECK (
    stage IN (
      'intake_received', 'scope_scheduled', 'scope_completed',
      'receipt_plan_sent', 'closed_won', 'closed_lost', 'quick_win_started'
    )
  ),
  quoted_min_usd_cents         bigint NOT NULL DEFAULT 0
    CHECK (quoted_min_usd_cents >= 0),
  quoted_max_usd_cents         bigint NOT NULL DEFAULT 0
    CHECK (quoted_max_usd_cents >= quoted_min_usd_cents),
  quoted_band_label            text NOT NULL DEFAULT 'unquoted',
  owner_role                   text NOT NULL CHECK (
    owner_role IN ('operator', 'reviewer', 'fulfillment_agent', 'owner')
  ),
  next_action_due_at           text,
  blocker_ref                  text,
  receipt_refs_json            text NOT NULL DEFAULT '[]',
  partner_route_flag           smallint NOT NULL DEFAULT 0
    CHECK (partner_route_flag IN (0, 1)),
  created_at                   text NOT NULL,
  updated_at                   text NOT NULL,
  stage_updated_at             text NOT NULL,
  business_signup_request_id   text,
  partner_route_state          text NOT NULL DEFAULT 'none' CHECK (
    partner_route_state IN (
      'none', 'candidate', 'offered', 'accepted', 'declined'
    )
  ),
  partner_peer_ref             text,
  partner_approval_receipt_ref text,
  partner_offer_ref            text,
  partner_scope_summary_ref    text,
  partner_due_window_ref       text,
  partner_budget_range_ref     text,
  partner_privacy_tier_ref     text,
  partner_route_updated_at     text
);

-- D1's business_starter_credit_window_cap TRIGGER is deliberately NOT
-- ported: it is a write-authority policy gate and D1 makes that decision.
CREATE TABLE IF NOT EXISTS business_starter_credit_grants (
  grant_ref                    text NOT NULL PRIMARY KEY,
  pipeline_ref                 text NOT NULL,
  account_ref                  text NOT NULL,
  engagement_ref               text NOT NULL,
  attribution_kind             text NOT NULL DEFAULT 'sales_starter_credit'
    CHECK (attribution_kind = 'sales_starter_credit'),
  transfer_policy              text NOT NULL DEFAULT 'non_transferable'
    CHECK (transfer_policy = 'non_transferable'),
  amount_usd_cents             bigint NOT NULL CHECK (amount_usd_cents > 0),
  amount_msat                  bigint NOT NULL CHECK (amount_msat > 0),
  amount_cap_usd_cents         bigint NOT NULL
    CHECK (amount_cap_usd_cents > 0),
  window_ref                   text NOT NULL,
  window_grant_cap             bigint NOT NULL CHECK (window_grant_cap > 0),
  credit_receipt_ref           text NOT NULL UNIQUE,
  redemption_receipt_refs_json text NOT NULL DEFAULT '[]',
  source_refs_json             text NOT NULL DEFAULT '[]',
  created_at                   text NOT NULL,
  updated_at                   text NOT NULL,
  CHECK (amount_usd_cents <= amount_cap_usd_cents)
);

CREATE TABLE IF NOT EXISTS business_affiliate_codes (
  code         text NOT NULL PRIMARY KEY,
  source_ref   text NOT NULL UNIQUE,
  owner_ref    text NOT NULL,
  issued_by_ref text NOT NULL,
  policy_state text NOT NULL DEFAULT 'active'
    CHECK (policy_state IN ('active', 'paused', 'archived')),
  created_at   text NOT NULL,
  updated_at   text NOT NULL,
  archived_at  text
);

-- One affiliate credit per business signup (UNIQUE business_signup_request_id).
CREATE TABLE IF NOT EXISTS business_affiliate_attributions (
  attribution_ref            text NOT NULL PRIMARY KEY,
  code                       text NOT NULL,
  source_ref                 text NOT NULL,
  owner_ref                  text NOT NULL,
  business_signup_request_id text NOT NULL UNIQUE,
  pipeline_ref               text,
  payment_receipt_ref        text,
  policy_state               text NOT NULL DEFAULT 'active'
    CHECK (policy_state IN ('active', 'archived')),
  created_at                 text NOT NULL,
  updated_at                 text NOT NULL,
  archived_at                text
);

-- ---------------------------------------------------------------------------
-- software orders + triage + fulfillment artifacts (worker 0030/0043/0051/
-- 0060/0061/0104)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS software_orders (
  id                              text NOT NULL PRIMARY KEY,
  user_id                         text NOT NULL,
  status                          text NOT NULL DEFAULT 'submitted' CHECK (
    status IN (
      'submitted', 'scoping', 'free_slice_ready', 'quote_ready',
      'agent_queued', 'agent_running', 'delivered', 'needs_customer_input',
      'declined', 'unavailable'
    )
  ),
  visibility                      text NOT NULL DEFAULT 'public'
    CHECK (visibility IN ('public')),
  request                         text NOT NULL,
  repository_provider             text CHECK (
    repository_provider IS NULL OR repository_provider IN ('github')
  ),
  repository_owner                text,
  repository_name                 text,
  repository_full_name            text,
  repository_private              smallint CHECK (
    repository_private IS NULL OR repository_private IN (0, 1)
  ),
  repository_default_branch       text,
  repository_html_url             text,
  public_work_acknowledged_at     text NOT NULL,
  data_use_acknowledged_at        text NOT NULL,
  compute_payment_acknowledged_at text NOT NULL,
  provider_account_required       smallint NOT NULL DEFAULT 0
    CHECK (provider_account_required IN (0, 1)),
  free_slice_cents                bigint NOT NULL DEFAULT 5000,
  quote_cents                     bigint,
  current_run_id                  text,
  agent_started_at                text,
  created_at                      text NOT NULL,
  updated_at                      text NOT NULL,
  archived_at                     text,
  agent_idempotency_key           text
);

-- CONSTRAINT (not a read accelerator): the customer agent-launch
-- idempotency window, ported verbatim from worker 0104.
CREATE UNIQUE INDEX IF NOT EXISTS software_orders_agent_idempotency_idx
  ON software_orders (user_id, agent_idempotency_key)
  WHERE agent_idempotency_key IS NOT NULL AND archived_at IS NULL;

CREATE TABLE IF NOT EXISTS order_triage_records (
  id                   text NOT NULL PRIMARY KEY,
  software_order_id    text NOT NULL,
  classification       text NOT NULL CHECK (
    classification IN (
      'runnable_site', 'runnable_general_autopilot', 'needs_clarification',
      'smoke_or_test', 'legal_sensitive_policy_review',
      'unavailable_or_declined'
    )
  ),
  operator_priority    bigint NOT NULL DEFAULT 100,
  first_batch_eligible smallint NOT NULL DEFAULT 0
    CHECK (first_batch_eligible IN (0, 1)),
  hold_reason          text,
  next_action          text NOT NULL,
  customer_safe_status text NOT NULL,
  customer_safe_summary text NOT NULL,
  reviewer_user_id     text,
  reviewed_at          text,
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  archived_at          text
);

-- CONSTRAINT (not a read accelerator): one ACTIVE triage record per
-- order, ported verbatim from worker 0043.
CREATE UNIQUE INDEX IF NOT EXISTS order_triage_records_active_order_idx
  ON order_triage_records (software_order_id)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS order_triage_events (
  id                text NOT NULL PRIMARY KEY,
  triage_record_id  text NOT NULL,
  software_order_id text NOT NULL,
  site_id           text,
  assignment_id     text,
  event_type        text NOT NULL,
  visibility        text NOT NULL
    CHECK (visibility IN ('private', 'team', 'public')),
  summary           text NOT NULL,
  actor_user_id     text,
  payload_json      text,
  created_at        text NOT NULL
);

CREATE TABLE IF NOT EXISTS order_fulfillment_artifacts (
  id                   text NOT NULL PRIMARY KEY,
  software_order_id    text NOT NULL,
  assignment_id        text,
  run_id               text,
  kind                 text NOT NULL CHECK (
    kind IN (
      'pull_request', 'branch', 'commit', 'diff', 'preview', 'notes',
      'attachment'
    )
  ),
  title                text NOT NULL,
  summary              text NOT NULL,
  url                  text,
  repository_full_name text,
  source_branch        text,
  target_branch        text,
  commit_sha           text,
  status               text NOT NULL CHECK (
    status IN (
      'draft', 'customer_review_ready', 'customer_accepted', 'superseded',
      'rejected'
    )
  ),
  visibility           text NOT NULL
    CHECK (visibility IN ('private', 'team', 'public')),
  metadata_json        text NOT NULL DEFAULT '{}',
  created_by_user_id   text,
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  archived_at          text
);

CREATE TABLE IF NOT EXISTS order_fulfillment_feedback (
  id                     text NOT NULL PRIMARY KEY,
  software_order_id      text NOT NULL,
  artifact_id            text,
  author_user_id         text NOT NULL,
  body                   text NOT NULL,
  status                 text NOT NULL CHECK (
    status IN (
      'submitted', 'queued', 'running', 'addressed', 'closed', 'rejected'
    )
  ),
  source                 text NOT NULL
    CHECK (source IN ('customer_order_ui', 'operator', 'agent')),
  visibility             text NOT NULL
    CHECK (visibility IN ('private', 'team', 'public')),
  adjutant_assignment_id text,
  adjutant_adjustment_id text,
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  archived_at            text
);

CREATE TABLE IF NOT EXISTS order_github_write_authority_receipts (
  id                   text NOT NULL PRIMARY KEY,
  software_order_id    text NOT NULL,
  assignment_id        text,
  user_id              text NOT NULL,
  repository_full_name text NOT NULL,
  repository_private   smallint NOT NULL CHECK (repository_private IN (0, 1)),
  requested_operation  text NOT NULL CHECK (
    requested_operation IN (
      'create_branch', 'push_commit', 'open_pull_request',
      'open_fork_pull_request'
    )
  ),
  decision             text NOT NULL CHECK (decision IN ('allowed', 'blocked')),
  authority_mode       text CHECK (
    authority_mode IS NULL OR authority_mode IN (
      'customer_grant', 'openagents_fork', 'openagents_app'
    )
  ),
  blocked_reason       text,
  connection_ref       text,
  grant_ref            text,
  approval_source      text CHECK (
    approval_source IS NULL OR approval_source IN (
      'customer_action', 'operator_action', 'system_policy'
    )
  ),
  approved_at          text,
  customer_message     text NOT NULL,
  metadata_json        text NOT NULL DEFAULT '{}',
  created_at           text NOT NULL,
  updated_at           text NOT NULL
);

-- ---------------------------------------------------------------------------
-- referral spine consumption + workflow events (worker 0067/0068/0069/0070)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS referral_invites (
  id                 text NOT NULL PRIMARY KEY,
  referral_source_id text NOT NULL,
  public_invite_ref  text NOT NULL UNIQUE,
  token_hash         text NOT NULL,
  scope              text NOT NULL
    CHECK (scope IN ('site_join', 'order_start', 'agent_claim')),
  audience_path      text NOT NULL CHECK (audience_path IN ('human', 'agent')),
  policy_state       text NOT NULL DEFAULT 'active' CHECK (
    policy_state IN ('active', 'redeemed', 'expired', 'disabled', 'disputed')
  ),
  expires_at         text,
  created_at         text NOT NULL,
  updated_at         text NOT NULL,
  archived_at        text
);

CREATE TABLE IF NOT EXISTS referral_attributions (
  id                 text NOT NULL PRIMARY KEY,
  referral_source_id text NOT NULL,
  referral_invite_id text,
  public_source_ref  text NOT NULL,
  public_invite_ref  text,
  capture_path       text NOT NULL CHECK (capture_path IN ('human', 'agent')),
  target             text NOT NULL
    CHECK (target IN ('home', 'order', 'agent_claim')),
  policy_state       text NOT NULL DEFAULT 'pending' CHECK (
    policy_state IN (
      'pending', 'claimed', 'expired', 'disabled', 'disputed', 'archived'
    )
  ),
  first_verified_at  text,
  claimed_user_id    text,
  expires_at         text NOT NULL,
  created_at         text NOT NULL,
  updated_at         text NOT NULL,
  archived_at        text
);

-- Consume-once: one referral credit per user (PK) and one claim per
-- pending attribution (UNIQUE referral_attribution_id).
CREATE TABLE IF NOT EXISTS user_referral_attributions (
  user_id                 text NOT NULL PRIMARY KEY,
  referral_attribution_id text NOT NULL UNIQUE,
  referral_source_id      text NOT NULL,
  referral_invite_id      text,
  capture_path            text NOT NULL
    CHECK (capture_path IN ('human', 'agent')),
  target                  text NOT NULL
    CHECK (target IN ('home', 'order', 'agent_claim')),
  first_verified_at       text NOT NULL,
  policy_state            text NOT NULL DEFAULT 'active'
    CHECK (policy_state IN ('active', 'disputed', 'archived')),
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  archived_at             text
);

-- Consume-once: one referral credit per converted order (PK).
CREATE TABLE IF NOT EXISTS order_referral_attributions (
  software_order_id       text NOT NULL PRIMARY KEY,
  user_id                 text NOT NULL,
  referral_attribution_id text NOT NULL,
  referral_source_id      text NOT NULL,
  referral_invite_id      text,
  capture_path            text NOT NULL
    CHECK (capture_path IN ('human', 'agent')),
  target                  text NOT NULL
    CHECK (target IN ('home', 'order', 'agent_claim')),
  linked_at               text NOT NULL,
  policy_state            text NOT NULL DEFAULT 'active'
    CHECK (policy_state IN ('active', 'disputed', 'archived')),
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  archived_at             text
);

-- Consume-once: one referral credit per claimed agent (PK).
CREATE TABLE IF NOT EXISTS agent_referral_attributions (
  agent_user_id           text NOT NULL PRIMARY KEY,
  owner_user_id           text,
  referral_attribution_id text NOT NULL,
  referral_source_id      text NOT NULL,
  referral_invite_id      text,
  capture_path            text NOT NULL
    CHECK (capture_path IN ('human', 'agent')),
  target                  text NOT NULL
    CHECK (target IN ('home', 'order', 'agent_claim')),
  claimed_at              text NOT NULL,
  policy_state            text NOT NULL DEFAULT 'active'
    CHECK (policy_state IN ('active', 'disputed', 'archived')),
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  archived_at             text
);

CREATE TABLE IF NOT EXISTS referral_workflow_events (
  id                      text NOT NULL PRIMARY KEY,
  idempotency_key         text NOT NULL UNIQUE,
  event_kind              text NOT NULL CHECK (
    event_kind IN (
      'paid_usage', 'site_checkout', 'l402_redemption', 'accepted_outcome',
      'refund', 'reversal', 'eligibility_hold', 'dispute_hold',
      'operator_adjustment'
    )
  ),
  referral_attribution_id text NOT NULL,
  referral_source_id      text NOT NULL,
  referral_invite_id      text,
  public_source_ref       text NOT NULL,
  public_invite_ref       text,
  software_order_id       text,
  site_id                 text,
  site_version_id         text,
  product_id              text,
  paid_action_id          text,
  payment_event_id        text,
  payment_evidence_ref    text,
  entitlement_ref         text,
  accepted_work_ref       text,
  related_event_id        text,
  public_receipt_ref      text NOT NULL,
  policy_state            text NOT NULL CHECK (
    policy_state IN (
      'recorded', 'eligible', 'held', 'disputed', 'refunded', 'reversed',
      'ignored'
    )
  ),
  amount                  numeric NOT NULL DEFAULT 0 CHECK (amount >= 0),
  asset                   text NOT NULL
    CHECK (asset IN ('none', 'credits', 'sats', 'usd')),
  metadata_json           text NOT NULL DEFAULT '{}',
  occurred_at             text NOT NULL,
  created_at              text NOT NULL,
  archived_at             text,
  CHECK (
    event_kind NOT IN ('refund', 'reversal') OR related_event_id IS NOT NULL
  )
);

-- ---------------------------------------------------------------------------
-- viral funnel, QA swarm, promise receipts (worker 0062/0148/0292)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS viral_agent_funnel_events (
  id               text NOT NULL PRIMARY KEY,
  event_kind       text NOT NULL CHECK (
    event_kind IN (
      'capability_manifest_read', 'openapi_read', 'agent_doc_read',
      'skill_doc_read', 'public_proof_read', 'public_challenge_read',
      'first_scoped_action_attempt'
    )
  ),
  route            text NOT NULL,
  actor_class      text NOT NULL CHECK (
    actor_class IN (
      'public_anonymous', 'signed_in_browser_possible',
      'scoped_agent_possible'
    )
  ),
  user_agent_class text NOT NULL
    CHECK (user_agent_class IN ('agent_or_cli', 'browser', 'crawler', 'unknown')),
  site_slug        text,
  proof_ref        text,
  metadata_json    text NOT NULL DEFAULT '{}',
  created_at       text NOT NULL
);

CREATE TABLE IF NOT EXISTS qa_swarm_first_engagements (
  receipt_ref                     text NOT NULL PRIMARY KEY,
  idempotency_key                 text NOT NULL UNIQUE,
  package_kind                    text NOT NULL
    CHECK (package_kind IN ('swarm_audit')),
  payment_path                    text NOT NULL CHECK (
    payment_path IN (
      'operator_sales_deposit_invoice', 'checkout_kickoff_receipt'
    )
  ),
  business_signup_request_id      text NOT NULL,
  user_id                         text NOT NULL,
  committed_amount_cents          bigint NOT NULL CHECK (
    committed_amount_cents >= 100000 AND committed_amount_cents <= 500000
  ),
  intake_receipt_ref              text NOT NULL,
  checkout_or_deposit_receipt_ref text NOT NULL,
  target_adapter_review_ref       text NOT NULL,
  package_contract_ref            text NOT NULL,
  workspace_id                    text NOT NULL,
  service_promise_contract_id     text NOT NULL,
  commitment_ref                  text NOT NULL UNIQUE,
  first_report_due_at             text NOT NULL,
  recorded_at                     text NOT NULL,
  created_at                      text NOT NULL,
  updated_at                      text NOT NULL
);

-- Public product-promises registry receipts (continuously servable; the
-- acceptance is full row-hash SET equality).
CREATE TABLE IF NOT EXISTS promise_transition_receipts (
  id                 text NOT NULL PRIMARY KEY,
  promise_id         text NOT NULL,
  from_state         text NOT NULL,
  to_state           text NOT NULL,
  registry_version   text NOT NULL,
  result             text NOT NULL,
  checks_json        text NOT NULL,
  evidence_refs_json text NOT NULL,
  exception_json     text,
  checked_at         text NOT NULL,
  created_at         text NOT NULL
);

-- ---------------------------------------------------------------------------
-- buy-mode dispatcher (worker 0152)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS buy_mode_campaigns (
  campaign_id          text NOT NULL PRIMARY KEY,
  idempotency_key_hash text NOT NULL UNIQUE,
  state                text NOT NULL
    CHECK (state IN ('disabled', 'enabled', 'halted')),
  spend_enabled        smallint NOT NULL CHECK (spend_enabled IN (0, 1)),
  per_job_cap_msats    bigint NOT NULL CHECK (per_job_cap_msats > 0),
  daily_cap_msats      bigint NOT NULL CHECK (daily_cap_msats > 0),
  spent_today_msats    bigint NOT NULL DEFAULT 0
    CHECK (spent_today_msats >= 0),
  day_key              text NOT NULL,
  operator_user_id     text NOT NULL,
  relay_url            text NOT NULL,
  last_alert_ref       text,
  created_at           text NOT NULL,
  updated_at           text NOT NULL
);

CREATE TABLE IF NOT EXISTS buy_mode_jobs (
  job_id               text NOT NULL PRIMARY KEY,
  campaign_id          text NOT NULL,
  idempotency_key_hash text NOT NULL UNIQUE,
  request_event_id     text NOT NULL UNIQUE,
  result_event_id      text UNIQUE,
  provider_pubkey      text,
  amount_msats         bigint NOT NULL CHECK (amount_msats > 0),
  state                text NOT NULL CHECK (
    state IN ('issued', 'settled', 'settlement_blocked', 'settlement_failed')
  ),
  receipt_ref          text,
  bolt11_ref           text,
  content_digest_ref   text,
  created_at           text NOT NULL,
  updated_at           text NOT NULL
);

CREATE TABLE IF NOT EXISTS buy_mode_alerts (
  alert_id    text NOT NULL PRIMARY KEY,
  campaign_id text NOT NULL,
  reason_ref  text NOT NULL,
  created_at  text NOT NULL
);

-- ---------------------------------------------------------------------------
-- customer-one cohort (worker 0201)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS customer_one_cohort_rows (
  team_cohort_ref       text NOT NULL PRIMARY KEY,
  state                 text NOT NULL,
  candidate_ref         text,
  invite_ref            text,
  vertical_ref          text,
  template_ref          text,
  workspace_ref         text,
  routing_ref           text,
  run_ref               text,
  artifact_ref          text,
  review_ref            text,
  verification_ref      text,
  completion_bundle_ref text,
  privacy_review_ref    text,
  blocker_refs_json     text NOT NULL DEFAULT '[]',
  caveat_refs_json      text NOT NULL DEFAULT '[]',
  updated_at            text NOT NULL,
  created_at            text NOT NULL
);
