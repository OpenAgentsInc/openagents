CREATE TABLE share_projections (
  id TEXT PRIMARY KEY,
  canonical_url TEXT NOT NULL,
  source_kind TEXT NOT NULL CHECK (
    source_kind IN ('agent-run', 'team-thread', 'team-project-thread')
  ),
  source_id TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  team_id TEXT,
  project_id TEXT,
  audience_json TEXT NOT NULL,
  title TEXT NOT NULL CHECK (length(title) > 0),
  summary TEXT,
  status TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('active', 'revoked')),
  projection_version INTEGER NOT NULL DEFAULT 1,
  projection_json TEXT NOT NULL,
  projection_object_key TEXT,
  redaction_policy_id TEXT NOT NULL DEFAULT 'default',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  revoked_at TEXT,
  expires_at TEXT,
  FOREIGN KEY (owner_user_id) REFERENCES users(id),
  FOREIGN KEY (team_id) REFERENCES teams(id),
  FOREIGN KEY (project_id) REFERENCES team_projects(id)
);

CREATE INDEX share_projections_source_idx
  ON share_projections(source_kind, source_id)
  WHERE revoked_at IS NULL;

CREATE INDEX share_projections_owner_idx
  ON share_projections(owner_user_id, created_at);

CREATE INDEX share_projections_team_idx
  ON share_projections(team_id, created_at)
  WHERE team_id IS NOT NULL;

CREATE TABLE share_projection_recipients (
  share_id TEXT NOT NULL,
  subject_kind TEXT NOT NULL CHECK (subject_kind IN ('user', 'email', 'team')),
  subject_id TEXT NOT NULL,
  display_name TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (share_id, subject_kind, subject_id),
  FOREIGN KEY (share_id) REFERENCES share_projections(id)
);

CREATE INDEX share_projection_recipients_subject_idx
  ON share_projection_recipients(subject_kind, subject_id);
