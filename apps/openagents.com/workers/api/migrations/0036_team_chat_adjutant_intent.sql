DROP INDEX IF EXISTS team_chat_messages_team_created_idx;
DROP INDEX IF EXISTS team_chat_messages_author_idx;
DROP INDEX IF EXISTS team_chat_messages_agent_run_idx;
DROP INDEX IF EXISTS team_chat_messages_team_active_created_idx;
DROP INDEX IF EXISTS team_chat_messages_team_project_active_created_idx;

ALTER TABLE team_chat_messages RENAME TO team_chat_messages_old;

CREATE TABLE team_chat_messages (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL,
  project_id TEXT REFERENCES team_projects(id),
  author_user_id TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'message'
    CHECK (kind IN ('message', 'autopilot_intent', 'adjutant_intent', 'system')),
  body TEXT NOT NULL CHECK (length(body) > 0 AND length(body) <= 4000),
  autopilot_thread_id TEXT,
  agent_run_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  archived_at TEXT,
  FOREIGN KEY (team_id) REFERENCES teams(id),
  FOREIGN KEY (author_user_id) REFERENCES users(id),
  FOREIGN KEY (agent_run_id) REFERENCES agent_runs(id)
);

INSERT INTO team_chat_messages (
  id,
  team_id,
  project_id,
  author_user_id,
  kind,
  body,
  autopilot_thread_id,
  agent_run_id,
  metadata_json,
  created_at,
  updated_at,
  deleted_at,
  archived_at
)
SELECT
  id,
  team_id,
  project_id,
  author_user_id,
  kind,
  body,
  autopilot_thread_id,
  agent_run_id,
  metadata_json,
  created_at,
  updated_at,
  deleted_at,
  archived_at
FROM team_chat_messages_old;

DROP TABLE team_chat_messages_old;

CREATE INDEX team_chat_messages_team_created_idx
  ON team_chat_messages(team_id, created_at);

CREATE INDEX team_chat_messages_author_idx
  ON team_chat_messages(author_user_id, created_at);

CREATE INDEX team_chat_messages_agent_run_idx
  ON team_chat_messages(agent_run_id)
  WHERE agent_run_id IS NOT NULL;

CREATE INDEX team_chat_messages_team_active_created_idx
  ON team_chat_messages(team_id, created_at)
  WHERE deleted_at IS NULL AND archived_at IS NULL;

CREATE INDEX team_chat_messages_team_project_active_created_idx
  ON team_chat_messages(team_id, project_id, created_at)
  WHERE deleted_at IS NULL AND archived_at IS NULL;
