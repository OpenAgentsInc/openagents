-- AUDIO-3 private retention authority. Media bytes MUST NOT enter Cloud SQL.
CREATE TABLE IF NOT EXISTS audio_retained_sessions (
  session_ref text NOT NULL, generation bigint NOT NULL CHECK (generation >= 0),
  receipt_id text NOT NULL UNIQUE, owner_ref text NOT NULL, device_ref text NOT NULL,
  thread_ref text NOT NULL, policy_version text NOT NULL, consent_version text NOT NULL,
  key_epoch text NOT NULL, accepted_at timestamptz NOT NULL, expires_at timestamptz NOT NULL,
  stopped_at timestamptz, legal_hold boolean NOT NULL DEFAULT false,
  PRIMARY KEY (session_ref, generation), CHECK (expires_at > accepted_at)
);
CREATE TABLE IF NOT EXISTS audio_segment_manifests (
  segment_id text PRIMARY KEY, session_ref text NOT NULL, generation bigint NOT NULL,
  first_sequence bigint NOT NULL CHECK (first_sequence >= 0), last_sequence bigint NOT NULL,
  digest_sha256 text NOT NULL CHECK (digest_sha256 ~ '^[0-9a-f]{64}$'),
  capture_started_at timestamptz NOT NULL, capture_ended_at timestamptz NOT NULL,
  server_received_at timestamptz NOT NULL, codec text NOT NULL, byte_length bigint NOT NULL CHECK (byte_length > 0),
  object_ref text NOT NULL UNIQUE, disposition_class text NOT NULL CHECK (disposition_class IN
    ('raw_audio','normalized_audio','transcript_hypothesis','transcript_final','transcript_correction','embedding_eval_training_copy','command_receipt','aggregate_metric')),
  receipt_id text NOT NULL, policy_version text NOT NULL, consent_version text NOT NULL,
  key_epoch text NOT NULL, expires_at timestamptz NOT NULL,
  deletion_state text NOT NULL DEFAULT 'active' CHECK (deletion_state IN ('active','deleted','expired','legal_hold')),
  exported_at timestamptz,
  FOREIGN KEY (session_ref, generation) REFERENCES audio_retained_sessions(session_ref, generation),
  CHECK (last_sequence >= first_sequence)
);
-- One range has exactly one digest. A retry with another digest must conflict,
-- never create a second accepted truth for the same sequences.
CREATE UNIQUE INDEX IF NOT EXISTS audio_segment_sequence_identity ON audio_segment_manifests(session_ref, generation, first_sequence, last_sequence);
CREATE INDEX IF NOT EXISTS audio_segment_expiry ON audio_segment_manifests(expires_at) WHERE deletion_state = 'active';
CREATE TABLE IF NOT EXISTS audio_sequence_gaps (
  session_ref text NOT NULL, generation bigint NOT NULL, first_sequence bigint NOT NULL,
  last_sequence bigint NOT NULL, reason text NOT NULL CHECK (reason IN ('transport_gap','storage_outage','quota_refused','policy_refused')),
  recorded_at timestamptz NOT NULL, PRIMARY KEY (session_ref, generation, first_sequence, last_sequence),
  FOREIGN KEY (session_ref, generation) REFERENCES audio_retained_sessions(session_ref, generation)
);
CREATE TABLE IF NOT EXISTS audio_access_receipts (
  receipt_id text PRIMARY KEY, operation text NOT NULL CHECK (operation IN ('read','export','delete','expire')),
  owner_ref text NOT NULL, session_ref text NOT NULL, occurred_at timestamptz NOT NULL,
  disposition_classes jsonb NOT NULL, segment_ids jsonb NOT NULL, remaining_lawful_records jsonb NOT NULL
);
