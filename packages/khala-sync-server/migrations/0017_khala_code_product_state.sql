-- KS-8.13 (#8324): Khala Code product state (threads/teams/workspaces)
-- D1 -> Cloud SQL twins plus Khala Sync scope-producing tables.
--
-- This migration mirrors the live D1 schemas for the product-state domain:
-- thread messages/files, teams/memberships/chat/projects/invites, prefilled
-- workspaces, workroom templates, cloud sandbox/fine-tuning sessions, Khala
-- feedback/head-to-head/download/outside-user/trace-plugin receipt surfaces,
-- and share projections. Cross-domain foreign keys are deliberately omitted
-- during the shadow phase; the verifier checks membership/message-chain
-- invariants while the D1 authority remains live.

CREATE TABLE IF NOT EXISTS thread_messages (
  id text PRIMARY KEY,
  thread_id text NOT NULL,
  org_id text NOT NULL,
  author_id text,
  body_json text NOT NULL,
  version bigint NOT NULL DEFAULT 1,
  deleted_at text,
  created_at text NOT NULL,
  updated_at text NOT NULL,
  archived_at text
);

CREATE INDEX IF NOT EXISTS thread_messages_thread_idx
  ON thread_messages(thread_id, created_at);

CREATE INDEX IF NOT EXISTS thread_messages_thread_active_created_idx
  ON thread_messages(thread_id, created_at)
  WHERE deleted_at IS NULL AND archived_at IS NULL;

CREATE TABLE IF NOT EXISTS teams (
  id text PRIMARY KEY,
  name text NOT NULL,
  slug text UNIQUE,
  kind text NOT NULL DEFAULT 'organization',
  plan text,
  logo_url text,
  credits bigint,
  owner_user_id text,
  status text NOT NULL,
  created_at text NOT NULL,
  updated_at text NOT NULL,
  archived_at text
);

CREATE INDEX IF NOT EXISTS teams_owner_user_idx ON teams(owner_user_id);
CREATE INDEX IF NOT EXISTS teams_status_idx ON teams(status);

CREATE TABLE IF NOT EXISTS team_memberships (
  id text PRIMARY KEY,
  team_id text NOT NULL,
  user_id text NOT NULL,
  role text NOT NULL,
  status text NOT NULL,
  invited_by_user_id text,
  joined_at text,
  created_at text NOT NULL,
  updated_at text NOT NULL,
  removed_at text,
  UNIQUE(team_id, user_id)
);

CREATE INDEX IF NOT EXISTS team_memberships_team_idx
  ON team_memberships(team_id);
CREATE INDEX IF NOT EXISTS team_memberships_user_status_idx
  ON team_memberships(user_id, status);
CREATE INDEX IF NOT EXISTS team_memberships_team_status_idx
  ON team_memberships(team_id, status);

CREATE TABLE IF NOT EXISTS team_projects (
  id text PRIMARY KEY,
  team_id text NOT NULL,
  slug text NOT NULL,
  name text NOT NULL,
  description text NOT NULL DEFAULT '',
  status text NOT NULL DEFAULT 'active',
  metadata_json text NOT NULL DEFAULT '{}',
  created_at text NOT NULL,
  updated_at text NOT NULL,
  archived_at text,
  UNIQUE(team_id, slug)
);

CREATE INDEX IF NOT EXISTS team_projects_team_active_idx
  ON team_projects(team_id, name)
  WHERE status = 'active' AND archived_at IS NULL;

CREATE TABLE IF NOT EXISTS team_chat_messages (
  id text PRIMARY KEY,
  team_id text NOT NULL,
  project_id text,
  author_user_id text NOT NULL,
  kind text NOT NULL DEFAULT 'message',
  body text NOT NULL,
  autopilot_thread_id text,
  agent_run_id text,
  metadata_json text NOT NULL DEFAULT '{}',
  created_at text NOT NULL,
  updated_at text NOT NULL,
  deleted_at text,
  archived_at text
);

CREATE INDEX IF NOT EXISTS team_chat_messages_team_created_idx
  ON team_chat_messages(team_id, created_at);
CREATE INDEX IF NOT EXISTS team_chat_messages_author_idx
  ON team_chat_messages(author_user_id, created_at);
CREATE INDEX IF NOT EXISTS team_chat_messages_agent_run_idx
  ON team_chat_messages(agent_run_id)
  WHERE agent_run_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS team_chat_messages_team_active_created_idx
  ON team_chat_messages(team_id, created_at)
  WHERE deleted_at IS NULL AND archived_at IS NULL;
CREATE INDEX IF NOT EXISTS team_chat_messages_team_project_active_created_idx
  ON team_chat_messages(team_id, project_id, created_at)
  WHERE deleted_at IS NULL AND archived_at IS NULL;

CREATE TABLE IF NOT EXISTS thread_files (
  id text PRIMARY KEY,
  scope text NOT NULL,
  thread_id text NOT NULL,
  team_id text,
  owner_user_id text NOT NULL,
  filename text NOT NULL,
  content_type text NOT NULL,
  size_bytes bigint NOT NULL,
  storage_provider text NOT NULL DEFAULT 'r2',
  object_key text NOT NULL UNIQUE,
  checksum_sha256 text,
  upload_status text NOT NULL DEFAULT 'uploaded',
  scan_status text NOT NULL DEFAULT 'skipped',
  metadata_json text NOT NULL DEFAULT '{}',
  created_at text NOT NULL,
  updated_at text NOT NULL,
  deleted_at text,
  download_enabled smallint NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS thread_files_personal_thread_idx
  ON thread_files(owner_user_id, thread_id, created_at)
  WHERE scope = 'personal' AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS thread_files_team_thread_idx
  ON thread_files(team_id, thread_id, created_at)
  WHERE scope = 'team' AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS thread_files_team_created_idx
  ON thread_files(team_id, created_at)
  WHERE scope = 'team' AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS thread_files_object_key_idx
  ON thread_files(object_key);

CREATE TABLE IF NOT EXISTS thread_file_message_refs (
  id text PRIMARY KEY,
  file_id text NOT NULL,
  team_id text,
  thread_id text NOT NULL,
  message_id text NOT NULL,
  reference_kind text NOT NULL,
  created_at text NOT NULL,
  updated_at text NOT NULL,
  deleted_at text,
  UNIQUE(file_id, message_id, reference_kind)
);

CREATE INDEX IF NOT EXISTS thread_file_message_refs_file_created_idx
  ON thread_file_message_refs(file_id, created_at)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS thread_file_message_refs_message_idx
  ON thread_file_message_refs(message_id)
  WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS team_workspace_invites (
  id text PRIMARY KEY,
  team_id text NOT NULL,
  project_id text,
  invitee_email text NOT NULL,
  invitee_email_normalized text NOT NULL,
  role text NOT NULL,
  status text NOT NULL,
  token_hash text NOT NULL UNIQUE,
  invited_by_actor_ref text NOT NULL,
  accepted_by_user_id text,
  email_message_id text,
  created_at text NOT NULL,
  updated_at text NOT NULL,
  expires_at text NOT NULL,
  accepted_at text,
  revoked_at text,
  last_sent_at text,
  send_count bigint NOT NULL DEFAULT 0,
  metadata_json text NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS team_workspace_invites_team_status_idx
  ON team_workspace_invites(team_id, status, updated_at DESC);
CREATE INDEX IF NOT EXISTS team_workspace_invites_project_status_idx
  ON team_workspace_invites(project_id, status, updated_at DESC)
  WHERE project_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS team_workspace_invites_invitee_status_idx
  ON team_workspace_invites(invitee_email_normalized, status, updated_at DESC);
CREATE INDEX IF NOT EXISTS team_workspace_invites_token_hash_idx
  ON team_workspace_invites(token_hash);
CREATE UNIQUE INDEX IF NOT EXISTS team_workspace_invites_pending_target_idx
  ON team_workspace_invites(team_id, COALESCE(project_id, ''), invitee_email_normalized)
  WHERE status = 'pending';

CREATE TABLE IF NOT EXISTS prefilled_workspaces (
  id text PRIMARY KEY,
  holder_user_id text,
  holder_ref text NOT NULL,
  project_name text NOT NULL,
  status text NOT NULL,
  intro_receipt_json text NOT NULL DEFAULT '{}',
  created_at text NOT NULL,
  updated_at text NOT NULL,
  archived_at text,
  invited_at text,
  first_viewed_at text,
  first_claimed_at text,
  first_run_at text,
  last_viewed_at text,
  revisit_count bigint NOT NULL DEFAULT 0,
  access_mode text NOT NULL DEFAULT 'public_safe',
  private_team_id text,
  private_project_id text
);

CREATE INDEX IF NOT EXISTS prefilled_workspaces_holder_idx
  ON prefilled_workspaces(holder_user_id, updated_at DESC)
  WHERE holder_user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS prefilled_workspaces_status_idx
  ON prefilled_workspaces(status, updated_at DESC)
  WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS prefilled_workspaces_engagement_idx
  ON prefilled_workspaces(status, first_claimed_at, first_run_at, updated_at DESC)
  WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS prefilled_workspaces_private_team_idx
  ON prefilled_workspaces(private_team_id, private_project_id, updated_at DESC)
  WHERE access_mode = 'private_team' AND archived_at IS NULL;

CREATE TABLE IF NOT EXISTS prefilled_workspace_seeded_memory (
  workspace_id text NOT NULL,
  position bigint NOT NULL,
  label text NOT NULL,
  value text NOT NULL,
  public_source_ref text NOT NULL,
  PRIMARY KEY(workspace_id, position)
);

CREATE TABLE IF NOT EXISTS prefilled_workspace_starter_workflows (
  workspace_id text NOT NULL,
  position bigint NOT NULL,
  title text NOT NULL,
  description text NOT NULL,
  outcome_kind text NOT NULL,
  status text NOT NULL,
  PRIMARY KEY(workspace_id, position)
);

CREATE TABLE IF NOT EXISTS workroom_kind_templates (
  kind text PRIMARY KEY,
  accepted_outcome_work_kind text NOT NULL,
  description_ref text NOT NULL,
  privacy_constraint text NOT NULL,
  proof_policy text NOT NULL,
  public_projection_policy text NOT NULL,
  review_policy text NOT NULL,
  closeout_requirements_json text NOT NULL DEFAULT '[]',
  required_artifacts_json text NOT NULL DEFAULT '[]',
  required_evidence_json text NOT NULL DEFAULT '[]',
  created_at text NOT NULL,
  updated_at text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workroom_kind_templates_work_kind
  ON workroom_kind_templates(accepted_outcome_work_kind, kind);

CREATE TABLE IF NOT EXISTS workroom_template_packages (
  id text PRIMARY KEY,
  package_ref text NOT NULL UNIQUE,
  version_ref text NOT NULL,
  display_name text NOT NULL,
  state text NOT NULL,
  authority_boundary text NOT NULL DEFAULT 'package_review_projection_only',
  no_deployment smallint NOT NULL DEFAULT 1,
  no_external_runner_launch smallint NOT NULL DEFAULT 1,
  no_marketplace_listing smallint NOT NULL DEFAULT 1,
  no_payment_mutation smallint NOT NULL DEFAULT 1,
  no_runtime_promotion smallint NOT NULL DEFAULT 1,
  approval_policy_refs_json text NOT NULL DEFAULT '[]',
  blocker_refs_json text NOT NULL DEFAULT '[]',
  caveat_refs_json text NOT NULL DEFAULT '[]',
  evidence_requirement_refs_json text NOT NULL DEFAULT '[]',
  operator_diagnostic_refs_json text NOT NULL DEFAULT '[]',
  org_private_enablement_refs_json text NOT NULL DEFAULT '[]',
  outcome_template_refs_json text NOT NULL DEFAULT '[]',
  proof_rule_refs_json text NOT NULL DEFAULT '[]',
  promotion_refs_json text NOT NULL DEFAULT '[]',
  public_projection_refs_json text NOT NULL DEFAULT '[]',
  required_artifact_refs_json text NOT NULL DEFAULT '[]',
  review_refs_json text NOT NULL DEFAULT '[]',
  runner_need_refs_json text NOT NULL DEFAULT '[]',
  source_refs_json text NOT NULL DEFAULT '[]',
  template_version_refs_json text NOT NULL DEFAULT '[]',
  ui_binding_refs_json text NOT NULL DEFAULT '[]',
  validation_refs_json text NOT NULL DEFAULT '[]',
  created_at text NOT NULL,
  updated_at text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workroom_template_packages_state_updated
  ON workroom_template_packages(state, updated_at DESC);

CREATE TABLE IF NOT EXISTS workroom_template_package_versions (
  id text PRIMARY KEY,
  package_id text NOT NULL,
  template_version_ref text NOT NULL,
  approval_policy_refs_json text NOT NULL DEFAULT '[]',
  caveat_refs_json text NOT NULL DEFAULT '[]',
  evidence_requirement_refs_json text NOT NULL DEFAULT '[]',
  outcome_template_refs_json text NOT NULL DEFAULT '[]',
  proof_rule_refs_json text NOT NULL DEFAULT '[]',
  required_artifact_refs_json text NOT NULL DEFAULT '[]',
  runner_need_refs_json text NOT NULL DEFAULT '[]',
  source_refs_json text NOT NULL DEFAULT '[]',
  ui_binding_refs_json text NOT NULL DEFAULT '[]',
  created_at text NOT NULL,
  updated_at text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workroom_template_package_versions_package
  ON workroom_template_package_versions(package_id, created_at DESC);

CREATE TABLE IF NOT EXISTS cloud_sandbox_sessions (
  sandbox_id text PRIMARY KEY,
  account_ref text NOT NULL,
  image text NOT NULL,
  ttl_seconds bigint NOT NULL,
  status text NOT NULL,
  connection_ref text,
  usage_json text,
  created_at text NOT NULL,
  updated_at text NOT NULL,
  expires_at_hint text,
  completed_at text
);

CREATE INDEX IF NOT EXISTS idx_cloud_sandbox_sessions_account
  ON cloud_sandbox_sessions(account_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS cloud_fine_tuning_jobs (
  job_id text PRIMARY KEY,
  account_ref text NOT NULL,
  base_model text NOT NULL,
  dataset_ref text NOT NULL,
  suffix text,
  status text NOT NULL,
  fine_tuned_model text,
  usage_json text,
  created_at text NOT NULL,
  updated_at text NOT NULL,
  completed_at text
);

CREATE INDEX IF NOT EXISTS idx_cloud_fine_tuning_jobs_account
  ON cloud_fine_tuning_jobs(account_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS cloud_fine_tuned_models (
  model_id text PRIMARY KEY,
  account_ref text NOT NULL,
  job_id text NOT NULL,
  base_model text NOT NULL,
  dataset_ref text NOT NULL,
  status text NOT NULL,
  created_at text NOT NULL,
  updated_at text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cloud_fine_tuned_models_account
  ON cloud_fine_tuned_models(account_ref, status, created_at DESC);

CREATE TABLE IF NOT EXISTS khala_feedback (
  feedback_ref text PRIMARY KEY,
  trace_ref text,
  feedback_text text NOT NULL,
  source text NOT NULL,
  client_version text,
  user_agent text,
  created_at text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_khala_feedback_created_at
  ON khala_feedback(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_khala_feedback_trace_ref
  ON khala_feedback(trace_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS khala_head_to_head_snapshots (
  head_to_head_ref text PRIMARY KEY,
  head_to_head_json text NOT NULL,
  published_at text NOT NULL,
  created_at text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_khala_head_to_head_snapshots_published_at
  ON khala_head_to_head_snapshots(published_at DESC, head_to_head_ref ASC);

CREATE TABLE IF NOT EXISTS khala_unsupported_requests (
  request_ref text PRIMARY KEY,
  source_kind text NOT NULL,
  source_ref text NOT NULL,
  title text NOT NULL,
  summary text NOT NULL DEFAULT '',
  triage_kind text NOT NULL,
  status text NOT NULL,
  forum_topic_ref text,
  github_issue_ref text,
  evidence_refs_json text NOT NULL DEFAULT '[]',
  suggested_issue_title text NOT NULL,
  created_at text NOT NULL,
  updated_at text NOT NULL,
  UNIQUE(source_kind, source_ref)
);

CREATE INDEX IF NOT EXISTS idx_khala_unsupported_requests_status_updated
  ON khala_unsupported_requests(status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_khala_unsupported_requests_triage_updated
  ON khala_unsupported_requests(triage_kind, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_khala_unsupported_requests_source
  ON khala_unsupported_requests(source_kind, source_ref);

CREATE TABLE IF NOT EXISTS khala_code_download_events (
  event_ref text PRIMARY KEY,
  product text NOT NULL,
  artifact_kind text NOT NULL,
  channel text NOT NULL,
  artifact_ref text NOT NULL,
  occurred_at text NOT NULL,
  public_countable smallint NOT NULL DEFAULT 1,
  source_ref text NOT NULL,
  idempotency_key text NOT NULL UNIQUE,
  created_at text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_khala_code_download_events_public_counts
  ON khala_code_download_events(product, public_countable, artifact_kind, channel, occurred_at);

CREATE TABLE IF NOT EXISTS khala_code_outside_user_run_receipts (
  receipt_ref text PRIMARY KEY,
  idempotency_key text NOT NULL UNIQUE,
  app_version text NOT NULL,
  platform text NOT NULL,
  arch text NOT NULL,
  distribution_channel text NOT NULL,
  codex_cli_state text NOT NULL,
  codex_auth_state text NOT NULL,
  pylon_state text NOT NULL,
  submitted_at text NOT NULL,
  created_at text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_khala_code_outside_user_run_receipts_submitted_at
  ON khala_code_outside_user_run_receipts(submitted_at);

CREATE TABLE IF NOT EXISTS khala_code_trace_plugin_revenue_share_precedents (
  receipt_ref text PRIMARY KEY,
  idempotency_key text NOT NULL UNIQUE,
  consented_trace_receipt_ref text NOT NULL,
  trace_digest_ref text NOT NULL,
  plugin_admission_receipt_ref text NOT NULL,
  plugin_registry_receipt_ref text NOT NULL,
  plugin_ref text NOT NULL,
  plugin_digest_ref text NOT NULL,
  plugin_route_ref text NOT NULL,
  routed_request_ref text NOT NULL,
  usage_event_ref text NOT NULL,
  usage_idempotency_ref text NOT NULL,
  contributor_attribution_ref text NOT NULL,
  gross_revenue_msats bigint NOT NULL,
  contributor_share_msats bigint NOT NULL,
  amount_envelope_ref text NOT NULL,
  payout_rail text NOT NULL,
  payout_receipt_ref text NOT NULL,
  settlement_receipt_ref text NOT NULL,
  recorded_at text NOT NULL,
  created_at text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_khala_code_trace_plugin_revenue_share_recorded_at
  ON khala_code_trace_plugin_revenue_share_precedents(recorded_at);
CREATE INDEX IF NOT EXISTS idx_khala_code_trace_plugin_revenue_share_plugin_ref
  ON khala_code_trace_plugin_revenue_share_precedents(plugin_ref, recorded_at);

CREATE TABLE IF NOT EXISTS share_projections (
  id text PRIMARY KEY,
  canonical_url text NOT NULL,
  source_kind text NOT NULL,
  source_id text NOT NULL,
  owner_user_id text NOT NULL,
  team_id text,
  project_id text,
  audience_json text NOT NULL,
  title text NOT NULL,
  summary text,
  status text NOT NULL DEFAULT 'active',
  projection_version bigint NOT NULL DEFAULT 1,
  projection_json text NOT NULL,
  projection_object_key text,
  redaction_policy_id text NOT NULL DEFAULT 'default',
  created_at text NOT NULL,
  updated_at text NOT NULL,
  revoked_at text,
  expires_at text
);

CREATE INDEX IF NOT EXISTS share_projections_source_idx
  ON share_projections(source_kind, source_id)
  WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS share_projections_owner_idx
  ON share_projections(owner_user_id, created_at);
CREATE INDEX IF NOT EXISTS share_projections_team_idx
  ON share_projections(team_id, created_at)
  WHERE team_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS share_projection_recipients (
  share_id text NOT NULL,
  subject_kind text NOT NULL,
  subject_id text NOT NULL,
  display_name text NOT NULL,
  created_at text NOT NULL,
  PRIMARY KEY(share_id, subject_kind, subject_id)
);

CREATE INDEX IF NOT EXISTS share_projection_recipients_subject_idx
  ON share_projection_recipients(subject_kind, subject_id);
