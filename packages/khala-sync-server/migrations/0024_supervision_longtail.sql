-- KS-8.17 (#8328): supervision long tail (Adjutant / Omni / Autopilot / ops)
-- — Postgres twins of the 29 D1 tables: `adjutant_*` (10), `omni_*` (9),
-- `autopilot_*` (6), `relay_health_*` (2), `backend_incident_events`,
-- `hygiene_debt_receipts`. Mirrors worker migrations 0010/0013/0019/
-- 0034-0040/0053-0055/0038/0091-0099/0140-0147/0171/0172/0178/0207/0224/
-- 0249/0258/0273.
-- Plan: docs/khala-sync/MIGRATION_PLAN.md §3.14 (universal porting rules in
-- §1); templates: 0021_forge_domain.sql (KS-8.16), 0016_treasury_domain.sql
-- (KS-8.8), 0014_forum_content.sql (KS-8.10).
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's byte
-- representation — TEXT ISO-8601 timestamps, TEXT refs/paths/digests, 0/1
-- flags as smallint, JSON payload columns as text (NOT jsonb: row-hash
-- reconciliation compares exact bytes), counters/amounts/token-counts as
-- bigint.
--
-- SECRETS (SPEC invariant 9): every column in this family is a public-safe
-- ref, path, digest, count, or JSON of the same. The twin is column-for-
-- column with D1 — no widening. `autopilot_onboarding_sessions`
-- (transcript_json), `omni_*` (metadata_json / entries), and receipt/result
-- JSON columns may carry operator/customer content, so diagnostics and
-- backfill/verify output print row KEYS and sha256 row hashes ONLY, never a
-- JSON value (enforced via `custodyColumns` in the shared registry).
--
-- UNIQUE / PARTIAL-UNIQUE INDEXES ARE DELIBERATELY NOT PORTED
-- MID-MIGRATION (the KS-8.6/KS-8.8/KS-8.16 rule): D1 stays the sole write
-- authority in this lane and enforces every uniqueness constraint
-- (active-assignment-per-order, idempotency_key uniqueness, work-order
-- idempotency, run/source-ref uniqueness, run/attempt uniqueness, …). The
-- Postgres twin is a fail-soft read-back mirror converging on the PK; a
-- transiently stale twin (mirror lag, backfill catch-up) must NEVER be able
-- to reject a converge upsert, so these constraints are re-added at the
-- read/write cutover follow-up. omni_idempotency_keys is the exception — its
-- PRIMARY KEY *is* the idempotency key, ported exactly.
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL QUERY PATTERNS (the KS-8.2 rule) — the
-- non-unique D1 lookup indexes (parent-ref + time listings, the retention
-- prune scans, the hot lookups) are ported so the verify/compare scans and
-- the eventual read cutover have support; the uniqueness-enforcement indexes
-- are intentionally left off (see above). Reads stay on D1 in this lane
-- (KHALA_SYNC_SUPERVISION_READS default d1).
--
-- NO FOREIGN KEYS, NO CHECK CONSTRAINTS (dual-write mirrors and the backfill
-- land per-row and out of parent/child order; integrity + enum validity are
-- owned by the D1 authority and verified by reconciliation — same as
-- 0005/0008/0010/0014/0021). CHECK enums live on D1.

-- ==========================================================================
-- adjutant_* (10) — assignment enrichment / research
-- ==========================================================================

CREATE TABLE IF NOT EXISTS adjutant_assignments (
  id                  text NOT NULL PRIMARY KEY,
  software_order_id   text,
  site_id             text,
  goal_id             text,
  current_run_id      text,
  team_id             text,
  project_id          text,
  agent_id            text NOT NULL,
  assigned_by_user_id text,
  assignment_kind     text NOT NULL,
  status              text NOT NULL,
  visibility          text NOT NULL,
  task_spec_path      text,
  commit_sha          text,
  objective           text NOT NULL,
  created_at          text NOT NULL,
  updated_at          text NOT NULL,
  completed_at        text,
  blocked_at          text,
  archived_at         text
);
-- listByGoal / listByTeamProject: parent ref + updated_at DESC (active rows).
CREATE INDEX IF NOT EXISTS idx_adjutant_assignments_goal_updated
  ON adjutant_assignments (goal_id, updated_at DESC) WHERE goal_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_adjutant_assignments_team_project_updated
  ON adjutant_assignments (team_id, project_id, updated_at DESC)
  WHERE team_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS adjutant_assignment_events (
  id                text NOT NULL PRIMARY KEY,
  assignment_id     text NOT NULL,
  software_order_id text,
  site_id           text,
  goal_id           text,
  run_id            text,
  event_type        text NOT NULL,
  visibility        text NOT NULL,
  summary           text NOT NULL,
  actor_user_id     text,
  payload_json      text,
  created_at        text NOT NULL,
  email_message_id  text
);
CREATE INDEX IF NOT EXISTS idx_adjutant_assignment_events_assignment_created
  ON adjutant_assignment_events (assignment_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_adjutant_assignment_events_order_created
  ON adjutant_assignment_events (software_order_id, created_at DESC)
  WHERE software_order_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS adjutant_adjustment_requests (
  id                   text NOT NULL PRIMARY KEY,
  assignment_id        text NOT NULL,
  software_order_id    text,
  site_id              text NOT NULL,
  goal_id              text,
  requested_by_user_id text,
  instruction          text NOT NULL,
  status               text NOT NULL,
  continuation_mode    text,
  source_run_id        text,
  continuation_run_id  text,
  resulting_version_id text,
  visibility           text NOT NULL,
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  completed_at         text,
  archived_at          text
);
CREATE INDEX IF NOT EXISTS idx_adjutant_adjustment_requests_assignment_created
  ON adjutant_adjustment_requests (assignment_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_adjutant_adjustment_requests_site_created
  ON adjutant_adjustment_requests (site_id, created_at DESC);

CREATE TABLE IF NOT EXISTS adjutant_public_source_refs (
  id                  text NOT NULL PRIMARY KEY,
  assignment_id       text NOT NULL,
  software_order_id   text,
  site_id             text,
  kind                text NOT NULL,
  status              text NOT NULL,
  url                 text NOT NULL,
  normalized_domain   text NOT NULL,
  label               text,
  public_safe         smallint NOT NULL DEFAULT 0,
  proposed_by_user_id text,
  reviewed_by_user_id text,
  review_reason       text,
  approved_at         text,
  rejected_at         text,
  created_at          text NOT NULL,
  updated_at          text NOT NULL,
  archived_at         text
);
CREATE INDEX IF NOT EXISTS idx_adjutant_public_source_refs_assignment_created
  ON adjutant_public_source_refs (assignment_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_adjutant_public_source_refs_status_updated
  ON adjutant_public_source_refs (status, updated_at DESC);

CREATE TABLE IF NOT EXISTS adjutant_usage_receipts (
  id                      text NOT NULL PRIMARY KEY,
  assignment_id           text NOT NULL,
  software_order_id       text,
  site_id                 text,
  adjustment_id           text,
  run_id                  text,
  category                text NOT NULL,
  visibility              text NOT NULL,
  billing_mode            text NOT NULL,
  summary                 text NOT NULL,
  quantity                bigint NOT NULL DEFAULT 0,
  unit                    text NOT NULL,
  credits_charged_cents   bigint NOT NULL DEFAULT 0,
  currency                text NOT NULL DEFAULT 'USD',
  billing_ledger_entry_id text,
  public_receipt_json     text NOT NULL DEFAULT '{}',
  team_receipt_json       text NOT NULL DEFAULT '{}',
  idempotency_key         text NOT NULL,
  created_at              text NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_adjutant_usage_receipts_assignment_created
  ON adjutant_usage_receipts (assignment_id, created_at DESC);

CREATE TABLE IF NOT EXISTS adjutant_research_briefs (
  id                         text NOT NULL PRIMARY KEY,
  assignment_id              text NOT NULL,
  enrichment_run_id          text,
  status                     text NOT NULL,
  summary                    text NOT NULL,
  grounded_facts_json        text NOT NULL DEFAULT '[]',
  suggested_sections_json    text NOT NULL DEFAULT '[]',
  unknowns_json              text NOT NULL DEFAULT '[]',
  claims_needing_review_json text NOT NULL DEFAULT '[]',
  source_cards_json          text NOT NULL DEFAULT '[]',
  created_by_user_id         text,
  reviewed_by_user_id        text,
  review_reason              text,
  approved_at                text,
  rejected_at                text,
  created_at                 text NOT NULL,
  updated_at                 text NOT NULL,
  archived_at                text
);
CREATE INDEX IF NOT EXISTS idx_adjutant_research_briefs_assignment_updated
  ON adjutant_research_briefs (assignment_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS adjutant_assignment_research_policies (
  assignment_id         text NOT NULL PRIMARY KEY,
  policy_mode           text NOT NULL,
  reason                text NOT NULL,
  customer_safe_summary text NOT NULL,
  actor_user_id         text,
  source_authority_ref  text,
  created_at            text NOT NULL,
  updated_at            text NOT NULL,
  archived_at           text
);
CREATE INDEX IF NOT EXISTS idx_adjutant_research_policies_mode_updated
  ON adjutant_assignment_research_policies (policy_mode, updated_at DESC);

CREATE TABLE IF NOT EXISTS adjutant_enrichment_jobs (
  id                   text NOT NULL PRIMARY KEY,
  assignment_id        text NOT NULL,
  enrichment_run_id    text,
  status               text NOT NULL,
  trigger_kind         text NOT NULL,
  refresh              smallint NOT NULL DEFAULT 0,
  requested_by_user_id text,
  request_json         text,
  error_code           text,
  error_summary        text,
  started_at           text,
  completed_at         text,
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  archived_at          text
);
CREATE INDEX IF NOT EXISTS idx_adjutant_enrichment_jobs_assignment_updated
  ON adjutant_enrichment_jobs (assignment_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS adjutant_task_packet_freshness (
  assignment_id              text NOT NULL PRIMARY KEY,
  task_spec_path             text NOT NULL,
  commit_sha                 text,
  status                     text NOT NULL,
  research_brief_id          text,
  research_brief_approved_at text,
  source_card_count          bigint NOT NULL DEFAULT 0,
  operator_keep_reason       text,
  customer_safe_summary      text,
  actor_user_id              text,
  stale_at                   text,
  kept_at                    text,
  created_at                 text NOT NULL,
  updated_at                 text NOT NULL,
  archived_at                text
);
CREATE INDEX IF NOT EXISTS idx_adjutant_task_packet_freshness_status_updated
  ON adjutant_task_packet_freshness (status, updated_at DESC);

CREATE TABLE IF NOT EXISTS adjutant_assignment_enrichments (
  assignment_id       text NOT NULL,
  enrichment_run_id   text NOT NULL,
  research_brief_id   text,
  status              text NOT NULL,
  required_for_launch smallint NOT NULL DEFAULT 0,
  approved_at         text,
  created_at          text NOT NULL,
  updated_at          text NOT NULL,
  PRIMARY KEY (assignment_id, enrichment_run_id)
);
CREATE INDEX IF NOT EXISTS idx_adjutant_assignment_enrichments_updated
  ON adjutant_assignment_enrichments (assignment_id, updated_at DESC);

-- ==========================================================================
-- omni_* (9) — workrooms / outcome contracts / evidence / idempotency
-- ==========================================================================

CREATE TABLE IF NOT EXISTS omni_accepted_outcome_contracts (
  id                          text NOT NULL PRIMARY KEY,
  idempotency_key             text NOT NULL,
  work_kind                   text NOT NULL,
  subject_ref                 text NOT NULL,
  customer_ref                text,
  expected_artifacts_json     text NOT NULL DEFAULT '[]',
  review_policy               text NOT NULL,
  acceptance_state            text NOT NULL,
  proof_policy                text NOT NULL,
  economic_state              text NOT NULL,
  closeout_requirements_json  text NOT NULL DEFAULT '[]',
  legal_sensitive             smallint NOT NULL DEFAULT 0,
  public_receipt_ref          text NOT NULL,
  metadata_json               text NOT NULL DEFAULT '{}',
  created_at                  text NOT NULL,
  updated_at                  text NOT NULL,
  archived_at                 text,
  committed_deliverables_json text NOT NULL DEFAULT '[]',
  service_promise_state       text NOT NULL DEFAULT 'not_promised',
  sla_terms_json              text NOT NULL DEFAULT '[]',
  fulfillment_receipts_json   text NOT NULL DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS idx_omni_accepted_outcome_contracts_subject
  ON omni_accepted_outcome_contracts (subject_ref, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_omni_accepted_outcome_contracts_work_kind
  ON omni_accepted_outcome_contracts (work_kind, acceptance_state, updated_at DESC);

CREATE TABLE IF NOT EXISTS omni_workrooms (
  id                           text NOT NULL PRIMARY KEY,
  idempotency_key              text NOT NULL,
  software_order_id            text NOT NULL,
  accepted_outcome_contract_id text,
  site_id                      text,
  assignment_id                text,
  work_kind                    text NOT NULL,
  status                       text NOT NULL,
  visibility                   text NOT NULL,
  customer_intent_ref          text NOT NULL,
  task_packet_ref              text,
  source_refs_json             text NOT NULL DEFAULT '[]',
  artifact_refs_json           text NOT NULL DEFAULT '[]',
  email_refs_json              text NOT NULL DEFAULT '[]',
  receipt_refs_json            text NOT NULL DEFAULT '[]',
  blocker_refs_json            text NOT NULL DEFAULT '[]',
  public_receipt_ref           text NOT NULL,
  metadata_json                text NOT NULL DEFAULT '{}',
  created_at                   text NOT NULL,
  updated_at                   text NOT NULL,
  archived_at                  text,
  data_classification          text NOT NULL DEFAULT 'customer',
  trust_tier                   text NOT NULL DEFAULT 'unverified',
  classification_caveat_ref    text NOT NULL DEFAULT 'classification_caveat_unreviewed'
);
CREATE INDEX IF NOT EXISTS idx_omni_workrooms_site_updated
  ON omni_workrooms (site_id, updated_at DESC) WHERE site_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_omni_workrooms_assignment_updated
  ON omni_workrooms (assignment_id, updated_at DESC)
  WHERE assignment_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_omni_workrooms_kind_status
  ON omni_workrooms (work_kind, status, updated_at DESC);

CREATE TABLE IF NOT EXISTS omni_evidence_bundles (
  id                          text NOT NULL PRIMARY KEY,
  idempotency_key             text NOT NULL,
  workroom_id                 text NOT NULL,
  work_kind                   text NOT NULL,
  status                      text NOT NULL,
  legal_sensitive             smallint NOT NULL DEFAULT 0,
  summary_ref                 text NOT NULL,
  source_authority_caveat_ref text,
  entries_json                text NOT NULL DEFAULT '[]',
  public_receipt_ref          text NOT NULL,
  metadata_json               text NOT NULL DEFAULT '{}',
  created_at                  text NOT NULL,
  updated_at                  text NOT NULL,
  archived_at                 text
);
CREATE INDEX IF NOT EXISTS idx_omni_evidence_bundles_workroom_updated
  ON omni_evidence_bundles (workroom_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS omni_workroom_lifecycle_decisions (
  id                            text NOT NULL PRIMARY KEY,
  idempotency_key               text NOT NULL,
  workroom_id                   text NOT NULL,
  work_kind                     text NOT NULL,
  actor_kind                    text NOT NULL,
  decision_kind                 text NOT NULL,
  resulting_state               text NOT NULL,
  customer_safe_explanation_ref text NOT NULL,
  receipt_ref                   text NOT NULL,
  site_revision_feedback_ref    text,
  followup_request_ref          text,
  artifact_ref                  text,
  no_settlement_implication     smallint NOT NULL DEFAULT 1,
  metadata_json                 text NOT NULL DEFAULT '{}',
  created_at                    text NOT NULL,
  archived_at                   text
);
CREATE INDEX IF NOT EXISTS idx_omni_workroom_lifecycle_workroom_created
  ON omni_workroom_lifecycle_decisions (workroom_id, created_at DESC);

CREATE TABLE IF NOT EXISTS omni_accepted_outcome_economics (
  id                           text NOT NULL PRIMARY KEY,
  idempotency_key              text NOT NULL,
  workroom_id                  text NOT NULL,
  accepted_outcome_contract_id text,
  work_kind                    text NOT NULL,
  funding_mode                 text NOT NULL,
  buyer_price_asset            text NOT NULL,
  buyer_price_cents            bigint NOT NULL DEFAULT 0,
  credits_charged              bigint NOT NULL DEFAULT 0,
  sats_charged                 bigint NOT NULL DEFAULT 0,
  runner_cost_cents            bigint NOT NULL DEFAULT 0,
  provider_cost_cents          bigint NOT NULL DEFAULT 0,
  retry_cost_cents             bigint NOT NULL DEFAULT 0,
  review_minutes               bigint NOT NULL DEFAULT 0,
  review_cost_cents            bigint NOT NULL DEFAULT 0,
  artifact_cost_cents          bigint NOT NULL DEFAULT 0,
  total_cost_cents             bigint NOT NULL DEFAULT 0,
  accepted_value_cents         bigint NOT NULL DEFAULT 0,
  gross_margin_cents           bigint NOT NULL DEFAULT 0,
  public_caveat_ref            text NOT NULL,
  internal_caveat_ref          text,
  no_settlement_implication    smallint NOT NULL DEFAULT 1,
  metadata_json                text NOT NULL DEFAULT '{}',
  created_at                   text NOT NULL,
  updated_at                   text NOT NULL,
  archived_at                  text
);
CREATE INDEX IF NOT EXISTS idx_omni_outcome_economics_workroom_updated
  ON omni_accepted_outcome_economics (workroom_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS omni_route_scorecards (
  id                        text NOT NULL PRIMARY KEY,
  idempotency_key           text NOT NULL,
  workroom_id               text NOT NULL,
  work_kind                 text NOT NULL,
  selected_route_ref        text NOT NULL,
  selected_provider_ref     text NOT NULL,
  selected_account_ref      text,
  selected_model_ref        text NOT NULL,
  selected_runtime_ref      text NOT NULL,
  rejected_candidates_json  text NOT NULL DEFAULT '[]',
  decision_reason_refs_json text NOT NULL DEFAULT '[]',
  observed_result_kind      text NOT NULL,
  observed_result_ref       text NOT NULL,
  post_closeout_score       bigint,
  cost_cents                bigint NOT NULL DEFAULT 0,
  latency_ms                bigint NOT NULL DEFAULT 0,
  privacy_tier              text NOT NULL,
  trust_tier                text NOT NULL,
  public_caveat_ref         text NOT NULL,
  metadata_json             text NOT NULL DEFAULT '{}',
  created_at                text NOT NULL,
  updated_at                text NOT NULL,
  archived_at               text
);
CREATE INDEX IF NOT EXISTS idx_omni_route_scorecards_workroom_updated
  ON omni_route_scorecards (workroom_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS omni_public_proof_bundles (
  id                        text NOT NULL PRIMARY KEY,
  idempotency_key           text NOT NULL,
  workroom_id               text NOT NULL,
  work_kind                 text NOT NULL,
  status                    text NOT NULL,
  legal_sensitive           smallint NOT NULL DEFAULT 0,
  source_refs_json          text NOT NULL DEFAULT '[]',
  artifact_refs_json        text NOT NULL DEFAULT '[]',
  receipt_refs_json         text NOT NULL DEFAULT '[]',
  review_state_ref          text NOT NULL,
  acceptance_state_ref      text NOT NULL,
  economics_caveat_ref      text NOT NULL,
  legal_caveat_ref          text,
  privacy_caveat_ref        text NOT NULL,
  public_receipt_ref        text NOT NULL,
  no_settlement_implication smallint NOT NULL DEFAULT 1,
  metadata_json             text NOT NULL DEFAULT '{}',
  created_at                text NOT NULL,
  updated_at                text NOT NULL,
  archived_at               text
);
-- Public proof-bundle listing + the shadow-compared public projection surface
-- (the §3.14 acceptance): WHERE workroom_id ORDER BY updated_at DESC.
CREATE INDEX IF NOT EXISTS idx_omni_public_proof_bundles_workroom_updated
  ON omni_public_proof_bundles (workroom_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_omni_public_proof_bundles_kind_status
  ON omni_public_proof_bundles (work_kind, status, updated_at DESC);

CREATE TABLE IF NOT EXISTS omni_market_memory_hooks (
  id                       text NOT NULL PRIMARY KEY,
  idempotency_key          text NOT NULL,
  workroom_id              text NOT NULL,
  lifecycle_decision_id    text NOT NULL,
  work_kind                text NOT NULL,
  outcome_state            text NOT NULL,
  category                 text NOT NULL,
  memory_ref               text NOT NULL,
  evidence_ref             text NOT NULL,
  source_ref               text NOT NULL,
  public_caveat_ref        text NOT NULL,
  route_scorecard_ref      text,
  economics_ref            text,
  authority_boundary       text NOT NULL DEFAULT 'evidence_only',
  no_routing_mutation      smallint NOT NULL DEFAULT 1,
  no_payout_mutation       smallint NOT NULL DEFAULT 1,
  no_public_claim_mutation smallint NOT NULL DEFAULT 1,
  no_module_promotion      smallint NOT NULL DEFAULT 1,
  metadata_json            text NOT NULL DEFAULT '{}',
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  archived_at              text
);
CREATE INDEX IF NOT EXISTS idx_omni_market_memory_hooks_workroom
  ON omni_market_memory_hooks (workroom_id);
CREATE INDEX IF NOT EXISTS idx_omni_market_memory_hooks_category
  ON omni_market_memory_hooks (category, outcome_state);

-- Pure idempotency table (worker 0010): the PRIMARY KEY *is* the idempotency
-- key — ported exactly (the §3.14 "port key-exactly" requirement).
CREATE TABLE IF NOT EXISTS omni_idempotency_keys (
  key         text NOT NULL PRIMARY KEY,
  scope       text NOT NULL,
  result_json text NOT NULL,
  created_at  text NOT NULL,
  expires_at  text
);
CREATE INDEX IF NOT EXISTS idx_omni_idempotency_keys_scope
  ON omni_idempotency_keys (scope);

-- ==========================================================================
-- autopilot_* (6) — work orders / continuations / onboarding / token usage
-- ==========================================================================

CREATE TABLE IF NOT EXISTS autopilot_token_usage (
  id                    text NOT NULL PRIMARY KEY,
  run_id                text NOT NULL,
  event_id              text NOT NULL,
  user_id               text NOT NULL,
  team_id               text,
  provider              text,
  model                 text,
  input_tokens          bigint NOT NULL DEFAULT 0,
  output_tokens         bigint NOT NULL DEFAULT 0,
  reasoning_tokens      bigint NOT NULL DEFAULT 0,
  cache_read_tokens     bigint NOT NULL DEFAULT 0,
  cache_write_5m_tokens bigint NOT NULL DEFAULT 0,
  cache_write_1h_tokens bigint NOT NULL DEFAULT 0,
  total_tokens          bigint NOT NULL DEFAULT 0,
  source                text NOT NULL,
  source_ref            text NOT NULL,
  created_at            text NOT NULL,
  account_ref           text
);
-- leaderboard aggregations (token-usage.ts): by user/team/run/account + time.
CREATE INDEX IF NOT EXISTS idx_autopilot_token_usage_user_created
  ON autopilot_token_usage (user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_autopilot_token_usage_run
  ON autopilot_token_usage (run_id);
CREATE INDEX IF NOT EXISTS idx_autopilot_token_usage_account
  ON autopilot_token_usage (account_ref, total_tokens DESC)
  WHERE account_ref IS NOT NULL;

CREATE TABLE IF NOT EXISTS autopilot_work_orders (
  id                       text NOT NULL PRIMARY KEY,
  work_order_ref           text NOT NULL,
  owner_user_id            text NOT NULL,
  agent_user_id            text NOT NULL,
  agent_credential_id      text NOT NULL,
  idempotency_key_hash     text NOT NULL,
  client_request_ref       text NOT NULL,
  request_json             text NOT NULL,
  state                    text NOT NULL,
  task_refs_json           text NOT NULL,
  access_request_refs_json text NOT NULL,
  payment_challenge_ref    text,
  status_url_ref           text NOT NULL,
  event_stream_ref         text NOT NULL,
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  archived_at              text,
  buyer_payment_proof_ref  text,
  placement_policy_json    text,
  execution_closeout_json  text,
  review_decision_json     text,
  scheduled_launch_json    text
);
CREATE INDEX IF NOT EXISTS idx_autopilot_work_orders_owner_created
  ON autopilot_work_orders (owner_user_id, created_at DESC);
-- the scheduled-launch dispatcher (AutopilotScheduledLaunches.dispatchDue):
-- WHERE scheduled_launch_json IS NOT NULL ORDER BY state, created_at.
CREATE INDEX IF NOT EXISTS idx_autopilot_work_orders_scheduled_launch
  ON autopilot_work_orders (state, created_at)
  WHERE scheduled_launch_json IS NOT NULL;

CREATE TABLE IF NOT EXISTS autopilot_decision_closeout_receipts (
  closeout_ref      text NOT NULL PRIMARY KEY,
  decision_ref      text NOT NULL,
  work_order_ref    text NOT NULL,
  action            text NOT NULL,
  resolved_state    text NOT NULL,
  outcome           text NOT NULL,
  actor_agent_user_id text NOT NULL,
  decided_at        text NOT NULL,
  receipt_refs_json text NOT NULL,
  has_answer        smallint NOT NULL DEFAULT 0,
  line              text NOT NULL,
  receipt_json      text NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_autopilot_decision_closeout_receipts_work
  ON autopilot_decision_closeout_receipts (work_order_ref, decided_at DESC);

CREATE TABLE IF NOT EXISTS autopilot_continuation_policies (
  user_id                   text NOT NULL PRIMARY KEY,
  enabled                   smallint NOT NULL DEFAULT 0,
  max_continuations_per_run bigint NOT NULL DEFAULT 2,
  max_continuations_per_day bigint NOT NULL DEFAULT 10,
  created_at                text NOT NULL,
  updated_at                text NOT NULL
);

CREATE TABLE IF NOT EXISTS autopilot_continuation_events (
  id         text NOT NULL PRIMARY KEY,
  user_id    text NOT NULL,
  run_id     text NOT NULL,
  goal_id    text,
  mode       text NOT NULL,
  decision   text NOT NULL,
  reason_ref text NOT NULL,
  attempt    bigint NOT NULL,
  created_at text NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_autopilot_continuation_events_user_created
  ON autopilot_continuation_events (user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_autopilot_continuation_events_run
  ON autopilot_continuation_events (run_id);

CREATE TABLE IF NOT EXISTS autopilot_onboarding_sessions (
  id               text NOT NULL PRIMARY KEY,
  vertical_overlay text,
  status           text NOT NULL DEFAULT 'interviewing',
  transcript_json  text NOT NULL DEFAULT '[]',
  output_spec_json text NOT NULL DEFAULT '{}',
  turn_count       bigint NOT NULL DEFAULT 0,
  created_at       text NOT NULL,
  updated_at       text NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_autopilot_onboarding_sessions_updated
  ON autopilot_onboarding_sessions (updated_at DESC);

-- ==========================================================================
-- ops (4) — relay health, backend incidents, hygiene debt receipts
-- ==========================================================================

CREATE TABLE IF NOT EXISTS relay_health_probes (
  id                text NOT NULL PRIMARY KEY,
  relay_url         text NOT NULL,
  probed_at         text NOT NULL,
  nip11_outcome     text NOT NULL,
  nip11_http_status bigint,
  nip11_latency_ms  bigint,
  nip11_relay_name  text,
  ws_outcome        text NOT NULL,
  ws_latency_ms     bigint,
  status            text NOT NULL,
  created_at        text NOT NULL
);
-- probe listings + the RelayHealth.probeTick retention prune: by relay_url +
-- probed_at (the prune scans probed_at < cutoff).
CREATE INDEX IF NOT EXISTS idx_relay_health_probes_relay_probed_at
  ON relay_health_probes (relay_url, probed_at DESC);
CREATE INDEX IF NOT EXISTS idx_relay_health_probes_probed_at
  ON relay_health_probes (probed_at);

CREATE TABLE IF NOT EXISTS relay_health_transitions (
  id          text NOT NULL PRIMARY KEY,
  relay_url   text NOT NULL,
  occurred_at text NOT NULL,
  kind        text NOT NULL,
  from_status text NOT NULL,
  to_status   text NOT NULL,
  probe_id    text NOT NULL,
  created_at  text NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_relay_health_transitions_relay_occurred_at
  ON relay_health_transitions (relay_url, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_relay_health_transitions_occurred_at
  ON relay_health_transitions (occurred_at);

CREATE TABLE IF NOT EXISTS backend_incident_events (
  id                text NOT NULL PRIMARY KEY,
  incident_ref      text NOT NULL,
  observed_at       text NOT NULL,
  source            text NOT NULL,
  kind              text NOT NULL,
  severity          text NOT NULL,
  route_pattern     text NOT NULL DEFAULT 'unknown',
  method            text NOT NULL DEFAULT 'UNKNOWN',
  status_code       bigint,
  error_name        text NOT NULL DEFAULT 'unknown',
  runtime_name      text NOT NULL DEFAULT 'cloudflare_workers',
  occurrence_count  bigint NOT NULL DEFAULT 1,
  safe_metadata_json text NOT NULL DEFAULT '{}',
  created_at        text NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_backend_incident_events_observed
  ON backend_incident_events (observed_at DESC);
CREATE INDEX IF NOT EXISTS idx_backend_incident_events_kind_observed
  ON backend_incident_events (kind, observed_at DESC);
CREATE INDEX IF NOT EXISTS idx_backend_incident_events_source_observed
  ON backend_incident_events (source, observed_at DESC);

CREATE TABLE IF NOT EXISTS hygiene_debt_receipts (
  debt_receipt_key               text NOT NULL PRIMARY KEY,
  state                          text NOT NULL DEFAULT 'payable',
  debt_receipt_ref               text NOT NULL,
  repo_baseline_ref              text NOT NULL,
  scope_digest                   text NOT NULL,
  objective_digest               text NOT NULL,
  merged_pr_ref                  text NOT NULL,
  reviewer_acceptance_ref        text NOT NULL,
  baseline_metric_refs_json      text NOT NULL,
  target_metric_refs_json        text NOT NULL,
  verification_command_refs_json text NOT NULL,
  settlement_authority_actor_ref text,
  budget_cap_sats                bigint NOT NULL,
  payable_sats                   bigint NOT NULL,
  settlement_input_json          text NOT NULL,
  created_at                     text NOT NULL,
  updated_at                     text NOT NULL,
  retired_at                     text,
  settlement_receipt_ref         text
);
CREATE INDEX IF NOT EXISTS idx_hygiene_debt_receipts_state
  ON hygiene_debt_receipts (state, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_hygiene_debt_receipts_merged_pr
  ON hygiene_debt_receipts (merged_pr_ref);
