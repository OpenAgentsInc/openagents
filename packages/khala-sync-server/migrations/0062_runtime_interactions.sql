-- CUT-16 #8696: owner-private durable provider questions, tool approvals,
-- and plan reviews. Full display/decision material is projected only to the
-- exact thread scope; no personal or public projection is stored here.

CREATE TABLE IF NOT EXISTS khala_sync_runtime_interactions (
  interaction_ref    text        PRIMARY KEY,
  thread_id          text        NOT NULL,
  turn_id            text        NOT NULL
    REFERENCES khala_sync_runtime_turns(turn_id) ON DELETE CASCADE,
  owner_user_id      text        NOT NULL,
  kind               text        NOT NULL
    CHECK (kind IN ('provider_question', 'tool_approval', 'plan_review')),
  status             text        NOT NULL
    CHECK (status IN ('pending', 'resolved', 'expired', 'revoked')),
  requested_sequence bigint      NOT NULL CHECK (requested_sequence >= 0),
  expires_at         text        NOT NULL,
  interaction_json   jsonb       NOT NULL,
  created_at         text        NOT NULL,
  updated_at         text        NOT NULL,
  CONSTRAINT khala_sync_runtime_interactions_ref_shape CHECK (
    interaction_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
    AND thread_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
    AND turn_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
  ),
  CONSTRAINT khala_sync_runtime_interactions_owner_nonempty
    CHECK (length(owner_user_id) > 0),
  CONSTRAINT khala_sync_runtime_interactions_post_image_match CHECK (
    (interaction_json ->> 'interactionRef') = interaction_ref
    AND (interaction_json ->> 'threadId') = thread_id
    AND (interaction_json ->> 'turnId') = turn_id
    AND ((interaction_json ->> 'requestedSequence')::bigint) = requested_sequence
    AND (interaction_json ->> 'expiresAt') = expires_at
    AND (interaction_json -> 'payload' ->> 'kind') = kind
    AND (interaction_json -> 'lifecycle' ->> 'status') = status
  )
);

CREATE INDEX IF NOT EXISTS khala_sync_runtime_interactions_thread_updated_idx
  ON khala_sync_runtime_interactions(thread_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS khala_sync_runtime_interactions_turn_status_idx
  ON khala_sync_runtime_interactions(turn_id, status, requested_sequence);

CREATE UNIQUE INDEX IF NOT EXISTS
  khala_sync_runtime_interactions_owner_decision_idx
  ON khala_sync_runtime_interactions(
    owner_user_id,
    (interaction_json -> 'lifecycle' -> 'envelope' ->> 'idempotencyKey')
  )
  WHERE status = 'resolved';
