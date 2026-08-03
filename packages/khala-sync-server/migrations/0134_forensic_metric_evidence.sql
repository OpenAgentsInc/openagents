CREATE TABLE IF NOT EXISTS forensic_metric_evidence (
  owner_ref TEXT NOT NULL,
  run_ref TEXT NOT NULL,
  record_ref TEXT NOT NULL,
  record_kind TEXT NOT NULL CHECK (
    record_kind IN ('run_event', 'provider_usage', 'adjudication', 'reviewer_burden')
  ),
  event_sequence BIGINT,
  canonical_digest TEXT NOT NULL CHECK (
    canonical_digest ~ '^sha256:[0-9a-f]{64}$'
  ),
  payload_json JSONB NOT NULL CHECK (jsonb_typeof(payload_json) = 'object'),
  observed_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (owner_ref, record_ref),
  CHECK (
    (record_kind = 'run_event' AND event_sequence IS NOT NULL AND event_sequence > 0)
    OR (record_kind <> 'run_event' AND event_sequence IS NULL)
  )
);

CREATE UNIQUE INDEX IF NOT EXISTS forensic_metric_evidence_run_sequence_unique
  ON forensic_metric_evidence (owner_ref, run_ref, event_sequence)
  WHERE record_kind = 'run_event';

CREATE INDEX IF NOT EXISTS forensic_metric_evidence_owner_run_observed
  ON forensic_metric_evidence (owner_ref, run_ref, observed_at, record_ref);
