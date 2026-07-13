CREATE TABLE IF NOT EXISTS khala_sync_portable_managed_targets (
  owner_user_id TEXT NOT NULL,
  session_ref TEXT NOT NULL,
  target_ref TEXT NOT NULL,
  attachment_ref TEXT NOT NULL,
  generation BIGINT NOT NULL CHECK (generation > 0),
  checkpoint_ref TEXT NOT NULL,
  resource_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('staged', 'active', 'quiesced', 'reclaimed')),
  accepting_work BOOLEAN NOT NULL DEFAULT FALSE,
  bundle_json JSONB NOT NULL,
  stage_receipt_json JSONB NOT NULL,
  authority_evidence_ref TEXT,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (owner_user_id, session_ref, target_ref),
  UNIQUE (owner_user_id, session_ref, attachment_ref, generation),
  FOREIGN KEY (session_ref)
    REFERENCES khala_sync_portable_sessions(session_ref)
    ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS khala_sync_portable_managed_target_operations (
  owner_user_id TEXT NOT NULL,
  session_ref TEXT NOT NULL,
  target_ref TEXT NOT NULL,
  operation_ref TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('stage', 'activate', 'abort', 'quiesce', 'checkpoint', 'cleanup')),
  fingerprint TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'completed')),
  result_json JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (owner_user_id, target_ref, operation_ref)
);

CREATE INDEX IF NOT EXISTS khala_sync_portable_managed_target_operations_session_idx
  ON khala_sync_portable_managed_target_operations(owner_user_id, session_ref, target_ref, status);
