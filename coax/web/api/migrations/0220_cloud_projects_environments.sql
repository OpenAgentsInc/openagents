-- Phase 1 (#14125): Enrolled computer workspace metadata & Project repo/environment bindings.
-- Policy is coax/lib/project/repo-match.coax, coax/lib/agent/environment/enroll-wire.coax,
-- and coax/lib/project/bindings-wire.coax (plain parameterized SQL only; zero PL/pgSQL routes).

ALTER TABLE chat.environment_connections_v1
  ADD COLUMN IF NOT EXISTS os text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS workspaces_json text NOT NULL DEFAULT '[]',
  ADD COLUMN IF NOT EXISTS last_seen_ms bigint NOT NULL DEFAULT 0;

ALTER TABLE projects.private_v1
  ADD COLUMN IF NOT EXISTS repo_url text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS repo_canonical text NOT NULL DEFAULT '';

CREATE TABLE projects.environment_bindings_v1 (
  account text NOT NULL REFERENCES auth.accounts(id) ON UPDATE CASCADE ON DELETE CASCADE,
  project text NOT NULL,
  host_pubkey text NOT NULL CHECK (host_pubkey ~ '^[0-9a-f]{64}$'),
  workspace_id text NOT NULL CHECK (octet_length(workspace_id) BETWEEN 1 AND 64),
  local_path text NOT NULL DEFAULT '',
  repo_remote text NOT NULL DEFAULT '',
  repo_canonical text NOT NULL DEFAULT '',
  updated_ms bigint NOT NULL DEFAULT 0,
  PRIMARY KEY (account, project, host_pubkey, workspace_id),
  FOREIGN KEY (account, project) REFERENCES projects.private_v1(account, id) ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE INDEX project_env_bindings_repo_v1 ON projects.environment_bindings_v1(account, repo_canonical);

INSERT INTO auth.life_columns (tbl, col, holds, on_merge, on_purge, export, omit, why) VALUES
 ('projects.environment_bindings_v1', 'account', 'account', 'end', 'delete', true, '{}', 'explicit project-to-computer workspace bindings; local host grants remain independent');
