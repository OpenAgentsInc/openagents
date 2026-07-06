CREATE TABLE IF NOT EXISTS cloud_coding_admission_events (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  account_ref TEXT NOT NULL,
  work_context_ref TEXT NOT NULL,
  lane TEXT NOT NULL CHECK (lane IN ('cloud-gcp', 'cloud-shc')),
  event_kind TEXT NOT NULL CHECK (event_kind IN ('admitted')),
  capacity_ref TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0)
);

CREATE INDEX IF NOT EXISTS idx_cloud_coding_admission_events_account_window
  ON cloud_coding_admission_events (account_ref, created_at_ms);

CREATE TABLE IF NOT EXISTS cloud_coding_admission_reservations (
  session_id TEXT PRIMARY KEY,
  account_ref TEXT NOT NULL,
  work_context_ref TEXT NOT NULL,
  lane TEXT NOT NULL CHECK (lane IN ('cloud-gcp', 'cloud-shc')),
  state TEXT NOT NULL CHECK (state IN ('admitted')),
  capacity_ref TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= created_at_ms)
);

CREATE INDEX IF NOT EXISTS idx_cloud_coding_admission_reservations_account_active
  ON cloud_coding_admission_reservations (account_ref, expires_at_ms);
