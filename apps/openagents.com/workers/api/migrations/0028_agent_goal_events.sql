CREATE TABLE IF NOT EXISTS agent_goal_events (
  id TEXT PRIMARY KEY,
  goal_id TEXT NOT NULL,
  run_id TEXT,
  expected_goal_id TEXT,
  caller_type TEXT NOT NULL CHECK (caller_type IN ('agent_tool', 'runtime', 'operator', 'browser')),
  event_type TEXT NOT NULL,
  status TEXT CHECK (
    status IS NULL OR status IN (
      'active',
      'paused',
      'blocked',
      'usage_limited',
      'budget_limited',
      'complete'
    )
  ),
  token_delta INTEGER NOT NULL DEFAULT 0,
  time_delta_seconds INTEGER NOT NULL DEFAULT 0,
  payload_json TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (goal_id) REFERENCES agent_goals(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_agent_goal_events_goal_sequence
  ON agent_goal_events(goal_id, created_at, id);

CREATE INDEX IF NOT EXISTS idx_agent_goal_events_run
  ON agent_goal_events(run_id)
  WHERE run_id IS NOT NULL;
