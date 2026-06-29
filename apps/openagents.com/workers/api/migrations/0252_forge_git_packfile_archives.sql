-- FORGE-3 (#6748): private R2 git packfile archive ledger.
--
-- Raw git packfile bytes live in the private ARTIFACTS R2 bucket. D1 stores
-- only refs, bounded metadata, object-format, byte count, digest, and JSON
-- command/capability summaries so Forge can deduplicate and inspect intake
-- without projecting private repository contents into relational rows.

CREATE TABLE IF NOT EXISTS forge_git_packfile_archives (
  tenant_ref TEXT NOT NULL,
  packfile_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  change_ref TEXT,
  receive_pack_ref TEXT,
  artifact_r2_key TEXT NOT NULL,
  packfile_sha256 TEXT NOT NULL,
  packfile_bytes INTEGER NOT NULL CHECK (packfile_bytes >= 0),
  object_format TEXT NOT NULL CHECK (object_format IN ('sha1', 'sha256', 'unknown')),
  command_count INTEGER NOT NULL CHECK (command_count >= 0),
  capabilities_json TEXT NOT NULL DEFAULT '[]',
  ref_updates_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  content_type TEXT NOT NULL DEFAULT 'application/x-git-packed-objects',
  visibility TEXT NOT NULL CHECK (visibility = 'operator_only'),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, packfile_ref)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_git_packfile_archives_digest
  ON forge_git_packfile_archives (tenant_ref, packfile_sha256);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_git_packfile_archives_r2_key
  ON forge_git_packfile_archives (artifact_r2_key);

CREATE INDEX IF NOT EXISTS idx_forge_git_packfile_archives_repository
  ON forge_git_packfile_archives (tenant_ref, repository_ref, created_at);

CREATE INDEX IF NOT EXISTS idx_forge_git_packfile_archives_change
  ON forge_git_packfile_archives (tenant_ref, change_ref, created_at)
  WHERE change_ref IS NOT NULL;
