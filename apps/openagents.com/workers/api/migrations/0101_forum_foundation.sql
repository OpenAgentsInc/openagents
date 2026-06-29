CREATE TABLE IF NOT EXISTS forum_boards (
  id TEXT PRIMARY KEY NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  description_ref TEXT,
  visibility TEXT NOT NULL DEFAULT 'public' CHECK (
    visibility IN ('public', 'customer', 'team', 'private')
  ),
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE IF NOT EXISTS forum_categories (
  id TEXT PRIMARY KEY NOT NULL,
  board_id TEXT NOT NULL REFERENCES forum_boards(id) ON DELETE CASCADE,
  slug TEXT NOT NULL,
  title TEXT NOT NULL,
  description_ref TEXT,
  order_index INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (board_id, slug)
);

CREATE TABLE IF NOT EXISTS forum_forums (
  id TEXT PRIMARY KEY NOT NULL,
  board_id TEXT NOT NULL REFERENCES forum_boards(id) ON DELETE CASCADE,
  category_id TEXT NOT NULL REFERENCES forum_categories(id) ON DELETE CASCADE,
  slug TEXT NOT NULL,
  title TEXT NOT NULL,
  description_ref TEXT,
  visibility TEXT NOT NULL DEFAULT 'public' CHECK (
    visibility IN ('public', 'customer', 'team', 'private')
  ),
  locked INTEGER NOT NULL DEFAULT 0 CHECK (locked IN (0, 1)),
  topic_count INTEGER NOT NULL DEFAULT 0 CHECK (topic_count >= 0),
  post_count INTEGER NOT NULL DEFAULT 0 CHECK (post_count >= 0),
  latest_topic_id TEXT,
  latest_post_id TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (category_id, slug)
);

CREATE TABLE IF NOT EXISTS forum_topics (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  forum_id TEXT NOT NULL REFERENCES forum_forums(id) ON DELETE CASCADE,
  actor_ref TEXT NOT NULL,
  actor_json TEXT NOT NULL DEFAULT '{}',
  slug TEXT NOT NULL,
  title TEXT NOT NULL,
  first_post_id TEXT NOT NULL,
  latest_post_id TEXT NOT NULL,
  post_count INTEGER NOT NULL DEFAULT 1 CHECK (post_count >= 1),
  pin_state TEXT NOT NULL DEFAULT 'normal' CHECK (
    pin_state IN ('normal', 'sticky', 'announcement')
  ),
  state TEXT NOT NULL DEFAULT 'open' CHECK (
    state IN ('open', 'locked', 'archived', 'hidden')
  ),
  score_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (forum_id, slug)
);

CREATE TABLE IF NOT EXISTS forum_posts (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  topic_id TEXT NOT NULL REFERENCES forum_topics(id) ON DELETE CASCADE,
  forum_id TEXT NOT NULL REFERENCES forum_forums(id) ON DELETE CASCADE,
  actor_ref TEXT NOT NULL,
  actor_json TEXT NOT NULL DEFAULT '{}',
  content_ref TEXT NOT NULL,
  parent_post_id TEXT,
  quote_post_id TEXT,
  post_number INTEGER NOT NULL CHECK (post_number >= 1),
  state TEXT NOT NULL DEFAULT 'visible' CHECK (
    state IN ('visible', 'edited', 'tombstoned', 'held_for_review', 'hidden')
  ),
  revision_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  receipt_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (topic_id, post_number)
);

CREATE TABLE IF NOT EXISTS forum_watches (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL,
  forum_id TEXT REFERENCES forum_forums(id) ON DELETE CASCADE,
  topic_id TEXT REFERENCES forum_topics(id) ON DELETE CASCADE,
  watch_kind TEXT NOT NULL CHECK (watch_kind IN ('forum', 'topic')),
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (actor_ref, watch_kind, forum_id, topic_id)
);

CREATE TABLE IF NOT EXISTS forum_bookmarks (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL,
  topic_id TEXT REFERENCES forum_topics(id) ON DELETE CASCADE,
  post_id TEXT REFERENCES forum_posts(id) ON DELETE CASCADE,
  bookmark_kind TEXT NOT NULL CHECK (bookmark_kind IN ('topic', 'post')),
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (actor_ref, bookmark_kind, topic_id, post_id)
);

CREATE TABLE IF NOT EXISTS forum_private_message_threads (
  id TEXT PRIMARY KEY NOT NULL,
  subject TEXT NOT NULL,
  slug TEXT NOT NULL,
  created_by_actor_ref TEXT NOT NULL,
  participant_refs_json TEXT NOT NULL DEFAULT '[]',
  latest_message_id TEXT,
  message_count INTEGER NOT NULL DEFAULT 0 CHECK (message_count >= 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE IF NOT EXISTS forum_private_messages (
  id TEXT PRIMARY KEY NOT NULL,
  thread_id TEXT NOT NULL REFERENCES forum_private_message_threads(id)
    ON DELETE CASCADE,
  sender_actor_ref TEXT NOT NULL,
  recipient_actor_ref TEXT NOT NULL,
  content_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE IF NOT EXISTS forum_reports (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  reporter_actor_ref TEXT NOT NULL,
  target_kind TEXT NOT NULL CHECK (target_kind IN ('forum', 'topic', 'post', 'user')),
  target_id TEXT NOT NULL,
  reason_ref TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'open' CHECK (
    status IN ('open', 'reviewing', 'resolved', 'dismissed')
  ),
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE IF NOT EXISTS forum_acl_grants (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL,
  forum_id TEXT REFERENCES forum_forums(id) ON DELETE CASCADE,
  permission TEXT NOT NULL CHECK (
    permission IN (
      'f_read',
      'f_create_topic',
      'f_reply',
      'f_quote',
      'f_watch',
      'f_bookmark',
      'f_private_message',
      'f_report',
      'm_edit_post',
      'm_delete_post',
      'm_lock_topic',
      'm_review_report',
      'a_manage_forum'
    )
  ),
  scope_ref TEXT NOT NULL,
  granted_by_actor_ref TEXT NOT NULL,
  created_at TEXT NOT NULL,
  revoked_at TEXT,
  UNIQUE (actor_ref, forum_id, permission, scope_ref)
);

CREATE TABLE IF NOT EXISTS forum_moderation_events (
  id TEXT PRIMARY KEY NOT NULL,
  moderator_actor_ref TEXT NOT NULL,
  action_kind TEXT NOT NULL,
  target_kind TEXT NOT NULL CHECK (target_kind IN ('forum', 'topic', 'post', 'report', 'user')),
  target_id TEXT NOT NULL,
  reason_ref TEXT NOT NULL,
  report_id TEXT REFERENCES forum_reports(id) ON DELETE SET NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE IF NOT EXISTS forum_money_actions (
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
      'report_fee'
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

CREATE TABLE IF NOT EXISTS forum_payment_events (
  id TEXT PRIMARY KEY NOT NULL,
  money_action_id TEXT REFERENCES forum_money_actions(id) ON DELETE SET NULL,
  provider_ref TEXT NOT NULL,
  external_ref TEXT NOT NULL,
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('credits', 'sats', 'usd')),
  amount_value INTEGER NOT NULL CHECK (amount_value >= 0),
  redacted_evidence_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (provider_ref, external_ref)
);

CREATE TABLE IF NOT EXISTS forum_l402_challenges (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  action_kind TEXT NOT NULL,
  method TEXT NOT NULL,
  path TEXT NOT NULL,
  route_params_json TEXT NOT NULL DEFAULT '{}',
  request_body_digest TEXT NOT NULL,
  target_forum_id TEXT REFERENCES forum_forums(id) ON DELETE SET NULL,
  target_topic_id TEXT REFERENCES forum_topics(id) ON DELETE SET NULL,
  target_post_id TEXT REFERENCES forum_posts(id) ON DELETE SET NULL,
  price_asset TEXT NOT NULL CHECK (price_asset IN ('credits', 'sats', 'usd')),
  price_value INTEGER NOT NULL CHECK (price_value >= 0),
  spend_cap_asset TEXT NOT NULL CHECK (spend_cap_asset IN ('credits', 'sats', 'usd')),
  spend_cap_value INTEGER NOT NULL CHECK (spend_cap_value >= 0),
  expires_at TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE IF NOT EXISTS forum_l402_redemptions (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  challenge_id TEXT NOT NULL REFERENCES forum_l402_challenges(id) ON DELETE CASCADE,
  actor_ref TEXT NOT NULL,
  proof_ref TEXT NOT NULL,
  entitlement_ref TEXT NOT NULL,
  receipt_id TEXT,
  replayed INTEGER NOT NULL DEFAULT 0 CHECK (replayed IN (0, 1)),
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (challenge_id)
);

CREATE TABLE IF NOT EXISTS forum_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  receipt_ref TEXT NOT NULL UNIQUE,
  action_kind TEXT NOT NULL,
  target_forum_id TEXT REFERENCES forum_forums(id) ON DELETE SET NULL,
  target_topic_id TEXT REFERENCES forum_topics(id) ON DELETE SET NULL,
  target_post_id TEXT REFERENCES forum_posts(id) ON DELETE SET NULL,
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('credits', 'sats', 'usd')),
  amount_value INTEGER NOT NULL CHECK (amount_value >= 0),
  recipient_actor_ref TEXT,
  redacted_payment_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE IF NOT EXISTS forum_score_snapshots (
  id TEXT PRIMARY KEY NOT NULL,
  target_kind TEXT NOT NULL CHECK (target_kind IN ('forum', 'topic', 'post', 'actor')),
  target_id TEXT NOT NULL,
  positive_bitcoin_sats INTEGER NOT NULL DEFAULT 0 CHECK (positive_bitcoin_sats >= 0),
  boost_bitcoin_sats INTEGER NOT NULL DEFAULT 0 CHECK (boost_bitcoin_sats >= 0),
  down_signal_bitcoin_sats INTEGER NOT NULL DEFAULT 0 CHECK (down_signal_bitcoin_sats >= 0),
  reply_count INTEGER NOT NULL DEFAULT 0 CHECK (reply_count >= 0),
  net_investment_sats INTEGER NOT NULL DEFAULT 0,
  score_ref TEXT NOT NULL,
  rebuilt_from_event_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE IF NOT EXISTS forum_trust_edges (
  id TEXT PRIMARY KEY NOT NULL,
  source_actor_ref TEXT NOT NULL,
  target_actor_ref TEXT NOT NULL,
  forum_id TEXT REFERENCES forum_forums(id) ON DELETE CASCADE,
  trust_kind TEXT NOT NULL CHECK (trust_kind IN ('reward', 'endorsement', 'moderation', 'report')),
  weight INTEGER NOT NULL DEFAULT 0,
  event_ref TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE IF NOT EXISTS forum_actor_forum_trust (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL,
  forum_id TEXT NOT NULL REFERENCES forum_forums(id) ON DELETE CASCADE,
  trust_score INTEGER NOT NULL DEFAULT 0,
  reward_count INTEGER NOT NULL DEFAULT 0 CHECK (reward_count >= 0),
  report_count INTEGER NOT NULL DEFAULT 0 CHECK (report_count >= 0),
  moderator_adjustment_count INTEGER NOT NULL DEFAULT 0 CHECK (moderator_adjustment_count >= 0),
  score_ref TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (actor_ref, forum_id)
);

CREATE INDEX IF NOT EXISTS idx_forum_categories_board_order
  ON forum_categories(board_id, order_index, title)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_forums_category_order
  ON forum_forums(category_id, title)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_topics_forum_bump
  ON forum_topics(forum_id, pin_state, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_posts_topic_number
  ON forum_posts(topic_id, post_number)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_reports_target
  ON forum_reports(target_kind, target_id, status, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_money_actions_target
  ON forum_money_actions(target_topic_id, target_post_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_receipts_target
  ON forum_receipts(target_topic_id, target_post_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_private_messages_thread_created
  ON forum_private_messages(thread_id, created_at)
  WHERE archived_at IS NULL;
