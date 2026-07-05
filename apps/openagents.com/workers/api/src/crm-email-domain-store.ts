// KS-8.11 (#8322): CRM / email / enrichment domain — D1 → Cloud SQL migration
// machinery, following the KS-8.6 artanis template (artanis-domain-store.ts).
//
// Thirty-six canonical tables (`crm_*` 13, `email_*` 11 — the `_0193_new`
// names were transient rebuild artifacts, verified superseded, no twins —
// `subscriber_lists` + `list_subscribers`, `business_outreach_*` 4,
// `exa_enrichment_*` 6) plus the `EmailCampaignDispatcher.dispatchDue` cron.
//
// SEAM CHOICE: this domain's SQL lives in FOURTEEN owning modules
// (crm-store, crm-email, crm-command, crm-mcp-grant, email, email-campaigns,
// email-campaign-dispatcher, email-preferences, email-sequence-authoring,
// email-onboarding-drip, native-lists, business-outreach,
// adjutant-enrichment-ledger/-operations, resend-webhooks) with no single
// store interface — exactly the artanis shape, so the seam is the
// DATABASE-SHAPED handle (KS-8.6 #8317), not the per-operation store (KS-8.5
// #8316) or the mirror-op union (KS-8.9 #8320, which fits wide table sets
// funneled through ONE routing module; this domain has no such funnel):
//
//  1. `CrmEmailDatabase = D1Database | CrmEmailDomainHandle` — the owning
//     modules' signatures take this union. A plain `D1Database` still works
//     (no mirroring, no routing — fail-safe), and `crmEmailAuthorityDb(db)`
//     recovers the authoritative D1 handle either way.
//     `makeCrmEmailDatabaseForEnv(env)` is the drop-in that upgrades the
//     write entry points — including the email-campaign dispatch cron.
//
//  2. `mirrorCrmEmailRows(db, table, keyColumn, keys)` — the dual-write.
//     After the authoritative D1 write, the RESOLVED row(s) are read back
//     from D1 by key and converged into Postgres as full-row upserts
//     (`ON CONFLICT (primary key) DO UPDATE`), so a row touched by
//     dual-write self-heals before the backfill reaches it and re-mirroring
//     is idempotent by construction. A Postgres failure NEVER fails the
//     request: it logs the typed `khala_sync_crm_dual_write_failed`
//     diagnostic (the drift metric) and moves on — an email send, a webhook
//     ack, or a CRM import must never fail because the mirror did.
//
//  3. `crmEmailRead(db, op, refs, readD1, readPostgres)` — flag-routed
//     reads: d1 (default), compare (read both, SERVE D1, log mismatches),
//     postgres (bounded retry, D1 fallback + diagnostic on exhaustion).
//     The SUPPRESSION COMPLIANCE GATE (isEmailSuppressed /
//     readEmailPreferenceAllows) routes through this seam so the send path
//     reads exactly ONE authoritative suppression store at every moment:
//     the flag flip is atomic per-read, and compare mode is the staging
//     evidence (a deliberately suppressed send attempt) before any flip.
//
// PRIVACY (the KS-8.11 gate): CRM rows carry names/emails/notes — PII.
// Diagnostics carry table names, keys, and hashes ONLY — never row contents
// — and any key value that is itself an email address is logged as a sha256
// prefix (`sha256:<12 hex>`), never raw.
//
// Flags (per KS-8 convention):
//   KHALA_SYNC_CRM_DUAL_WRITE  (default ON; '0'|'off'|'false'|'disabled'|'no')
//   KHALA_SYNC_CRM_READS       (default 'd1'; 'd1'|'postgres'|'compare')
// With no KHALA_SYNC_DB binding everything degrades to plain D1.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "CRM / email / enrichment
// domain cutover"): dual-write on → backfill (khala-sync-server
// scripts/backfill-crm-email.ts) → second sweep → --verify (PII-safe:
// counts/tallies/key-hashes only) → compare reads (must include a suppressed
// send attempt in staging) → postgres reads → re-home the dispatch cron →
// drop the D1 tables in the follow-up decommission issue.
import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
  defaultMakeKhalaSyncSqlClient,
} from './khala-sync-push-routes'
import { logWorkerRouteWarning } from './observability'
import { openAgentsDatabase } from './runtime'

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type CrmEmailDomainReadsMode = 'd1' | 'postgres' | 'compare'

export type CrmEmailDomainFlags = Readonly<{
  dualWrite: boolean
  reads: CrmEmailDomainReadsMode
}>

