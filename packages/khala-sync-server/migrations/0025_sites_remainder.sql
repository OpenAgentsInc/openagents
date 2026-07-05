-- KS-8.12 REMAINDER (#8357): Sites domain follow-up — Postgres twins of the
-- 36 remainder D1 tables the core lane (#8323, migration `0020_sites_core.sql`)
-- deliberately deferred. Parent: epic #8282. Plan:
-- docs/khala-sync/MIGRATION_PLAN.md §3.9; procedure: docs/khala-sync/RUNBOOK.md
-- "Sites content domain cutover". Universal porting rules: §1. Template:
-- 0020_sites_core.sql (the same shared registry
-- packages/khala-sync-server/src/sites-content-tables.ts drives both the
-- Worker mirror and the backfill verifier for these tables too).
--
-- SCOPE (the KS-8.12 remainder family, per issue #8357):
--   A — content satellites: site_build_validations, site_revision_feedback,
--       site_compatibility_checks, site_provisioning_plans,
--       site_storage_bindings, site_source_exports, and the referral
--       source/policy family (site_referral_sources, referral_invites,
--       site_referral_policy_events).
--   B — site_environment_values: SECRET-BEARING (SPEC invariant 9: secret
--       material NEVER rides the sync path). The Postgres twin carries ONLY
--       metadata + the `secret_ref` INDIRECTION — the `plain_value` column is
--       DELIBERATELY OMITTED here and from the shared registry column list, so
--       neither the dual-write mirror nor the backfill ever reads or ships it.
--       Same posture as the KS-8.5 credential deferral: keep/route secret
--       payloads through the secret-ref indirection, migrate metadata only.
--   C — site COMMERCE / money (money discipline): site_commerce_* (5),
--       site_mdk_checkout_intents, site_mdk_account_bindings,
--       site_payment_catalog_items, site_referral_payout_ledger_entries. These
--       reference the KS-8.7 (#8318) billing and KS-8.8 (#8319) treasury rails
--       BY ID (payment_evidence_ref / entitlement_ref / nexus_receipt_ref /
--       treasury_receipt_ref / ldk_settlement_receipt_ref / payout_ref /
--       referral_attribution_id …) and MUST NEVER FORK them: those columns are
--       plain text refs here, never FKs into a mirrored rail. D1 stays the
--       money authority for the whole dual-write window; this twin is
--       mirror-only. Verification is commerce totals to the cent (SUM(amount)
--       per asset — see SITES_CONTENT_SCALAR_TALLIES) plus set-membership
--       referential checks across stores (no cross-store joins).
--   D — targeted sites + hostnames + legacy: targeted_site_* (14 of 15;
--       `targeted_site_campaign_metric_events` is DELIBERATELY EXCLUDED — it is
--       the Analytics-Engine-candidate campaign firehose called out in
--       MIGRATION_PLAN §3.9, and a high-volume metric event stream is not
--       blind-copied into a relational twin; it stays on D1/AE pending a
--       dedicated telemetry-sink decision), `tenant_custom_hostnames`, and the
--       legacy `deployments` / `deployment_events` pair (the 0019 rebuild
--       schema, distinct from `site_deployments`).
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's byte
-- representation — TEXT ISO-8601 timestamps, JSON payload columns as text (NOT
-- jsonb: row-hash reconciliation compares exact bytes), INTEGER counters/flags
-- as bigint, money/score NUMERIC columns as numeric
-- (site_commerce_payment_events.amount, targeted_site_prospects.
-- discovery_confidence, targeted_site_quality_audits.overall_score). Stores
-- cast reads with Number(); the row-hash canonicalizes both sides via String().
--
-- IDEMPOTENCY / NATURAL KEYS PORT EXACTLY (MIGRATION_PLAN §1): every
-- `idempotency_key` / `idempotency_key_hash` / `*_ref` / natural composite
-- UNIQUE that DEDUPES a write ports verbatim (site_provisioning_plans,
-- site_source_exports, all targeted_site_* idempotency keys,
-- targeted_site_prospects (campaign_id, normalized_domain), the MDK/commerce
-- decision-ref and checkout-ref uniques, deployment_events (deploy_id,
-- sequence)/(deploy_id, external_event_id), etc.). These are the write-dedupe
-- keys the D1 SELECT-before-INSERT / INSERT-OR-IGNORE paths key on, and the
-- contract suite asserts they reject a duplicate on a new id on BOTH stores.
--
-- DELIBERATELY NOT PORTED MID-MIGRATION (the 0020 rationale): partial uniques
-- that enforce a live business invariant — site_environment_values
-- (site_id, key) WHERE deleted_at IS NULL, site_commerce_products/paid_actions
-- (site_id, *_key) WHERE archived_at IS NULL. D1 stays the enforcement
-- authority for the dual-write window; the mirror replays per-statement
-- read-back snapshots, so porting those partials would make the mirror reject
-- exactly those replays and CREATE drift. They return at read cutover.
--
-- INDEXES (the KS-8.2 rule + Scope E read-deferral): sites read serving from
-- Postgres is DEFERRED for this whole lane (KHALA_SYNC_SITES_READS stays `d1`),
-- so read-serving secondary indexes are RE-DERIVED at the read cutover from
-- actual query patterns rather than blind-copied from D1 now. This migration
-- creates ONLY: (1) the correctness-bearing PK + dedupe UNIQUEs above, and
-- (2) the handful of parent-key / money-relationship indexes the read-back
-- mirror's bounded fan-out and the verify set-membership checks actually touch,
-- each justified inline. The long tail returns at read cutover.
--
-- NO FOREIGN KEYS (dual-write mirrors and the backfill land per-row; integrity
-- is verified by reconciliation — same as 0005/0008/0010/0014/0020). This is
-- especially load-bearing for the money tables: the KS-8.7/KS-8.8 rails are
-- referenced by id only and are NOT mirrored here.

CREATE TABLE IF NOT EXISTS site_build_validations (
  id                         text NOT NULL PRIMARY KEY,
  site_id                    text,
  compatibility_check_id     text,
  source_kind                text,
  source_repository_json     text,
  source_commit_sha          text,
  source_hash                text,
  status                     text,
  package_manager            text,
  requested_build_command    text,
  build_command              text,
  output_kind                text,
  output_path                text,
  worker_module_path         text,
  manifest_json              text,
  bounded_logs_json          text,
  log_line_count             bigint,
  log_truncated              bigint,
  findings_json              text,
  blockers_json              text,
  warnings_json              text,
  evidence_refs_json         text,
  customer_safe_status       text,
  customer_safe_next_action  text,
  validated_by_user_id       text,
  created_at                 text,
  archived_at                text
);

CREATE TABLE IF NOT EXISTS site_revision_feedback (
  id                      text NOT NULL PRIMARY KEY,
  software_order_id       text,
  site_id                 text,
  site_version_id         text,
  site_deployment_id      text,
  author_user_id          text,
  body                    text,
  status                  text,
  source                  text,
  visibility              text,
  created_at              text,
  updated_at              text,
  archived_at             text,
  adjutant_assignment_id  text,
  adjutant_adjustment_id  text
);

CREATE TABLE IF NOT EXISTS site_compatibility_checks (
  id                         text NOT NULL PRIMARY KEY,
  site_id                    text,
  source_kind                text,
  source_repository_json     text,
  status                     text,
  confidence                 text,
  package_manager            text,
  build_command              text,
  output_kind                text,
  output_path                text,
  worker_module_path         text,
  needs_d1                   bigint,
  needs_r2                   bigint,
  needs_workspace_auth       bigint,
  needs_public_auth          bigint,
  env_keys_json              text,
  findings_json              text,
  blockers_json              text,
  warnings_json              text,
  evidence_refs_json         text,
  customer_safe_status       text,
  customer_safe_next_action  text,
  checked_by_user_id         text,
  created_at                 text,
  archived_at                text
);

CREATE TABLE IF NOT EXISTS site_provisioning_plans (
  id                      text NOT NULL PRIMARY KEY,
  idempotency_key         text,
  site_id                 text,
  status                  text,
  requested_by_user_id    text,
  reviewed_by_user_id     text,
  resource_manifest_json  text,
  receipt_json            text,
  created_at              text,
  reviewed_at             text,
  updated_at              text,
  archived_at             text,
  CONSTRAINT site_provisioning_plans_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS site_storage_bindings (
  id                       text NOT NULL PRIMARY KEY,
  site_id                  text,
  kind                     text,
  binding_name             text,
  cloudflare_resource_ref  text,
  scope                    text,
  created_at               text,
  updated_at               text,
  CONSTRAINT site_storage_bindings_site_id_kind_binding_name_key UNIQUE (site_id, kind, binding_name)
);

CREATE TABLE IF NOT EXISTS site_source_exports (
  id                            text NOT NULL PRIMARY KEY,
  idempotency_key               text,
  site_id                       text,
  version_id                    text,
  status                        text,
  export_kind                   text,
  actor_user_id                 text,
  approved_by_user_id           text,
  destination_provider          text,
  destination_owner             text,
  destination_repository        text,
  destination_branch            text,
  destination_pull_request_url  text,
  destination_url               text,
  source_archive_r2_key         text,
  artifact_manifest_r2_key      text,
  worker_module_r2_key          text,
  source_artifact_ref           text,
  token_ref                     text,
  token_hash                    text,
  token_expires_at              text,
  secret_scan_status            text,
  secret_scan_ref               text,
  receipt_json                  text,
  created_at                    text,
  updated_at                    text,
  archived_at                   text,
  CONSTRAINT site_source_exports_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS site_referral_sources (
  id                 text NOT NULL PRIMARY KEY,
  site_id            text,
  site_version_id    text,
  referrer_user_id   text,
  public_source_ref  text,
  public_slug        text,
  campaign_ref       text,
  source_label       text,
  policy_state       text,
  created_at         text,
  updated_at         text,
  archived_at        text,
  CONSTRAINT site_referral_sources_public_source_ref_key UNIQUE (public_source_ref)
);

CREATE TABLE IF NOT EXISTS referral_invites (
  id                  text NOT NULL PRIMARY KEY,
  referral_source_id  text,
  public_invite_ref   text,
  token_hash          text,
  scope               text,
  audience_path       text,
  policy_state        text,
  expires_at          text,
  created_at          text,
  updated_at          text,
  archived_at         text,
  CONSTRAINT referral_invites_public_invite_ref_key UNIQUE (public_invite_ref)
);

CREATE TABLE IF NOT EXISTS site_referral_policy_events (
  id                          text NOT NULL PRIMARY KEY,
  idempotency_key             text,
  subject_kind                text,
  subject_ref                 text,
  referral_attribution_id     text,
  referral_source_id          text,
  referral_invite_id          text,
  referral_workflow_event_id  text,
  software_order_id           text,
  site_id                     text,
  previous_state              text,
  decision_state              text,
  policy_reason               text,
  eligibility                 text,
  customer_status             text,
  operator_actor_user_id      text,
  operator_note_ref           text,
  metadata_json               text,
  decided_at                  text,
  created_at                  text,
  archived_at                 text,
  CONSTRAINT site_referral_policy_events_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS site_environment_values (
  id          text NOT NULL PRIMARY KEY,
  site_id     text,
  key         text,
  kind        text,
  secret_ref  text,
  created_at  text,
  updated_at  text,
  deleted_at  text
);

CREATE TABLE IF NOT EXISTS site_commerce_products (
  id                               text NOT NULL PRIMARY KEY,
  site_id                          text,
  site_version_id                  text,
  product_key                      text,
  name                             text,
  asset                            text,
  amount                           bigint,
  checkout_path                    text,
  entitlement_scope                text,
  agent_readable                   bigint,
  settlement_mode                  text,
  customer_data_requirements_json  text,
  public_projection_state          text,
  created_by_user_id               text,
  created_at                       text,
  updated_at                       text,
  archived_at                      text
);

CREATE TABLE IF NOT EXISTS site_commerce_paid_actions (
  id                               text NOT NULL PRIMARY KEY,
  site_id                          text,
  site_version_id                  text,
  action_key                       text,
  name                             text,
  method                           text,
  path                             text,
  asset                            text,
  amount                           bigint,
  checkout_path                    text,
  entitlement_scope                text,
  agent_readable                   bigint,
  settlement_mode                  text,
  customer_data_requirements_json  text,
  public_projection_state          text,
  created_by_user_id               text,
  created_at                       text,
  updated_at                       text,
  archived_at                      text
);

CREATE TABLE IF NOT EXISTS site_commerce_payment_events (
  id                    text NOT NULL PRIMARY KEY,
  site_id               text,
  site_version_id       text,
  software_order_id     text,
  product_id            text,
  paid_action_id        text,
  customer_ref          text,
  referral_source_ref   text,
  payment_evidence_ref  text,
  entitlement_ref       text,
  public_receipt_ref    text,
  event_kind            text,
  amount                numeric,
  asset                 text,
  created_at            text,
  CONSTRAINT site_commerce_payment_events_public_receipt_ref_key UNIQUE (public_receipt_ref)
);

CREATE TABLE IF NOT EXISTS site_commerce_revenue_share_links (
  id                                 text NOT NULL PRIMARY KEY,
  payment_event_id                   text,
  accepted_work_ref                  text,
  requested_contributor_asset        text,
  provider_payout_claimed            bigint,
  nexus_receipt_ref                  text,
  treasury_receipt_ref               text,
  ldk_settlement_receipt_ref         text,
  referral_reward_trigger            text,
  provider_payout_eligibility_state  text,
  withdrawal_posture                 text,
  projection_json                    text,
  created_at                         text
);

CREATE TABLE IF NOT EXISTS site_commerce_review_decisions (
  id                                    text NOT NULL PRIMARY KEY,
  decision_ref                          text,
  idempotency_key_hash                  text,
  site_id                               text,
  site_version_id                       text,
  catalog_ref                           text,
  review_status                         text,
  reason_refs_json                      text,
  customer_input_requirement_refs_json  text,
  actor_ref                             text,
  public_projection_json                text,
  created_at                            text,
  updated_at                            text,
  archived_at                           text,
  CONSTRAINT site_commerce_review_decisions_decision_ref_key UNIQUE (decision_ref),
  CONSTRAINT site_commerce_review_decisions_idempotency_key_hash_key UNIQUE (idempotency_key_hash),
  CONSTRAINT site_commerce_review_decisions_site_id_site_version_id_catalog_ref_key UNIQUE (site_id, site_version_id, catalog_ref)
);

CREATE TABLE IF NOT EXISTS site_mdk_checkout_intents (
  id                               text NOT NULL PRIMARY KEY,
  checkout_intent_ref              text,
  idempotency_key_hash             text,
  site_id                          text,
  site_version_id                  text,
  catalog_ref                      text,
  product_id                       text,
  challenge_ref                    text,
  checkout_ref                     text,
  checkout_url_ref                 text,
  checkout_launch_path             text,
  provider_ref                     text,
  status                           text,
  environment                      text,
  sandbox                          bigint,
  amount_asset                     text,
  amount_denomination              text,
  amount_minor_units               bigint,
  success_return_path              text,
  cancel_return_path               text,
  metadata_refs_json               text,
  hosted_checkout_projection_json  text,
  public_projection_json           text,
  created_at                       text,
  updated_at                       text,
  archived_at                      text,
  CONSTRAINT site_mdk_checkout_intents_checkout_intent_ref_key UNIQUE (checkout_intent_ref),
  CONSTRAINT site_mdk_checkout_intents_idempotency_key_hash_key UNIQUE (idempotency_key_hash),
  CONSTRAINT site_mdk_checkout_intents_checkout_ref_key UNIQUE (checkout_ref)
);

CREATE TABLE IF NOT EXISTS site_mdk_account_bindings (
  id                         text NOT NULL PRIMARY KEY,
  binding_ref                text,
  idempotency_key_hash       text,
  site_id                    text,
  site_version_id            text,
  customer_ref               text,
  order_ref                  text,
  requested_provider_mode    text,
  environment                text,
  review_status              text,
  secret_binding_refs_json   text,
  allowed_catalog_refs_json  text,
  allowed_product_refs_json  text,
  allowed_action_refs_json   text,
  reviewer_refs_json         text,
  caveat_refs_json           text,
  public_projection_json     text,
  created_at                 text,
  updated_at                 text,
  archived_at                text,
  CONSTRAINT site_mdk_account_bindings_idempotency_key_hash_key UNIQUE (idempotency_key_hash),
  CONSTRAINT site_mdk_account_bindings_site_id_binding_ref_key UNIQUE (site_id, binding_ref)
);

CREATE TABLE IF NOT EXISTS site_payment_catalog_items (
  id                               text NOT NULL PRIMARY KEY,
  catalog_ref                      text,
  item_kind                        text,
  site_id                          text,
  site_version_id                  text,
  deployment_id                    text,
  order_ref                        text,
  workroom_ref                     text,
  manifest_ref                     text,
  source_manifest_digest           text,
  product_id                       text,
  action_id                        text,
  action_ref                       text,
  method                           text,
  path                             text,
  display_ref                      text,
  checkout_path                    text,
  price_asset                      text,
  price_denomination               text,
  price_amount_minor_units         bigint,
  entitlement_scope                text,
  settlement_mode                  text,
  public_projection_state          text,
  sandbox                          bigint,
  agent_readable                   bigint,
  status                           text,
  metadata_refs_json               text,
  customer_data_requirements_json  text,
  paid_endpoint_product_json       text,
  public_projection_json           text,
  created_at                       text,
  updated_at                       text,
  archived_at                      text,
  CONSTRAINT site_payment_catalog_items_catalog_ref_key UNIQUE (catalog_ref)
);

CREATE TABLE IF NOT EXISTS site_referral_payout_ledger_entries (
  id                       text NOT NULL PRIMARY KEY,
  payout_ref               text,
  idempotency_key          text,
  referral_attribution_id  text,
  referral_source_id       text,
  referral_invite_id       text,
  referrer_user_id         text,
  referred_user_id         text,
  qualifying_event_ref     text,
  qualifying_event_kind    text,
  qualifying_amount_sats   bigint,
  amount_sats              bigint,
  period_key               text,
  state                    text,
  state_reason_ref         text,
  previous_entry_id        text,
  reversal_of_entry_id     text,
  evidence_refs_json       text,
  policy_refs_json         text,
  caveat_refs_json         text,
  created_at               text,
  archived_at              text,
  CONSTRAINT site_referral_payout_ledger_entries_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS targeted_site_campaigns (
  id                      text NOT NULL PRIMARY KEY,
  slug                    text,
  name                    text,
  owner_user_id           text,
  operator_user_id        text,
  vertical                text,
  geography               text,
  source_authority_ref    text,
  budget_cap_ref          text,
  suppression_policy_ref  text,
  operator_state          text,
  metadata_json           text,
  created_at              text,
  updated_at              text,
  archived_at             text,
  CONSTRAINT targeted_site_campaigns_slug_key UNIQUE (slug)
);

CREATE TABLE IF NOT EXISTS targeted_site_prospects (
  id                    text NOT NULL PRIMARY KEY,
  campaign_id           text,
  idempotency_key       text,
  normalized_domain     text,
  origin_url            text,
  company_name          text,
  site_name             text,
  contact_refs_json     text,
  vertical              text,
  geography             text,
  source_ref            text,
  discovery_confidence  numeric,
  suppression_state     text,
  capture_state         text,
  review_state          text,
  metadata_json         text,
  discovered_at         text,
  created_at            text,
  updated_at            text,
  archived_at           text,
  CONSTRAINT targeted_site_prospects_idempotency_key_key UNIQUE (idempotency_key),
  CONSTRAINT targeted_site_prospects_campaign_id_normalized_domain_key UNIQUE (campaign_id, normalized_domain)
);

CREATE TABLE IF NOT EXISTS targeted_site_capture_policy_events (
  id                      text NOT NULL PRIMARY KEY,
  idempotency_key         text,
  campaign_id             text,
  prospect_id             text,
  normalized_domain       text,
  source_ref              text,
  decision                text,
  fetchable               bigint,
  reason                  text,
  robots_ref              text,
  sitemap_ref             text,
  suppression_ref         text,
  customer_authority_ref  text,
  paid_escalation_ref     text,
  operator_actor_user_id  text,
  operator_note_ref       text,
  metadata_json           text,
  decided_at              text,
  created_at              text,
  archived_at             text,
  CONSTRAINT targeted_site_capture_policy_events_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS targeted_site_static_capture_runs (
  id                       text NOT NULL PRIMARY KEY,
  idempotency_key          text,
  campaign_id              text,
  prospect_id              text,
  normalized_domain        text,
  capture_policy_event_id  text,
  state                    text,
  reason                   text,
  homepage_url             text,
  homepage_ref             text,
  robots_ref               text,
  sitemap_ref              text,
  source_pack_ref          text,
  source_hash              text,
  page_refs_json           text,
  asset_refs_json          text,
  response_summary_json    text,
  metadata_json            text,
  started_at               text,
  completed_at             text,
  created_at               text,
  archived_at              text,
  CONSTRAINT targeted_site_static_capture_runs_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS targeted_site_rendered_capture_runs (
  id                       text NOT NULL PRIMARY KEY,
  idempotency_key          text,
  campaign_id              text,
  prospect_id              text,
  normalized_domain        text,
  capture_policy_event_id  text,
  static_capture_run_id    text,
  state                    text,
  reason                   text,
  target_url               text,
  provider_ref             text,
  screenshot_ref           text,
  rendered_html_ref        text,
  markdown_ref             text,
  links_ref                text,
  structured_json_ref      text,
  crawl_ref                text,
  viewport_ref             text,
  device_ref               text,
  usage_summary_json       text,
  metadata_json            text,
  started_at               text,
  completed_at             text,
  created_at               text,
  archived_at              text,
  CONSTRAINT targeted_site_rendered_capture_runs_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS targeted_site_capture_provider_adapter_runs (
  id                       text NOT NULL PRIMARY KEY,
  idempotency_key          text,
  campaign_id              text,
  prospect_id              text,
  normalized_domain        text,
  capture_policy_event_id  text,
  static_capture_run_id    text,
  rendered_capture_run_id  text,
  provider_kind            text,
  state                    text,
  reason                   text,
  paid_escalation_ref      text,
  provider_request_ref     text,
  provider_receipt_ref     text,
  output_pack_ref          text,
  usage_ref                text,
  cost_ref                 text,
  metadata_json            text,
  requested_at             text,
  completed_at             text,
  created_at               text,
  archived_at              text,
  CONSTRAINT targeted_site_capture_provider_adapter_runs_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS targeted_site_quality_audits (
  id                       text NOT NULL PRIMARY KEY,
  idempotency_key          text,
  campaign_id              text,
  prospect_id              text,
  normalized_domain        text,
  static_capture_run_id    text,
  rendered_capture_run_id  text,
  provider_adapter_run_id  text,
  state                    text,
  recommendation           text,
  overall_score            numeric,
  legal_sensitive          bigint,
  dimensions_json          text,
  evidence_refs_json       text,
  metadata_json            text,
  audited_at               text,
  created_at               text,
  archived_at              text,
  CONSTRAINT targeted_site_quality_audits_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS targeted_site_remake_briefs (
  id                             text NOT NULL PRIMARY KEY,
  idempotency_key                text,
  campaign_id                    text,
  prospect_id                    text,
  normalized_domain              text,
  quality_audit_id               text,
  static_capture_run_id          text,
  rendered_capture_run_id        text,
  provider_adapter_run_id        text,
  state                          text,
  legal_sensitive                bigint,
  source_authority_pack_json     text,
  audit_finding_refs_json        text,
  original_screenshot_refs_json  text,
  copied_text_refs_json          text,
  copied_image_refs_json         text,
  generation_constraints_json    text,
  metadata_json                  text,
  prepared_at                    text,
  reviewed_at                    text,
  created_at                     text,
  archived_at                    text,
  CONSTRAINT targeted_site_remake_briefs_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS targeted_site_remake_preview_generations (
  id                           text NOT NULL PRIMARY KEY,
  idempotency_key              text,
  campaign_id                  text,
  prospect_id                  text,
  normalized_domain            text,
  remake_brief_id              text,
  quality_audit_id             text,
  static_capture_run_id        text,
  rendered_capture_run_id      text,
  provider_adapter_run_id      text,
  state                        text,
  preview_url                  text,
  concept_slug                 text,
  source_authority_pack_ref    text,
  generated_artifact_ref       text,
  generated_source_ref         text,
  candidate_site_project_ref   text,
  candidate_site_version_ref   text,
  generation_receipt_ref       text,
  failure_ref                  text,
  legal_sensitive              bigint,
  generation_constraints_json  text,
  metadata_json                text,
  requested_at                 text,
  completed_at                 text,
  created_at                   text,
  archived_at                  text,
  CONSTRAINT targeted_site_remake_preview_generations_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS targeted_site_operator_review_events (
  id                      text NOT NULL PRIMARY KEY,
  idempotency_key         text,
  campaign_id             text,
  prospect_id             text,
  normalized_domain       text,
  remake_brief_id         text,
  preview_generation_id   text,
  decision                text,
  previous_state          text,
  next_state              text,
  operator_actor_user_id  text,
  operator_note_ref       text,
  outreach_draft_ref      text,
  meeting_cta_ref         text,
  suppression_state       text,
  evidence_refs_json      text,
  metadata_json           text,
  decided_at              text,
  created_at              text,
  archived_at             text,
  CONSTRAINT targeted_site_operator_review_events_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS targeted_site_remake_outreach_email_dispatches (
  id                        text NOT NULL PRIMARY KEY,
  idempotency_key           text,
  campaign_id               text,
  prospect_id               text,
  normalized_domain         text,
  preview_generation_id     text,
  operator_review_event_id  text,
  email_message_id          text,
  recipient_ref             text,
  template_slug             text,
  suppression_state         text,
  dispatch_state            text,
  error_name                text,
  error_message             text,
  metadata_json             text,
  dispatched_at             text,
  created_at                text,
  archived_at               text,
  CONSTRAINT targeted_site_remake_outreach_email_dispatches_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS targeted_site_agent_toolkit_grants (
  id                      text NOT NULL PRIMARY KEY,
  idempotency_key         text,
  campaign_id             text,
  owner_user_id           text,
  agent_ref               text,
  scopes_json             text,
  dry_run_default         bigint,
  spend_cap_cents         bigint,
  daily_send_cap          bigint,
  suppression_policy_ref  text,
  approval_policy         text,
  status                  text,
  metadata_json           text,
  created_at              text,
  updated_at              text,
  expires_at              text,
  revoked_at              text,
  archived_at             text,
  CONSTRAINT targeted_site_agent_toolkit_grants_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS targeted_site_agent_toolkit_actions (
  id                    text NOT NULL PRIMARY KEY,
  idempotency_key       text,
  grant_id              text,
  campaign_id           text,
  agent_ref             text,
  action_kind           text,
  dry_run               bigint,
  requested_cost_cents  bigint,
  requested_send_count  bigint,
  suppression_state     text,
  approval_state        text,
  result_state          text,
  receipt_ref           text,
  reason                text,
  metadata_json         text,
  created_at            text,
  archived_at           text,
  CONSTRAINT targeted_site_agent_toolkit_actions_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS targeted_site_sales_reward_policy_events (
  id                        text NOT NULL PRIMARY KEY,
  idempotency_key           text,
  campaign_id               text,
  agent_ref                 text,
  prospect_id               text,
  outcome_kind              text,
  policy_state              text,
  reward_asset              text,
  reward_amount             bigint,
  buyer_payment_ref         text,
  referral_attribution_ref  text,
  accepted_work_ref         text,
  payout_intent_ref         text,
  settlement_caveat_ref     text,
  dispute_ref               text,
  public_receipt_ref        text,
  related_event_id          text,
  metadata_json             text,
  occurred_at               text,
  created_at                text,
  archived_at               text,
  CONSTRAINT targeted_site_sales_reward_policy_events_idempotency_key_key UNIQUE (idempotency_key)
);

CREATE TABLE IF NOT EXISTS tenant_custom_hostnames (
  id                  text NOT NULL PRIMARY KEY,
  team_id             text,
  hostname            text,
  status              text,
  verification_token  text,
  verified_at         text,
  created_at          text,
  updated_at          text,
  CONSTRAINT tenant_custom_hostnames_hostname_key UNIQUE (hostname)
);

CREATE TABLE IF NOT EXISTS deployments (
  id                   text NOT NULL PRIMARY KEY,
  user_id              text,
  team_id              text,
  service              text,
  runtime              text,
  primary_backend      text,
  fallback_backend     text,
  repository_provider  text,
  repository_owner     text,
  repository_repo      text,
  repository_ref       text,
  external_deploy_id   text,
  status               text,
  event_cursor         bigint,
  assignment_json      text,
  created_at           text,
  updated_at           text,
  started_at           text,
  completed_at         text,
  failed_at            text,
  canceled_at          text
);

CREATE TABLE IF NOT EXISTS deployment_events (
  id                  text NOT NULL PRIMARY KEY,
  deploy_id           text,
  sequence            bigint,
  type                text,
  summary             text,
  status              text,
  source              text,
  payload_json        text,
  artifact_refs_json  text,
  external_event_id   text,
  created_at          text,
  CONSTRAINT deployment_events_deploy_id_sequence_key UNIQUE (deploy_id, sequence),
  CONSTRAINT deployment_events_deploy_id_external_event_id_key UNIQUE (deploy_id, external_event_id)
);

-- ---------------------------------------------------------------------------
-- Justified indexes (parent-key mirror fan-out + money set-membership verify).
-- Read-serving secondary indexes are deferred to the read cutover (see header).
-- ---------------------------------------------------------------------------

-- Scope B: env-value resolution keys by site (the read-back mirror's site_id
-- fan-out and the eventual per-site env lookup).
CREATE INDEX IF NOT EXISTS site_environment_values_site_idx
  ON site_environment_values (site_id);

-- Scope C money relationships (set-membership verify + the revenue-share and
-- ledger lookups; no cross-store joins — these back single-store key scans).
CREATE INDEX IF NOT EXISTS site_commerce_payment_events_site_created_idx
  ON site_commerce_payment_events (site_id, created_at);
CREATE INDEX IF NOT EXISTS site_commerce_revenue_share_links_payment_event_idx
  ON site_commerce_revenue_share_links (payment_event_id);
CREATE INDEX IF NOT EXISTS site_referral_payout_ledger_source_idx
  ON site_referral_payout_ledger_entries (referral_source_id);
CREATE INDEX IF NOT EXISTS site_referral_payout_ledger_referrer_period_idx
  ON site_referral_payout_ledger_entries (referrer_user_id, period_key);
CREATE INDEX IF NOT EXISTS referral_invites_source_idx
  ON referral_invites (referral_source_id);

-- Scope D targeted prospecting: the campaign board is the one hot read the
-- pipeline drives (prospects and their policy events per campaign).
CREATE INDEX IF NOT EXISTS targeted_site_prospects_campaign_idx
  ON targeted_site_prospects (campaign_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS targeted_site_capture_policy_events_campaign_idx
  ON targeted_site_capture_policy_events (campaign_id, decided_at DESC);

-- Legacy deployments timeline (deployment_events replay by deploy id).
CREATE INDEX IF NOT EXISTS deployment_events_deploy_sequence_idx
  ON deployment_events (deploy_id, sequence);
