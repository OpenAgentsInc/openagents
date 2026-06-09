ALTER TABLE auth_identities ADD COLUMN provider_username TEXT;

CREATE INDEX auth_identities_provider_username_idx
  ON auth_identities(provider, provider_username);

CREATE TABLE teams (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  slug TEXT UNIQUE,
  kind TEXT NOT NULL DEFAULT 'organization'
    CHECK (kind IN ('organization', 'personal')),
  plan TEXT
    CHECK (plan IS NULL OR plan IN ('free', 'pro', 'team', 'enterprise')),
  logo_url TEXT,
  credits INTEGER,
  owner_user_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('active', 'archived')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (owner_user_id) REFERENCES users(id)
);

CREATE INDEX teams_owner_user_idx
  ON teams(owner_user_id);

CREATE INDEX teams_status_idx
  ON teams(status);

CREATE TABLE team_memberships (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member', 'viewer')),
  status TEXT NOT NULL CHECK (status IN ('active', 'invited', 'removed')),
  invited_by_user_id TEXT,
  joined_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  removed_at TEXT,
  UNIQUE(team_id, user_id),
  FOREIGN KEY (team_id) REFERENCES teams(id),
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (invited_by_user_id) REFERENCES users(id)
);

CREATE INDEX team_memberships_team_idx
  ON team_memberships(team_id);

CREATE INDEX team_memberships_user_status_idx
  ON team_memberships(user_id, status);

CREATE INDEX team_memberships_team_status_idx
  ON team_memberships(team_id, status);

-- One-time bootstrap only. Future GitHub signups are not auto-joined by Worker code.
INSERT INTO teams
  (id, name, slug, kind, plan, owner_user_id, status, created_at, updated_at)
VALUES (
  'team_openagents_core',
  'OpenAgents Core Team',
  'openagents-core-team',
  'organization',
  'team',
  COALESCE(
    (
      SELECT id
      FROM users
      WHERE id = 'github:14167547'
        AND kind = 'human'
        AND status = 'active'
        AND deleted_at IS NULL
      LIMIT 1
    ),
    (
      SELECT id
      FROM users
      WHERE kind = 'human'
        AND status = 'active'
        AND deleted_at IS NULL
      ORDER BY created_at ASC
      LIMIT 1
    )
  ),
  'active',
  datetime('now'),
  datetime('now')
);

INSERT INTO team_memberships
  (id, team_id, user_id, role, status, joined_at, created_at, updated_at)
SELECT
  'team_member_openagents_core_' ||
    replace(replace(replace(users.id, ':', '_'), '-', '_'), '.', '_'),
  'team_openagents_core',
  users.id,
  CASE
    WHEN users.id = (
      SELECT owner_user_id
      FROM teams
      WHERE id = 'team_openagents_core'
    )
    THEN 'owner'
    ELSE 'member'
  END,
  'active',
  datetime('now'),
  datetime('now'),
  datetime('now')
FROM users
WHERE users.kind = 'human'
  AND users.status = 'active'
  AND users.deleted_at IS NULL;
