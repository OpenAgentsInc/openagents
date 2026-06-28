-- FORGE SU-3 (#6771): smart-Git receive-pack canonical intake.
--
-- Raw packfile bytes remain in private R2 via forge_git_packfile_archives.
-- These D1 tables hold only bounded receive-pack metadata, canonical ref tips,
-- tip object refs, and ref-lock receipts. They do not store raw repository
-- contents, prompts, local paths, provider payloads, secrets, or wallet data.

CREATE TABLE IF NOT EXISTS forge_git_receive_pack_intakes (
  tenant_ref TEXT NOT NULL,
  receive_pack_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  token_ref TEXT NOT NULL,
  subject_ref TEXT NOT NULL,
  change_ref TEXT,
  packfile_ref TEXT,
  packfile_sha256 TEXT,
  packfile_bytes INTEGER NOT NULL CHECK (packfile_bytes >= 0),
  object_format TEXT NOT NULL CHECK (object_format IN ('sha1', 'sha256', 'unknown')),
  state TEXT NOT NULL CHECK (state IN ('accepted', 'rejected')),
  command_count INTEGER NOT NULL CHECK (command_count >= 0),
  ref_updates_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  rejection_code TEXT,
  rejection_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, receive_pack_ref)
);

CREATE INDEX IF NOT EXISTS idx_forge_git_receive_pack_intakes_repository
  ON forge_git_receive_pack_intakes (tenant_ref, repository_ref, created_at);

CREATE INDEX IF NOT EXISTS idx_forge_git_receive_pack_intakes_change
  ON forge_git_receive_pack_intakes (tenant_ref, change_ref, created_at)
  WHERE change_ref IS NOT NULL;

CREATE TABLE IF NOT EXISTS forge_git_refs (
  tenant_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  ref_name TEXT NOT NULL,
  object_id TEXT,
  previous_object_id TEXT,
  object_format TEXT NOT NULL CHECK (object_format IN ('sha1', 'sha256', 'unknown')),
  state TEXT NOT NULL CHECK (state IN ('active', 'deleted')),
  updated_by_change_ref TEXT NOT NULL,
  updated_by_packfile_ref TEXT NOT NULL,
  updated_by_receive_pack_ref TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, repository_ref, ref_name),
  CHECK (
    (state = 'active' AND object_id IS NOT NULL)
    OR (state = 'deleted' AND object_id IS NULL)
  )
);

CREATE INDEX IF NOT EXISTS idx_forge_git_refs_repository_state
  ON forge_git_refs (tenant_ref, repository_ref, state, updated_at);

CREATE TABLE IF NOT EXISTS forge_git_objects (
  tenant_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  object_id TEXT NOT NULL,
  object_format TEXT NOT NULL CHECK (object_format IN ('sha1', 'sha256')),
  packfile_ref TEXT NOT NULL,
  packfile_sha256 TEXT NOT NULL,
  first_seen_at TEXT NOT NULL,
  latest_seen_at TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, repository_ref, object_id)
);

CREATE INDEX IF NOT EXISTS idx_forge_git_objects_packfile
  ON forge_git_objects (tenant_ref, packfile_ref);

CREATE TABLE IF NOT EXISTS forge_git_ref_locks (
  tenant_ref TEXT NOT NULL,
  lock_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  ref_name TEXT NOT NULL,
  receive_pack_ref TEXT NOT NULL,
  expected_old_object_id TEXT NOT NULL,
  new_object_id TEXT NOT NULL,
  action TEXT NOT NULL CHECK (action IN ('create', 'update', 'delete')),
  state TEXT NOT NULL CHECK (state IN ('held', 'applied', 'rejected')),
  acquired_at TEXT NOT NULL,
  released_at TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, lock_ref)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_git_ref_locks_held_ref
  ON forge_git_ref_locks (tenant_ref, repository_ref, ref_name)
  WHERE state = 'held';

CREATE INDEX IF NOT EXISTS idx_forge_git_ref_locks_receive_pack
  ON forge_git_ref_locks (tenant_ref, receive_pack_ref);
