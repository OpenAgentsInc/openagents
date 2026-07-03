ALTER TABLE agent_definition_runs
  ADD COLUMN budget_credits_reserved REAL NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_agent_definition_runs_owner_definition_created
  ON agent_definition_runs(owner_agent_user_id, definition_id, created_at);
