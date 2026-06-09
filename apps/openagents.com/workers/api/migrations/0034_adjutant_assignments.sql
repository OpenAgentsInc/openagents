CREATE TABLE IF NOT EXISTS adjutant_assignments (
  id TEXT PRIMARY KEY NOT NULL,
  software_order_id TEXT REFERENCES software_orders(id) ON DELETE SET NULL,
  site_id TEXT REFERENCES site_projects(id) ON DELETE SET NULL,
  goal_id TEXT REFERENCES agent_goals(id) ON DELETE SET NULL,
  current_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  project_id TEXT REFERENCES team_projects(id) ON DELETE SET NULL,
  agent_id TEXT NOT NULL CHECK (length(agent_id) > 0),
  assigned_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  assignment_kind TEXT NOT NULL CHECK (
    assignment_kind IN (
      'site_generation',
      'site_adjustment',
      'site_review',
      'site_deployment',
      'general_order_fulfillment'
    )
  ),
  status TEXT NOT NULL CHECK (
    status IN (
      'draft',
      'preflight_pending',
      'blocked',
      'queued',
      'running',
      'review_needed',
      'deployed',
      'delivered',
      'complete',
      'canceled'
    )
  ),
  visibility TEXT NOT NULL CHECK (visibility IN ('private', 'team', 'public')),
  task_spec_path TEXT,
  commit_sha TEXT,
  objective TEXT NOT NULL CHECK (length(objective) > 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT,
  blocked_at TEXT,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS adjutant_assignments_active_order_idx
  ON adjutant_assignments(software_order_id)
  WHERE software_order_id IS NOT NULL
    AND archived_at IS NULL
    AND status NOT IN ('complete', 'canceled');

CREATE UNIQUE INDEX IF NOT EXISTS adjutant_assignments_active_site_idx
  ON adjutant_assignments(site_id)
  WHERE site_id IS NOT NULL
    AND archived_at IS NULL
    AND status NOT IN ('complete', 'canceled');

CREATE INDEX IF NOT EXISTS adjutant_assignments_goal_updated_idx
  ON adjutant_assignments(goal_id, updated_at DESC)
  WHERE goal_id IS NOT NULL
    AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS adjutant_assignments_team_project_updated_idx
  ON adjutant_assignments(team_id, project_id, updated_at DESC)
  WHERE team_id IS NOT NULL
    AND archived_at IS NULL;
