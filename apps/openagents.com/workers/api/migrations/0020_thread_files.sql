CREATE TABLE thread_files (
  id TEXT PRIMARY KEY,
  scope TEXT NOT NULL CHECK (scope IN ('personal', 'team')),
  thread_id TEXT NOT NULL,
  team_id TEXT,
  owner_user_id TEXT NOT NULL,
  filename TEXT NOT NULL CHECK (length(filename) > 0),
  content_type TEXT NOT NULL,
  size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
  storage_provider TEXT NOT NULL DEFAULT 'r2' CHECK (storage_provider IN ('r2')),
  object_key TEXT NOT NULL UNIQUE,
  checksum_sha256 TEXT,
  upload_status TEXT NOT NULL DEFAULT 'uploaded'
    CHECK (upload_status IN ('uploaded', 'failed')),
  scan_status TEXT NOT NULL DEFAULT 'skipped'
    CHECK (scan_status IN ('pending', 'passed', 'failed', 'skipped')),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  CHECK (
    (scope = 'personal' AND team_id IS NULL)
    OR (scope = 'team' AND team_id IS NOT NULL)
  ),
  FOREIGN KEY (team_id) REFERENCES teams(id),
  FOREIGN KEY (owner_user_id) REFERENCES users(id)
);

CREATE INDEX thread_files_personal_thread_idx
  ON thread_files(owner_user_id, thread_id, created_at)
  WHERE scope = 'personal' AND deleted_at IS NULL;

CREATE INDEX thread_files_team_thread_idx
  ON thread_files(team_id, thread_id, created_at)
  WHERE scope = 'team' AND deleted_at IS NULL;

CREATE INDEX thread_files_team_created_idx
  ON thread_files(team_id, created_at)
  WHERE scope = 'team' AND deleted_at IS NULL;

CREATE INDEX thread_files_object_key_idx
  ON thread_files(object_key);
