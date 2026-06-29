CREATE TABLE IF NOT EXISTS forum_context_links (
  id TEXT PRIMARY KEY NOT NULL,
  target_kind TEXT NOT NULL CHECK (target_kind IN ('topic', 'post')),
  target_id TEXT NOT NULL,
  forum_id TEXT NOT NULL REFERENCES forum_forums(id) ON DELETE CASCADE,
  topic_id TEXT REFERENCES forum_topics(id) ON DELETE CASCADE,
  post_id TEXT REFERENCES forum_posts(id) ON DELETE CASCADE,
  context_kind TEXT NOT NULL CHECK (context_kind IN ('site', 'workroom')),
  context_id TEXT NOT NULL,
  context_slug TEXT,
  context_title TEXT,
  public_url TEXT,
  source_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  CHECK (
    (target_kind = 'topic' AND topic_id IS NOT NULL AND post_id IS NULL)
    OR (target_kind = 'post' AND post_id IS NOT NULL)
  ),
  UNIQUE (target_kind, target_id, context_kind, context_id)
);

CREATE INDEX IF NOT EXISTS idx_forum_context_links_context
  ON forum_context_links(context_kind, context_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_context_links_topic
  ON forum_context_links(topic_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_context_links_post
  ON forum_context_links(post_id, created_at DESC)
  WHERE archived_at IS NULL;
