CREATE TABLE IF NOT EXISTS agent_definition_triggers (
  trigger_id TEXT PRIMARY KEY,
  owner_agent_user_id TEXT NOT NULL,
  owner_ref TEXT NOT NULL,
  definition_id TEXT NOT NULL,
  trigger_ref TEXT NOT NULL,
  trigger_kind TEXT NOT NULL,
  trigger_json TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('enabled', 'paused')),
  consecutive_failures INTEGER NOT NULL DEFAULT 0 CHECK (consecutive_failures >= 0),
  next_run_at TEXT,
  paused_at TEXT,
  pause_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(owner_agent_user_id, trigger_ref)
);

CREATE INDEX IF NOT EXISTS idx_agent_definition_triggers_owner_definition
  ON agent_definition_triggers(owner_agent_user_id, definition_id);

CREATE INDEX IF NOT EXISTS idx_agent_definition_triggers_due
  ON agent_definition_triggers(state, next_run_at);

CREATE INDEX IF NOT EXISTS idx_agent_definition_triggers_kind
  ON agent_definition_triggers(trigger_kind);
