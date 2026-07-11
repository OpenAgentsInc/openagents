-- #8676: immutable WorkContext snapshot for native conversation runs.
--
-- The runtime turn remains the single execution authority. These columns let
-- its canonical agent_run mirror rebuild after later events/host restart
-- without re-reading a mutable chat-thread repository binding.

ALTER TABLE khala_sync_runtime_turns
  ADD COLUMN IF NOT EXISTS work_context_ref text,
  ADD COLUMN IF NOT EXISTS goal_message_id text,
  ADD COLUMN IF NOT EXISTS repository_provider text,
  ADD COLUMN IF NOT EXISTS repository_owner text,
  ADD COLUMN IF NOT EXISTS repository_name text,
  ADD COLUMN IF NOT EXISTS repository_ref text;

CREATE INDEX IF NOT EXISTS idx_khala_sync_runtime_turns_goal_message
  ON khala_sync_runtime_turns (goal_message_id)
  WHERE goal_message_id IS NOT NULL;
