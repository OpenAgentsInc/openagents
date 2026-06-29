-- Artanis operator chat thread/message ledger (#6401).
--
-- This is the dedicated D1 home for multi-turn, multi-agent operator chats:
-- owner-started Artanis chats plus agent-started operator chats from Codex,
-- Claude, and future local/remote agents. Long-term preferences and durable
-- memory stay in artanis_owner_memory; conversational logs live here.
--
-- Private/operator scoped. Rows may contain bounded chat text and local agent
-- refs, so they must not feed public projections, counters, Forum posts, or
-- product-promise copy.

CREATE TABLE IF NOT EXISTS artanis_threads (
  thread_ref TEXT PRIMARY KEY,
  caller_id TEXT NOT NULL,
  caller_kind TEXT NOT NULL CHECK (
    caller_kind IN ('owner', 'agent', 'operator', 'system')
  ),
  subject_agent_ref TEXT NOT NULL,
  subject_agent_kind TEXT NOT NULL CHECK (
    subject_agent_kind IN ('artanis', 'claude', 'codex', 'other')
  ),
  title TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'open' CHECK (
    status IN ('open', 'archived')
  ),
  source_ref TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  last_message_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS artanis_messages (
  message_ref TEXT PRIMARY KEY,
  thread_ref TEXT NOT NULL REFERENCES artanis_threads(thread_ref) ON DELETE CASCADE,
  caller_id TEXT NOT NULL,
  author_id TEXT NOT NULL,
  author_kind TEXT NOT NULL CHECK (
    author_kind IN ('owner', 'agent', 'operator', 'system', 'tool')
  ),
  body TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL
);

-- Primary thread list path: one caller's threads, newest activity first.
CREATE INDEX IF NOT EXISTS idx_artanis_threads_caller_last_message
  ON artanis_threads(caller_id, last_message_at DESC);

-- Secondary thread audit path: one caller's threads by creation time.
CREATE INDEX IF NOT EXISTS idx_artanis_threads_caller_created
  ON artanis_threads(caller_id, created_at DESC);

-- Primary transcript path: one thread in chronological order.
CREATE INDEX IF NOT EXISTS idx_artanis_messages_thread_created
  ON artanis_messages(thread_ref, created_at ASC);

-- Cross-thread caller audit path requested by #6401.
CREATE INDEX IF NOT EXISTS idx_artanis_messages_caller_created
  ON artanis_messages(caller_id, created_at DESC);
