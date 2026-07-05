import type { SyncSql, SyncTransactionSql } from "./sql.js"

/**
 * KS-8.17 (#8328): supervision long tail (Adjutant / Omni / Autopilot / ops).
 *
 * Shared table metadata and Postgres converge/upsert helpers for the 29
 * `adjutant_*` (10), `omni_*` (9), `autopilot_*` (6), `relay_health_*` (2),
 * `backend_incident_events` (1), and `hygiene_debt_receipts` (1) tables
 * moving from D1 to Cloud SQL (khala-sync migration
 * `0024_supervision_longtail.sql`, mirroring worker migrations
 * 0010/0013/0019/0034-0040/0053-0055/0038/0091-0099/0140-0147/0171/0172/
 * 0178/0207/0224/0249/0258/0273). Imported by both the Worker dual-write
 * mirror
 * (`apps/openagents.com/workers/api/src/supervision-longtail-domain-store.ts`)
 * and the backfill/verify CLI (`scripts/backfill-supervision-longtail.ts`) —
 * ONE registry, so mirror and backfill can never fight: both write identical
 * converge upserts keyed on the table's (composite) primary key.
 *
 * SECRETS (SPEC invariant 9): every column in this family is a public-safe
 * ref, path, digest, count, or JSON of the same — no raw prompts, diffs,
 * transcripts, credentials, or wallet material. `autopilot_onboarding_sessions`
 * (`transcript_json`) and `omni_*` (`metadata_json`, entries) may carry
 * operator/customer content, so diagnostics and backfill/verify output print
 * row KEYS and sha256 row hashes ONLY — never a JSON value. Those columns are
 * declared in `custodyColumns` so callers keep them out of any human-facing
 * output.
 *
 * Runtime-neutral on purpose: no Node built-ins (the Worker imports this).
 */

export type SupervisionLongtailTable =
  // adjutant_* (10)
  | "adjutant_assignments"
  | "adjutant_assignment_events"
  | "adjutant_adjustment_requests"
  | "adjutant_public_source_refs"
  | "adjutant_usage_receipts"
  | "adjutant_research_briefs"
  | "adjutant_assignment_research_policies"
  | "adjutant_enrichment_jobs"
  | "adjutant_task_packet_freshness"
  | "adjutant_assignment_enrichments"
  // omni_* (9)
  | "omni_accepted_outcome_contracts"
  | "omni_workrooms"
  | "omni_evidence_bundles"
  | "omni_workroom_lifecycle_decisions"
  | "omni_accepted_outcome_economics"
  | "omni_route_scorecards"
  | "omni_public_proof_bundles"
  | "omni_market_memory_hooks"
  | "omni_idempotency_keys"
  // autopilot_* (6)
  | "autopilot_token_usage"
  | "autopilot_work_orders"
  | "autopilot_decision_closeout_receipts"
  | "autopilot_continuation_policies"
  | "autopilot_continuation_events"
  | "autopilot_onboarding_sessions"
  // ops (4)
  | "relay_health_probes"
  | "relay_health_transitions"
  | "backend_incident_events"
  | "hygiene_debt_receipts"

export type SupervisionLongtailTableSpec = Readonly<{
  columns: ReadonlyArray<string>
  /**
   * The table's (composite) PRIMARY KEY — the converge key used by the
   * backfill, the mirror read-back, and ON CONFLICT.
   */
  keyColumns: ReadonlyArray<string>
  /** Newest-first ordering column for hash verification. */
  orderColumn: string
  /**
   * Columns that may carry operator/customer free-text or nested content.
   * Values from these columns must never appear in diagnostics, logs, or
   * backfill/verify output — keys and sha256 row hashes only. (They still
   * participate in the row hash — a hash, never the value.)
   */
  custodyColumns?: ReadonlyArray<string>
}>

export type SupervisionLongtailRow = Readonly<Record<string, unknown>>

/** Backfill/verify sweep order (parents-before-children is cosmetic — there
 * are no FKs on the Postgres side — but keeps output readable). */
