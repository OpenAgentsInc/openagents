CREATE TABLE team_projects (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL,
  slug TEXT NOT NULL,
  name TEXT NOT NULL CHECK (length(name) > 0),
  description TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (team_id, slug),
  FOREIGN KEY (team_id) REFERENCES teams(id)
);

CREATE INDEX team_projects_team_active_idx
  ON team_projects(team_id, name)
  WHERE status = 'active' AND archived_at IS NULL;

INSERT INTO team_projects (
  id,
  team_id,
  slug,
  name,
  description,
  status,
  metadata_json,
  created_at,
  updated_at
)
VALUES (
  'project_artanis',
  'team_openagents_core',
  'artanis',
  'Artanis',
  'OpenAgents public-agent and Pylon workstream.',
  'active',
  '{"program":"artanis","surface":"openagents-core-team"}',
  '2026-06-03T22:30:00.000Z',
  '2026-06-03T22:30:00.000Z'
)
ON CONFLICT(id) DO UPDATE SET
  team_id = excluded.team_id,
  slug = excluded.slug,
  name = excluded.name,
  description = excluded.description,
  status = excluded.status,
  metadata_json = excluded.metadata_json,
  archived_at = NULL,
  updated_at = excluded.updated_at;

ALTER TABLE team_chat_messages
  ADD COLUMN project_id TEXT REFERENCES team_projects(id);

CREATE INDEX team_chat_messages_team_project_active_created_idx
  ON team_chat_messages(team_id, project_id, created_at)
  WHERE deleted_at IS NULL AND archived_at IS NULL;

ALTER TABLE agent_runs
  ADD COLUMN project_id TEXT REFERENCES team_projects(id);

CREATE INDEX agent_runs_project_active_created_idx
  ON agent_runs(project_id, created_at)
  WHERE project_id IS NOT NULL AND archived_at IS NULL;
