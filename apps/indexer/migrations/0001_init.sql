-- Moltbook indexer: normalized tables + derived signals.
-- Apply: npx wrangler d1 migrations apply openagents-moltbook-index

-- posts
CREATE TABLE IF NOT EXISTS moltbook_posts (
  id TEXT PRIMARY KEY,
  created_at TEXT,
  submolt TEXT,
  title TEXT,
  content TEXT,
  url TEXT,
  author_name TEXT,
  author_id TEXT,
  score INTEGER,
  comment_count INTEGER,
  raw_r2_key TEXT,
  ingested_at TEXT NOT NULL
);

-- comments
CREATE TABLE IF NOT EXISTS moltbook_comments (
  id TEXT PRIMARY KEY,
  post_id TEXT NOT NULL,
  parent_id TEXT,
  created_at TEXT,
  author_name TEXT,
  author_id TEXT,
  content TEXT,
  score INTEGER,
  raw_r2_key TEXT,
  ingested_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_comments_post_id ON moltbook_comments(post_id);

-- authors (best-effort snapshot, not authoritative)
CREATE TABLE IF NOT EXISTS moltbook_authors (
  name TEXT PRIMARY KEY,
  last_seen_at TEXT,
  raw_profile_r2_key TEXT
);

-- cursors/state mirrored from KV for query/debug (optional)
CREATE TABLE IF NOT EXISTS indexer_state (
  k TEXT PRIMARY KEY,
  v TEXT,
  updated_at TEXT NOT NULL
);

-- derived signals (wallet readiness, openclaw mentions, etc.)
CREATE TABLE IF NOT EXISTS derived_signals (
  object_type TEXT NOT NULL,
  object_id TEXT NOT NULL,
  signal TEXT NOT NULL,
  value TEXT,
  created_at TEXT NOT NULL,
  PRIMARY KEY (object_type, object_id, signal)
);
