CREATE TABLE tenant_custom_hostnames (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL,
  hostname TEXT NOT NULL UNIQUE,
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'verified', 'active', 'disabled')),
  verification_token TEXT NOT NULL,
  verified_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (team_id) REFERENCES teams(id)
);

CREATE INDEX tenant_custom_hostnames_team_idx
  ON tenant_custom_hostnames(team_id);

CREATE INDEX tenant_custom_hostnames_status_idx
  ON tenant_custom_hostnames(status);
