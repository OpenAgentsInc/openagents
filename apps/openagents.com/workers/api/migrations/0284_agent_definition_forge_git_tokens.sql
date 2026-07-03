-- BA-D1 (#8200): definition dispatch mints per-task Forge git tokens.
--
-- Work records keep public-safe token refs only. Raw token values remain in
-- neither D1 nor workspaces; token rows continue to store hashes/prefixes plus
-- bounded scope metadata.

ALTER TABLE forge_coordination_issues
  ADD COLUMN git_token_refs_json TEXT NOT NULL DEFAULT '[]';

ALTER TABLE forge_git_access_tokens
  ADD COLUMN ref_restrictions_json TEXT NOT NULL DEFAULT '[]';

ALTER TABLE agent_definition_runs
  ADD COLUMN forge_repository_ref TEXT;

ALTER TABLE agent_definition_runs
  ADD COLUMN forge_git_token_refs_json TEXT NOT NULL DEFAULT '[]';

CREATE INDEX IF NOT EXISTS idx_agent_definition_runs_forge_git_tokens
  ON agent_definition_runs(forge_tenant_ref, forge_repository_ref);
