CREATE TABLE agent_runs (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  team_id TEXT,
  runtime TEXT NOT NULL CHECK (runtime IN ('opencode')),
  backend TEXT NOT NULL CHECK (backend IN ('shc_vm', 'gcloud_vm', 'local_fake')),
  runner_id TEXT NOT NULL,
  assignment_kind TEXT NOT NULL CHECK (assignment_kind IN ('workroom_agent')),
  repository_provider TEXT NOT NULL CHECK (repository_provider IN ('github')),
  repository_owner TEXT NOT NULL,
  repository_repo TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  goal TEXT NOT NULL,
  provider_account_ref TEXT,
  auth_grant_ref TEXT,
  external_run_id TEXT,
  status TEXT NOT NULL CHECK (
    status IN ('queued', 'running', 'waiting_for_input', 'completed', 'failed', 'canceled')
  ),
  event_cursor INTEGER NOT NULL DEFAULT 0,
  assignment_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT,
  failed_at TEXT,
  canceled_at TEXT,
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (team_id) REFERENCES teams(id)
);

CREATE INDEX agent_runs_user_created_idx
  ON agent_runs(user_id, created_at);

CREATE INDEX agent_runs_status_idx
  ON agent_runs(status);

CREATE TABLE agent_run_events (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  type TEXT NOT NULL,
  summary TEXT NOT NULL,
  status TEXT,
  source TEXT NOT NULL,
  payload_json TEXT,
  artifact_refs_json TEXT NOT NULL DEFAULT '[]',
  external_event_id TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (run_id) REFERENCES agent_runs(id),
  UNIQUE (run_id, sequence),
  UNIQUE (run_id, external_event_id)
);

CREATE INDEX agent_run_events_run_sequence_idx
  ON agent_run_events(run_id, sequence);

CREATE TABLE agent_artifacts (
  id TEXT PRIMARY KEY,
  run_id TEXT,
  deploy_id TEXT,
  kind TEXT NOT NULL,
  object_ref TEXT NOT NULL,
  digest TEXT,
  size_bytes INTEGER,
  visibility TEXT NOT NULL CHECK (visibility IN ('private', 'public', 'operator')),
  redaction_state TEXT NOT NULL CHECK (
    redaction_state IN ('pending', 'redacted', 'safe', 'blocked')
  ),
  retention_mode TEXT NOT NULL CHECK (
    retention_mode IN ('openagents_durable', 'local_only')
  ),
  created_at TEXT NOT NULL
);

CREATE INDEX agent_artifacts_run_idx
  ON agent_artifacts(run_id, created_at);

CREATE INDEX agent_artifacts_deploy_idx
  ON agent_artifacts(deploy_id, created_at);

CREATE TABLE deployments (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  team_id TEXT,
  service TEXT NOT NULL,
  runtime TEXT NOT NULL CHECK (runtime IN ('opencode')),
  primary_backend TEXT NOT NULL CHECK (primary_backend IN ('shc_vm', 'gcloud_vm', 'local_fake')),
  fallback_backend TEXT NOT NULL CHECK (fallback_backend IN ('shc_vm', 'gcloud_vm', 'local_fake')),
  repository_provider TEXT NOT NULL CHECK (repository_provider IN ('github')),
  repository_owner TEXT NOT NULL,
  repository_repo TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  external_deploy_id TEXT,
  status TEXT NOT NULL CHECK (
    status IN ('queued', 'running', 'promoted', 'rolled_back', 'failed', 'canceled')
  ),
  event_cursor INTEGER NOT NULL DEFAULT 0,
  assignment_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT,
  failed_at TEXT,
  canceled_at TEXT,
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (team_id) REFERENCES teams(id)
);

CREATE INDEX deployments_user_created_idx
  ON deployments(user_id, created_at);

CREATE INDEX deployments_status_idx
  ON deployments(status);

CREATE TABLE deployment_events (
  id TEXT PRIMARY KEY,
  deploy_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  type TEXT NOT NULL,
  summary TEXT NOT NULL,
  status TEXT,
  source TEXT NOT NULL,
  payload_json TEXT,
  artifact_refs_json TEXT NOT NULL DEFAULT '[]',
  external_event_id TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (deploy_id) REFERENCES deployments(id),
  UNIQUE (deploy_id, sequence),
  UNIQUE (deploy_id, external_event_id)
);

CREATE INDEX deployment_events_deploy_sequence_idx
  ON deployment_events(deploy_id, sequence);

CREATE TABLE omni_idempotency_keys (
  key TEXT PRIMARY KEY,
  scope TEXT NOT NULL,
  result_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT
);

CREATE INDEX omni_idempotency_keys_scope_idx
  ON omni_idempotency_keys(scope, created_at);
