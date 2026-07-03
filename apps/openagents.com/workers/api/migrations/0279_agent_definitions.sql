CREATE TABLE IF NOT EXISTS agent_definitions (
  id TEXT PRIMARY KEY,
  owner_agent_user_id TEXT NOT NULL,
  owner_ref TEXT NOT NULL,
  schema_literal TEXT NOT NULL,
  name TEXT NOT NULL,
  slug TEXT NOT NULL,
  goal TEXT NOT NULL,
  harness_json TEXT NOT NULL,
  toolset_json TEXT NOT NULL,
  triggers_json TEXT NOT NULL,
  lane TEXT NOT NULL,
  budget_json TEXT NOT NULL,
  escalation_json TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  definition_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE(owner_agent_user_id, slug)
);

CREATE INDEX IF NOT EXISTS idx_agent_definitions_owner_updated
  ON agent_definitions(owner_agent_user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_agent_definitions_owner_slug
  ON agent_definitions(owner_agent_user_id, slug);