export const SUPERVISION_LONGTAIL_TABLES: ReadonlyArray<SupervisionLongtailTable> =
  [
    "adjutant_assignments",
    "adjutant_assignment_events",
    "adjutant_adjustment_requests",
    "adjutant_public_source_refs",
    "adjutant_usage_receipts",
    "adjutant_research_briefs",
    "adjutant_assignment_research_policies",
    "adjutant_enrichment_jobs",
    "adjutant_task_packet_freshness",
    "adjutant_assignment_enrichments",
    "omni_accepted_outcome_contracts",
    "omni_workrooms",
    "omni_evidence_bundles",
    "omni_workroom_lifecycle_decisions",
    "omni_accepted_outcome_economics",
    "omni_route_scorecards",
    "omni_public_proof_bundles",
    "omni_market_memory_hooks",
    "omni_idempotency_keys",
    "autopilot_token_usage",
    "autopilot_work_orders",
    "autopilot_decision_closeout_receipts",
    "autopilot_continuation_policies",
    "autopilot_continuation_events",
    "autopilot_onboarding_sessions",
    "relay_health_probes",
    "relay_health_transitions",
    "backend_incident_events",
    "hygiene_debt_receipts",
  ]

export const SUPERVISION_LONGTAIL_TABLE_SPECS: Readonly<
  Record<SupervisionLongtailTable, SupervisionLongtailTableSpec>
