-- Nostr mirror pipeline (Phase 3): track posts to publish to Nostr.
-- See docs/openclaw/bitcoin-wallets-plan.md (Mirror Moltbook -> Nostr).

CREATE TABLE IF NOT EXISTS nostr_mirrors (
  post_id TEXT PRIMARY KEY,
  source TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  event_id TEXT,
  last_published_at TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_nostr_mirrors_status ON nostr_mirrors(status);
CREATE INDEX IF NOT EXISTS idx_nostr_mirrors_created_at ON nostr_mirrors(created_at);

CREATE TABLE IF NOT EXISTS nostr_publish_receipts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  post_id TEXT NOT NULL,
  relay_url TEXT NOT NULL,
  event_id TEXT,
  status TEXT NOT NULL,
  at TEXT NOT NULL,
  FOREIGN KEY (post_id) REFERENCES nostr_mirrors(post_id)
);

CREATE INDEX IF NOT EXISTS idx_nostr_receipts_post_id ON nostr_publish_receipts(post_id);
