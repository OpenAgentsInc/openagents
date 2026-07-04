-- MC-1 (#8352): owner-private chat mutator business tables.
--
-- These rows are the server-authoritative state for
-- chat.createThread/chat.appendMessage/chat.renameThread. Each mutator writes
-- these tables and the Khala Sync changelog in one Postgres transaction. The
-- replicated scopes are owner-private only:
--
--   scope.user.<owner_user_id>       thread index, no message bodies
--   scope.thread.<thread_id>         thread metadata + message bodies
--
-- Thread read authority for new MC-1 threads is the existing
-- khala_sync_scope_owners row for scope.thread.<thread_id>; legacy
-- agent-run/autopilot thread mappings remain supported separately.

CREATE TABLE IF NOT EXISTS khala_sync_chat_threads (
  thread_id      text        PRIMARY KEY,
  owner_user_id  text        NOT NULL,
  title          text        NOT NULL DEFAULT '',
  status         text        NOT NULL DEFAULT 'active'
    CHECK (status = 'active'),
  message_count  integer     NOT NULL DEFAULT 0
    CHECK (message_count >= 0),
  last_message_at text,
  created_at     text        NOT NULL,
  updated_at     text        NOT NULL,
  CONSTRAINT khala_sync_chat_threads_ref_shape
    CHECK (thread_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'),
  CONSTRAINT khala_sync_chat_threads_owner_nonempty
    CHECK (length(owner_user_id) > 0),
  CONSTRAINT khala_sync_chat_threads_title_bound
    CHECK (length(title) <= 160)
);

CREATE INDEX IF NOT EXISTS khala_sync_chat_threads_owner_updated_idx
  ON khala_sync_chat_threads(owner_user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS khala_sync_chat_messages (
  message_id     text        PRIMARY KEY,
  thread_id      text        NOT NULL
    REFERENCES khala_sync_chat_threads(thread_id) ON DELETE CASCADE,
  author_user_id text        NOT NULL,
  body           text        NOT NULL,
  created_at     text        NOT NULL,
  updated_at     text        NOT NULL,
  deleted_at     text,
  CONSTRAINT khala_sync_chat_messages_ref_shape
    CHECK (message_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'),
  CONSTRAINT khala_sync_chat_messages_author_nonempty
    CHECK (length(author_user_id) > 0),
  CONSTRAINT khala_sync_chat_messages_body_bound
    CHECK (length(body) BETWEEN 1 AND 20000)
);

CREATE INDEX IF NOT EXISTS khala_sync_chat_messages_thread_created_idx
  ON khala_sync_chat_messages(thread_id, created_at);
