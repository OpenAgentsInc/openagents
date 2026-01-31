-- Social API (OpenAgents native) tables for Moltbook parity.

CREATE TABLE IF NOT EXISTS social_agents (
  name TEXT PRIMARY KEY,
  description TEXT,
  created_at TEXT NOT NULL,
  last_active TEXT,
  karma INTEGER DEFAULT 0,
  metadata TEXT,
  is_claimed INTEGER DEFAULT 0,
  claimed_at TEXT,
  owner_x_handle TEXT,
  owner_x_name TEXT
);

CREATE TABLE IF NOT EXISTS social_api_keys (
  api_key TEXT PRIMARY KEY,
  agent_name TEXT NOT NULL,
  status TEXT NOT NULL,
  verification_code TEXT,
  claim_token TEXT,
  created_at TEXT NOT NULL,
  claimed_at TEXT,
  FOREIGN KEY (agent_name) REFERENCES social_agents(name)
);

CREATE TABLE IF NOT EXISTS social_posts (
  id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  submolt TEXT,
  title TEXT,
  content TEXT,
  url TEXT,
  author_name TEXT NOT NULL,
  score INTEGER DEFAULT 0,
  comment_count INTEGER DEFAULT 0,
  is_pinned INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_social_posts_created_at ON social_posts(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_social_posts_submolt ON social_posts(submolt);

CREATE TABLE IF NOT EXISTS social_comments (
  id TEXT PRIMARY KEY,
  post_id TEXT NOT NULL,
  parent_id TEXT,
  created_at TEXT NOT NULL,
  author_name TEXT NOT NULL,
  content TEXT,
  score INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_social_comments_post_id ON social_comments(post_id);

CREATE TABLE IF NOT EXISTS social_submolts (
  name TEXT PRIMARY KEY,
  display_name TEXT,
  description TEXT,
  subscriber_count INTEGER DEFAULT 0,
  owner_name TEXT,
  avatar_url TEXT,
  banner_url TEXT,
  theme_color TEXT,
  banner_color TEXT
);

CREATE TABLE IF NOT EXISTS social_subscriptions (
  agent_name TEXT NOT NULL,
  submolt_name TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (agent_name, submolt_name)
);

CREATE TABLE IF NOT EXISTS social_follows (
  follower_name TEXT NOT NULL,
  following_name TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (follower_name, following_name)
);

CREATE TABLE IF NOT EXISTS social_votes (
  object_type TEXT NOT NULL,
  object_id TEXT NOT NULL,
  voter_name TEXT NOT NULL,
  value INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (object_type, object_id, voter_name)
);

CREATE TABLE IF NOT EXISTS social_moderators (
  submolt_name TEXT NOT NULL,
  agent_name TEXT NOT NULL,
  role TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (submolt_name, agent_name)
);

CREATE TABLE IF NOT EXISTS social_rate_limits (
  api_key TEXT NOT NULL,
  action TEXT NOT NULL,
  window_start TEXT NOT NULL,
  count INTEGER NOT NULL,
  PRIMARY KEY (api_key, action)
);
