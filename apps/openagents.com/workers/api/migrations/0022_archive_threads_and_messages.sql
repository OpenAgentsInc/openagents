ALTER TABLE agent_runs
  ADD COLUMN archived_at TEXT;

ALTER TABLE team_chat_messages
  ADD COLUMN archived_at TEXT;

ALTER TABLE thread_messages
  ADD COLUMN archived_at TEXT;

CREATE INDEX agent_runs_user_active_created_idx
  ON agent_runs(user_id, created_at)
  WHERE archived_at IS NULL;

CREATE INDEX team_chat_messages_team_active_created_idx
  ON team_chat_messages(team_id, created_at)
  WHERE deleted_at IS NULL AND archived_at IS NULL;

CREATE INDEX thread_messages_thread_active_created_idx
  ON thread_messages(thread_id, created_at)
  WHERE deleted_at IS NULL AND archived_at IS NULL;
