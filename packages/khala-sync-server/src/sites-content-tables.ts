/**
 * KS-8.12 (#8323): Sites domain CORE — the SHARED table registry and
 * Postgres converge-upsert core for the fifteen sites content/builder
 * tables in khala-sync migration `0020_sites_core.sql`.
 *
 * ONE source of truth for column lists, primary keys, secondary mirror
 * keys, and conflict semantics, consumed by BOTH sides of the migration
 * machinery (the KS-8.10 sharing rule — a schema change cannot silently
 * drift the two):
 *
 *   - the Worker's dual-write mirror
 *     (`apps/openagents.com/workers/api/src/sites-content-store.ts`)
 *   - the backfill + verify core (`./sites-content-backfill.ts`) and CLI
 *     (`scripts/backfill-sites-content.ts`).
 *
 * This module is imported by Worker code, so it uses NO node built-ins
 * (hashing lives in sites-content-backfill.ts, which only the CLI and
 * tests load).
 *
 * CONFLICT SEMANTICS: every table converges on its PRIMARY KEY
 * (`ON CONFLICT (pk) DO UPDATE SET …` = the D1 snapshot value). D1 never
 * re-issues an id for these tables (the builder INSERT OR IGNORE paths
 * dedupe on UNIQUE(idempotency_key), and the mirror only ever replays
 * rows that EXIST in D1, so a converge on the PK reproduces exactly the
 * D1 row set). Secondary uniques exist on the Postgres twins for parity;
 * a violation there surfaces as a logged dual-write failure, which IS the
 * drift signal.
 *
 * SECONDARY MIRROR KEYS: unlike the forum domain, several sites writes
 * are keyed by a PARENT id, not the row PK — deployment state
 * transitions (`UPDATE site_deployments … WHERE site_id = ?`) and
 * library archival (`UPDATE site_builder_sessions … WHERE site_id = ?`).
 * `SITES_CONTENT_TABLE_MIRROR_KEYS` registers, per table, the columns a
 * read-back mirror may key on; the fan-out per key is bounded (a site's
 * deployments / sessions, a session's satellites).
 */

import type { SyncSql } from "./sql.js"

export type SitesContentTable =
  | "site_projects"
  | "site_versions"
  | "site_deployments"
  | "site_deployment_attempts"
  | "site_access_grants"
  | "site_events"
  | "site_builder_sessions"
  | "site_builder_messages"
  | "site_builder_events"
  | "site_builder_phase_runs"
  | "site_builder_file_snapshots"
  | "site_builder_previews"
  | "site_builder_artifacts"
  | "site_builder_repair_attempts"
  | "site_builder_saved_versions"
  // KS-8.12 REMAINDER (#8357): satellites, secrets, commerce/money,
  // targeted sites, custom hostnames, legacy deployments.
  | "site_build_validations"
  | "site_revision_feedback"
  | "site_compatibility_checks"
  | "site_provisioning_plans"
  | "site_storage_bindings"
  | "site_source_exports"
  | "site_referral_sources"
  | "referral_invites"
  | "site_referral_policy_events"
  | "site_environment_values"
  | "site_commerce_products"
  | "site_commerce_paid_actions"
  | "site_commerce_payment_events"
  | "site_commerce_revenue_share_links"
  | "site_commerce_review_decisions"
  | "site_mdk_checkout_intents"
  | "site_mdk_account_bindings"
  | "site_payment_catalog_items"
  | "site_referral_payout_ledger_entries"
  | "targeted_site_campaigns"
  | "targeted_site_prospects"
  | "targeted_site_capture_policy_events"
  | "targeted_site_static_capture_runs"
  | "targeted_site_rendered_capture_runs"
  | "targeted_site_capture_provider_adapter_runs"
  | "targeted_site_quality_audits"
  | "targeted_site_remake_briefs"
  | "targeted_site_remake_preview_generations"
  | "targeted_site_operator_review_events"
  | "targeted_site_remake_outreach_email_dispatches"
  | "targeted_site_agent_toolkit_grants"
  | "targeted_site_agent_toolkit_actions"
  | "targeted_site_sales_reward_policy_events"
  | "tenant_custom_hostnames"
  | "deployments"
  | "deployment_events"


/**
 * Projects first, then versions/deployments (chain parents), then the
 * satellites, then builder sessions before their satellites, so backfill
 * pages always land parents before children (no FKs, but the verify
 * joins expect it).
 */
