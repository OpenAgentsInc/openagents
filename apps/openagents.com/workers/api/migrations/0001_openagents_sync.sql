CREATE TABLE sync_scopes (
  scope TEXT PRIMARY KEY,
  last_seq INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE sync_changes (
  scope TEXT NOT NULL,
  seq INTEGER NOT NULL,
  collection TEXT NOT NULL,
  op TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  value_json TEXT,
  patch_json TEXT,
  mutation_id TEXT,
  actor_id TEXT,
  created_at TEXT NOT NULL,
  PRIMARY KEY (scope, seq)
);

CREATE INDEX sync_changes_scope_seq_idx
  ON sync_changes(scope, seq);

CREATE TABLE sync_mutations (
  mutation_id TEXT PRIMARY KEY,
  scope TEXT NOT NULL,
  actor_id TEXT NOT NULL,
  status TEXT NOT NULL,
  result_json TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE thread_messages (
  id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL,
  org_id TEXT NOT NULL,
  author_id TEXT,
  body_json TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  deleted_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX thread_messages_thread_idx
  ON thread_messages(thread_id, created_at);
