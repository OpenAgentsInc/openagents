/**
 * KS-8.14 (#8325): business funnel / orders / referrals domain — the shared
 * table metadata for the D1 → Cloud SQL migration lane.
 *
 * This file is the single source of truth for the domain's column orders,
 * natural keys, and mirror lookup keys, shared by
 *
 *   - the Worker mirroring database
 *     (`apps/openagents.com/workers/api/src/business-domain-store.ts`),
 *     which read-backs the FRESH authoritative D1 row(s) after a scoped
 *     write and converge-upserts the byte-identical copy into Postgres, and
 *   - the backfill + verify core (`./business-backfill.ts`), whose row
 *     hashes must normalize identically on both sides.
 *
 * Every conflict target is CONVERGE (`ON CONFLICT (pk) DO UPDATE SET col =
 * EXCLUDED.col`): mirror and backfill both copy rows D1 has already
 * accepted, so converging to the D1 snapshot value can never re-make a
 * consume-once attribution decision or an idempotency decision — the
 * write-side dedupe (INSERT OR IGNORE on the attribution PKs /
 * `idempotency_key`s / `event_ref`s, plus the D1 starter-credit window-cap
 * trigger) happens on D1, the sole authority. Converge (rather than DO
 * NOTHING) is required because signup fulfillment state, pipeline stages,
 * promise/fulfillment-loop state, buy-mode spend counters, triage records,
 * and attribution policy_state are legitimately UPDATEd in place on D1.
 *
 * Column orders mirror the FINAL D1 schema (after every rewrite/ALTER
 * migration) and khala-sync migration `0023_business_funnel.sql` exactly.
 * The `business_funnel_events_0275` / `business_service_promises_0275`
 * rewrite artifacts were renamed back by worker 0277/0275 and do not exist
 * as live tables (nothing to migrate — the decommission follow-up verifies
 * absence).
 *
 * This module is imported by Worker code, so it uses NO node built-ins
 * (the hash-bearing verify helpers live in ./business-backfill.ts).
 */

import type { SyncSql } from "./sql.js"

export type BusinessDomainTable =
  | "business_signup_requests"
  | "business_signup_fulfillments"
  | "business_signup_referral_attributions"
  | "business_funnel_events"
  | "business_service_promises"
  | "business_fulfillment_motion_receipts"
  | "business_fulfillment_escalation_pages"
  | "business_checkout_kickoffs"
  | "business_commitment_ledger"
  | "business_pipeline_rows"
  | "business_starter_credit_grants"
  | "business_affiliate_codes"
  | "business_affiliate_attributions"
  | "software_orders"
  | "order_triage_records"
  | "order_triage_events"
  | "order_fulfillment_artifacts"
  | "order_fulfillment_feedback"
  | "order_github_write_authority_receipts"
  | "referral_invites"
  | "referral_attributions"
  | "user_referral_attributions"
  | "order_referral_attributions"
  | "agent_referral_attributions"
  | "referral_workflow_events"
  | "viral_agent_funnel_events"
  | "qa_swarm_first_engagements"
  | "promise_transition_receipts"
  | "buy_mode_campaigns"
  | "buy_mode_jobs"
  | "buy_mode_alerts"
  | "customer_one_cohort_rows"

export const BUSINESS_DOMAIN_TABLES: ReadonlyArray<BusinessDomainTable> = [
  "business_signup_requests",
  "business_signup_fulfillments",
  "business_signup_referral_attributions",
  "business_funnel_events",
  "business_service_promises",
  "business_fulfillment_motion_receipts",
  "business_fulfillment_escalation_pages",
  "business_checkout_kickoffs",
  "business_commitment_ledger",
  "business_pipeline_rows",
  "business_starter_credit_grants",
  "business_affiliate_codes",
  "business_affiliate_attributions",
  "software_orders",
  "order_triage_records",
  "order_triage_events",
  "order_fulfillment_artifacts",
  "order_fulfillment_feedback",
  "order_github_write_authority_receipts",
  "referral_invites",
  "referral_attributions",
  "user_referral_attributions",
  "order_referral_attributions",
  "agent_referral_attributions",
  "referral_workflow_events",
  "viral_agent_funnel_events",
  "qa_swarm_first_engagements",
  "promise_transition_receipts",
  "buy_mode_campaigns",
  "buy_mode_jobs",
  "buy_mode_alerts",
  "customer_one_cohort_rows",
]