export const SITES_CONTENT_TABLES: ReadonlyArray<SitesContentTable> = [
  "site_projects",
  "site_versions",
  "site_deployments",
  "site_deployment_attempts",
  "site_access_grants",
  "site_events",
  "site_builder_sessions",
  "site_builder_messages",
  "site_builder_events",
  "site_builder_phase_runs",
  "site_builder_file_snapshots",
  "site_builder_previews",
  "site_builder_artifacts",
  "site_builder_repair_attempts",
  "site_builder_saved_versions",
]

/**
 * KS-8.12 REMAINDER (#8357): the 36 follow-up tables mirrored by
 * `0025_sites_remainder.sql` — Scope A satellites, Scope B secrets
 * (`site_environment_values`, plain_value EXCLUDED — see
 * `SITES_ENV_VALUES_SECRET_EXCLUDED_COLUMNS`), Scope C commerce/money
 * (mirror-only; KS-8.7/8.8 rails referenced by id, never forked), and
 * Scope D targeted sites / hostnames / legacy deployments. Ordered
 * parents-before-children so paged backfill lands referenced rows first.
 * `targeted_site_campaign_metric_events` is intentionally absent — it is
 * the Analytics-Engine-candidate campaign firehose (MIGRATION_PLAN §3.9)
 * and is not blind-copied into a relational twin.
 */
export const SITES_REMAINDER_TABLES: ReadonlyArray<SitesContentTable> = [
  "site_build_validations",
  "site_revision_feedback",
  "site_compatibility_checks",
  "site_provisioning_plans",
  "site_storage_bindings",
  "site_source_exports",
  "site_referral_sources",
  "referral_invites",
  "site_referral_policy_events",
  "site_environment_values",
  "site_commerce_products",
  "site_commerce_paid_actions",
  "site_commerce_payment_events",
  "site_commerce_revenue_share_links",
  "site_commerce_review_decisions",
  "site_mdk_checkout_intents",
  "site_mdk_account_bindings",
  "site_payment_catalog_items",
  "site_referral_payout_ledger_entries",
  "targeted_site_campaigns",
  "targeted_site_prospects",
  "targeted_site_capture_policy_events",
  "targeted_site_static_capture_runs",
  "targeted_site_rendered_capture_runs",
  "targeted_site_capture_provider_adapter_runs",
  "targeted_site_quality_audits",
  "targeted_site_remake_briefs",
  "targeted_site_remake_preview_generations",
  "targeted_site_operator_review_events",
  "targeted_site_remake_outreach_email_dispatches",
  "targeted_site_agent_toolkit_grants",
  "targeted_site_agent_toolkit_actions",
  "targeted_site_sales_reward_policy_events",
  "tenant_custom_hostnames",
  "deployments",
  "deployment_events",

]

/** Core (0020) + remainder (0024): every sites table behind the seam. */
export const ALL_SITES_CONTENT_TABLES: ReadonlyArray<SitesContentTable> = [
  ...SITES_CONTENT_TABLES,
  ...SITES_REMAINDER_TABLES,
]

/**
 * SPEC invariant 9 (secrets NEVER ride the sync path):
 * `site_environment_values.plain_value` is DELIBERATELY excluded from the
 * mirrored column list below, so the dual-write mirror and the backfill
 * project it away and it never reaches Postgres. Only metadata + the
 * `secret_ref` indirection is mirrored. Same posture as the KS-8.5
 * credential handling.
 */
export const SITES_ENV_VALUES_SECRET_EXCLUDED_COLUMNS: ReadonlyArray<string> = [
  "plain_value",
]

/**
 * Column lists in D1 PHYSICAL order (`SELECT *` order — SQLite ALTER ADD
 * COLUMN appends, so `email_message_id` trails site_events). Row hashes
 * iterate this order on both stores; keep it stable.
 */
export const SITES_CONTENT_TABLE_COLUMNS: Readonly<
  Record<SitesContentTable, ReadonlyArray<string>>
