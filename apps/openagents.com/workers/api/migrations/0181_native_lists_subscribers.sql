CREATE TABLE IF NOT EXISTS subscriber_lists (
  id TEXT PRIMARY KEY NOT NULL,
  owner_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  slug TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('active', 'paused', 'archived')),
  source_authority_ref TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS subscriber_lists_status_updated_idx
  ON subscriber_lists(status, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS subscriber_lists_owner_idx
  ON subscriber_lists(owner_user_id, updated_at DESC)
  WHERE owner_user_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS list_subscribers (
  id TEXT PRIMARY KEY NOT NULL,
  list_id TEXT NOT NULL REFERENCES subscriber_lists(id) ON DELETE CASCADE,
  email TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('active', 'unsubscribed', 'bounced')
  ),
  source_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  subscribed_at TEXT NOT NULL,
  unsubscribed_at TEXT,
  bounced_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(list_id, email)
);

CREATE INDEX IF NOT EXISTS list_subscribers_list_status_idx
  ON list_subscribers(list_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS list_subscribers_email_idx
  ON list_subscribers(email, updated_at DESC);
