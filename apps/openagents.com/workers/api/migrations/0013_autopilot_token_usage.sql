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

CREATE INDEX autopilot_token_usage_created_idx
  ON autopilot_token_usage(created_at);

CREATE INDEX autopilot_token_usage_run_idx
  ON autopilot_token_usage(run_id, created_at);

CREATE INDEX autopilot_token_usage_user_idx
  ON autopilot_token_usage(user_id, total_tokens DESC);

CREATE INDEX autopilot_token_usage_team_idx
  ON autopilot_token_usage(team_id, total_tokens DESC)
  WHERE team_id IS NOT NULL;
