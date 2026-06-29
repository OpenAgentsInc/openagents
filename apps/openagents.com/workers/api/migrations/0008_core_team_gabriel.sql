INSERT OR IGNORE INTO team_memberships
  (id, team_id, user_id, role, status, joined_at, created_at, updated_at)
SELECT
  'team_member_openagents_core_github_51541072',
  'team_openagents_core',
  users.id,
  'member',
  'active',
  datetime('now'),
  datetime('now'),
  datetime('now')
FROM users
WHERE users.id = 'github:51541072'
  AND users.kind = 'human'
  AND users.status = 'active'
  AND users.deleted_at IS NULL;

UPDATE team_memberships
SET role = 'member',
    status = 'active',
    removed_at = NULL,
    updated_at = datetime('now')
WHERE team_id = 'team_openagents_core'
  AND user_id = 'github:51541072';
