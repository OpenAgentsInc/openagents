CREATE TABLE IF NOT EXISTS team_workspace_invites (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL,
  project_id TEXT,
  invitee_email TEXT NOT NULL,
  invitee_email_normalized TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('admin', 'member', 'viewer')),
  status TEXT NOT NULL CHECK (status IN ('pending', 'accepted', 'expired', 'revoked')),
  token_hash TEXT NOT NULL UNIQUE,
  invited_by_actor_ref TEXT NOT NULL,
  accepted_by_user_id TEXT,
  email_message_id TEXT REFERENCES email_messages(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  accepted_at TEXT,
  revoked_at TEXT,
  last_sent_at TEXT,
  send_count INTEGER NOT NULL DEFAULT 0,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  FOREIGN KEY (team_id) REFERENCES teams(id),
  FOREIGN KEY (project_id) REFERENCES team_projects(id),
  FOREIGN KEY (accepted_by_user_id) REFERENCES users(id)
);

CREATE INDEX IF NOT EXISTS team_workspace_invites_team_status_idx
  ON team_workspace_invites(team_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS team_workspace_invites_project_status_idx
  ON team_workspace_invites(project_id, status, updated_at DESC)
  WHERE project_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS team_workspace_invites_invitee_status_idx
  ON team_workspace_invites(invitee_email_normalized, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS team_workspace_invites_token_hash_idx
  ON team_workspace_invites(token_hash);

CREATE UNIQUE INDEX IF NOT EXISTS team_workspace_invites_pending_target_idx
  ON team_workspace_invites(
    team_id,
    COALESCE(project_id, ''),
    invitee_email_normalized
  )
  WHERE status = 'pending';
