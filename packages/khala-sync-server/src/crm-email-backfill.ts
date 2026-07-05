/**
 * KS-8.11 (#8322): CRM / email / enrichment domain backfill core — D1 →
 * Postgres.
 *
 * Testable core behind `scripts/backfill-crm-email.ts`, following the
 * KS-8.6 artanis lane (`artanis-backfill.ts`). Takes raw D1 rows
 * (snake_case objects, exactly as `wrangler d1 execute --json` returns
 * them) and upserts them into the Postgres twins from migration
 * `0022_crm_email_domain.sql` with `ON CONFLICT ... DO NOTHING` — so the
 * backfill NEVER fights the live dual-write mirror (MIGRATION_PLAN §1.2:
 * rows the mirror already converged are fresher than any snapshot page;
 * rows the mirror never touched are filled here). Running a batch twice is
 * a no-op by construction (idempotency test: `crm-email-backfill.test.ts`).
 *
 * PRIVACY (the KS-8.11 gate): these rows carry names/emails/notes — PII.
 * Nothing in this module (or the CLI on top of it) ever emits row contents:
 * verification reports exact counts, per-status tallies over NON-PII status
 * columns, opaque row HASHES, and — for the compliance-bearing suppression
 * and preference tables — a whole-set digest. Key values that could be
 * emails are hashed before display (`piiSafeKey`).
 *
 * Verification (`verify*`): exact row counts, per-status/state tallies,
 * newest-N row-hash comparison over a canonical column serialization, and a
 * FULL-SET digest for `email_suppression_entries`, `email_preferences`, and
 * `business_outreach_suppressions` — the issue's "suppression set equality
 * (exact)" acceptance, proven without printing a single address. Nothing
 * "close enough": exact or explain.
 *
 * D1 and Postgres table names are IDENTICAL for this domain (all 36
 * canonical tables keep their names; the `_0193_new` D1 names were
 * transient rebuild artifacts and have no twins).
 */

import { createHash } from "node:crypto"
import type { SyncSql } from "./sql.js"

// ---------------------------------------------------------------------------
// Table registry (column lists mirror migration 0022 exactly, which mirrors
// the live D1 schema: worker migrations 0026/0038/0041/0063/0064/0181/0193/
// 0218/0219/0220/0296)
// ---------------------------------------------------------------------------

export type CrmEmailBackfillTable =
  | "crm_contacts"
  | "crm_accounts"
  | "crm_contact_lists"
  | "crm_contact_list_memberships"
  | "crm_activities"
  | "crm_engagement_snapshots"
  | "crm_opportunities"
  | "crm_opportunity_contact_roles"
  | "crm_source_import_runs"
  | "crm_email_templates"
  | "crm_email_messages"
  | "crm_contact_commands"
  | "crm_mcp_grants"
  | "email_templates"
  | "email_messages"
  | "email_deliveries"
  | "email_drafts"
  | "email_provider_events"
  | "email_campaigns"
  | "email_campaign_steps"
  | "email_campaign_enrollments"
  | "email_campaign_sends"
  | "email_preferences"
  | "email_suppression_entries"
  | "subscriber_lists"
  | "list_subscribers"
  | "business_outreach_template_approvals"
  | "business_outreach_suppressions"
  | "business_outreach_drafts"
  | "business_outreach_sends"
  | "exa_enrichment_runs"
  | "exa_enrichment_queries"
  | "exa_enrichment_sources"
  | "exa_enrichment_budget_events"
  | "exa_enrichment_cache_entries"
  | "exa_enrichment_metric_events"

export type CrmEmailTableSpec = Readonly<{
  /** Column list in canonical (migration) order. */
  columns: ReadonlyArray<string>
  /** Conflict target for the DO NOTHING upsert (the table's primary key). */
  conflictKey: string
  /** Column newest-N verification orders by (text ISO timestamps sort). */
  orderColumn: string
  /** NON-PII column the per-status tally groups by during verification. */
  statusColumn: string
  /** Compliance-bearing tables get a whole-set digest at verify time. */
  fullSetDigest?: boolean
}>

export const CRM_EMAIL_TABLE_SPECS: Readonly<
  Record<CrmEmailBackfillTable, CrmEmailTableSpec>
