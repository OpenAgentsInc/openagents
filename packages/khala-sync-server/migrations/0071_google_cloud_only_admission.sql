-- Google Cloud is the sole managed compute authority.
CREATE TABLE IF NOT EXISTS cloud_coding_admission_events (
  id text PRIMARY KEY,
  session_id text NOT NULL,
  account_ref text NOT NULL,
  work_context_ref text NOT NULL,
  lane text NOT NULL CHECK (lane = 'cloud-gcp'),
  event_kind text NOT NULL CHECK (event_kind = 'admitted'),
  capacity_ref text NOT NULL,
  created_at_ms bigint NOT NULL CHECK (created_at_ms >= 0)
);

CREATE INDEX IF NOT EXISTS idx_cloud_coding_admission_events_account_window
  ON cloud_coding_admission_events (account_ref, created_at_ms);

CREATE TABLE IF NOT EXISTS cloud_coding_admission_reservations (
  session_id text PRIMARY KEY,
  account_ref text NOT NULL,
  work_context_ref text NOT NULL,
  lane text NOT NULL CHECK (lane = 'cloud-gcp'),
  state text NOT NULL CHECK (state = 'admitted'),
  capacity_ref text NOT NULL,
  created_at_ms bigint NOT NULL CHECK (created_at_ms >= 0),
  expires_at_ms bigint NOT NULL CHECK (expires_at_ms >= created_at_ms)
);

CREATE INDEX IF NOT EXISTS idx_cloud_coding_admission_reservations_account_active
  ON cloud_coding_admission_reservations (account_ref, expires_at_ms);

ALTER TABLE backend_incident_events
  ALTER COLUMN runtime_name SET DEFAULT 'gcp_cloud_run';
