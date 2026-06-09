PRAGMA foreign_keys = OFF;

DROP INDEX IF EXISTS agent_runs_user_created_idx;
DROP INDEX IF EXISTS agent_runs_status_idx;
DROP INDEX IF EXISTS agent_run_events_run_sequence_idx;
DROP INDEX IF EXISTS team_chat_messages_team_created_idx;
DROP INDEX IF EXISTS team_chat_messages_author_idx;
DROP INDEX IF EXISTS team_chat_messages_agent_run_idx;
DROP INDEX IF EXISTS autopilot_token_usage_created_idx;
DROP INDEX IF EXISTS autopilot_token_usage_run_idx;
DROP INDEX IF EXISTS autopilot_token_usage_user_idx;
DROP INDEX IF EXISTS autopilot_token_usage_team_idx;
DROP INDEX IF EXISTS idx_billing_ledger_entries_user_created;
DROP INDEX IF EXISTS idx_billing_ledger_entries_run;
DROP INDEX IF EXISTS idx_billing_usage_cursors_user;

ALTER TABLE agent_runs RENAME TO agent_runs_old;

CREATE TABLE agent_runs (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  team_id TEXT,
  runtime TEXT NOT NULL CHECK (runtime IN ('opencode_codex', 'codex')),
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

INSERT INTO agent_runs
SELECT
  id,
  user_id,
  team_id,
  CASE runtime WHEN 'opencode' THEN 'opencode_codex' ELSE runtime END,
  backend,
  runner_id,
  assignment_kind,
  repository_provider,
  repository_owner,
  repository_repo,
  repository_ref,
  goal,
  provider_account_ref,
  auth_grant_ref,
  external_run_id,
  status,
  event_cursor,
  assignment_json,
  created_at,
  updated_at,
  started_at,
  completed_at,
  failed_at,
  canceled_at
FROM agent_runs_old;

ALTER TABLE agent_run_events RENAME TO agent_run_events_old;

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

INSERT INTO agent_run_events
SELECT
  id,
  run_id,
  sequence,
  type,
  summary,
  status,
  source,
  payload_json,
  artifact_refs_json,
  external_event_id,
  created_at
FROM agent_run_events_old;

DROP TABLE agent_run_events_old;

ALTER TABLE team_chat_messages RENAME TO team_chat_messages_old;

CREATE TABLE team_chat_messages (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL,
  author_user_id TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'message'
    CHECK (kind IN ('message', 'autopilot_intent', 'system')),
  body TEXT NOT NULL CHECK (length(body) > 0 AND length(body) <= 4000),
  autopilot_thread_id TEXT,
  agent_run_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  FOREIGN KEY (team_id) REFERENCES teams(id),
  FOREIGN KEY (author_user_id) REFERENCES users(id),
  FOREIGN KEY (agent_run_id) REFERENCES agent_runs(id)
);

INSERT INTO team_chat_messages
SELECT
  id,
  team_id,
  author_user_id,
  kind,
  body,
  autopilot_thread_id,
  agent_run_id,
  metadata_json,
  created_at,
  updated_at,
  deleted_at
FROM team_chat_messages_old;

DROP TABLE team_chat_messages_old;

ALTER TABLE autopilot_token_usage RENAME TO autopilot_token_usage_old;

CREATE TABLE autopilot_token_usage (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  event_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  provider TEXT,
  model TEXT,
  input_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0,
  reasoning_tokens INTEGER NOT NULL DEFAULT 0,
  cache_read_tokens INTEGER NOT NULL DEFAULT 0,
  cache_write_5m_tokens INTEGER NOT NULL DEFAULT 0,
  cache_write_1h_tokens INTEGER NOT NULL DEFAULT 0,
  total_tokens INTEGER NOT NULL DEFAULT 0,
  source TEXT NOT NULL,
  source_ref TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(run_id, source_ref),
  FOREIGN KEY (run_id) REFERENCES agent_runs(id),
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (team_id) REFERENCES teams(id)
);

INSERT INTO autopilot_token_usage
SELECT
  id,
  run_id,
  event_id,
  user_id,
  team_id,
  provider,
  model,
  input_tokens,
  output_tokens,
  reasoning_tokens,
  cache_read_tokens,
  cache_write_5m_tokens,
  cache_write_1h_tokens,
  total_tokens,
  source,
  source_ref,
  created_at
FROM autopilot_token_usage_old;

DROP TABLE autopilot_token_usage_old;

ALTER TABLE billing_ledger_entries RENAME TO billing_ledger_entries_old;

CREATE TABLE billing_ledger_entries (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
  source TEXT NOT NULL CHECK (
    source IN (
      'trial_grant',
      'coupon',
      'credit_card_placeholder',
      'container_usage',
      'codex_usage',
      'manual_adjustment'
    )
  ),
  description TEXT NOT NULL,
  amount_cents INTEGER NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  quantity INTEGER,
  unit TEXT,
  unit_rate_cents INTEGER,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

INSERT INTO billing_ledger_entries
SELECT
  id,
  user_id,
  team_id,
  run_id,
  source,
  description,
  amount_cents,
  currency,
  quantity,
  unit,
  unit_rate_cents,
  metadata_json,
  idempotency_key,
  created_at
FROM billing_ledger_entries_old;

ALTER TABLE billing_coupon_redemptions RENAME TO billing_coupon_redemptions_old;

CREATE TABLE billing_coupon_redemptions (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  coupon_code TEXT NOT NULL,
  ledger_entry_id TEXT NOT NULL REFERENCES billing_ledger_entries(id) ON DELETE CASCADE,
  redeemed_at TEXT NOT NULL,
  PRIMARY KEY (user_id, coupon_code)
);

INSERT INTO billing_coupon_redemptions
SELECT
  user_id,
  coupon_code,
  ledger_entry_id,
  redeemed_at
FROM billing_coupon_redemptions_old;

DROP TABLE billing_coupon_redemptions_old;
DROP TABLE billing_ledger_entries_old;

ALTER TABLE billing_usage_cursors RENAME TO billing_usage_cursors_old;

CREATE TABLE billing_usage_cursors (
  run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
  meter TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  last_billed_at TEXT NOT NULL,
  total_billed_quantity INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (run_id, meter)
);

INSERT INTO billing_usage_cursors
SELECT
  run_id,
  meter,
  user_id,
  team_id,
  last_billed_at,
  total_billed_quantity,
  updated_at
FROM billing_usage_cursors_old;

DROP TABLE billing_usage_cursors_old;
DROP TABLE agent_runs_old;

CREATE INDEX agent_runs_user_created_idx
  ON agent_runs(user_id, created_at);

CREATE INDEX agent_runs_status_idx
  ON agent_runs(status);

CREATE INDEX agent_run_events_run_sequence_idx
  ON agent_run_events(run_id, sequence);

CREATE INDEX team_chat_messages_team_created_idx
  ON team_chat_messages(team_id, created_at);

CREATE INDEX team_chat_messages_author_idx
  ON team_chat_messages(author_user_id, created_at);

CREATE INDEX team_chat_messages_agent_run_idx
  ON team_chat_messages(agent_run_id)
  WHERE agent_run_id IS NOT NULL;

CREATE INDEX autopilot_token_usage_created_idx
  ON autopilot_token_usage(created_at);

CREATE INDEX autopilot_token_usage_run_idx
  ON autopilot_token_usage(run_id, created_at);

CREATE INDEX autopilot_token_usage_user_idx
  ON autopilot_token_usage(user_id, total_tokens DESC);

CREATE INDEX autopilot_token_usage_team_idx
  ON autopilot_token_usage(team_id, total_tokens DESC)
  WHERE team_id IS NOT NULL;

CREATE INDEX idx_billing_ledger_entries_user_created
  ON billing_ledger_entries(user_id, created_at DESC);

CREATE INDEX idx_billing_ledger_entries_run
  ON billing_ledger_entries(run_id, created_at DESC);

CREATE INDEX idx_billing_usage_cursors_user
  ON billing_usage_cursors(user_id, updated_at DESC);

DROP INDEX IF EXISTS deployments_user_created_idx;
DROP INDEX IF EXISTS deployments_status_idx;
DROP INDEX IF EXISTS deployment_events_deploy_sequence_idx;

ALTER TABLE deployments RENAME TO deployments_old;

CREATE TABLE deployments (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  team_id TEXT,
  service TEXT NOT NULL,
  runtime TEXT NOT NULL CHECK (runtime IN ('opencode_codex', 'codex')),
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

INSERT INTO deployments
SELECT
  id,
  user_id,
  team_id,
  service,
  CASE runtime WHEN 'opencode' THEN 'opencode_codex' ELSE runtime END,
  primary_backend,
  fallback_backend,
  repository_provider,
  repository_owner,
  repository_repo,
  repository_ref,
  external_deploy_id,
  status,
  event_cursor,
  assignment_json,
  created_at,
  updated_at,
  started_at,
  completed_at,
  failed_at,
  canceled_at
FROM deployments_old;

ALTER TABLE deployment_events RENAME TO deployment_events_old;

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

INSERT INTO deployment_events
SELECT
  id,
  deploy_id,
  sequence,
  type,
  summary,
  status,
  source,
  payload_json,
  artifact_refs_json,
  external_event_id,
  created_at
FROM deployment_events_old;

DROP TABLE deployment_events_old;
DROP TABLE deployments_old;

CREATE INDEX deployments_user_created_idx
  ON deployments(user_id, created_at);

CREATE INDEX deployments_status_idx
  ON deployments(status);

CREATE INDEX deployment_events_deploy_sequence_idx
  ON deployment_events(deploy_id, sequence);

PRAGMA foreign_keys = ON;