> = {
  site_access_grants: [
    "id",
    "site_id",
    "principal_kind",
    "principal_ref",
    "role",
    "created_at",
    "revoked_at",
  ],
  site_builder_artifacts: [
    "id",
    "idempotency_key",
    "session_id",
    "artifact_kind",
    "artifact_ref",
    "content_hash",
    "byte_size",
    "manifest_ref",
    "metadata_json",
    "created_at",
    "archived_at",
  ],
  site_builder_events: [
    "id",
    "idempotency_key",
    "session_id",
    "sequence",
    "event_kind",
    "phase_kind",
    "visibility",
    "status",
    "title",
    "summary",
    "source_ref",
    "payload_json",
    "created_at",
    "archived_at",
  ],
  site_builder_file_snapshots: [
    "id",
    "idempotency_key",
    "session_id",
    "path",
    "sequence",
    "language",
    "content_hash",
    "byte_size",
    "source_ref",
    "artifact_ref",
    "preview_text",
    "visibility",
    "metadata_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_builder_messages: [
    "id",
    "idempotency_key",
    "session_id",
    "sequence",
    "actor_kind",
    "visibility",
    "body",
    "metadata_json",
    "created_at",
    "archived_at",
  ],
  site_builder_phase_runs: [
    "id",
    "idempotency_key",
    "session_id",
    "sequence",
    "phase_kind",
    "status",
    "title",
    "summary",
    "started_at",
    "completed_at",
    "metadata_json",
    "created_at",
    "archived_at",
  ],
  site_builder_previews: [
    "id",
    "idempotency_key",
    "session_id",
    "preview_kind",
    "status",
    "preview_url",
    "version_ref",
    "artifact_ref",
    "health_ref",
    "metadata_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_builder_repair_attempts: [
    "id",
    "idempotency_key",
    "session_id",
    "preview_id",
    "phase_kind",
    "attempt_number",
    "retry_budget",
    "status",
    "failure_kind",
    "redacted_summary",
    "stop_reason",
    "metadata_json",
    "created_at",
    "completed_at",
    "archived_at",
  ],
  site_builder_saved_versions: [
    "id",
    "idempotency_key",
    "session_id",
    "site_id",
    "site_version_id",
    "preview_id",
    "artifact_ref",
    "build_receipt_ref",
    "source_hash",
    "notes",
    "site_metadata_json",
    "created_at",
    "archived_at",
  ],
  site_builder_sessions: [
    "id",
    "idempotency_key",
    "site_id",
    "order_id",
    "workroom_id",
    "owner_user_id",
    "customer_user_id",
    "created_by_actor_ref",
    "status",
    "prompt_summary",
    "source_site_version_id",
    "source_revision_id",
    "active_preview_id",
    "active_artifact_id",
    "metadata_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_deployment_attempts: [
    "id",
    "site_id",
    "version_id",
    "deployment_id",
    "runtime_kind",
    "runtime_script_name",
    "dispatch_namespace",
    "external_deployment_id",
    "status",
    "upload_receipt_ref",
    "health_status",
    "health_url",
    "health_ref",
    "rollback_ref",
    "observability_ref",
    "metadata_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_deployments: [
    "id",
    "site_id",
    "version_id",
    "slug",
    "url",
    "runtime_kind",
    "runtime_script_name",
    "dispatch_namespace",
    "status",
    "deployed_by_user_id",
    "external_deployment_id",
    "started_at",
    "activated_at",
    "failed_at",
    "disabled_at",
    "rolled_back_at",
    "created_at",
    "updated_at",
  ],
  site_events: [
    "id",
    "site_id",
    "version_id",
    "deployment_id",
    "type",
    "summary",
    "actor_user_id",
    "actor_run_id",
    "payload_json",
    "created_at",
    // Appended by worker migration 0038 (SQLite ALTER appends; D1
    // physical column order kept for SELECT * row-hash parity).
    "email_message_id",
  ],
  site_projects: [
    "id",
    "software_order_id",
    "owner_user_id",
    "team_id",
    "project_id",
    "slug",
    "title",
    "prompt",
    "status",
    "access_mode",
    "visibility",
    "source_repository_provider",
    "source_repository_owner",
    "source_repository_name",
    "source_repository_ref",
    "active_version_id",
    "active_deployment_id",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_versions: [
    "id",
    "site_id",
    "source_kind",
    "source_commit_sha",
    "source_archive_r2_key",
    "artifact_manifest_r2_key",
    "build_log_r2_key",
    "build_status",
    "build_command",
    "worker_module_r2_key",
    "static_assets_manifest_json",
    "d1_binding_name",
    "r2_binding_name",
    "metadata_json",
    "created_by_user_id",
    "created_by_run_id",
    "created_at",
    "saved_at",
    "rejected_at",
  ],
  site_build_validations: [
    "id",
    "site_id",
    "compatibility_check_id",
    "source_kind",
    "source_repository_json",
    "source_commit_sha",
    "source_hash",
    "status",
    "package_manager",
    "requested_build_command",
    "build_command",
    "output_kind",
    "output_path",
    "worker_module_path",
    "manifest_json",
    "bounded_logs_json",
    "log_line_count",
    "log_truncated",
    "findings_json",
    "blockers_json",
    "warnings_json",
    "evidence_refs_json",
    "customer_safe_status",
    "customer_safe_next_action",
    "validated_by_user_id",
    "created_at",
    "archived_at",
  ],
  site_revision_feedback: [
    "id",
    "software_order_id",
    "site_id",
    "site_version_id",
    "site_deployment_id",
    "author_user_id",
    "body",
    "status",
    "source",
    "visibility",
    "created_at",
    "updated_at",
    "archived_at",
    "adjutant_assignment_id",
    "adjutant_adjustment_id",
  ],
  site_compatibility_checks: [
    "id",
    "site_id",
    "source_kind",
    "source_repository_json",
    "status",
    "confidence",
    "package_manager",
    "build_command",
    "output_kind",
    "output_path",
    "worker_module_path",
    "needs_d1",
    "needs_r2",
    "needs_workspace_auth",
    "needs_public_auth",
    "env_keys_json",
    "findings_json",
    "blockers_json",
    "warnings_json",
    "evidence_refs_json",
    "customer_safe_status",
    "customer_safe_next_action",
    "checked_by_user_id",
    "created_at",
    "archived_at",
  ],
  site_provisioning_plans: [
    "id",
    "idempotency_key",
    "site_id",
    "status",
    "requested_by_user_id",
    "reviewed_by_user_id",
    "resource_manifest_json",
    "receipt_json",
    "created_at",
    "reviewed_at",
    "updated_at",
    "archived_at",
  ],
  site_storage_bindings: [
    "id",
    "site_id",
    "kind",
    "binding_name",
    "cloudflare_resource_ref",
    "scope",
    "created_at",
    "updated_at",
  ],
  site_source_exports: [
    "id",
    "idempotency_key",
    "site_id",
    "version_id",
    "status",
    "export_kind",
    "actor_user_id",
    "approved_by_user_id",
    "destination_provider",
    "destination_owner",
    "destination_repository",
    "destination_branch",
    "destination_pull_request_url",
    "destination_url",
    "source_archive_r2_key",
    "artifact_manifest_r2_key",
    "worker_module_r2_key",
    "source_artifact_ref",
    "token_ref",
    "token_hash",
    "token_expires_at",
    "secret_scan_status",
    "secret_scan_ref",
    "receipt_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_referral_sources: [
    "id",
    "site_id",
    "site_version_id",
    "referrer_user_id",
    "public_source_ref",
    "public_slug",
    "campaign_ref",
    "source_label",
    "policy_state",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  referral_invites: [
    "id",
    "referral_source_id",
    "public_invite_ref",
    "token_hash",
    "scope",
    "audience_path",
    "policy_state",
    "expires_at",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_referral_policy_events: [
    "id",
    "idempotency_key",
    "subject_kind",
    "subject_ref",
    "referral_attribution_id",
    "referral_source_id",
    "referral_invite_id",
    "referral_workflow_event_id",
    "software_order_id",
    "site_id",
    "previous_state",
    "decision_state",
    "policy_reason",
    "eligibility",
    "customer_status",
    "operator_actor_user_id",
    "operator_note_ref",
    "metadata_json",
    "decided_at",
    "created_at",
    "archived_at",
  ],
  site_environment_values: [
    "id",
    "site_id",
    "key",
    "kind",
    "secret_ref",
    "created_at",
    "updated_at",
    "deleted_at",
  ],
  site_commerce_products: [
    "id",
    "site_id",
    "site_version_id",
    "product_key",
    "name",
    "asset",
    "amount",
    "checkout_path",
    "entitlement_scope",
    "agent_readable",
    "settlement_mode",
    "customer_data_requirements_json",
    "public_projection_state",
    "created_by_user_id",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_commerce_paid_actions: [
    "id",
    "site_id",
    "site_version_id",
    "action_key",
    "name",
    "method",
    "path",
    "asset",
    "amount",
    "checkout_path",
    "entitlement_scope",
    "agent_readable",
    "settlement_mode",
    "customer_data_requirements_json",
    "public_projection_state",
    "created_by_user_id",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_commerce_payment_events: [
    "id",
    "site_id",
    "site_version_id",
    "software_order_id",
    "product_id",
    "paid_action_id",
    "customer_ref",
    "referral_source_ref",
    "payment_evidence_ref",
    "entitlement_ref",
    "public_receipt_ref",
    "event_kind",
    "amount",
    "asset",
    "created_at",
  ],
  site_commerce_revenue_share_links: [
    "id",
    "payment_event_id",
    "accepted_work_ref",
    "requested_contributor_asset",
    "provider_payout_claimed",
    "nexus_receipt_ref",
    "treasury_receipt_ref",
    "ldk_settlement_receipt_ref",
    "referral_reward_trigger",
    "provider_payout_eligibility_state",
    "withdrawal_posture",
    "projection_json",
    "created_at",
  ],
  site_commerce_review_decisions: [
    "id",
    "decision_ref",
    "idempotency_key_hash",
    "site_id",
    "site_version_id",
    "catalog_ref",
    "review_status",
    "reason_refs_json",
    "customer_input_requirement_refs_json",
    "actor_ref",
    "public_projection_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_mdk_checkout_intents: [
    "id",
    "checkout_intent_ref",
    "idempotency_key_hash",
    "site_id",
    "site_version_id",
    "catalog_ref",
    "product_id",
    "challenge_ref",
    "checkout_ref",
    "checkout_url_ref",
    "checkout_launch_path",
    "provider_ref",
    "status",
    "environment",
    "sandbox",
    "amount_asset",
    "amount_denomination",
    "amount_minor_units",
    "success_return_path",
    "cancel_return_path",
    "metadata_refs_json",
    "hosted_checkout_projection_json",
    "public_projection_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_mdk_account_bindings: [
    "id",
    "binding_ref",
    "idempotency_key_hash",
    "site_id",
    "site_version_id",
    "customer_ref",
    "order_ref",
    "requested_provider_mode",
    "environment",
    "review_status",
    "secret_binding_refs_json",
    "allowed_catalog_refs_json",
    "allowed_product_refs_json",
    "allowed_action_refs_json",
    "reviewer_refs_json",
    "caveat_refs_json",
    "public_projection_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_payment_catalog_items: [
    "id",
    "catalog_ref",
    "item_kind",
    "site_id",
    "site_version_id",
    "deployment_id",
    "order_ref",
    "workroom_ref",
    "manifest_ref",
    "source_manifest_digest",
    "product_id",
    "action_id",
    "action_ref",
    "method",
    "path",
    "display_ref",
    "checkout_path",
    "price_asset",
    "price_denomination",
    "price_amount_minor_units",
    "entitlement_scope",
    "settlement_mode",
    "public_projection_state",
    "sandbox",
    "agent_readable",
    "status",
    "metadata_refs_json",
    "customer_data_requirements_json",
    "paid_endpoint_product_json",
    "public_projection_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  site_referral_payout_ledger_entries: [
    "id",
    "payout_ref",
    "idempotency_key",
    "referral_attribution_id",
    "referral_source_id",
    "referral_invite_id",
    "referrer_user_id",
    "referred_user_id",
    "qualifying_event_ref",
    "qualifying_event_kind",
    "qualifying_amount_sats",
    "amount_sats",
    "period_key",
    "state",
    "state_reason_ref",
    "previous_entry_id",
    "reversal_of_entry_id",
    "evidence_refs_json",
    "policy_refs_json",
    "caveat_refs_json",
    "created_at",
    "archived_at",
  ],
  targeted_site_campaigns: [
    "id",
    "slug",
    "name",
    "owner_user_id",
    "operator_user_id",
    "vertical",
    "geography",
    "source_authority_ref",
    "budget_cap_ref",
    "suppression_policy_ref",
    "operator_state",
    "metadata_json",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  targeted_site_prospects: [
    "id",
    "campaign_id",
    "idempotency_key",
    "normalized_domain",
    "origin_url",
    "company_name",
    "site_name",
    "contact_refs_json",
    "vertical",
    "geography",
    "source_ref",
    "discovery_confidence",
    "suppression_state",
    "capture_state",
    "review_state",
    "metadata_json",
    "discovered_at",
    "created_at",
    "updated_at",
    "archived_at",
  ],
  targeted_site_capture_policy_events: [
    "id",
    "idempotency_key",
    "campaign_id",
    "prospect_id",
    "normalized_domain",
    "source_ref",
    "decision",
    "fetchable",
    "reason",
    "robots_ref",
    "sitemap_ref",
    "suppression_ref",
    "customer_authority_ref",
    "paid_escalation_ref",
    "operator_actor_user_id",
    "operator_note_ref",
    "metadata_json",
    "decided_at",
    "created_at",
    "archived_at",
  ],
  targeted_site_static_capture_runs: [
    "id",
    "idempotency_key",
    "campaign_id",
    "prospect_id",
    "normalized_domain",
    "capture_policy_event_id",
    "state",
    "reason",
    "homepage_url",
    "homepage_ref",
    "robots_ref",
    "sitemap_ref",
    "source_pack_ref",
    "source_hash",
    "page_refs_json",
    "asset_refs_json",
    "response_summary_json",
    "metadata_json",
    "started_at",
    "completed_at",
    "created_at",
    "archived_at",
  ],
  targeted_site_rendered_capture_runs: [
    "id",
    "idempotency_key",
    "campaign_id",
    "prospect_id",
    "normalized_domain",
    "capture_policy_event_id",
    "static_capture_run_id",
    "state",
    "reason",
    "target_url",
    "provider_ref",
    "screenshot_ref",
    "rendered_html_ref",
    "markdown_ref",
    "links_ref",
    "structured_json_ref",
    "crawl_ref",
    "viewport_ref",
    "device_ref",
    "usage_summary_json",
    "metadata_json",
    "started_at",
    "completed_at",
    "created_at",
    "archived_at",
  ],
  targeted_site_capture_provider_adapter_runs: [
    "id",
    "idempotency_key",
    "campaign_id",
    "prospect_id",
    "normalized_domain",
    "capture_policy_event_id",
    "static_capture_run_id",
    "rendered_capture_run_id",
    "provider_kind",
    "state",
    "reason",
    "paid_escalation_ref",
    "provider_request_ref",
    "provider_receipt_ref",
    "output_pack_ref",
    "usage_ref",
    "cost_ref",
    "metadata_json",
    "requested_at",
    "completed_at",
    "created_at",
    "archived_at",
  ],
  targeted_site_quality_audits: [
    "id",
    "idempotency_key",
    "campaign_id",
    "prospect_id",
    "normalized_domain",
    "static_capture_run_id",
    "rendered_capture_run_id",
    "provider_adapter_run_id",
    "state",
    "recommendation",
    "overall_score",
    "legal_sensitive",
    "dimensions_json",
    "evidence_refs_json",
    "metadata_json",
    "audited_at",
    "created_at",
    "archived_at",
  ],
  targeted_site_remake_briefs: [
    "id",
    "idempotency_key",
    "campaign_id",
    "prospect_id",
    "normalized_domain",
    "quality_audit_id",
    "static_capture_run_id",
    "rendered_capture_run_id",
    "provider_adapter_run_id",
    "state",
    "legal_sensitive",
    "source_authority_pack_json",
    "audit_finding_refs_json",
    "original_screenshot_refs_json",
    "copied_text_refs_json",
    "copied_image_refs_json",
    "generation_constraints_json",
    "metadata_json",
    "prepared_at",
    "reviewed_at",
    "created_at",
    "archived_at",
  ],
  targeted_site_remake_preview_generations: [
    "id",
    "idempotency_key",
    "campaign_id",
    "prospect_id",
    "normalized_domain",
    "remake_brief_id",
    "quality_audit_id",
    "static_capture_run_id",
    "rendered_capture_run_id",
    "provider_adapter_run_id",
    "state",
    "preview_url",
    "concept_slug",
    "source_authority_pack_ref",
    "generated_artifact_ref",
    "generated_source_ref",
    "candidate_site_project_ref",
    "candidate_site_version_ref",
    "generation_receipt_ref",
    "failure_ref",
    "legal_sensitive",
    "generation_constraints_json",
    "metadata_json",
    "requested_at",
    "completed_at",
    "created_at",
    "archived_at",
  ],
  targeted_site_operator_review_events: [
    "id",
    "idempotency_key",
    "campaign_id",
    "prospect_id",
    "normalized_domain",
    "remake_brief_id",
    "preview_generation_id",
    "decision",
    "previous_state",
    "next_state",
    "operator_actor_user_id",
    "operator_note_ref",
    "outreach_draft_ref",
    "meeting_cta_ref",
    "suppression_state",
    "evidence_refs_json",
    "metadata_json",
    "decided_at",
    "created_at",
    "archived_at",
  ],
  targeted_site_remake_outreach_email_dispatches: [
    "id",
    "idempotency_key",
    "campaign_id",
    "prospect_id",
    "normalized_domain",
    "preview_generation_id",
    "operator_review_event_id",
    "email_message_id",
    "recipient_ref",
    "template_slug",
    "suppression_state",
    "dispatch_state",
    "error_name",
    "error_message",
    "metadata_json",
    "dispatched_at",
    "created_at",
    "archived_at",
  ],
  targeted_site_agent_toolkit_grants: [
    "id",
    "idempotency_key",
    "campaign_id",
    "owner_user_id",
    "agent_ref",
    "scopes_json",
    "dry_run_default",
    "spend_cap_cents",
    "daily_send_cap",
    "suppression_policy_ref",
    "approval_policy",
    "status",
    "metadata_json",
    "created_at",
    "updated_at",
    "expires_at",
    "revoked_at",
    "archived_at",
  ],
  targeted_site_agent_toolkit_actions: [
    "id",
    "idempotency_key",
    "grant_id",
    "campaign_id",
    "agent_ref",
    "action_kind",
    "dry_run",
    "requested_cost_cents",
    "requested_send_count",
    "suppression_state",
    "approval_state",
    "result_state",
    "receipt_ref",
    "reason",
    "metadata_json",
    "created_at",
    "archived_at",
  ],
  targeted_site_sales_reward_policy_events: [
    "id",
    "idempotency_key",
    "campaign_id",
    "agent_ref",
    "prospect_id",
    "outcome_kind",
    "policy_state",
    "reward_asset",
    "reward_amount",
    "buyer_payment_ref",
    "referral_attribution_ref",
    "accepted_work_ref",
    "payout_intent_ref",
    "settlement_caveat_ref",
    "dispute_ref",
    "public_receipt_ref",
    "related_event_id",
    "metadata_json",
    "occurred_at",
    "created_at",
    "archived_at",
  ],
  tenant_custom_hostnames: [
    "id",
    "team_id",
    "hostname",
    "status",
    "verification_token",
    "verified_at",
    "created_at",
    "updated_at",
  ],
  deployments: [
    "id",
    "user_id",
    "team_id",
    "service",
    "runtime",
    "primary_backend",
    "fallback_backend",
    "repository_provider",
    "repository_owner",
    "repository_repo",
    "repository_ref",
    "external_deploy_id",
    "status",
    "event_cursor",
    "assignment_json",
    "created_at",
    "updated_at",
    "started_at",
    "completed_at",
    "failed_at",
    "canceled_at",
  ],
  deployment_events: [
    "id",
    "deploy_id",
    "sequence",
    "type",
    "summary",
    "status",
    "source",
    "payload_json",
    "artifact_refs_json",
    "external_event_id",
    "created_at",
  ],

}

/**
 * Primary-key column per table — the converge-upsert arbiter. Every
 * scoped table keys on `id`, and D1 never replaces a row's id, so the PK
 * is the stable arbiter.
 */
export const SITES_CONTENT_TABLE_PK: Readonly<
  Record<SitesContentTable, string>
> = {
  site_access_grants: "id",
  site_builder_artifacts: "id",
  site_builder_events: "id",
  site_builder_file_snapshots: "id",
  site_builder_messages: "id",
  site_builder_phase_runs: "id",
  site_builder_previews: "id",
  site_builder_repair_attempts: "id",
  site_builder_saved_versions: "id",
  site_builder_sessions: "id",
  site_deployment_attempts: "id",
  site_deployments: "id",
  site_events: "id",
  site_projects: "id",
  site_versions: "id",
  site_build_validations: "id",
  site_revision_feedback: "id",
  site_compatibility_checks: "id",
  site_provisioning_plans: "id",
  site_storage_bindings: "id",
  site_source_exports: "id",
  site_referral_sources: "id",
  referral_invites: "id",
  site_referral_policy_events: "id",
  site_environment_values: "id",
  site_commerce_products: "id",
  site_commerce_paid_actions: "id",
  site_commerce_payment_events: "id",
  site_commerce_revenue_share_links: "id",
  site_commerce_review_decisions: "id",
  site_mdk_checkout_intents: "id",
  site_mdk_account_bindings: "id",
  site_payment_catalog_items: "id",
  site_referral_payout_ledger_entries: "id",
  targeted_site_campaigns: "id",
  targeted_site_prospects: "id",
  targeted_site_capture_policy_events: "id",
  targeted_site_static_capture_runs: "id",
  targeted_site_rendered_capture_runs: "id",
  targeted_site_capture_provider_adapter_runs: "id",
  targeted_site_quality_audits: "id",
  targeted_site_remake_briefs: "id",
  targeted_site_remake_preview_generations: "id",
  targeted_site_operator_review_events: "id",
  targeted_site_remake_outreach_email_dispatches: "id",
  targeted_site_agent_toolkit_grants: "id",
  targeted_site_agent_toolkit_actions: "id",
  targeted_site_sales_reward_policy_events: "id",
  tenant_custom_hostnames: "id",
  deployments: "id",
  deployment_events: "id",

}

/**
 * Columns (besides the PK) a read-back mirror may key on when a scoped
 * UPDATE has no PK equality in its WHERE clause — the deployment
 * rollback/disable transitions and library archival key on `site_id`;
 * builder satellites could in principle transition by `session_id`. The
 * fan-out per key value is bounded (one site's deployments/sessions, one
 * session's satellites), so a keyed read-back stays a small page.
 */
export const SITES_CONTENT_TABLE_MIRROR_KEYS: Readonly<
  Record<SitesContentTable, ReadonlyArray<string>>
> = {
  site_access_grants: ["site_id"],
  site_builder_artifacts: ["session_id"],
  site_builder_events: ["session_id"],
  site_builder_file_snapshots: ["session_id"],
  site_builder_messages: ["session_id"],
  site_builder_phase_runs: ["session_id"],
  site_builder_previews: ["session_id"],
  site_builder_repair_attempts: ["session_id"],
  site_builder_saved_versions: ["session_id", "site_id"],
  site_builder_sessions: ["site_id"],
  site_deployment_attempts: ["site_id"],
  site_deployments: ["site_id"],
  site_events: ["site_id"],
  site_projects: [],
  site_versions: ["site_id"],
  site_build_validations: ["site_id"],
  site_revision_feedback: ["site_id", "software_order_id"],
  site_compatibility_checks: ["site_id"],
  site_provisioning_plans: ["site_id"],
  site_storage_bindings: ["site_id"],
  site_source_exports: ["site_id"],
  site_referral_sources: ["site_id"],
  referral_invites: ["referral_source_id"],
  site_referral_policy_events: ["referral_source_id", "site_id"],
  site_environment_values: ["site_id"],
  site_commerce_products: ["site_id"],
  site_commerce_paid_actions: ["site_id"],
  site_commerce_payment_events: ["site_id"],
  site_commerce_revenue_share_links: ["payment_event_id"],
  site_commerce_review_decisions: ["site_id"],
  site_mdk_checkout_intents: ["site_id"],
  site_mdk_account_bindings: ["site_id"],
  site_payment_catalog_items: ["site_id"],
  site_referral_payout_ledger_entries: ["referral_source_id"],
  targeted_site_campaigns: ["owner_user_id"],
  targeted_site_prospects: ["campaign_id"],
  targeted_site_capture_policy_events: ["campaign_id", "prospect_id"],
  targeted_site_static_capture_runs: ["campaign_id", "prospect_id"],
  targeted_site_rendered_capture_runs: ["campaign_id", "prospect_id"],
  targeted_site_capture_provider_adapter_runs: ["campaign_id", "prospect_id"],
  targeted_site_quality_audits: ["campaign_id", "prospect_id"],
  targeted_site_remake_briefs: ["campaign_id", "prospect_id"],
  targeted_site_remake_preview_generations: ["campaign_id", "prospect_id"],
  targeted_site_operator_review_events: ["campaign_id", "prospect_id"],
  targeted_site_remake_outreach_email_dispatches: ["campaign_id", "prospect_id"],
  targeted_site_agent_toolkit_grants: ["campaign_id"],
  targeted_site_agent_toolkit_actions: ["grant_id", "campaign_id"],
  targeted_site_sales_reward_policy_events: ["campaign_id", "prospect_id"],
  tenant_custom_hostnames: ["team_id"],
  deployments: ["user_id"],
  deployment_events: ["deploy_id"],

}

export const isSitesContentTable = (
  value: string,
): value is SitesContentTable =>
  Object.prototype.hasOwnProperty.call(SITES_CONTENT_TABLE_PK, value)

export type SitesContentRow = Readonly<Record<string, unknown>>

/**
 * D1 → Postgres value normalization: D1/SQLite hands back TEXT / INTEGER /
 * REAL / NULL; booleans arrive as 0/1 already. Keep bytes identical so
 * row-hash reconciliation compares equal.
 */
export const normalizeSitesContentValue = (
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
 * it locally (same note as the pylon/token/agent-runtime/forum
 * backfills). Both runtime drivers here run with `prepare: false`, so
 * `unsafe` uses the unnamed statement and is transaction-pooler/
 * Hyperdrive-safe.
 */
export type SitesContentUnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

export const requireSitesContentUnsafe = (
  sql: SyncSql,
): SitesContentUnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: SitesContentUnsafeQuery })
    .unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "sites content store requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
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
export const upsertSitesContentRows = async (
  sql: SyncSql,
  table: SitesContentTable,
  rows: ReadonlyArray<SitesContentRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireSitesContentUnsafe(sql)
  const columns = SITES_CONTENT_TABLE_COLUMNS[table]
  const pk = SITES_CONTENT_TABLE_PK[table]
  const setClauses = columns
    .filter((column) => column !== pk)
    .map((column) => `${column} = EXCLUDED.${column}`)
    .join(", ")

  let touched = 0
  for (const row of rows) {
    const values = columns.map((column) =>
      normalizeSitesContentValue(row[column]),
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