export type CrmEmailDomainFlagEnv = Readonly<{
  KHALA_SYNC_CRM_DUAL_WRITE?: string | undefined
  KHALA_SYNC_CRM_READS?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.11 migration flags from Worker vars. Dual-write defaults ON
 * (this lane lands with the mirror active wherever the binding exists);
 * reads default to D1 authority until the runbook's cutover sequence flips
 * them. Unknown read values fall back to 'd1' — never fail open into an
 * unproven read path on a typo (this domain's reads include the suppression
 * compliance gate).
 */
export const crmEmailDomainFlagsFromEnv = (
  env: CrmEmailDomainFlagEnv,
): CrmEmailDomainFlags => {
  const dualWriteRaw = env.KHALA_SYNC_CRM_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_CRM_READS?.trim().toLowerCase()

  return {
    dualWrite: dualWriteRaw === undefined || !FLAG_OFF_VALUES.has(dualWriteRaw),
    reads: readsRaw === 'postgres' || readsRaw === 'compare' ? readsRaw : 'd1',
  }
}

// ---------------------------------------------------------------------------
// Diagnostics (the drift metric) — keys/hashes only, never row contents
// ---------------------------------------------------------------------------

export type CrmEmailDomainDiagnosticEvent =
  | 'khala_sync_crm_dual_write_failed'
  | 'khala_sync_crm_read_compare_mismatch'
  | 'khala_sync_crm_postgres_read_failed'
  | 'khala_sync_crm_postgres_read_fallback'

export type CrmEmailDomainDiagnostic = Readonly<{
  /** The mirrored table or read operation, e.g. 'email_campaign_sends'. */
  op: string
  /** PII-safe refs: keys, with email-valued keys as sha256 prefixes. */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no SQL). */
  messageSafe: string
}>

