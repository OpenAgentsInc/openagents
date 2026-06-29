ALTER TABLE agent_goal_events ADD COLUMN external_event_id TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_goal_events_goal_external_event
  ON agent_goal_events(goal_id, external_event_id)
  WHERE external_event_id IS NOT NULL;