> = {
  business_outreach_drafts: {
    columns: [
      "draft_ref",
      "pipeline_ref",
      "subject_ref",
      "template_version_ref",
      "segment_ref",
      "audit_report_ref",
      "finding_refs_json",
      "body_text",
      "claim_lint_refs_json",
      "source_ref",
      "state",
      "created_at",
    ],
    conflictKey: "draft_ref",
    orderColumn: "created_at",
    statusColumn: "state",
  },
  business_outreach_sends: {
    columns: [
      "send_ref",
      "pipeline_ref",
      "draft_ref",
      "subject_ref",
      "template_version_ref",
      "mailbox_ref",
      "channel",
      "source_ref",
      "approval_receipt_ref",
      "send_receipt_ref",
      "sent_at",
      "created_at",
    ],
    conflictKey: "send_ref",
    orderColumn: "created_at",
    statusColumn: "channel",
  },
  business_outreach_suppressions: {
    columns: [
      "suppression_ref",
      "subject_ref",
      "reason",
      "source_ref",
      "created_at",
    ],
    conflictKey: "suppression_ref",
    fullSetDigest: true,
    orderColumn: "created_at",
    statusColumn: "reason",
  },
  business_outreach_template_approvals: {
    columns: [
      "approval_receipt_ref",
      "template_version_ref",
      "approved_by_ref",
      "source_ref",
      "created_at",
    ],
    conflictKey: "approval_receipt_ref",
    orderColumn: "created_at",
    statusColumn: "template_version_ref",
  },
  crm_accounts: {
    columns: [
      "id",
      "tenant_ref",
      "name",
      "domain",
      "account_type",
      "status",
      "website_url",
      "notes",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  crm_activities: {
    columns: [
      "id",
      "tenant_ref",
      "contact_id",
      "account_id",
      "activity_type",
      "subject",
      "summary",
      "occurred_at",
      "actor_ref",
      "source_system",
      "source_record_type",
      "source_record_id",
      "metadata_json",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "activity_type",
  },
  crm_contact_commands: {
    columns: [
      "id",
      "tenant_ref",
      "contact_id",
      "command_kind",
      "status",
      "proposed_by_ref",
      "approval_state",
      "payload_json",
      "result_json",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  crm_contact_list_memberships: {
    columns: [
      "id",
      "tenant_ref",
      "contact_id",
      "list_id",
      "membership_status",
      "source",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "membership_status",
  },
  crm_contact_lists: {
    columns: [
      "id",
      "tenant_ref",
      "slug",
      "name",
      "description",
      "is_system",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "is_system",
  },
  crm_contacts: {
    columns: [
      "id",
      "tenant_ref",
      "primary_email",
      "secondary_email",
      "full_name",
      "first_name",
      "last_name",
      "job_title",
      "contact_type",
      "relationship_stage",
      "lifecycle_stage",
      "account_id",
      "portal_access_status",
      "engagement_score",
      "last_contacted_at",
      "last_engaged_at",
      "last_replied_at",
      "external_source_label",
      "external_source_id",
      "notes",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    conflictKey: "id",
    fullSetDigest: true,
    orderColumn: "updated_at",
    statusColumn: "lifecycle_stage",
  },
  crm_email_messages: {
    columns: [
      "id",
      "tenant_ref",
      "contact_id",
      "template_id",
      "channel",
      "from_email",
      "to_email",
      "subject",
      "body_markdown",
      "body_html",
      "status",
      "send_reason",
      "provider_message_id",
      "provider_draft_id",
      "error_message",
      "sent_at",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  crm_email_templates: {
    columns: [
      "id",
      "tenant_ref",
      "slug",
      "name",
      "subject_template",
      "body_markdown_template",
      "status",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  crm_engagement_snapshots: {
    columns: [
      "id",
      "tenant_ref",
      "contact_id",
      "last_email_sent_at",
      "last_email_opened_at",
      "last_email_clicked_at",
      "last_email_replied_at",
      "email_sent_count_30d",
      "email_open_count_30d",
      "email_click_count_30d",
      "engagement_score",
      "snapshot_metadata_json",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "tenant_ref",
  },
  crm_mcp_grants: {
    columns: [
      "id",
      "grant_ref",
      "token_hash",
      "tenant_ref",
      "authority_classes_json",
      "label",
      "status",
      "created_at",
      "expires_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    statusColumn: "status",
  },
  crm_opportunities: {
    columns: [
      "id",
      "tenant_ref",
      "account_id",
      "name",
      "round_name",
      "stage",
      "status",
      "target_amount_cents",
      "expected_amount_cents",
      "conviction_probability",
      "target_close_date",
      "summary",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  crm_opportunity_contact_roles: {
    columns: [
      "id",
      "tenant_ref",
      "opportunity_id",
      "contact_id",
      "role_type",
      "status",
      "notes",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  crm_source_import_runs: {
    columns: [
      "id",
      "tenant_ref",
      "source_label",
      "status",
      "total_rows",
      "imported_rows",
      "updated_rows",
      "duplicate_rows",
      "failed_rows",
      "error_summary",
      "metadata_json",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  email_campaign_enrollments: {
    columns: [
      "id",
      "campaign_id",
      "user_id",
      "email",
      "status",
      "idempotency_key",
      "source_authority_ref",
      "metadata_json",
      "enrolled_at",
      "completed_at",
      "canceled_at",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  email_campaign_sends: {
    columns: [
      "id",
      "campaign_id",
      "step_id",
      "enrollment_id",
      "user_id",
      "email",
      "due_at",
      "status",
      "idempotency_key",
      "source_authority_ref",
      "email_message_id",
      "provider_event_id",
      "error_name",
      "error_message",
      "metadata_json",
      "claimed_at",
      "sent_at",
      "skipped_at",
      "failed_at",
      "created_at",
      "updated_at",
      "attempt_count",
      "next_attempt_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  email_campaign_steps: {
    columns: [
      "id",
      "campaign_id",
      "step_key",
      "name",
      "delay_seconds",
      "template_slug",
      "lifecycle_kind",
      "status",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  email_campaigns: {
    columns: [
      "id",
      "slug",
      "name",
      "audience",
      "status",
      "source_authority_ref",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  email_deliveries: {
    columns: [
      "id",
      "message_id",
      "provider",
      "provider_message_id",
      "provider_thread_id",
      "provider_request_id",
      "provider_idempotency_key",
      "status",
      "error_name",
      "error_message",
      "provider_payload_summary_json",
      "attempted_at",
      "completed_at",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  email_drafts: {
    columns: [
      "id",
      "message_id",
      "provider",
      "provider_draft_id",
      "provider_message_id",
      "provider_thread_id",
      "status",
      "provenance_json",
      "created_at",
      "updated_at",
      "sent_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  email_messages: {
    columns: [
      "id",
      "kind",
      "actor_user_id",
      "target_user_id",
      "to_email",
      "from_email",
      "reply_to_email",
      "subject",
      "text_body",
      "html_body",
      "template_id",
      "template_slug",
      "template_context_json",
      "status",
      "provider",
      "provider_message_id",
      "provider_draft_id",
      "provider_thread_id",
      "idempotency_key",
      "source_authority_ref",
      "action_submission_id",
      "metadata_json",
      "error_name",
      "error_message",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  email_preferences: {
    columns: [
      "id",
      "user_id",
      "email",
      "marketing_opt_in",
      "drip_opt_in",
      "transactional_opt_in",
      "source_authority_ref",
      "updated_by_user_id",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    fullSetDigest: true,
    orderColumn: "updated_at",
    statusColumn: "drip_opt_in",
  },
  email_provider_events: {
    columns: [
      "id",
      "provider",
      "provider_event_id",
      "event_type",
      "email",
      "email_message_id",
      "provider_message_id",
      "occurred_at",
      "payload_summary_json",
      "source_authority_ref",
      "created_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    statusColumn: "event_type",
  },
  email_suppression_entries: {
    columns: [
      "id",
      "email",
      "reason",
      "scope",
      "active",
      "source_authority_ref",
      "provider_event_id",
      "note",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    conflictKey: "id",
    fullSetDigest: true,
    orderColumn: "updated_at",
    statusColumn: "reason",
  },
  email_templates: {
    columns: [
      "id",
      "kind",
      "slug",
      "name",
      "subject_template",
      "text_template",
      "html_template",
      "variable_schema_version",
      "status",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  exa_enrichment_budget_events: {
    columns: [
      "id",
      "assignment_id",
      "run_id",
      "day_key",
      "request_units",
      "reason",
      "created_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    statusColumn: "reason",
  },
  exa_enrichment_cache_entries: {
    columns: [
      "id",
      "cache_key",
      "source_category",
      "search_type",
      "freshness_max_age_hours",
      "results_json",
      "result_count",
      "cost_dollars",
      "created_at",
      "expires_at",
      "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    statusColumn: "source_category",
  },
  exa_enrichment_metric_events: {
    columns: [
      "id",
      "assignment_id",
      "run_id",
      "query_id",
      "event_name",
      "status",
      "error_code",
      "search_type",
      "source_category",
      "result_count",
      "source_card_count",
      "latency_ms",
      "cost_dollars",
      "cache_status",
      "created_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    statusColumn: "event_name",
  },
  exa_enrichment_queries: {
    columns: [
      "id",
      "run_id",
      "assignment_id",
      "query_hash",
      "query_text",
      "source_category",
      "search_type",
      "freshness_max_age_hours",
      "status",
      "result_count",
      "latency_ms",
      "cost_dollars",
      "error_code",
      "error_summary",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  exa_enrichment_runs: {
    columns: [
      "id",
      "assignment_id",
      "software_order_id",
      "site_id",
      "plan_id",
      "subject",
      "status",
      "request_budget",
      "request_count",
      "cache_hit_count",
      "source_count",
      "approved_source_count",
      "cost_dollars",
      "error_code",
      "error_summary",
      "started_at",
      "completed_at",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  exa_enrichment_sources: {
    columns: [
      "id",
      "run_id",
      "query_id",
      "assignment_id",
      "software_order_id",
      "site_id",
      "source_category",
      "review_status",
      "title",
      "url",
      "domain",
      "published_date",
      "highlight_text",
      "selected_text_hash",
      "exa_request_id",
      "search_type",
      "public_safe",
      "rejected_reason",
      "approved_at",
      "rejected_at",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "review_status",
  },
  list_subscribers: {
    columns: [
      "id",
      "list_id",
      "email",
      "status",
      "source_ref",
      "idempotency_key",
      "metadata_json",
      "subscribed_at",
      "unsubscribed_at",
      "bounced_at",
      "created_at",
      "updated_at",
    ],
    conflictKey: "id",
    fullSetDigest: true,
    orderColumn: "updated_at",
    statusColumn: "status",
  },
  subscriber_lists: {
    columns: [
      "id",
      "owner_user_id",
      "team_id",
      "slug",
      "name",
      "status",
      "source_authority_ref",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    statusColumn: "status",
  },
}

export const CRM_EMAIL_BACKFILL_TABLES = Object.keys(
  CRM_EMAIL_TABLE_SPECS,
) as ReadonlyArray<CrmEmailBackfillTable>

export type D1SourceRow = Readonly<Record<string, unknown>>

const normalizeValue = (value: unknown): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

/** Keys never leave this module raw if they could be an email address. */
export const piiSafeKey = (key: string): string =>
  key.includes("@")
    ? `sha256:${createHash("sha256").update(key).digest("hex").slice(0, 12)}`
    : key

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)` for
 * dynamic-text parameterized statements; the structural `SyncSql` seam
 * deliberately does not, so this module widens it locally. The backfill
 * runs over DIRECT connections (never Hyperdrive).
 */
type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "crm/email backfill requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/**
 * Upsert one page of D1 rows into `table`. `ON CONFLICT (primary key) DO
 * NOTHING`: rows the dual-write mirror already owns win. Returns how many
 * rows were actually inserted (0 on a re-run — the idempotency contract).
 */
export const upsertCrmEmailRows = async (
  sql: SyncSql,
  table: CrmEmailBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireUnsafe(sql)
  const spec = CRM_EMAIL_TABLE_SPECS[table]
  let inserted = 0
  for (const row of rows) {
    const values = spec.columns.map((column) => normalizeValue(row[column]))
    const columnsSql = spec.columns.join(", ")
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${columnsSql}) VALUES (${placeholders}) ON CONFLICT (${spec.conflictKey}) DO NOTHING RETURNING ${spec.conflictKey}`,
      values as Array<unknown>,
    )
    inserted += result.length
  }
  return inserted
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

export type CrmEmailVerifyTally = Readonly<{
  total: number
  byStatus: Readonly<Record<string, number>>
}>

/** Count + per-status tally over the Postgres side of one table. */
export const postgresCrmEmailTally = async (
  sql: SyncSql,
  table: CrmEmailBackfillTable,
): Promise<CrmEmailVerifyTally> => {
  const statusColumn = CRM_EMAIL_TABLE_SPECS[table].statusColumn
  const rows = (await requireUnsafe(sql)(
    `SELECT ${statusColumn}::text AS status_value, count(*) AS row_count FROM ${table} GROUP BY ${statusColumn} ORDER BY ${statusColumn}`,
    [],
  )) as Array<{ status_value: string | null; row_count: unknown }>
  const byStatus: Record<string, number> = {}
  let total = 0
  for (const row of rows) {
    const count = Number(row.row_count)
    byStatus[row.status_value ?? "<null>"] = count
    total += count
  }
  return { byStatus, total }
}

/**
 * Canonical row hash: the migration-order column values joined with unit
 * separators, sha256'd. Column normalization matches `upsertCrmEmailRows`,
 * so the SAME D1 export row and its Postgres twin hash identically. This is
 * the ONLY form in which row contents ever surface — an opaque digest.
 */
export const crmEmailRowHash = (
  table: CrmEmailBackfillTable,
  row: D1SourceRow,
): string => {
  const hash = createHash("sha256")
  for (const column of CRM_EMAIL_TABLE_SPECS[table].columns) {
    const value = normalizeValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ key: string; hash: string }>

/**
 * Newest-N row hashes on the Postgres side, keyed by the table's primary
 * key (PII-safed), newest-first by the table's order column.
 */
export const postgresCrmEmailNewestRowHashes = async (
  sql: SyncSql,
  table: CrmEmailBackfillTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const spec = CRM_EMAIL_TABLE_SPECS[table]
  const rows = await requireUnsafe(sql)(
    `SELECT * FROM ${table} ORDER BY ${spec.orderColumn} DESC, ${spec.conflictKey} DESC LIMIT $1`,
    [limit],
  )
  return rows.map((row) => ({
    hash: crmEmailRowHash(table, row),
    key: piiSafeKey(String(row[spec.conflictKey])),
  }))
}

/** Same newest-N hashing over D1 export rows (already fetched by the CLI). */
export const d1CrmEmailNewestRowHashes = (
  table: CrmEmailBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> => {
  const keyColumn = CRM_EMAIL_TABLE_SPECS[table].conflictKey
  return rows.map((row) => ({
    hash: crmEmailRowHash(table, row),
    key: piiSafeKey(String(row[keyColumn])),
  }))
}

/**
 * Whole-set digest for the compliance-bearing tables: sha256 over the
 * SORTED per-row hashes. Set equality (the issue's suppression acceptance)
 * without emitting a single row: identical sets ⇒ identical digests,
 * regardless of row order on either side.
 */
export const crmEmailSetDigest = (
  hashes: ReadonlyArray<string>,
): string => {
  const digest = createHash("sha256")
  for (const hash of [...hashes].sort()) {
    digest.update(hash)
    digest.update("\n")
  }
  return digest.digest("hex")
}

/** Whole-set digest over the Postgres side of one table. */
export const postgresCrmEmailSetDigest = async (
  sql: SyncSql,
  table: CrmEmailBackfillTable,
): Promise<Readonly<{ digest: string; total: number }>> => {
  const rows = await requireUnsafe(sql)(`SELECT * FROM ${table}`, [])
  return {
    digest: crmEmailSetDigest(rows.map((row) => crmEmailRowHash(table, row))),
    total: rows.length,
  }
}

/** Whole-set digest over D1 export rows (already fetched by the CLI). */
export const d1CrmEmailSetDigest = (
  table: CrmEmailBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): Readonly<{ digest: string; total: number }> => ({
  digest: crmEmailSetDigest(rows.map((row) => crmEmailRowHash(table, row))),
  total: rows.length,
})

export type CrmEmailVerifyTableReport = Readonly<{
  table: CrmEmailBackfillTable
  d1Total: number
  postgresTotal: number
  countsMatch: boolean
  statusMismatches: ReadonlyArray<{
    status: string
    d1: number
    postgres: number
  }>
  newestHashMismatches: ReadonlyArray<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }>
  /** Present only for fullSetDigest tables. */
  setDigestsMatch?: boolean
}>

export const compareCrmEmailTallies = (
  table: CrmEmailBackfillTable,
  d1: CrmEmailVerifyTally,
  postgres: CrmEmailVerifyTally,
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
  setDigests?: Readonly<{ d1: string; postgres: string }>,
): CrmEmailVerifyTableReport => {
  const statuses = new Set([
    ...Object.keys(d1.byStatus),
    ...Object.keys(postgres.byStatus),
  ])
  const statusMismatches: Array<{
    status: string
    d1: number
    postgres: number
  }> = []
  for (const status of [...statuses].sort()) {
    const d1Count = d1.byStatus[status] ?? 0
    const postgresCount = postgres.byStatus[status] ?? 0
    if (d1Count !== postgresCount) {
      statusMismatches.push({ d1: d1Count, postgres: postgresCount, status })
    }
  }

  const postgresByKey = new Map(
    postgresNewest.map((entry) => [entry.key, entry.hash]),
  )
  const newestHashMismatches: Array<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }> = []
  for (const entry of d1Newest) {
    const postgresHash = postgresByKey.get(entry.key)
    if (postgresHash !== entry.hash) {
      newestHashMismatches.push({
        d1Hash: entry.hash,
        key: entry.key,
        postgresHash,
      })
    }
  }

  return {
    countsMatch: d1.total === postgres.total,
    d1Total: d1.total,
    newestHashMismatches,
    postgresTotal: postgres.total,
    statusMismatches,
    table,
    ...(setDigests === undefined
      ? {}
      : { setDigestsMatch: setDigests.d1 === setDigests.postgres }),
  }
}
