-- Our team on the other hosts this site serves (everglade.world, docs/verse/everglade-domain.md).
-- A browser's cookies are per host, so the oa_team cookie that marks an admin's browser on
-- openagents.com (0037_analytics_team.sql) never reaches everglade.world, and the team's play there
-- counted as visitors. An admin opens /api/admin/team-link?host=everglade.world on openagents.com
-- (admin.c team_link): it keeps a one-use token here, good for ten minutes, and sends the browser to
-- that host's /api/analytics/team?t=TOKEN (analytics.c team_mark), which uses it up and sets oa_team
-- there. Only the token's hash is kept. Additive, and safe to run twice.
CREATE TABLE IF NOT EXISTS analytics.team_links (
  token_sha256 text PRIMARY KEY,
  pubkey text NOT NULL,          -- the admin who made it
  host text NOT NULL,            -- the host it marks
  expires_ms bigint NOT NULL,
  used_ms bigint
);
