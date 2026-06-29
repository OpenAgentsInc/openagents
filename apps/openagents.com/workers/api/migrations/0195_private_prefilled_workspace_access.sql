-- Private team/project access mode for prefilled workspaces (#5154).
--
-- Existing rows remain public_safe and keep the first signed-in holder claim
-- behavior. private_team rows must be gated by active team membership before
-- seeded material is returned.

ALTER TABLE prefilled_workspaces
  ADD COLUMN access_mode TEXT NOT NULL DEFAULT 'public_safe'
    CHECK (access_mode IN ('public_safe', 'private_team'));

ALTER TABLE prefilled_workspaces
  ADD COLUMN private_team_id TEXT REFERENCES teams(id) ON DELETE SET NULL;

ALTER TABLE prefilled_workspaces
  ADD COLUMN private_project_id TEXT REFERENCES team_projects(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS prefilled_workspaces_private_team_idx
  ON prefilled_workspaces(private_team_id, private_project_id, updated_at DESC)
  WHERE access_mode = 'private_team'
    AND archived_at IS NULL;
