-- MH-6 (#8585): durable log of the three MH-0 typed fleet steering intents
-- (`khala.fleet_intent.v1` from @openagentsinc/khala-fleet-intents):
-- fleet_run_control (pause/resume/drain/stop), approval_decision (allow/deny),
-- and steer_message. This is the RECEIPT + AUTHORITY OBSERVATION spine:
--
--   * Every applied steering mutation writes exactly one row here (the durable
--     receipt), inside the same push-engine transaction as the projected
--     post-image change — both attributable to the mutation ref.
--   * `seq` is the resumable watermark the desktop/daemon authority polls
--     (`readPendingFleetSteeringIntents`) to OBSERVE mobile-dispatched intents
--     and actually change local dispatch behavior. Mobile is never a second
--     supervisor; it only appends typed intents here and reads projected state.
--
-- Modeled on khala_sync_runtime_control_intents (migration 0029/0032): a
-- client-minted text primary key, a jsonb intent value bound as an OBJECT
-- (never a pre-stringified string — that double-encodes into a jsonb scalar),
-- an owner+idempotency unique index for exactly-once apply, and a
-- GENERATED-IDENTITY `seq` for watermark ordering.
--
-- Unlike runtime control intents there is NO body-free constraint: a
-- steer_message legitimately carries the body the authority must deliver to the
-- in-flight worker. The PROJECTED fleet_steer post-image stays body-free (only
-- refs + a body carrier tag); this authoritative table may carry the body, the
-- same discipline chat_messages/runtime_events already use server-side.

CREATE TABLE IF NOT EXISTS khala_sync_fleet_steering_intents (
  intent_id            text        PRIMARY KEY,
  scope                text        NOT NULL,
  run_ref              text        NOT NULL,
  kind                 text        NOT NULL
    CHECK (kind IN ('fleet_run_control', 'approval_decision', 'steer_message')),
  -- Denormalized, kind-specific convenience columns for the observer's filter
  -- (the authoritative value is always intent_json). NULL when N/A for a kind.
  action               text
    CHECK (action IS NULL OR action IN ('pause', 'resume', 'drain', 'stop')),
  approval_ref         text,
  decision             text
    CHECK (decision IS NULL OR decision IN ('allow', 'deny')),
  surface              text        NOT NULL,
  requested_by_user_id text        NOT NULL,
  idempotency_key      text        NOT NULL,
  intent_json          jsonb       NOT NULL,
  mutation_ref         text        NOT NULL,
  created_at           text        NOT NULL,
  seq                  bigint      GENERATED ALWAYS AS IDENTITY,
  CONSTRAINT khala_sync_fleet_steering_intents_scope_shape
    CHECK (scope ~ '^scope\.fleet_run\.[A-Za-z0-9._:-]+$'),
  CONSTRAINT khala_sync_fleet_steering_intents_ref_shape
    CHECK (
      intent_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
      AND run_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
      AND idempotency_key ~ '^[A-Za-z0-9][A-Za-z0-9._:-]*$'
    ),
  CONSTRAINT khala_sync_fleet_steering_intents_owner_nonempty
    CHECK (length(requested_by_user_id) > 0),
  -- run_control carries an action; approval_decision carries approval_ref +
  -- decision; steer_message carries neither denormalized field.
  CONSTRAINT khala_sync_fleet_steering_intents_run_control_shape
    CHECK ((kind = 'fleet_run_control') = (action IS NOT NULL)),
  CONSTRAINT khala_sync_fleet_steering_intents_approval_shape
    CHECK ((kind = 'approval_decision') = (approval_ref IS NOT NULL AND decision IS NOT NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS
  khala_sync_fleet_steering_intents_owner_idempotency_idx
  ON khala_sync_fleet_steering_intents(requested_by_user_id, idempotency_key);

CREATE UNIQUE INDEX IF NOT EXISTS
  khala_sync_fleet_steering_intents_seq_idx
  ON khala_sync_fleet_steering_intents(seq);

CREATE INDEX IF NOT EXISTS
  khala_sync_fleet_steering_intents_scope_seq_idx
  ON khala_sync_fleet_steering_intents(scope, seq);
