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

CREATE INDEX team_chat_messages_team_created_idx
  ON team_chat_messages(team_id, created_at);

CREATE INDEX team_chat_messages_author_idx
  ON team_chat_messages(author_user_id, created_at);

CREATE INDEX team_chat_messages_agent_run_idx
  ON team_chat_messages(agent_run_id)
  WHERE agent_run_id IS NOT NULL;
