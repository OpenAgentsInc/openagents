CREATE TABLE IF NOT EXISTS token_usage_events (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  observed_at TEXT NOT NULL,
  ingested_at TEXT NOT NULL,
  producer_system TEXT NOT NULL,
  source_route TEXT NOT NULL,
  actor_user_id TEXT,
  actor_team_id TEXT,
  account_ref TEXT,
  anonymized_source_ref TEXT,
  run_ref TEXT,
  session_ref TEXT,
  task_ref TEXT,
  repository_ref TEXT,
  provider TEXT,
  model TEXT,
  role_ref TEXT,
  backend_profile TEXT,
  input_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0,
  reasoning_tokens INTEGER NOT NULL DEFAULT 0,
  cache_read_tokens INTEGER NOT NULL DEFAULT 0,
  cache_write_5m_tokens INTEGER NOT NULL DEFAULT 0,
  cache_write_1h_tokens INTEGER NOT NULL DEFAULT 0,
  total_tokens INTEGER NOT NULL DEFAULT 0,
  usage_truth TEXT NOT NULL,
  cost_amount REAL,
  currency TEXT,
  leaderboard_eligible INTEGER NOT NULL DEFAULT 1,
  privacy_opt_out INTEGER NOT NULL DEFAULT 0,
  safe_metadata_json TEXT NOT NULL DEFAULT '{}',
  CHECK (input_tokens >= 0),
  CHECK (output_tokens >= 0),
  CHECK (reasoning_tokens >= 0),
  CHECK (cache_read_tokens >= 0),
  CHECK (cache_write_5m_tokens >= 0),
  CHECK (cache_write_1h_tokens >= 0),
  CHECK (total_tokens >= 0)
);

CREATE INDEX IF NOT EXISTS idx_token_usage_events_observed_at
  ON token_usage_events (observed_at);

CREATE INDEX IF NOT EXISTS idx_token_usage_events_provider_model
  ON token_usage_events (provider, model, observed_at);

CREATE INDEX IF NOT EXISTS idx_token_usage_events_source_route
  ON token_usage_events (producer_system, source_route, observed_at);

CREATE INDEX IF NOT EXISTS idx_token_usage_events_actor_user
  ON token_usage_events (actor_user_id, observed_at);

CREATE INDEX IF NOT EXISTS idx_token_usage_events_actor_team
  ON token_usage_events (actor_team_id, observed_at);

CREATE INDEX IF NOT EXISTS idx_token_usage_events_leaderboard
  ON token_usage_events (leaderboard_eligible, privacy_opt_out, observed_at);
