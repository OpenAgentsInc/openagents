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

-- Preserve historical records without leaving retired provider identifiers in
-- the active schema. `retired_pilot` is provenance only: the application
-- dispatch schema accepts `gcloud_vm` exclusively, and the constraint permits
-- the retired value only on already-terminal runs.
ALTER TABLE agent_runs
  DROP CONSTRAINT IF EXISTS agent_runs_backend_check;

UPDATE agent_runs
SET backend = 'retired_pilot'
WHERE backend = 'shc_vm';

ALTER TABLE agent_runs
  ADD CONSTRAINT agent_runs_backend_check
  CHECK (backend IN ('gcloud_vm', 'local_fake', 'retired_pilot'));

ALTER TABLE agent_runs
  DROP CONSTRAINT IF EXISTS agent_runs_retired_pilot_terminal_check;

ALTER TABLE agent_runs
  ADD CONSTRAINT agent_runs_retired_pilot_terminal_check
  CHECK (
    backend <> 'retired_pilot'
    OR status IN ('completed', 'failed', 'canceled')
  );

UPDATE backend_incident_events
SET runtime_name = 'retired_edge_runtime'
WHERE runtime_name = 'cloudflare_workers';
