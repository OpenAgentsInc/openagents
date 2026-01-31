-- Social API feed state (personalized feeds).

CREATE TABLE IF NOT EXISTS social_feed_state (
  agent_name TEXT PRIMARY KEY,
  last_seen_at TEXT,
  updated_at TEXT NOT NULL
);
