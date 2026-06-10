PRAGMA defer_foreign_keys = true;

CREATE TABLE forum_money_actions_new (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  action_kind TEXT NOT NULL CHECK (
    action_kind IN (
      'topic_create_fee',
      'post_reply_fee',
      'post_reward',
      'post_boost',
      'topic_boost',
      'topic_fund',
      'post_down_signal',
      'report_fee',
      'orange_check'
    )
  ),
  target_forum_id TEXT REFERENCES forum_forums(id) ON DELETE SET NULL,
  target_topic_id TEXT REFERENCES forum_topics(id) ON DELETE SET NULL,
  target_post_id TEXT REFERENCES forum_posts(id) ON DELETE SET NULL,
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('credits', 'sats', 'usd')),
  amount_value INTEGER NOT NULL CHECK (amount_value >= 0),
  payment_event_id TEXT,
  receipt_id TEXT,
  earning_actor_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

INSERT INTO forum_money_actions_new SELECT * FROM forum_money_actions;

DROP TABLE forum_money_actions;

ALTER TABLE forum_money_actions_new RENAME TO forum_money_actions;

CREATE INDEX IF NOT EXISTS idx_forum_money_actions_target
  ON forum_money_actions(target_topic_id, target_post_id, created_at DESC)
  WHERE archived_at IS NULL;
