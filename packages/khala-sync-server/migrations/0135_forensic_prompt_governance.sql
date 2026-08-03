-- Durable owner-scoped forensic prompt governance: one active pointer plus its
-- append-only transition history. The pointer is derived from the history; a
-- decision never edits a recorded transition.
CREATE TABLE IF NOT EXISTS forensic_prompt_active_pointers (
  owner_ref TEXT PRIMARY KEY,
  -- NULL exactly when the last transition rolled back to the genesis state,
  -- where no governed prompt has ever been active.
  active_prompt_digest TEXT CHECK (
    active_prompt_digest IS NULL OR active_prompt_digest ~ '^sha256:[0-9a-f]{64}$'
  ),
  revision BIGINT NOT NULL CHECK (revision > 0),
  updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS forensic_prompt_transitions (
  owner_ref TEXT NOT NULL,
  sequence BIGINT NOT NULL CHECK (sequence > 0),
  transition_ref TEXT NOT NULL,
  transition_digest TEXT NOT NULL CHECK (
    transition_digest ~ '^sha256:[0-9a-f]{64}$'
  ),
  transition_json JSONB NOT NULL CHECK (jsonb_typeof(transition_json) = 'object'),
  decided_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (owner_ref, sequence),
  UNIQUE (owner_ref, transition_ref),
  UNIQUE (owner_ref, transition_digest)
);

CREATE INDEX IF NOT EXISTS forensic_prompt_transitions_owner_decided
  ON forensic_prompt_transitions (owner_ref, decided_at DESC, sequence DESC);