export type CrmEmailDomainLog = (
  event: CrmEmailDomainDiagnosticEvent,
  fields: CrmEmailDomainDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

// WebCrypto sha256; falls back to FULL redaction (never a weaker hash) if
// the runtime lacks subtle crypto. Refs are only produced for diagnostics,
// so the async hash is fine — the mirror path is already async.
const hashRef = async (value: string): Promise<string> => {
  try {
    const digest = await crypto.subtle.digest(
      'SHA-256',
      new TextEncoder().encode(value),
    )
    const hex = [...new Uint8Array(digest)]
      .map(byte => byte.toString(16).padStart(2, '0'))
      .join('')
    return `sha256:${hex.slice(0, 12)}`
  } catch {
    return 'sha256:redacted'
  }
}

/** Email-valued keys never appear raw in diagnostics. */
export const publicSafeRefs = async (
  keys: ReadonlyArray<string | number>,
): Promise<ReadonlyArray<string>> =>
  Promise.all(
    keys.map(key => {
      const text = String(key)
      return text.includes('@') ? hashRef(text) : Promise.resolve(text)
    }),
  )

// ---------------------------------------------------------------------------
// Table registry
// ---------------------------------------------------------------------------
//
// Column lists mirror khala-sync-server migration 0022_crm_email_domain.sql
// (which mirrors the live D1 schema: worker migrations 0026/0038/0041/0063/
// 0064/0181/0193/0218/0219/0220/0296) and the registry in
// packages/khala-sync-server/src/crm-email-backfill.ts. The contract test
// proves the registry against BOTH engines' real SQL.

export type CrmEmailDomainTable =
  | 'crm_contacts'
  | 'crm_accounts'
  | 'crm_contact_lists'
  | 'crm_contact_list_memberships'
  | 'crm_activities'
  | 'crm_engagement_snapshots'
  | 'crm_opportunities'
  | 'crm_opportunity_contact_roles'
  | 'crm_source_import_runs'
  | 'crm_email_templates'
  | 'crm_email_messages'
  | 'crm_contact_commands'
  | 'crm_mcp_grants'
  | 'email_templates'
  | 'email_messages'
  | 'email_deliveries'
  | 'email_drafts'
  | 'email_provider_events'
  | 'email_campaigns'
  | 'email_campaign_steps'
  | 'email_campaign_enrollments'
  | 'email_campaign_sends'
  | 'email_preferences'
  | 'email_suppression_entries'
  | 'subscriber_lists'
  | 'list_subscribers'
  | 'business_outreach_template_approvals'
  | 'business_outreach_suppressions'
  | 'business_outreach_drafts'
  | 'business_outreach_sends'
  | 'exa_enrichment_runs'
  | 'exa_enrichment_queries'
  | 'exa_enrichment_sources'
  | 'exa_enrichment_budget_events'
  | 'exa_enrichment_cache_entries'
  | 'exa_enrichment_metric_events'

type CrmEmailDomainTableSpec = Readonly<{
  columns: ReadonlyArray<string>
  /** Conflict target for the converge upsert (the table's primary key). */
  conflictKey: string
  /** Columns modules may key mirrors/reads by (validated, never dynamic). */
  keyColumns: ReadonlyArray<string>
  /** Column latest-N reads order by (text ISO timestamps sort correctly). */
  orderColumn: string
}>

export const CRM_EMAIL_DOMAIN_TABLES: Readonly<
  Record<CrmEmailDomainTable, CrmEmailDomainTableSpec>
> = {
  business_outreach_drafts: {
    columns: [
      'draft_ref',
      'pipeline_ref',
      'subject_ref',
      'template_version_ref',
      'segment_ref',
      'audit_report_ref',
      'finding_refs_json',
      'body_text',
      'claim_lint_refs_json',
      'source_ref',
      'state',
      'created_at',
    ],
    conflictKey: 'draft_ref',
    keyColumns: ['draft_ref', 'pipeline_ref'],
    orderColumn: 'created_at',
  },
  business_outreach_sends: {
    columns: [
      'send_ref',
      'pipeline_ref',
      'draft_ref',
      'subject_ref',
      'template_version_ref',
      'mailbox_ref',
      'channel',
      'source_ref',
      'approval_receipt_ref',
      'send_receipt_ref',
      'sent_at',
      'created_at',
    ],
    conflictKey: 'send_ref',
    keyColumns: ['send_ref', 'pipeline_ref'],
    orderColumn: 'created_at',
  },
  business_outreach_suppressions: {
    columns: [
      'suppression_ref',
      'subject_ref',
      'reason',
      'source_ref',
      'created_at',
    ],
    conflictKey: 'suppression_ref',
    keyColumns: ['suppression_ref', 'subject_ref'],
    orderColumn: 'created_at',
  },
  business_outreach_template_approvals: {
    columns: [
      'approval_receipt_ref',
      'template_version_ref',
      'approved_by_ref',
      'source_ref',
      'created_at',
    ],
    conflictKey: 'approval_receipt_ref',
    keyColumns: ['approval_receipt_ref', 'template_version_ref'],
    orderColumn: 'created_at',
  },
  crm_accounts: {
    columns: [
      'id',
      'tenant_ref',
      'name',
      'domain',
      'account_type',
      'status',
      'website_url',
      'notes',
      'metadata_json',
      'created_at',
      'updated_at',
      'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'updated_at',
  },
  crm_activities: {
    columns: [
      'id',
      'tenant_ref',
      'contact_id',
      'account_id',
      'activity_type',
      'subject',
      'summary',
      'occurred_at',
      'actor_ref',
      'source_system',
      'source_record_type',
      'source_record_id',
      'metadata_json',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'contact_id'],
    orderColumn: 'updated_at',
  },
  crm_contact_commands: {
    columns: [
      'id',
      'tenant_ref',
      'contact_id',
      'command_kind',
      'status',
      'proposed_by_ref',
      'approval_state',
      'payload_json',
      'result_json',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'updated_at',
  },
  crm_contact_list_memberships: {
    columns: [
      'id',
      'tenant_ref',
      'contact_id',
      'list_id',
      'membership_status',
      'source',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    // Membership upserts conflict on (contact_id, list_id) and keep the OLD
    // row id — mirroring by contact_id reads back the resolved row(s)
    // whichever id survived.
    keyColumns: ['id', 'contact_id'],
    orderColumn: 'updated_at',
  },
  crm_contact_lists: {
    columns: [
      'id',
      'tenant_ref',
      'slug',
      'name',
      'description',
      'is_system',
      'created_at',
      'updated_at',
      'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'updated_at',
  },
  crm_contacts: {
    columns: [
      'id',
      'tenant_ref',
      'primary_email',
      'secondary_email',
      'full_name',
      'first_name',
      'last_name',
      'job_title',
      'contact_type',
      'relationship_stage',
      'lifecycle_stage',
      'account_id',
      'portal_access_status',
      'engagement_score',
      'last_contacted_at',
      'last_engaged_at',
      'last_replied_at',
      'external_source_label',
      'external_source_id',
      'notes',
      'metadata_json',
      'created_at',
      'updated_at',
      'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'updated_at',
  },
  crm_email_messages: {
    columns: [
      'id',
      'tenant_ref',
      'contact_id',
      'template_id',
      'channel',
      'from_email',
      'to_email',
      'subject',
      'body_markdown',
      'body_html',
      'status',
      'send_reason',
      'provider_message_id',
      'provider_draft_id',
      'error_message',
      'sent_at',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'contact_id'],
    orderColumn: 'updated_at',
  },
  crm_email_templates: {
    columns: [
      'id',
      'tenant_ref',
      'slug',
      'name',
      'subject_template',
      'body_markdown_template',
      'status',
      'created_at',
      'updated_at',
      'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'updated_at',
  },
  crm_engagement_snapshots: {
    columns: [
      'id',
      'tenant_ref',
      'contact_id',
      'last_email_sent_at',
      'last_email_opened_at',
      'last_email_clicked_at',
      'last_email_replied_at',
      'email_sent_count_30d',
      'email_open_count_30d',
      'email_click_count_30d',
      'engagement_score',
      'snapshot_metadata_json',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'contact_id'],
    orderColumn: 'updated_at',
  },
  crm_mcp_grants: {
    columns: [
      'id',
      'grant_ref',
      'token_hash',
      'tenant_ref',
      'authority_classes_json',
      'label',
      'status',
      'created_at',
      'expires_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'grant_ref'],
    orderColumn: 'created_at',
  },
  crm_opportunities: {
    columns: [
      'id',
      'tenant_ref',
      'account_id',
      'name',
      'round_name',
      'stage',
      'status',
      'target_amount_cents',
      'expected_amount_cents',
      'conviction_probability',
      'target_close_date',
      'summary',
      'metadata_json',
      'created_at',
      'updated_at',
      'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'updated_at',
  },
  crm_opportunity_contact_roles: {
    columns: [
      'id',
      'tenant_ref',
      'opportunity_id',
      'contact_id',
      'role_type',
      'status',
      'notes',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'opportunity_id'],
    orderColumn: 'updated_at',
  },
  crm_source_import_runs: {
    columns: [
      'id',
      'tenant_ref',
      'source_label',
      'status',
      'total_rows',
      'imported_rows',
      'updated_rows',
      'duplicate_rows',
      'failed_rows',
      'error_summary',
      'metadata_json',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'updated_at',
  },
  email_campaign_enrollments: {
    columns: [
      'id',
      'campaign_id',
      'user_id',
      'email',
      'status',
      'idempotency_key',
      'source_authority_ref',
      'metadata_json',
      'enrolled_at',
      'completed_at',
      'canceled_at',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'idempotency_key'],
    orderColumn: 'updated_at',
  },
  email_campaign_sends: {
    columns: [
      'id',
      'campaign_id',
      'step_id',
      'enrollment_id',
      'user_id',
      'email',
      'due_at',
      'status',
      'idempotency_key',
      'source_authority_ref',
      'email_message_id',
      'provider_event_id',
      'error_name',
      'error_message',
      'metadata_json',
      'claimed_at',
      'sent_at',
      'skipped_at',
      'failed_at',
      'created_at',
      'updated_at',
      'attempt_count',
      'next_attempt_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'idempotency_key'],
    orderColumn: 'updated_at',
  },
  email_campaign_steps: {
    columns: [
      'id',
      'campaign_id',
      'step_key',
      'name',
      'delay_seconds',
      'template_slug',
      'lifecycle_kind',
      'status',
      'metadata_json',
      'created_at',
      'updated_at',
      'archived_at',
    ],
    conflictKey: 'id',
    // Step upserts conflict on (campaign_id, step_key) and keep the OLD row
    // id — mirroring by campaign_id converges the campaign's whole (small)
    // step set whichever ids survived.
    keyColumns: ['id', 'campaign_id'],
    orderColumn: 'updated_at',
  },
  email_campaigns: {
    columns: [
      'id',
      'slug',
      'name',
      'audience',
      'status',
      'source_authority_ref',
      'metadata_json',
      'created_at',
      'updated_at',
      'archived_at',
    ],
    conflictKey: 'id',
    // Campaign upserts conflict on slug and keep the OLD row id.
    keyColumns: ['id', 'slug'],
    orderColumn: 'updated_at',
  },
  email_deliveries: {
    columns: [
      'id',
      'message_id',
      'provider',
      'provider_message_id',
      'provider_thread_id',
      'provider_request_id',
      'provider_idempotency_key',
      'status',
      'error_name',
      'error_message',
      'provider_payload_summary_json',
      'attempted_at',
      'completed_at',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'message_id', 'provider_message_id'],
    orderColumn: 'updated_at',
  },
  email_drafts: {
    columns: [
      'id',
      'message_id',
      'provider',
      'provider_draft_id',
      'provider_message_id',
      'provider_thread_id',
      'status',
      'provenance_json',
      'created_at',
      'updated_at',
      'sent_at',
    ],
    conflictKey: 'id',
    // Draft upserts conflict on (provider, provider_draft_id) and keep the
    // OLD row id — mirroring by provider_draft_id reads back the survivor.
    keyColumns: ['id', 'message_id', 'provider_draft_id'],
    orderColumn: 'updated_at',
  },
  email_messages: {
    columns: [
      'id',
      'kind',
      'actor_user_id',
      'target_user_id',
      'to_email',
      'from_email',
      'reply_to_email',
      'subject',
      'text_body',
      'html_body',
      'template_id',
      'template_slug',
      'template_context_json',
      'status',
      'provider',
      'provider_message_id',
      'provider_draft_id',
      'provider_thread_id',
      'idempotency_key',
      'source_authority_ref',
      'action_submission_id',
      'metadata_json',
      'error_name',
      'error_message',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'idempotency_key'],
    orderColumn: 'updated_at',
  },
  email_preferences: {
    columns: [
      'id',
      'user_id',
      'email',
      'marketing_opt_in',
      'drip_opt_in',
      'transactional_opt_in',
      'source_authority_ref',
      'updated_by_user_id',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    // Preference upserts conflict on email and keep the OLD row id —
    // mirroring by email reads back the survivor. Email-valued keys are
    // hashed before they reach any diagnostic (publicSafeRefs).
    keyColumns: ['id', 'email'],
    orderColumn: 'updated_at',
  },
  email_provider_events: {
    columns: [
      'id',
      'provider',
      'provider_event_id',
      'event_type',
      'email',
      'email_message_id',
      'provider_message_id',
      'occurred_at',
      'payload_summary_json',
      'source_authority_ref',
      'created_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'provider_event_id'],
    orderColumn: 'created_at',
  },
  email_suppression_entries: {
    columns: [
      'id',
      'email',
      'reason',
      'scope',
      'active',
      'source_authority_ref',
      'provider_event_id',
      'note',
      'created_at',
      'updated_at',
      'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'email'],
    orderColumn: 'updated_at',
  },
  email_templates: {
    columns: [
      'id',
      'kind',
      'slug',
      'name',
      'subject_template',
      'text_template',
      'html_template',
      'variable_schema_version',
      'status',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'slug'],
    orderColumn: 'updated_at',
  },
  exa_enrichment_budget_events: {
    columns: [
      'id',
      'assignment_id',
      'run_id',
      'day_key',
      'request_units',
      'reason',
      'created_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'created_at',
  },
  exa_enrichment_cache_entries: {
    columns: [
      'id',
      'cache_key',
      'source_category',
      'search_type',
      'freshness_max_age_hours',
      'results_json',
      'result_count',
      'cost_dollars',
      'created_at',
      'expires_at',
      'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'cache_key'],
    orderColumn: 'created_at',
  },
  exa_enrichment_metric_events: {
    columns: [
      'id',
      'assignment_id',
      'run_id',
      'query_id',
      'event_name',
      'status',
      'error_code',
      'search_type',
      'source_category',
      'result_count',
      'source_card_count',
      'latency_ms',
      'cost_dollars',
      'cache_status',
      'created_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'created_at',
  },
  exa_enrichment_queries: {
    columns: [
      'id',
      'run_id',
      'assignment_id',
      'query_hash',
      'query_text',
      'source_category',
      'search_type',
      'freshness_max_age_hours',
      'status',
      'result_count',
      'latency_ms',
      'cost_dollars',
      'error_code',
      'error_summary',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'run_id'],
    orderColumn: 'updated_at',
  },
  exa_enrichment_runs: {
    columns: [
      'id',
      'assignment_id',
      'software_order_id',
      'site_id',
      'plan_id',
      'subject',
      'status',
      'request_budget',
      'request_count',
      'cache_hit_count',
      'source_count',
      'approved_source_count',
      'cost_dollars',
      'error_code',
      'error_summary',
      'started_at',
      'completed_at',
      'created_at',
      'updated_at',
      'archived_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'updated_at',
  },
  exa_enrichment_sources: {
    columns: [
      'id',
      'run_id',
      'query_id',
      'assignment_id',
      'software_order_id',
      'site_id',
      'source_category',
      'review_status',
      'title',
      'url',
      'domain',
      'published_date',
      'highlight_text',
      'selected_text_hash',
      'exa_request_id',
      'search_type',
      'public_safe',
      'rejected_reason',
      'approved_at',
      'rejected_at',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'run_id'],
    orderColumn: 'updated_at',
  },
  list_subscribers: {
    columns: [
      'id',
      'list_id',
      'email',
      'status',
      'source_ref',
      'idempotency_key',
      'metadata_json',
      'subscribed_at',
      'unsubscribed_at',
      'bounced_at',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id', 'idempotency_key', 'list_id'],
    orderColumn: 'updated_at',
  },
  subscriber_lists: {
    columns: [
      'id',
      'owner_user_id',
      'team_id',
      'slug',
      'name',
      'status',
      'source_authority_ref',
      'metadata_json',
      'created_at',
      'updated_at',
      'archived_at',
    ],
    conflictKey: 'id',
    // List upserts conflict on slug and keep the OLD row id.
    keyColumns: ['id', 'slug'],
    orderColumn: 'updated_at',
  },
}

export type CrmEmailDomainRow = Readonly<Record<string, unknown>>

class CrmEmailDomainKeyColumnError extends TypeError {}

class CrmEmailDomainSqlCapabilityError extends TypeError {}

const requireKeyColumn = (
  table: CrmEmailDomainTable,
  keyColumn: string,
): string => {
  if (!CRM_EMAIL_DOMAIN_TABLES[table].keyColumns.includes(keyColumn)) {
    throw new CrmEmailDomainKeyColumnError(
      `crm/email domain store: ${keyColumn} is not a registered key column of ${table}`,
    )
  }
  return keyColumn
}

// ---------------------------------------------------------------------------
// Postgres store (registry-driven, single parameterized statements)
// ---------------------------------------------------------------------------

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)` for
 * dynamic-text parameterized statements; the structural `SyncSql` seam
 * deliberately does not, so this module widens it locally (the same
 * discipline as the khala-sync-server backfill cores). Every statement
 * built here is ONE parameterized statement whose dynamic text comes only
 * from the compile-time table registry — no session state, so it stays
 * Hyperdrive transaction-mode safe.
 */
type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== 'function') {
    throw new CrmEmailDomainSqlCapabilityError(
      'crm/email domain store requires a driver exposing unsafe(text, params)',
    )
  }
  return unsafe
}

const normalizeValue = (value: unknown): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === 'number') return value
  if (typeof value === 'boolean') return value ? 1 : 0
  return String(value)
}

export type PostgresCrmEmailDomainStore = Readonly<{
  /**
   * Converge Postgres to the RESOLVED rows the authoritative D1 write
   * produced — full-row `ON CONFLICT (primary key) DO UPDATE` upserts, so
   * a row touched by dual-write self-heals even before the backfill
   * reaches it, and re-mirroring the same row is a no-op.
   */
  upsertRows: (
    table: CrmEmailDomainTable,
    rows: ReadonlyArray<CrmEmailDomainRow>,
  ) => Promise<void>
  /** Registry-validated key lookup (read cutover + compare mode). */
  selectRowsByKey: (
    table: CrmEmailDomainTable,
    keyColumn: string,
    keys: ReadonlyArray<string | number>,
  ) => Promise<Array<CrmEmailDomainRow>>
  /** Latest-N by the table's order column (read cutover + compare mode). */
  selectLatestRows: (
    table: CrmEmailDomainTable,
    limit: number,
  ) => Promise<Array<CrmEmailDomainRow>>
}>

export type MakePostgresCrmEmailDomainStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the same discipline as the push route.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresCrmEmailDomainStore = (
  deps: MakePostgresCrmEmailDomainStoreDependencies,
): PostgresCrmEmailDomainStore => {
  const withSql = async <A>(
    fn: (unsafe: UnsafeQuery) => Promise<A>,
  ): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(requireUnsafe(client.sql))
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the push route.
      }
    }
  }

  return {
    selectLatestRows: (table, limit) =>
      withSql(unsafe => {
        const spec = CRM_EMAIL_DOMAIN_TABLES[table]
        return unsafe(
          `SELECT ${spec.columns.join(', ')} FROM ${table} ORDER BY ${spec.orderColumn} DESC, ${spec.conflictKey} DESC LIMIT $1`,
          [Math.max(1, Math.min(200, Math.trunc(limit)))],
        )
      }),

    selectRowsByKey: (table, keyColumn, keys) =>
      keys.length === 0
        ? Promise.resolve([])
        : withSql(unsafe => {
            const spec = CRM_EMAIL_DOMAIN_TABLES[table]
            const column = requireKeyColumn(table, keyColumn)
            const placeholders = keys
              .map((_, index) => `$${index + 1}`)
              .join(', ')
            return unsafe(
              `SELECT ${spec.columns.join(', ')} FROM ${table} WHERE ${column} IN (${placeholders})`,
              [...keys],
            )
          }),

    upsertRows: (table, rows) =>
      rows.length === 0
        ? Promise.resolve()
        : withSql(async unsafe => {
            const spec = CRM_EMAIL_DOMAIN_TABLES[table]
            const columnsSql = spec.columns.join(', ')
            const updates = spec.columns
              .filter(column => column !== spec.conflictKey)
              .map(column => `${column} = EXCLUDED.${column}`)
              .join(', ')
            for (const row of rows) {
              const values = spec.columns.map(column =>
                normalizeValue(row[column]),
              )
              const placeholders = values
                .map((_, index) => `$${index + 1}`)
                .join(', ')
              await unsafe(
                `INSERT INTO ${table} (${columnsSql}) VALUES (${placeholders}) ON CONFLICT (${spec.conflictKey}) DO UPDATE SET ${updates}`,
                values as Array<unknown>,
              )
            }
          }),
  }
}

// ---------------------------------------------------------------------------
// The seam handle
// ---------------------------------------------------------------------------

export type CrmEmailDomainHandle = Readonly<{
  /** Brand — discriminates the handle from a bare D1Database. */
  crmEmailDomainSeam: true
  /** The authoritative D1 database (writes and default reads). */
  d1: D1Database
  flags: CrmEmailDomainFlags
  log: CrmEmailDomainLog
  /** Undefined when no KHALA_SYNC_DB binding: plain-D1 degradation. */
  postgres: PostgresCrmEmailDomainStore | undefined
  /** Bounded-retry backoff hook (tests inject a no-op). */
  wait: (ms: number) => Promise<void>
}>

/**
 * What the owning modules' signatures take. A plain `D1Database` keeps
 * working (no mirroring, no routing), so unrelated call sites and tests
 * need no ceremony; `makeCrmEmailDatabaseForEnv` upgrades the CRM/email
 * write entry points — including the dispatch cron — to the dual-write
 * seam.
 */
export type CrmEmailDatabase = D1Database | CrmEmailDomainHandle

export const isCrmEmailDomainHandle = (
  db: CrmEmailDatabase,
): db is CrmEmailDomainHandle =>
  (db as { crmEmailDomainSeam?: unknown }).crmEmailDomainSeam === true

/** The authoritative D1 handle, whichever side of the union arrived. */
export const crmEmailAuthorityDb = (db: CrmEmailDatabase): D1Database =>
  isCrmEmailDomainHandle(db) ? db.d1 : db

const READ_RETRY_DELAYS_MS: ReadonlyArray<number> = [50, 150]

export type MakeCrmEmailDomainHandleDependencies = Readonly<{
  d1: D1Database
  flags: CrmEmailDomainFlags
  log?: CrmEmailDomainLog | undefined
  postgres: PostgresCrmEmailDomainStore | undefined
  wait?: ((ms: number) => Promise<void>) | undefined
}>

export const makeCrmEmailDomainHandle = (
  deps: MakeCrmEmailDomainHandleDependencies,
): CrmEmailDomainHandle => ({
  crmEmailDomainSeam: true,
  d1: deps.d1,
  flags: deps.flags,
  log: deps.log ?? (() => {}),
  postgres: deps.postgres,
  wait:
    deps.wait ??
    ((ms: number) => new Promise(resolve => setTimeout(resolve, ms))),
})

// ---------------------------------------------------------------------------
// Dual-write mirror
// ---------------------------------------------------------------------------

/**
 * Best-effort Postgres mirror after an authoritative D1 write: reads the
 * RESOLVED row(s) back from D1 by `keyColumn` and converges the Postgres
 * twins. NEVER throws — any failure (including the D1 read-back) logs the
 * `khala_sync_crm_dual_write_failed` diagnostic and returns; an email send,
 * a webhook ack, or a CRM import must never fail because the mirror did.
 * On a plain D1Database, a missing binding, or dual-write off it is a
 * no-op. Diagnostics carry keys/hashes only (email-valued keys hashed).
 */
export const mirrorCrmEmailRows = async (
  db: CrmEmailDatabase,
  table: CrmEmailDomainTable,
  keyColumn: string,
  keys: ReadonlyArray<string | number>,
): Promise<void> => {
  if (!isCrmEmailDomainHandle(db)) return
  const { d1, flags, log, postgres } = db
  if (postgres === undefined || !flags.dualWrite || keys.length === 0) return

  try {
    const spec = CRM_EMAIL_DOMAIN_TABLES[table]
    const column = requireKeyColumn(table, keyColumn)
    const placeholders = keys.map(() => '?').join(', ')
    const result = await d1
      .prepare(
        `SELECT ${spec.columns.join(', ')} FROM ${table} WHERE ${column} IN (${placeholders})`,
      )
      .bind(...keys)
      .all<CrmEmailDomainRow>()
    const rows = result.results ?? []
    if (rows.length === 0) return
    await postgres.upsertRows(table, rows)
  } catch (error) {
    log('khala_sync_crm_dual_write_failed', {
      messageSafe: safeMessage(error),
      op: table,
      refs: await publicSafeRefs(keys),
    })
  }
}

// ---------------------------------------------------------------------------
// Flag-routed reads
// ---------------------------------------------------------------------------

const stableStringify = (value: unknown): string =>
  JSON.stringify(value, (_key, val: unknown) =>
    val !== null && typeof val === 'object' && !Array.isArray(val)
      ? Object.fromEntries(
          Object.entries(val as Record<string, unknown>).sort(([a], [b]) =>
            a < b ? -1 : a > b ? 1 : 0,
          ),
        )
      : val,
  )

/**
 * Flag-routed read: d1 (default) | postgres (bounded retry + D1 fallback)
 * | compare (read both, SERVE D1, log mismatches — mismatch logs carry the
 * op name and key-hash refs only, never values). Reads with no Postgres
 * twin yet pass no `readPostgres` and stay on D1 regardless of the flag.
 *
 * The suppression compliance gate routes through here: because every read
 * consults the flag exactly once and serves exactly one store, the send
 * path reads ONE authoritative suppression store at every moment of the
 * cutover — the flip is atomic per-read, verified in staging by a
 * deliberately suppressed send attempt under each mode.
 */
export const crmEmailRead = async <A>(
  db: CrmEmailDatabase,
  op: string,
  refs: ReadonlyArray<string>,
  readD1: () => Promise<A>,
  readPostgres?: (postgres: PostgresCrmEmailDomainStore) => Promise<A>,
): Promise<A> => {
  if (!isCrmEmailDomainHandle(db)) return readD1()
  const { flags, log, postgres, wait } = db
  if (
    postgres === undefined ||
    readPostgres === undefined ||
    flags.reads === 'd1'
  ) {
    return readD1()
  }

  const safeRefs = await publicSafeRefs(refs)

  if (flags.reads === 'postgres') {
    for (let attempt = 0; ; attempt++) {
      try {
        return await readPostgres(postgres)
      } catch (error) {
        const delay = READ_RETRY_DELAYS_MS[attempt]
        if (delay === undefined) {
          log('khala_sync_crm_postgres_read_fallback', {
            messageSafe: safeMessage(error),
            op,
            refs: safeRefs,
          })
          return readD1()
        }
        log('khala_sync_crm_postgres_read_failed', {
          messageSafe: safeMessage(error),
          op,
          refs: safeRefs,
        })
        await wait(delay)
      }
    }
  }

  // compare
  const d1Result = await readD1()
  try {
    const postgresResult = await readPostgres(postgres)
    if (stableStringify(d1Result) !== stableStringify(postgresResult)) {
      log('khala_sync_crm_read_compare_mismatch', {
        messageSafe: 'postgres read differs from d1 authority',
        op,
        refs: safeRefs,
      })
    }
  } catch (error) {
    log('khala_sync_crm_postgres_read_failed', {
      messageSafe: safeMessage(error),
      op,
      refs: safeRefs,
    })
  }
  return d1Result
}

// ---------------------------------------------------------------------------
// Env factory (the index.ts drop-in)
// ---------------------------------------------------------------------------

export type CrmEmailDomainStoreEnv = CrmEmailDomainFlagEnv &
  Readonly<{
    OPENAGENTS_DB: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeCrmEmailDatabaseForEnvOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: CrmEmailDomainLog | undefined
  /**
   * Authority D1 override — compose this seam OVER another domain's
   * mirroring D1Database (e.g. the KS-8.12 sites proxy,
   * `sitesContentDatabaseForEnv`) when one route file writes both
   * domains: non-CRM statements pass through to (and mirror via) the
   * wrapped database, while CRM/email tables ride this seam.
   */
  d1?: D1Database | undefined
}>

const defaultLog: CrmEmailDomainLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

/**
 * The production `CrmEmailDatabase` factory: D1 authority + flag-gated
 * Postgres dual-write/reads. Replaces bare `openAgentsDatabase(env)` at
 * the CRM/email write entry points, including the email-campaign dispatch
 * cron. With no KHALA_SYNC_DB binding (or everything flagged off) it
 * returns the plain D1Database — behavior-identical to before this lane.
 */
export const makeCrmEmailDatabaseForEnv = (
  env: CrmEmailDomainStoreEnv,
  options: MakeCrmEmailDatabaseForEnvOptions = {},
): CrmEmailDatabase => {
  const d1 = options.d1 ?? openAgentsDatabase(env)
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  const flags = crmEmailDomainFlagsFromEnv(env)

  if (
    connectionString === undefined ||
    connectionString.length === 0 ||
    (!flags.dualWrite && flags.reads === 'd1')
  ) {
    return d1
  }

  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  const postgres = makePostgresCrmEmailDomainStore({
    acquireSql: () => makeSqlClient(connectionString),
  })

  return makeCrmEmailDomainHandle({
    d1,
    flags,
    log: options.log ?? defaultLog,
    postgres,
  })
}
