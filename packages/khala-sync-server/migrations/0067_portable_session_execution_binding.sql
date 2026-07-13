-- PORT-03 #8748: explicit canonical run/repository identity for movable
-- coding sessions. Existing PORT-01 rows remain readable, but movement fails
-- closed without one owner/session binding; no host-derived backfill exists.

CREATE TABLE IF NOT EXISTS khala_sync_portable_session_execution_bindings (
  session_ref text PRIMARY KEY REFERENCES khala_sync_portable_sessions(session_ref) ON DELETE CASCADE,
  owner_user_id text NOT NULL,
  run_ref text NOT NULL,
  repository_ref text NOT NULL,
  pinned_base_ref text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (owner_user_id, run_ref)
);
