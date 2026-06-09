CREATE TABLE agent_goals (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  user_id TEXT REFERENCES users(id),
  team_id TEXT REFERENCES teams(id),
  project_id TEXT REFERENCES team_projects(id),
  objective TEXT NOT NULL CHECK (length(objective) > 0),
  status TEXT NOT NULL CHECK (
    status IN ('active', 'paused', 'blocked', 'usage_limited', 'budget_limited', 'complete')
  ),
  visibility TEXT NOT NULL CHECK (visibility IN ('private', 'team', 'public')),
  current_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
  token_budget INTEGER CHECK (token_budget IS NULL OR token_budget > 0),
  tokens_used INTEGER NOT NULL DEFAULT 0 CHECK (tokens_used >= 0),
  time_used_seconds INTEGER NOT NULL DEFAULT 0 CHECK (time_used_seconds >= 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT,
  paused_at TEXT,
  blocked_at TEXT,
  archived_at TEXT
);

CREATE UNIQUE INDEX agent_goals_current_scope_idx
  ON agent_goals(
    agent_id,
    COALESCE(user_id, ''),
    COALESCE(team_id, ''),
    COALESCE(project_id, '')
  )
  WHERE archived_at IS NULL;

CREATE INDEX agent_goals_user_updated_idx
  ON agent_goals(user_id, updated_at)
  WHERE user_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX agent_goals_team_project_updated_idx
  ON agent_goals(team_id, project_id, updated_at)
  WHERE team_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX agent_goals_public_updated_idx
  ON agent_goals(agent_id, updated_at)
  WHERE visibility = 'public' AND archived_at IS NULL;

ALTER TABLE agent_runs
  ADD COLUMN goal_id TEXT REFERENCES agent_goals(id) ON DELETE SET NULL;

CREATE INDEX agent_runs_goal_created_idx
  ON agent_runs(goal_id, created_at)
  WHERE goal_id IS NOT NULL AND archived_at IS NULL;
