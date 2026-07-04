import {
  EntityId,
  EntityType,
  teamScope,
  threadScope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import type { SyncSql, SyncTransactionSql } from "./sql.js"

/**
 * KS-8.13 (#8324): Khala Code product-state domain.
 *
 * Shared table metadata and Postgres converge/upsert helpers for the
 * product-state tables that move from D1 to Cloud SQL while also becoming
 * Khala Sync scope producers. This file is imported by both the Worker mirror
 * and the backfill/verify CLI, so keep it runtime-neutral: no Node built-ins.
 */

export type KhalaCodeProductStateTable =
  | "thread_messages"
  | "thread_files"
  | "thread_file_message_refs"
  | "teams"
  | "team_memberships"
  | "team_chat_messages"
  | "team_projects"
  | "team_workspace_invites"
  | "prefilled_workspaces"
  | "prefilled_workspace_seeded_memory"
  | "prefilled_workspace_starter_workflows"
  | "workroom_kind_templates"
  | "workroom_template_packages"
  | "workroom_template_package_versions"
  | "cloud_sandbox_sessions"
  | "cloud_fine_tuning_jobs"
  | "cloud_fine_tuned_models"
  | "khala_feedback"
  | "khala_head_to_head_snapshots"
  | "khala_unsupported_requests"
  | "khala_code_download_events"
  | "khala_code_outside_user_run_receipts"
  | "khala_code_trace_plugin_revenue_share_precedents"
  | "share_projections"
  | "share_projection_recipients"

export type KhalaCodeProductStateTableSpec = Readonly<{
  columns: ReadonlyArray<string>
  /** Natural converge key used by backfill, mirror read-back, and ON CONFLICT. */
  keyColumns: ReadonlyArray<string>
  /** Newest-first ordering column for hash verification. */
  orderColumn: string
}>

export type KhalaCodeProductStateRow = Readonly<Record<string, unknown>>

export const KHALA_CODE_PRODUCT_STATE_TABLES: ReadonlyArray<KhalaCodeProductStateTable> = [
  "teams",
  "team_memberships",
  "team_projects",
  "thread_messages",
  "team_chat_messages",
  "thread_files",
  "thread_file_message_refs",
  "team_workspace_invites",
  "prefilled_workspaces",
  "prefilled_workspace_seeded_memory",
  "prefilled_workspace_starter_workflows",
  "workroom_kind_templates",
  "workroom_template_packages",
  "workroom_template_package_versions",
  "cloud_sandbox_sessions",
  "cloud_fine_tuning_jobs",
  "cloud_fine_tuned_models",
  "khala_feedback",
  "khala_head_to_head_snapshots",
  "khala_unsupported_requests",
  "khala_code_download_events",
  "khala_code_outside_user_run_receipts",
  "khala_code_trace_plugin_revenue_share_precedents",
  "share_projections",
  "share_projection_recipients",
]

export const KHALA_CODE_PRODUCT_STATE_TABLE_SPECS: Readonly<
  Record<KhalaCodeProductStateTable, KhalaCodeProductStateTableSpec>
> = {
  cloud_fine_tuned_models: {
    columns: [
      "model_id",
      "account_ref",
      "job_id",
      "base_model",
      "dataset_ref",
      "status",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["model_id"],
    orderColumn: "updated_at",
  },
  cloud_fine_tuning_jobs: {
    columns: [
      "job_id",
      "account_ref",
      "base_model",
      "dataset_ref",
      "suffix",
      "status",
      "fine_tuned_model",
      "usage_json",
      "created_at",
      "updated_at",
      "completed_at",
    ],
    keyColumns: ["job_id"],
    orderColumn: "updated_at",
  },
  cloud_sandbox_sessions: {
    columns: [
      "sandbox_id",
      "account_ref",
      "image",
      "ttl_seconds",
      "status",
      "connection_ref",
      "usage_json",
      "created_at",
      "updated_at",
      "expires_at_hint",
      "completed_at",
    ],
    keyColumns: ["sandbox_id"],
    orderColumn: "updated_at",
  },
  khala_code_download_events: {
    columns: [
      "event_ref",
      "product",
      "artifact_kind",
      "channel",
      "artifact_ref",
      "occurred_at",
      "public_countable",
      "source_ref",
      "idempotency_key",
      "created_at",
    ],
    keyColumns: ["event_ref"],
    orderColumn: "created_at",
  },
  khala_code_outside_user_run_receipts: {
    columns: [
      "receipt_ref",
      "idempotency_key",
      "app_version",
      "platform",
      "arch",
      "distribution_channel",
      "codex_cli_state",
      "codex_auth_state",
      "pylon_state",
      "submitted_at",
      "created_at",
    ],
    keyColumns: ["receipt_ref"],
    orderColumn: "submitted_at",
  },
  khala_code_trace_plugin_revenue_share_precedents: {
    columns: [
      "receipt_ref",
      "idempotency_key",
      "consented_trace_receipt_ref",
      "trace_digest_ref",
      "plugin_admission_receipt_ref",
      "plugin_registry_receipt_ref",
      "plugin_ref",
      "plugin_digest_ref",
      "plugin_route_ref",
      "routed_request_ref",
      "usage_event_ref",
      "usage_idempotency_ref",
      "contributor_attribution_ref",
      "gross_revenue_msats",
      "contributor_share_msats",
      "amount_envelope_ref",
      "payout_rail",
      "payout_receipt_ref",
      "settlement_receipt_ref",
      "recorded_at",
      "created_at",
    ],
    keyColumns: ["receipt_ref"],
    orderColumn: "recorded_at",
  },
  khala_feedback: {
    columns: [
      "feedback_ref",
      "trace_ref",
      "feedback_text",
      "source",
      "client_version",
      "user_agent",
      "created_at",
    ],
    keyColumns: ["feedback_ref"],
    orderColumn: "created_at",
  },
  khala_head_to_head_snapshots: {
    columns: [
      "head_to_head_ref",
      "head_to_head_json",
      "published_at",
      "created_at",
    ],
    keyColumns: ["head_to_head_ref"],
    orderColumn: "published_at",
  },
  khala_unsupported_requests: {
    columns: [
      "request_ref",
      "source_kind",
      "source_ref",
      "title",
      "summary",
      "triage_kind",
      "status",
      "forum_topic_ref",
      "github_issue_ref",
      "evidence_refs_json",
      "suggested_issue_title",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["source_kind", "source_ref"],
    orderColumn: "updated_at",
  },
  prefilled_workspace_seeded_memory: {
    columns: [
      "workspace_id",
      "position",
      "label",
      "value",
      "public_source_ref",
    ],
    keyColumns: ["workspace_id", "position"],
    orderColumn: "position",
  },
  prefilled_workspace_starter_workflows: {
    columns: [
      "workspace_id",
      "position",
      "title",
      "description",
      "outcome_kind",
      "status",
    ],
    keyColumns: ["workspace_id", "position"],
    orderColumn: "position",
  },
  prefilled_workspaces: {
    columns: [
      "id",
      "holder_user_id",
      "holder_ref",
      "project_name",
      "status",
      "intro_receipt_json",
      "created_at",
      "updated_at",
      "archived_at",
      "invited_at",
      "first_viewed_at",
      "first_claimed_at",
      "first_run_at",
      "last_viewed_at",
      "revisit_count",
      "access_mode",
      "private_team_id",
      "private_project_id",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  share_projection_recipients: {
    columns: [
      "share_id",
      "subject_kind",
      "subject_id",
      "display_name",
      "created_at",
    ],
    keyColumns: ["share_id", "subject_kind", "subject_id"],
    orderColumn: "created_at",
  },
  share_projections: {
    columns: [
      "id",
      "canonical_url",
      "source_kind",
      "source_id",
      "owner_user_id",
      "team_id",
      "project_id",
      "audience_json",
      "title",
      "summary",
      "status",
      "projection_version",
      "projection_json",
      "projection_object_key",
      "redaction_policy_id",
      "created_at",
      "updated_at",
      "revoked_at",
      "expires_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  team_chat_messages: {
    columns: [
      "id",
      "team_id",
      "project_id",
      "author_user_id",
      "kind",
      "body",
      "autopilot_thread_id",
      "agent_run_id",
      "metadata_json",
      "created_at",
      "updated_at",
      "deleted_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  team_memberships: {
    columns: [
      "id",
      "team_id",
      "user_id",
      "role",
      "status",
      "invited_by_user_id",
      "joined_at",
      "created_at",
      "updated_at",
      "removed_at",
    ],
    keyColumns: ["team_id", "user_id"],
    orderColumn: "updated_at",
  },
  team_projects: {
    columns: [
      "id",
      "team_id",
      "slug",
      "name",
      "description",
      "status",
      "metadata_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  team_workspace_invites: {
    columns: [
      "id",
      "team_id",
      "project_id",
      "invitee_email",
      "invitee_email_normalized",
      "role",
      "status",
      "token_hash",
      "invited_by_actor_ref",
      "accepted_by_user_id",
      "email_message_id",
      "created_at",
      "updated_at",
      "expires_at",
      "accepted_at",
      "revoked_at",
      "last_sent_at",
      "send_count",
      "metadata_json",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  teams: {
    columns: [
      "id",
      "name",
      "slug",
      "kind",
      "plan",
      "logo_url",
      "credits",
      "owner_user_id",
      "status",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  thread_file_message_refs: {
    columns: [
      "id",
      "file_id",
      "team_id",
      "thread_id",
      "message_id",
      "reference_kind",
      "created_at",
      "updated_at",
      "deleted_at",
    ],
    keyColumns: ["file_id", "message_id", "reference_kind"],
    orderColumn: "created_at",
  },
  thread_files: {
    columns: [
      "id",
      "scope",
      "thread_id",
      "team_id",
      "owner_user_id",
      "filename",
      "content_type",
      "size_bytes",
      "storage_provider",
      "object_key",
      "checksum_sha256",
      "upload_status",
      "scan_status",
      "metadata_json",
      "created_at",
      "updated_at",
      "deleted_at",
      "download_enabled",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  thread_messages: {
    columns: [
      "id",
      "thread_id",
      "org_id",
      "author_id",
      "body_json",
      "version",
      "deleted_at",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  workroom_kind_templates: {
    columns: [
      "kind",
      "accepted_outcome_work_kind",
      "description_ref",
      "privacy_constraint",
      "proof_policy",
      "public_projection_policy",
      "review_policy",
      "closeout_requirements_json",
      "required_artifacts_json",
      "required_evidence_json",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["kind"],
    orderColumn: "updated_at",
  },
  workroom_template_package_versions: {
    columns: [
      "id",
      "package_id",
      "template_version_ref",
      "approval_policy_refs_json",
      "caveat_refs_json",
      "evidence_requirement_refs_json",
      "outcome_template_refs_json",
      "proof_rule_refs_json",
      "required_artifact_refs_json",
      "runner_need_refs_json",
      "source_refs_json",
      "ui_binding_refs_json",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  workroom_template_packages: {
    columns: [
      "id",
      "package_ref",
      "version_ref",
      "display_name",
      "state",
      "authority_boundary",
      "no_deployment",
      "no_external_runner_launch",
      "no_marketplace_listing",
      "no_payment_mutation",
      "no_runtime_promotion",
      "approval_policy_refs_json",
      "blocker_refs_json",
      "caveat_refs_json",
      "evidence_requirement_refs_json",
      "operator_diagnostic_refs_json",
      "org_private_enablement_refs_json",
      "outcome_template_refs_json",
      "proof_rule_refs_json",
      "promotion_refs_json",
      "public_projection_refs_json",
      "required_artifact_refs_json",
      "review_refs_json",
      "runner_need_refs_json",
      "source_refs_json",
      "template_version_refs_json",
      "ui_binding_refs_json",
      "validation_refs_json",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
}

export const isKhalaCodeProductStateTable = (
  value: string,
): value is KhalaCodeProductStateTable =>
  Object.prototype.hasOwnProperty.call(
    KHALA_CODE_PRODUCT_STATE_TABLE_SPECS,
    value,
  )

export const normalizeKhalaCodeProductStateValue = (
  value: unknown,
): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "bigint") return value.toString()
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

export type KhalaCodeProductStateUnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

export const requireKhalaCodeProductStateUnsafe = (
  sql: SyncSql,
): KhalaCodeProductStateUnsafeQuery => {
  const unsafe = (
    sql as SyncSql & { unsafe?: KhalaCodeProductStateUnsafeQuery }
  ).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "Khala Code product-state store requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

const conflictTarget = (columns: ReadonlyArray<string>): string =>
  columns.join(", ")

export const upsertKhalaCodeProductStateRows = async (
  sql: SyncSql | SyncTransactionSql,
  table: KhalaCodeProductStateTable,
  rows: ReadonlyArray<KhalaCodeProductStateRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireKhalaCodeProductStateUnsafe(sql as SyncSql)
  const spec = KHALA_CODE_PRODUCT_STATE_TABLE_SPECS[table]
  const setClauses = spec.columns
    .filter((column) => !spec.keyColumns.includes(column))
    .map((column) => `${column} = EXCLUDED.${column}`)
    .join(", ")
  const updateClause =
    setClauses.length === 0 ? "DO NOTHING" : `DO UPDATE SET ${setClauses}`

  let touched = 0
  for (const row of rows) {
    const values = spec.columns.map((column) =>
      normalizeKhalaCodeProductStateValue(row[column]),
    )
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${spec.columns.join(", ")}) VALUES (${placeholders}) ON CONFLICT (${conflictTarget(spec.keyColumns)}) ${updateClause} RETURNING 1 AS touched`,
      values as Array<unknown>,
    )
    touched += result.length
  }
  return touched
}

export const deleteKhalaCodeProductStateRows = async (
  sql: SyncSql | SyncTransactionSql,
  table: KhalaCodeProductStateTable,
  whereColumns: ReadonlyArray<string>,
  whereValues: ReadonlyArray<unknown>,
): Promise<number> => {
  if (whereColumns.length === 0 || whereColumns.length !== whereValues.length) {
    return 0
  }
  const unsafe = requireKhalaCodeProductStateUnsafe(sql as SyncSql)
  const clauses = whereColumns.map(
    (column, index) => `${column} IS NOT DISTINCT FROM $${index + 1}`,
  )
  const result = await unsafe(
    `DELETE FROM ${table} WHERE ${clauses.join(" AND ")} RETURNING 1 AS touched`,
    whereValues.map(normalizeKhalaCodeProductStateValue) as Array<unknown>,
  )
  return result.length
}

export type KhalaCodeProductStateScopeChange = Readonly<{
  scope: SyncScope
  entityType: EntityType
  entityId: EntityId
  postImage: KhalaCodeProductStateRow
}>

const stringValue = (row: KhalaCodeProductStateRow, column: string): string | undefined => {
  const value = row[column]
  return value === undefined || value === null || String(value).length === 0
    ? undefined
    : String(value)
}

const entityIdForRow = (
  table: KhalaCodeProductStateTable,
  row: KhalaCodeProductStateRow,
): EntityId => {
  const spec = KHALA_CODE_PRODUCT_STATE_TABLE_SPECS[table]
  const raw = spec.keyColumns.map((column) => stringValue(row, column) ?? "").join(":")
  return EntityId.make(raw.length === 0 ? table : raw)
}

const entityTypeForTable = (table: KhalaCodeProductStateTable): EntityType =>
  EntityType.make(table.endsWith("s") ? table.slice(0, -1) : table)

/**
 * Route a mirrored row into the Khala Sync scopes that clients subscribe to.
 * The full post-image is the row copied from D1, keeping the client store
 * self-healing and avoiding a second projection format during migration.
 */
export const scopeChangesForKhalaCodeProductStateRow = (
  table: KhalaCodeProductStateTable,
  row: KhalaCodeProductStateRow,
): ReadonlyArray<KhalaCodeProductStateScopeChange> => {
  const changes: Array<KhalaCodeProductStateScopeChange> = []
  const push = (scope: SyncScope, entityType = entityTypeForTable(table), entityId = entityIdForRow(table, row)) => {
    changes.push({ entityId, entityType, postImage: row, scope })
  }
  const teamId = stringValue(row, "team_id")
  const threadId = stringValue(row, "thread_id")
  const autopilotThreadId = stringValue(row, "autopilot_thread_id")

  switch (table) {
    case "teams": {
      const id = stringValue(row, "id")
      if (id !== undefined) push(teamScope(id))
      break
    }
    case "team_memberships":
    case "team_projects":
    case "team_workspace_invites":
      if (teamId !== undefined) push(teamScope(teamId))
      break
    case "team_chat_messages":
      if (teamId !== undefined) push(teamScope(teamId))
      if (autopilotThreadId !== undefined) push(threadScope(autopilotThreadId))
      break
    case "thread_messages":
      if (threadId !== undefined) push(threadScope(threadId))
      break
    case "thread_files":
    case "thread_file_message_refs":
      if (teamId !== undefined) push(teamScope(teamId))
      if (threadId !== undefined) push(threadScope(threadId))
      break
    case "prefilled_workspaces": {
      const privateTeamId = stringValue(row, "private_team_id")
      if (privateTeamId !== undefined) push(teamScope(privateTeamId))
      break
    }
    case "share_projections":
      if (teamId !== undefined) push(teamScope(teamId))
      break
    default:
      break
  }

  return changes
}
