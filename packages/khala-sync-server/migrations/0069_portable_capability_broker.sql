-- PORT-03: durable, refs-only capability broker aggregate.
--
-- The aggregate row, one active move claim, and operation evidence are
-- updated under one transaction and revision CAS. Raw credential material,
-- host paths, provider payloads, and private repository content have no
-- columns in this schema.

CREATE TABLE IF NOT EXISTS khala_sync_portable_capability_brokers (
  owner_user_id text NOT NULL,
  session_ref text NOT NULL REFERENCES khala_sync_portable_sessions(session_ref) ON DELETE CASCADE,
  revision bigint NOT NULL DEFAULT 0 CHECK (revision >= 0),
  state_json jsonb,
  active_move_ref text,
  active_move_fingerprint text,
  active_command_ref text,
  active_source_attachment_ref text,
  active_source_generation bigint CHECK (active_source_generation > 0),
  active_destination_target_ref text,
  claim_acquired_at timestamptz,
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (owner_user_id, session_ref),
  CHECK (
    (active_move_ref IS NULL AND active_move_fingerprint IS NULL AND
     active_command_ref IS NULL AND active_source_attachment_ref IS NULL AND
     active_source_generation IS NULL AND active_destination_target_ref IS NULL AND
     claim_acquired_at IS NULL)
    OR
    (active_move_ref IS NOT NULL AND active_move_fingerprint IS NOT NULL AND
     active_command_ref IS NOT NULL AND active_source_attachment_ref IS NOT NULL AND
     active_source_generation IS NOT NULL AND active_destination_target_ref IS NOT NULL AND
     claim_acquired_at IS NOT NULL)
  )
);

CREATE TABLE IF NOT EXISTS khala_sync_portable_capability_evidence (
  owner_user_id text NOT NULL,
  session_ref text NOT NULL,
  evidence_ref text NOT NULL,
  operation_ref text NOT NULL,
  broker_revision bigint NOT NULL CHECK (broker_revision > 0),
  evidence_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (owner_user_id, session_ref, evidence_ref),
  UNIQUE (owner_user_id, session_ref, operation_ref),
  FOREIGN KEY (owner_user_id, session_ref)
    REFERENCES khala_sync_portable_capability_brokers(owner_user_id, session_ref)
    ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS khala_sync_portable_capability_active_moves
  ON khala_sync_portable_capability_brokers(active_move_ref)
  WHERE active_move_ref IS NOT NULL;
