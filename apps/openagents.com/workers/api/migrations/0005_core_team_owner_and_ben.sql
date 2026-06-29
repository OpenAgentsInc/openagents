UPDATE teams
SET owner_user_id = 'github:14167547',
    updated_at = datetime('now')
WHERE id = 'team_openagents_core';

UPDATE team_memberships
SET role = 'member',
    updated_at = datetime('now')
WHERE team_id = 'team_openagents_core'
  AND user_id <> 'github:14167547'
  AND role = 'owner';

INSERT OR IGNORE INTO team_memberships
  (id, team_id, user_id, role, status, joined_at, created_at, updated_at)
SELECT
  'team_member_openagents_core_github_14167547',
  'team_openagents_core',
  users.id,
  'owner',
  'active',
  datetime('now'),
  datetime('now'),
  datetime('now')
FROM users
WHERE users.id = 'github:14167547'
  AND users.kind = 'human'
  AND users.status = 'active'
  AND users.deleted_at IS NULL;

UPDATE team_memberships
SET role = 'owner',
    status = 'active',
    removed_at = NULL,
    updated_at = datetime('now')
WHERE team_id = 'team_openagents_core'
  AND user_id = 'github:14167547';

INSERT OR IGNORE INTO team_memberships
  (id, team_id, user_id, role, status, joined_at, created_at, updated_at)
SELECT
  'team_member_openagents_core_github_81483518',
  'team_openagents_core',
  users.id,
  'member',
  'active',
  datetime('now'),
  datetime('now'),
  datetime('now')
FROM users
WHERE users.id = 'github:81483518'
  AND users.kind = 'human'
  AND users.status = 'active'
  AND users.deleted_at IS NULL;

UPDATE team_memberships
SET role = 'member',
    status = 'active',
    removed_at = NULL,
    updated_at = datetime('now')
WHERE team_id = 'team_openagents_core'
  AND user_id = 'github:81483518';
