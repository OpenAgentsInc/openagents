-- #8370: Khala Code runtime control/event state for AI SDK-shaped Sync.
--
-- These rows are server-authoritative state for the runtime.* mutators.
-- Runtime turns and body-free control intents project to the owner's
-- personal scope and the private thread scope. Full runtime events project
-- only to scope.thread.<thread_id>, because they can carry prompt/text/tool
-- stream material. Nothing in this migration writes public scopes.

CREATE TABLE IF NOT EXISTS khala_sync_runtime_turns (
  turn_id          text        PRIMARY KEY,
  thread_id        text        NOT NULL,
  owner_user_id    text        NOT NULL,
  lane             text        NOT NULL,
  status           text        NOT NULL
    CHECK (status IN (
      'queued',
      'running',
      'waiting_for_input',
      'completed',
      'failed',
      'interrupted',
      'closed'
    )),
  event_count      integer     NOT NULL DEFAULT 0
    CHECK (event_count >= 0),
  latest_intent_id text,
  started_at       text,
  settled_at       text,
  created_at       text        NOT NULL,
  updated_at       text        NOT NULL,
  CONSTRAINT khala_sync_runtime_turns_ref_shape
    CHECK (
      turn_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
      AND thread_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
      AND (latest_intent_id IS NULL
        OR latest_intent_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$')
    ),
  CONSTRAINT khala_sync_runtime_turns_owner_nonempty
    CHECK (length(owner_user_id) > 0)
);

CREATE INDEX IF NOT EXISTS khala_sync_runtime_turns_owner_updated_idx
  ON khala_sync_runtime_turns(owner_user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS khala_sync_runtime_turns_thread_updated_idx
  ON khala_sync_runtime_turns(thread_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS khala_sync_runtime_control_intents (
  intent_id       text        PRIMARY KEY,
  thread_id       text        NOT NULL,
  turn_id         text,
  owner_user_id   text        NOT NULL,
  kind            text        NOT NULL,
  status          text        NOT NULL
    CHECK (status IN ('accepted', 'settled')),
  idempotency_key text        NOT NULL,
  intent_json     jsonb       NOT NULL,
  created_at      text        NOT NULL,
  updated_at      text        NOT NULL,
  CONSTRAINT khala_sync_runtime_control_intents_ref_shape
    CHECK (
      intent_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
      AND thread_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
      AND (turn_id IS NULL OR turn_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$')
      AND idempotency_key ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
    ),
  CONSTRAINT khala_sync_runtime_control_intents_owner_nonempty
    CHECK (length(owner_user_id) > 0),
  CONSTRAINT khala_sync_runtime_control_intents_body_free
    CHECK (NOT (intent_json ? 'body'))
);

CREATE UNIQUE INDEX IF NOT EXISTS
  khala_sync_runtime_control_intents_owner_idempotency_idx
  ON khala_sync_runtime_control_intents(owner_user_id, idempotency_key);

CREATE INDEX IF NOT EXISTS khala_sync_runtime_control_intents_thread_created_idx
  ON khala_sync_runtime_control_intents(thread_id, created_at);

CREATE INDEX IF NOT EXISTS khala_sync_runtime_control_intents_turn_created_idx
  ON khala_sync_runtime_control_intents(turn_id, created_at)
  WHERE turn_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS khala_sync_runtime_events (
  event_id      text        PRIMARY KEY,
  turn_id       text        NOT NULL
    REFERENCES khala_sync_runtime_turns(turn_id) ON DELETE CASCADE,
  thread_id     text        NOT NULL,
  owner_user_id text        NOT NULL,
  kind          text        NOT NULL,
  sequence      bigint      NOT NULL
    CHECK (sequence >= 0),
  observed_at   text        NOT NULL,
  event_json    jsonb       NOT NULL,
  created_at    text        NOT NULL,
  CONSTRAINT khala_sync_runtime_events_ref_shape
    CHECK (
      event_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
      AND turn_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
      AND thread_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
    ),
  CONSTRAINT khala_sync_runtime_events_owner_nonempty
    CHECK (length(owner_user_id) > 0),
  CONSTRAINT khala_sync_runtime_events_turn_thread_match
    CHECK ((event_json ->> 'turnId') = turn_id
      AND (event_json ->> 'threadId') = thread_id
      AND ((event_json ->> 'sequence')::bigint) = sequence)
);

CREATE UNIQUE INDEX IF NOT EXISTS
  khala_sync_runtime_events_turn_sequence_idx
  ON khala_sync_runtime_events(turn_id, sequence);

CREATE INDEX IF NOT EXISTS khala_sync_runtime_events_thread_sequence_idx
  ON khala_sync_runtime_events(thread_id, sequence);