> = {
  // -------------------------------------------------------------------------
  // adjutant_* (10)
  // -------------------------------------------------------------------------
  adjutant_assignments: {
    columns: [
      "id",
      "software_order_id",
      "site_id",
      "goal_id",
      "current_run_id",
      "team_id",
      "project_id",
      "agent_id",
      "assigned_by_user_id",
      "assignment_kind",
      "status",
      "visibility",
      "task_spec_path",
      "commit_sha",
      "objective",
      "created_at",
      "updated_at",
      "completed_at",
      "blocked_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  adjutant_assignment_events: {
    columns: [
      "id",
      "assignment_id",
      "software_order_id",
      "site_id",
      "goal_id",
      "run_id",
      "event_type",
      "visibility",
      "summary",
      "actor_user_id",
      "payload_json",
      "created_at",
      "email_message_id",
    ],
    custodyColumns: ["payload_json"],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  adjutant_adjustment_requests: {
    columns: [
      "id",
      "assignment_id",
      "software_order_id",
      "site_id",
      "goal_id",
      "requested_by_user_id",
      "instruction",
      "status",
      "continuation_mode",
      "source_run_id",
      "continuation_run_id",
      "resulting_version_id",
      "visibility",
      "created_at",
      "updated_at",
      "completed_at",
      "archived_at",
    ],
    custodyColumns: ["instruction"],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  adjutant_public_source_refs: {
    columns: [
      "id",
      "assignment_id",
      "software_order_id",
      "site_id",
      "kind",
      "status",
      "url",
      "normalized_domain",
      "label",
      "public_safe",
      "proposed_by_user_id",
      "reviewed_by_user_id",
      "review_reason",
      "approved_at",
      "rejected_at",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  adjutant_usage_receipts: {
    columns: [
      "id",
      "assignment_id",
      "software_order_id",
      "site_id",
      "adjustment_id",
      "run_id",
      "category",
      "visibility",
      "billing_mode",
      "summary",
      "quantity",
      "unit",
      "credits_charged_cents",
      "currency",
      "billing_ledger_entry_id",
      "public_receipt_json",
      "team_receipt_json",
      "idempotency_key",
      "created_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  adjutant_research_briefs: {
    columns: [
      "id",
      "assignment_id",
      "enrichment_run_id",
      "status",
      "summary",
      "grounded_facts_json",
      "suggested_sections_json",
      "unknowns_json",
      "claims_needing_review_json",
      "source_cards_json",
      "created_by_user_id",
      "reviewed_by_user_id",
      "review_reason",
      "approved_at",
      "rejected_at",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    custodyColumns: [
      "summary",
      "grounded_facts_json",
      "suggested_sections_json",
      "unknowns_json",
      "claims_needing_review_json",
      "source_cards_json",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  adjutant_assignment_research_policies: {
    columns: [
      "assignment_id",
      "policy_mode",
      "reason",
      "customer_safe_summary",
      "actor_user_id",
      "source_authority_ref",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["assignment_id"],
    orderColumn: "updated_at",
  },
  adjutant_enrichment_jobs: {
    columns: [
      "id",
      "assignment_id",
      "enrichment_run_id",
      "status",
      "trigger_kind",
      "refresh",
      "requested_by_user_id",
      "request_json",
      "error_code",
      "error_summary",
      "started_at",
      "completed_at",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    custodyColumns: ["request_json", "error_summary"],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  adjutant_task_packet_freshness: {
    columns: [
      "assignment_id",
      "task_spec_path",
      "commit_sha",
      "status",
      "research_brief_id",
      "research_brief_approved_at",
      "source_card_count",
      "operator_keep_reason",
      "customer_safe_summary",
      "actor_user_id",
      "stale_at",
      "kept_at",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["assignment_id"],
    orderColumn: "updated_at",
  },
  adjutant_assignment_enrichments: {
    columns: [
      "assignment_id",
      "enrichment_run_id",
      "research_brief_id",
      "status",
      "required_for_launch",
      "approved_at",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["assignment_id", "enrichment_run_id"],
    orderColumn: "updated_at",
  },
  // -------------------------------------------------------------------------
  // omni_* (9)
  // -------------------------------------------------------------------------
  omni_accepted_outcome_contracts: {
    columns: [
      "id",
      "idempotency_key",
      "work_kind",
      "subject_ref",
      "customer_ref",
      "expected_artifacts_json",
      "review_policy",
      "acceptance_state",
      "proof_policy",
      "economic_state",
      "closeout_requirements_json",
      "legal_sensitive",
      "public_receipt_ref",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
      "committed_deliverables_json",
      "service_promise_state",
      "sla_terms_json",
      "fulfillment_receipts_json",
    ],
    custodyColumns: ["metadata_json"],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  omni_workrooms: {
    columns: [
      "id",
      "idempotency_key",
      "software_order_id",
      "accepted_outcome_contract_id",
      "site_id",
      "assignment_id",
      "work_kind",
      "status",
      "visibility",
      "customer_intent_ref",
      "task_packet_ref",
      "source_refs_json",
      "artifact_refs_json",
      "email_refs_json",
      "receipt_refs_json",
      "blocker_refs_json",
      "public_receipt_ref",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
      "data_classification",
      "trust_tier",
      "classification_caveat_ref",
    ],
    custodyColumns: ["metadata_json"],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  omni_evidence_bundles: {
    columns: [
      "id",
      "idempotency_key",
      "workroom_id",
      "work_kind",
      "status",
      "legal_sensitive",
      "summary_ref",
      "source_authority_caveat_ref",
      "entries_json",
      "public_receipt_ref",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    custodyColumns: ["entries_json", "metadata_json"],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  omni_workroom_lifecycle_decisions: {
    columns: [
      "id",
      "idempotency_key",
      "workroom_id",
      "work_kind",
      "actor_kind",
      "decision_kind",
      "resulting_state",
      "customer_safe_explanation_ref",
      "receipt_ref",
      "site_revision_feedback_ref",
      "followup_request_ref",
      "artifact_ref",
      "no_settlement_implication",
      "metadata_json",
      "created_at",
      "archived_at",
    ],
    custodyColumns: ["metadata_json"],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  omni_accepted_outcome_economics: {
    columns: [
      "id",
      "idempotency_key",
      "workroom_id",
      "accepted_outcome_contract_id",
      "work_kind",
      "funding_mode",
      "buyer_price_asset",
      "buyer_price_cents",
      "credits_charged",
      "sats_charged",
      "runner_cost_cents",
      "provider_cost_cents",
      "retry_cost_cents",
      "review_minutes",
      "review_cost_cents",
      "artifact_cost_cents",
      "total_cost_cents",
      "accepted_value_cents",
      "gross_margin_cents",
      "public_caveat_ref",
      "internal_caveat_ref",
      "no_settlement_implication",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    custodyColumns: ["metadata_json"],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  omni_route_scorecards: {
    columns: [
      "id",
      "idempotency_key",
      "workroom_id",
      "work_kind",
      "selected_route_ref",
      "selected_provider_ref",
      "selected_account_ref",
      "selected_model_ref",
      "selected_runtime_ref",
      "rejected_candidates_json",
      "decision_reason_refs_json",
      "observed_result_kind",
      "observed_result_ref",
      "post_closeout_score",
      "cost_cents",
      "latency_ms",
      "privacy_tier",
      "trust_tier",
      "public_caveat_ref",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    custodyColumns: ["metadata_json"],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  omni_public_proof_bundles: {
    columns: [
      "id",
      "idempotency_key",
      "workroom_id",
      "work_kind",
      "status",
      "legal_sensitive",
      "source_refs_json",
      "artifact_refs_json",
      "receipt_refs_json",
      "review_state_ref",
      "acceptance_state_ref",
      "economics_caveat_ref",
      "legal_caveat_ref",
      "privacy_caveat_ref",
      "public_receipt_ref",
      "no_settlement_implication",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    custodyColumns: ["metadata_json"],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  omni_market_memory_hooks: {
    columns: [
      "id",
      "idempotency_key",
      "workroom_id",
      "lifecycle_decision_id",
      "work_kind",
      "outcome_state",
      "category",
      "memory_ref",
      "evidence_ref",
      "source_ref",
      "public_caveat_ref",
      "route_scorecard_ref",
      "economics_ref",
      "authority_boundary",
      "no_routing_mutation",
      "no_payout_mutation",
      "no_public_claim_mutation",
      "no_module_promotion",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    custodyColumns: ["metadata_json"],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  omni_idempotency_keys: {
    // A pure idempotency table (worker migration 0010). Its PRIMARY KEY IS
    // the idempotency key — ported exactly. `result_json` is the memoized
    // response body: custody-guarded.
    columns: ["key", "scope", "result_json", "created_at", "expires_at"],
    custodyColumns: ["result_json"],
    keyColumns: ["key"],
    orderColumn: "created_at",
  },
  // -------------------------------------------------------------------------
  // autopilot_* (6)
  // -------------------------------------------------------------------------
  autopilot_token_usage: {
    columns: [
      "id",
      "run_id",
      "event_id",
      "user_id",
      "team_id",
      "provider",
      "model",
      "input_tokens",
      "output_tokens",
      "reasoning_tokens",
      "cache_read_tokens",
      "cache_write_5m_tokens",
      "cache_write_1h_tokens",
      "total_tokens",
      "source",
      "source_ref",
      "created_at",
      "account_ref",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  autopilot_work_orders: {
    columns: [
      "id",
      "work_order_ref",
      "owner_user_id",
      "agent_user_id",
      "agent_credential_id",
      "idempotency_key_hash",
      "client_request_ref",
      "request_json",
      "state",
      "task_refs_json",
      "access_request_refs_json",
      "payment_challenge_ref",
      "status_url_ref",
      "event_stream_ref",
      "created_at",
      "updated_at",
      "archived_at",
      "buyer_payment_proof_ref",
      "placement_policy_json",
      "execution_closeout_json",
      "review_decision_json",
      "scheduled_launch_json",
    ],
    custodyColumns: ["request_json"],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  autopilot_decision_closeout_receipts: {
    columns: [
      "closeout_ref",
      "decision_ref",
      "work_order_ref",
      "action",
      "resolved_state",
      "outcome",
      "actor_agent_user_id",
      "decided_at",
      "receipt_refs_json",
      "has_answer",
      "line",
      "receipt_json",
    ],
    custodyColumns: ["receipt_json"],
    keyColumns: ["closeout_ref"],
    orderColumn: "decided_at",
  },
  autopilot_continuation_policies: {
    columns: [
      "user_id",
      "enabled",
      "max_continuations_per_run",
      "max_continuations_per_day",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["user_id"],
    orderColumn: "updated_at",
  },
  autopilot_continuation_events: {
    columns: [
      "id",
      "user_id",
      "run_id",
      "goal_id",
      "mode",
      "decision",
      "reason_ref",
      "attempt",
      "created_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  autopilot_onboarding_sessions: {
    columns: [
      "id",
      "vertical_overlay",
      "status",
      "transcript_json",
      "output_spec_json",
      "turn_count",
      "created_at",
      "updated_at",
    ],
    custodyColumns: ["transcript_json", "output_spec_json"],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  // -------------------------------------------------------------------------
  // ops (4)
  // -------------------------------------------------------------------------
  relay_health_probes: {
    columns: [
      "id",
      "relay_url",
      "probed_at",
      "nip11_outcome",
      "nip11_http_status",
      "nip11_latency_ms",
      "nip11_relay_name",
      "ws_outcome",
      "ws_latency_ms",
      "status",
      "created_at",
    ],
    keyColumns: ["id"],
    orderColumn: "probed_at",
  },
  relay_health_transitions: {
    columns: [
      "id",
      "relay_url",
      "occurred_at",
      "kind",
      "from_status",
      "to_status",
      "probe_id",
      "created_at",
    ],
    keyColumns: ["id"],
    orderColumn: "occurred_at",
  },
  backend_incident_events: {
    columns: [
      "id",
      "incident_ref",
      "observed_at",
      "source",
      "kind",
      "severity",
      "route_pattern",
      "method",
      "status_code",
      "error_name",
      "runtime_name",
      "occurrence_count",
      "safe_metadata_json",
      "created_at",
    ],
    keyColumns: ["id"],
    orderColumn: "observed_at",
  },
  hygiene_debt_receipts: {
    columns: [
      "debt_receipt_key",
      "state",
      "debt_receipt_ref",
      "repo_baseline_ref",
      "scope_digest",
      "objective_digest",
      "merged_pr_ref",
      "reviewer_acceptance_ref",
      "baseline_metric_refs_json",
      "target_metric_refs_json",
      "verification_command_refs_json",
      "settlement_authority_actor_ref",
      "budget_cap_sats",
      "payable_sats",
      "settlement_input_json",
      "created_at",
      "updated_at",
      "retired_at",
      "settlement_receipt_ref",
    ],
    custodyColumns: ["settlement_input_json"],
    keyColumns: ["debt_receipt_key"],
    orderColumn: "updated_at",
  },
}

export const isSupervisionLongtailTable = (
  value: string,
): value is SupervisionLongtailTable =>
  Object.prototype.hasOwnProperty.call(
    SUPERVISION_LONGTAIL_TABLE_SPECS,
    value,
  )

export const normalizeSupervisionLongtailValue = (
  value: unknown,
): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "bigint") return value.toString()
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

export type SupervisionLongtailUnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)` for
 * dynamic-text parameterized statements; the structural `SyncSql` seam
 * deliberately does not, so this module widens it locally (the same
 * discipline as the other KS-8 backfill cores). Every statement built from
 * this registry is ONE parameterized statement whose dynamic text comes only
 * from compile-time table specs — Hyperdrive transaction-mode safe.
 */
export const requireSupervisionLongtailUnsafe = (
  sql: SyncSql,
): SupervisionLongtailUnsafeQuery => {
  const unsafe = (
    sql as SyncSql & { unsafe?: SupervisionLongtailUnsafeQuery }
  ).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "supervision long-tail domain store requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/**
 * Converge Postgres to the given D1 snapshot rows: full-row
 * `ON CONFLICT (composite PK) DO UPDATE` upserts. Idempotent — re-running
 * the same rows converges to the identical state; the mirror can never
 * invent a row: it only copies what the D1 authority already holds.
 */
export const upsertSupervisionLongtailRows = async (
  sql: SyncSql | SyncTransactionSql,
  table: SupervisionLongtailTable,
  rows: ReadonlyArray<SupervisionLongtailRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireSupervisionLongtailUnsafe(sql as SyncSql)
  const spec = SUPERVISION_LONGTAIL_TABLE_SPECS[table]
  const setClauses = spec.columns
    .filter((column) => !spec.keyColumns.includes(column))
    .map((column) => `${column} = EXCLUDED.${column}`)
    .join(", ")
  const updateClause =
    setClauses.length === 0 ? "DO NOTHING" : `DO UPDATE SET ${setClauses}`

  let touched = 0
  for (const row of rows) {
    const values = spec.columns.map((column) =>
      normalizeSupervisionLongtailValue(row[column]),
    )
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${spec.columns.join(", ")}) VALUES (${placeholders}) ON CONFLICT (${spec.keyColumns.join(", ")}) ${updateClause} RETURNING 1 AS touched`,
      values as Array<unknown>,
    )
    touched += result.length
  }
  return touched
}
