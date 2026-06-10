-- Artanis forum responder state (issues #4714-#4715; promise
-- artanis.pylon_support_responder.v1). The scan cursor is a single row;
-- responder actions dedupe by topic and carry the typed lifecycle:
-- proposed -> responded -> tipped, with skipped/blocked as terminal
-- honesty states. Public-safe content only: refs, classes, timestamps.

CREATE TABLE IF NOT EXISTS artanis_responder_state (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  scan_cursor_iso TEXT NOT NULL,
  responses_today INTEGER NOT NULL DEFAULT 0,
  responses_day TEXT NOT NULL DEFAULT '',
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS artanis_responder_actions (
  id TEXT PRIMARY KEY,
  topic_id TEXT NOT NULL UNIQUE,
  first_post_id TEXT,
  question_class TEXT,
  state TEXT NOT NULL CHECK (
    state IN ('proposed', 'responded', 'tipped', 'skipped', 'blocked')
  ),
  proposal_json TEXT NOT NULL DEFAULT '{}',
  reply_post_id TEXT,
  asked_at TEXT,
  replied_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artanis_responder_actions_state
  ON artanis_responder_actions (state, created_at DESC);