const BUSINESS_DOMAIN_TABLE_SET: ReadonlySet<string> = new Set(
  BUSINESS_DOMAIN_TABLES,
)

export const isBusinessDomainTable = (
  table: string,
): table is BusinessDomainTable => BUSINESS_DOMAIN_TABLE_SET.has(table)

export type BusinessDomainTableSpec = Readonly<{
  /** Full column list in FINAL D1 schema order (hash + upsert order). */
  columns: ReadonlyArray<string>
  /** Natural key = the converge conflict target (PK on both sides). */
  keyColumns: ReadonlyArray<string>
  /**
   * Columns a live write statement may address the row by, beyond the PK
   * (each is UNIQUE or partial-unique-active on D1). The mirroring
   * database reads rows back by whichever lookup column the statement
   * used; the converge upsert still lands on the true PK.
   */
  lookupColumns: ReadonlyArray<string>
  /** Newest-first ordering column for the verify hash sample. */
  orderColumn: string
}>

export const BUSINESS_DOMAIN_TABLE_SPECS: Readonly<
  Record<BusinessDomainTable, BusinessDomainTableSpec>
> = {
  agent_referral_attributions: {
    columns: [
      "agent_user_id",
      "owner_user_id",
      "referral_attribution_id",
      "referral_source_id",
      "referral_invite_id",
      "capture_path",
      "target",
      "claimed_at",
      "policy_state",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["agent_user_id"],
    lookupColumns: [],
    orderColumn: "created_at",
  },
  business_affiliate_attributions: {
    columns: [
      "attribution_ref",
      "code",
      "source_ref",
      "owner_ref",
      "business_signup_request_id",
      "pipeline_ref",
      "payment_receipt_ref",
      "policy_state",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["attribution_ref"],
    // UNIQUE on D1; the pipeline/payment linkage UPDATEs address rows by it.
    lookupColumns: ["business_signup_request_id"],
    orderColumn: "updated_at",
  },
  business_affiliate_codes: {
    columns: [
      "code",
      "source_ref",
      "owner_ref",
      "issued_by_ref",
      "policy_state",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["code"],
    lookupColumns: [],
    orderColumn: "updated_at",
  },
  business_checkout_kickoffs: {
    columns: [
      "checkout_session_id",
      "business_signup_request_id",
      "user_id",
      "total_amount_cents",
      "setup_fee_cents",
      "credit_grant_cents",
      "workspace_id",
      "service_promise_contract_id",
      "public_receipt_ref",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["checkout_session_id"],
    lookupColumns: [],
    orderColumn: "created_at",
  },
  business_commitment_ledger: {
    columns: [
      "id",
      "commitment_ref",
      "engagement_ref",
      "owner_ref",
      "vertical_ref",
      "promised_object_ref",
      "commitment_kind",
      "due_state",
      "due_at",
      "shipped_at",
      "weekly_review_ref",
      "source_refs_json",
      "blocker_refs_json",
      "evidence_refs_json",
      "created_at",
      "updated_at",
      "pipeline_ref",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "updated_at",
  },
  business_fulfillment_escalation_pages: {
    columns: [
      "id",
      "promise_id",
      "promise_ref",
      "escalation_date",
      "receipt_ref",
      "page_ref",
      "owner_notification_ref",
      "agent_definition_ref",
      "blocking_reason_ref",
      "blocked_at",
      "workspace_ref",
      "stakeholder_refs_json",
      "source_refs_json",
      "created_at",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "created_at",
  },
  business_fulfillment_motion_receipts: {
    columns: [
      "id",
      "promise_id",
      "promise_ref",
      "motion_date",
      "receipt_ref",
      "agent_definition_ref",
      "crm_state_ref",
      "stakeholder_refs_json",
      "stakeholder_flag_refs_json",
      "forward_motion_ref",
      "client_comms_draft_ref",
      "approval_gate_ref",
      "outbound_allowed",
      "blocker_refs_json",
      "source_refs_json",
      "created_at",
      "cadence",
      "client_comms_email_ledger_ref",
      "customer_visible_workroom_update_ref",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "created_at",
  },
  business_funnel_events: {
    columns: [
      "id",
      "event_ref",
      "stage",
      "source_kind",
      "source_ref",
      "occurred_at",
      "observed_at",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "occurred_at",
  },
  business_pipeline_rows: {
    columns: [
      "pipeline_ref",
      "vertical",
      "source_ref",
      "stage",
      "quoted_min_usd_cents",
      "quoted_max_usd_cents",
      "quoted_band_label",
      "owner_role",
      "next_action_due_at",
      "blocker_ref",
      "receipt_refs_json",
      "partner_route_flag",
      "created_at",
      "updated_at",
      "stage_updated_at",
      "business_signup_request_id",
      "partner_route_state",
      "partner_peer_ref",
      "partner_approval_receipt_ref",
      "partner_offer_ref",
      "partner_scope_summary_ref",
      "partner_due_window_ref",
      "partner_budget_range_ref",
      "partner_privacy_tier_ref",
      "partner_route_updated_at",
    ],
    keyColumns: ["pipeline_ref"],
    lookupColumns: [],
    orderColumn: "updated_at",
  },
  business_service_promises: {
    columns: [
      "id",
      "promise_ref",
      "accepted_outcome_contract_id",
      "workspace_ref",
      "crm_state_ref",
      "stakeholder_refs_json",
      "state",
      "cadence",
      "next_motion_due_at",
      "last_motion_receipt_ref",
      "source_refs_json",
      "metadata_json",
      "created_at",
      "updated_at",
      "blocking_reason_ref",
      "blocked_at",
      "last_escalation_page_ref",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "updated_at",
  },
  business_signup_fulfillments: {
    columns: [
      "id",
      "business_signup_request_id",
      "status",
      "reason",
      "enrichment_ref",
      "team_id",
      "project_id",
      "workspace_id",
      "invite_id",
      "email_message_id",
      "email_delivery_status",
      "metadata_json",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["id"],
    // The live writer upserts ON CONFLICT(business_signup_request_id)
    // (UNIQUE): on conflict the SURVIVING row keeps its original id, so
    // the mirror must read back by this column, not the bound id.
    lookupColumns: ["business_signup_request_id"],
    orderColumn: "updated_at",
  },
  business_signup_referral_attributions: {
    columns: [
      "business_signup_request_id",
      "referral_attribution_id",
      "referral_source_id",
      "referral_invite_id",
      "capture_path",
      "target",
      "linked_at",
      "policy_state",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["business_signup_request_id"],
    lookupColumns: [],
    orderColumn: "created_at",
  },
  business_signup_requests: {
    columns: [
      "id",
      "business_name",
      "contact_email",
      "website",
      "phone",
      "help_with",
      "request_slack_channel",
      "slack_connect_status",
      "source_route",
      "created_at",
      "updated_at",
      "referral_code",
      "referral_attribution_id",
      "fulfillment_status",
      "fulfillment_ref",
      "fulfillment_reason",
      "source_ref",
      "linked_pipeline_ref",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "updated_at",
  },
  business_starter_credit_grants: {
    columns: [
      "grant_ref",
      "pipeline_ref",
      "account_ref",
      "engagement_ref",
      "attribution_kind",
      "transfer_policy",
      "amount_usd_cents",
      "amount_msat",
      "amount_cap_usd_cents",
      "window_ref",
      "window_grant_cap",
      "credit_receipt_ref",
      "redemption_receipt_refs_json",
      "source_refs_json",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["grant_ref"],
    lookupColumns: [],
    orderColumn: "updated_at",
  },
  buy_mode_alerts: {
    columns: ["alert_id", "campaign_id", "reason_ref", "created_at"],
    keyColumns: ["alert_id"],
    lookupColumns: [],
    orderColumn: "created_at",
  },
  buy_mode_campaigns: {
    columns: [
      "campaign_id",
      "idempotency_key_hash",
      "state",
      "spend_enabled",
      "per_job_cap_msats",
      "daily_cap_msats",
      "spent_today_msats",
      "day_key",
      "operator_user_id",
      "relay_url",
      "last_alert_ref",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["campaign_id"],
    lookupColumns: [],
    orderColumn: "updated_at",
  },
  buy_mode_jobs: {
    columns: [
      "job_id",
      "campaign_id",
      "idempotency_key_hash",
      "request_event_id",
      "result_event_id",
      "provider_pubkey",
      "amount_msats",
      "state",
      "receipt_ref",
      "bolt11_ref",
      "content_digest_ref",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["job_id"],
    lookupColumns: [],
    orderColumn: "updated_at",
  },
  customer_one_cohort_rows: {
    columns: [
      "team_cohort_ref",
      "state",
      "candidate_ref",
      "invite_ref",
      "vertical_ref",
      "template_ref",
      "workspace_ref",
      "routing_ref",
      "run_ref",
      "artifact_ref",
      "review_ref",
      "verification_ref",
      "completion_bundle_ref",
      "privacy_review_ref",
      "blocker_refs_json",
      "caveat_refs_json",
      "updated_at",
      "created_at",
    ],
    keyColumns: ["team_cohort_ref"],
    lookupColumns: [],
    orderColumn: "updated_at",
  },
  order_fulfillment_artifacts: {
    columns: [
      "id",
      "software_order_id",
      "assignment_id",
      "run_id",
      "kind",
      "title",
      "summary",
      "url",
      "repository_full_name",
      "source_branch",
      "target_branch",
      "commit_sha",
      "status",
      "visibility",
      "metadata_json",
      "created_by_user_id",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "updated_at",
  },
  order_fulfillment_feedback: {
    columns: [
      "id",
      "software_order_id",
      "artifact_id",
      "author_user_id",
      "body",
      "status",
      "source",
      "visibility",
      "adjutant_assignment_id",
      "adjutant_adjustment_id",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "updated_at",
  },
  order_github_write_authority_receipts: {
    columns: [
      "id",
      "software_order_id",
      "assignment_id",
      "user_id",
      "repository_full_name",
      "repository_private",
      "requested_operation",
      "decision",
      "authority_mode",
      "blocked_reason",
      "connection_ref",
      "grant_ref",
      "approval_source",
      "approved_at",
      "customer_message",
      "metadata_json",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "created_at",
  },
  order_referral_attributions: {
    columns: [
      "software_order_id",
      "user_id",
      "referral_attribution_id",
      "referral_source_id",
      "referral_invite_id",
      "capture_path",
      "target",
      "linked_at",
      "policy_state",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["software_order_id"],
    lookupColumns: [],
    orderColumn: "created_at",
  },
  order_triage_events: {
    columns: [
      "id",
      "triage_record_id",
      "software_order_id",
      "site_id",
      "assignment_id",
      "event_type",
      "visibility",
      "summary",
      "actor_user_id",
      "payload_json",
      "created_at",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "created_at",
  },
  order_triage_records: {
    columns: [
      "id",
      "software_order_id",
      "classification",
      "operator_priority",
      "first_batch_eligible",
      "hold_reason",
      "next_action",
      "customer_safe_status",
      "customer_safe_summary",
      "reviewer_user_id",
      "reviewed_at",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    // Partial-unique-active on D1; the operator triage UPDATE addresses
    // the active record by order id.
    lookupColumns: ["software_order_id"],
    orderColumn: "updated_at",
  },
  promise_transition_receipts: {
    columns: [
      "id",
      "promise_id",
      "from_state",
      "to_state",
      "registry_version",
      "result",
      "checks_json",
      "evidence_refs_json",
      "exception_json",
      "checked_at",
      "created_at",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "checked_at",
  },
  qa_swarm_first_engagements: {
    columns: [
      "receipt_ref",
      "idempotency_key",
      "package_kind",
      "payment_path",
      "business_signup_request_id",
      "user_id",
      "committed_amount_cents",
      "intake_receipt_ref",
      "checkout_or_deposit_receipt_ref",
      "target_adapter_review_ref",
      "package_contract_ref",
      "workspace_id",
      "service_promise_contract_id",
      "commitment_ref",
      "first_report_due_at",
      "recorded_at",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["receipt_ref"],
    lookupColumns: [],
    orderColumn: "recorded_at",
  },
  referral_attributions: {
    columns: [
      "id",
      "referral_source_id",
      "referral_invite_id",
      "public_source_ref",
      "public_invite_ref",
      "capture_path",
      "target",
      "policy_state",
      "first_verified_at",
      "claimed_user_id",
      "expires_at",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "created_at",
  },
  referral_invites: {
    columns: [
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
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "created_at",
  },
  referral_workflow_events: {
    columns: [
      "id",
      "idempotency_key",
      "event_kind",
      "referral_attribution_id",
      "referral_source_id",
      "referral_invite_id",
      "public_source_ref",
      "public_invite_ref",
      "software_order_id",
      "site_id",
      "site_version_id",
      "product_id",
      "paid_action_id",
      "payment_event_id",
      "payment_evidence_ref",
      "entitlement_ref",
      "accepted_work_ref",
      "related_event_id",
      "public_receipt_ref",
      "policy_state",
      "amount",
      "asset",
      "metadata_json",
      "occurred_at",
      "created_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "occurred_at",
  },
  software_orders: {
    columns: [
      "id",
      "user_id",
      "status",
      "visibility",
      "request",
      "repository_provider",
      "repository_owner",
      "repository_name",
      "repository_full_name",
      "repository_private",
      "repository_default_branch",
      "repository_html_url",
      "public_work_acknowledged_at",
      "data_use_acknowledged_at",
      "compute_payment_acknowledged_at",
      "provider_account_required",
      "free_slice_cents",
      "quote_cents",
      "current_run_id",
      "agent_started_at",
      "created_at",
      "updated_at",
      "archived_at",
      "agent_idempotency_key",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "updated_at",
  },
  user_referral_attributions: {
    columns: [
      "user_id",
      "referral_attribution_id",
      "referral_source_id",
      "referral_invite_id",
      "capture_path",
      "target",
      "first_verified_at",
      "policy_state",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["user_id"],
    lookupColumns: [],
    orderColumn: "created_at",
  },
  viral_agent_funnel_events: {
    columns: [
      "id",
      "event_kind",
      "route",
      "actor_class",
      "user_agent_class",
      "site_slug",
      "proof_ref",
      "metadata_json",
      "created_at",
    ],
    keyColumns: ["id"],
    lookupColumns: [],
    orderColumn: "created_at",
  },
}

/**
 * Normalize a D1/driver value into the canonical scalar used for both the
 * Postgres bind parameter and the reconciliation row hash. Identical to the
 * KS-8.2/8.7 normalization so the SAME logical row hashes identically
 * whether it came from a wrangler D1 JSON export or a postgres.js read.
 */
export const normalizeBusinessValue = (
  value: unknown,
): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "bigint") return Number(value)
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)`; the
 * structural `SyncSql` seam deliberately does not, so this module widens
 * it locally (shared by the Worker mirror store and the backfill core).
 */
export const requireBusinessUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "business domain store requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/** The converge upsert statement text for one table (shared with tests). */
export const businessConvergeUpsertSql = (
  table: BusinessDomainTable,
  rowCount: number,
): string => {
  const spec = BUSINESS_DOMAIN_TABLE_SPECS[table]
  const columns = spec.columns
  const tuples: Array<string> = []
  for (let row = 0; row < rowCount; row++) {
    const placeholders = columns.map(
      (_, index) => `$${row * columns.length + index + 1}`,
    )
    tuples.push(`(${placeholders.join(", ")})`)
  }
  const setClauses = columns
    .filter((column) => !spec.keyColumns.includes(column))
    .map((column) => `${column} = EXCLUDED.${column}`)
    .join(", ")
  return `INSERT INTO ${table} (${columns.join(", ")}) VALUES ${tuples.join(", ")} ON CONFLICT (${spec.keyColumns.join(", ")}) DO UPDATE SET ${setClauses}`
}

/**
 * Converge one page of D1 rows into `table` as ONE multi-row statement.
 * Returns the page size (every row is inserted-or-converged; a re-run
 * against identical mirror rows is a byte-level no-op).
 */
export const upsertBusinessRows = async (
  sql: SyncSql,
  table: BusinessDomainTable,
  rows: ReadonlyArray<Readonly<Record<string, unknown>>>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireBusinessUnsafe(sql)
  const columns = BUSINESS_DOMAIN_TABLE_SPECS[table].columns
  const params: Array<unknown> = []
  for (const row of rows) {
    for (const column of columns) {
      params.push(normalizeBusinessValue(row[column]))
    }
  }
  await unsafe(businessConvergeUpsertSql(table, rows.length), params)
  return rows.length
}
