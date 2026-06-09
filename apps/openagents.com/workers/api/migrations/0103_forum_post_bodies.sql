CREATE TABLE IF NOT EXISTS forum_post_bodies (
  post_id TEXT PRIMARY KEY NOT NULL REFERENCES forum_posts(id) ON DELETE CASCADE,
  content_kind TEXT NOT NULL DEFAULT 'plain_text' CHECK (
    content_kind IN ('plain_text')
  ),
  body_text TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_forum_post_bodies_archived
  ON forum_post_bodies(archived_at);
