CREATE TABLE IF NOT EXISTS site_projects (
  id TEXT PRIMARY KEY NOT NULL,
  software_order_id TEXT REFERENCES software_orders(id) ON DELETE SET NULL,
  owner_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  project_id TEXT REFERENCES team_projects(id) ON DELETE SET NULL,
  slug TEXT NOT NULL CHECK (length(slug) > 0),
  title TEXT NOT NULL CHECK (length(title) > 0),
  prompt TEXT NOT NULL CHECK (length(prompt) > 0),
  status TEXT NOT NULL CHECK (
    status IN (
      'draft',
      'generating',
      'generated',
      'needs_review',
      'approved',
      'archived',
      'disabled'
    )
  ),
  access_mode TEXT NOT NULL CHECK (
    access_mode IN (
      'owner_admins',
      'openagents_core',
      'customer_owner',
      'custom_users',
      'public'
    )
  ),
  visibility TEXT NOT NULL CHECK (visibility IN ('private', 'team', 'public')),
  source_repository_provider TEXT CHECK (
    source_repository_provider IS NULL OR source_repository_provider IN ('github')
  ),
  source_repository_owner TEXT,
  source_repository_name TEXT,
  source_repository_ref TEXT,
  active_version_id TEXT,
  active_deployment_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS site_projects_slug_active_idx
  ON site_projects(slug)
  WHERE archived_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS site_projects_order_active_idx
  ON site_projects(software_order_id)
  WHERE software_order_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS site_projects_owner_updated_idx
  ON site_projects(owner_user_id, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS site_projects_team_project_updated_idx
  ON site_projects(team_id, project_id, updated_at DESC)
  WHERE team_id IS NOT NULL AND archived_at IS NULL;

CREATE TABLE IF NOT EXISTS site_versions (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL REFERENCES site_projects(id) ON DELETE CASCADE,
  source_kind TEXT NOT NULL CHECK (
    source_kind IN ('autopilot_generated', 'github_import', 'operator_static')
  ),
  source_commit_sha TEXT,
  source_archive_r2_key TEXT,
  artifact_manifest_r2_key TEXT,
  build_log_r2_key TEXT,
  build_status TEXT NOT NULL CHECK (
    build_status IN ('planned', 'building', 'build_failed', 'saved', 'rejected', 'superseded')
  ),
  build_command TEXT,
  worker_module_r2_key TEXT,
  static_assets_manifest_json TEXT NOT NULL DEFAULT '{}',
  d1_binding_name TEXT,
  r2_binding_name TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_by_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  saved_at TEXT,
  rejected_at TEXT
);

CREATE INDEX IF NOT EXISTS site_versions_site_created_idx
  ON site_versions(site_id, created_at DESC);

CREATE INDEX IF NOT EXISTS site_versions_site_status_idx
  ON site_versions(site_id, build_status, created_at DESC);

CREATE TABLE IF NOT EXISTS site_deployments (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL REFERENCES site_projects(id) ON DELETE CASCADE,
  version_id TEXT NOT NULL REFERENCES site_versions(id) ON DELETE CASCADE,
  slug TEXT NOT NULL,
  url TEXT NOT NULL,
  runtime_kind TEXT NOT NULL CHECK (
    runtime_kind IN ('omega_static_r2', 'workers_for_platforms')
  ),
  runtime_script_name TEXT,
  dispatch_namespace TEXT,
  status TEXT NOT NULL CHECK (
    status IN ('queued', 'deploying', 'active', 'failed', 'disabled', 'rolled_back')
  ),
  deployed_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  external_deployment_id TEXT,
  started_at TEXT,
  activated_at TEXT,
  failed_at TEXT,
  disabled_at TEXT,
  rolled_back_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS site_deployments_site_active_idx
  ON site_deployments(site_id)
  WHERE status = 'active';

CREATE INDEX IF NOT EXISTS site_deployments_site_created_idx
  ON site_deployments(site_id, created_at DESC);

CREATE INDEX IF NOT EXISTS site_deployments_slug_status_idx
  ON site_deployments(slug, status, updated_at DESC);

CREATE TABLE IF NOT EXISTS site_storage_bindings (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL REFERENCES site_projects(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('d1', 'r2', 'kv')),
  binding_name TEXT NOT NULL CHECK (length(binding_name) > 0),
  cloudflare_resource_ref TEXT,
  scope TEXT NOT NULL CHECK (scope IN ('shared_prefix', 'dedicated_resource')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS site_storage_bindings_site_kind_name_idx
  ON site_storage_bindings(site_id, kind, binding_name);

CREATE TABLE IF NOT EXISTS site_environment_values (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL REFERENCES site_projects(id) ON DELETE CASCADE,
  key TEXT NOT NULL CHECK (length(key) > 0),
  kind TEXT NOT NULL CHECK (kind IN ('plain', 'secret')),
  secret_ref TEXT,
  plain_value TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  CHECK (
    (kind = 'secret' AND secret_ref IS NOT NULL AND plain_value IS NULL)
    OR
    (kind = 'plain' AND secret_ref IS NULL)
  )
);

CREATE UNIQUE INDEX IF NOT EXISTS site_environment_values_active_key_idx
  ON site_environment_values(site_id, key)
  WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS site_access_grants (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL REFERENCES site_projects(id) ON DELETE CASCADE,
  principal_kind TEXT NOT NULL CHECK (
    principal_kind IN ('user', 'team', 'admin', 'public')
  ),
  principal_ref TEXT NOT NULL CHECK (length(principal_ref) > 0),
  role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'viewer')),
  created_at TEXT NOT NULL,
  revoked_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS site_access_grants_active_principal_idx
  ON site_access_grants(site_id, principal_kind, principal_ref, role)
  WHERE revoked_at IS NULL;

CREATE TABLE IF NOT EXISTS site_events (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL REFERENCES site_projects(id) ON DELETE CASCADE,
  version_id TEXT REFERENCES site_versions(id) ON DELETE SET NULL,
  deployment_id TEXT REFERENCES site_deployments(id) ON DELETE SET NULL,
  type TEXT NOT NULL CHECK (length(type) > 0),
  summary TEXT NOT NULL CHECK (length(summary) > 0),
  actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  actor_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
  payload_json TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS site_events_site_created_idx
  ON site_events(site_id, created_at DESC);

CREATE INDEX IF NOT EXISTS site_events_version_created_idx
  ON site_events(version_id, created_at DESC)
  WHERE version_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS site_events_deployment_created_idx
  ON site_events(deployment_id, created_at DESC)
  WHERE deployment_id IS NOT NULL;
