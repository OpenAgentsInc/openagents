-- #9285: Runtime-authored proof that an owner-waived Sarah session did not
-- reserve, charge, or otherwise mutate the platform credit ledger.

CREATE TABLE IF NOT EXISTS sarah_voice_unmetered_authority_captures (
  session_ref                 text PRIMARY KEY
    REFERENCES sarah_realtime_voice_sessions (session_ref),
  generation                  integer NOT NULL CHECK (generation > 0),
  authority                   text NOT NULL CHECK (
    authority = 'owner_waived_unmetered_v1'
  ),
  start_ledger_state_digest   text NOT NULL CHECK (
    start_ledger_state_digest ~ '^[0-9a-f]{64}$'
  ),
  start_balance_state_digest  text NOT NULL CHECK (
    start_balance_state_digest ~ '^[0-9a-f]{64}$'
  ),
  end_balance_state_digest    text CHECK (
    end_balance_state_digest IS NULL
    OR end_balance_state_digest ~ '^[0-9a-f]{64}$'
  ),
  end_ledger_state_digest     text CHECK (
    end_ledger_state_digest IS NULL
    OR end_ledger_state_digest ~ '^[0-9a-f]{64}$'
  ),
  ledger_mutation_count       bigint NOT NULL DEFAULT 0 CHECK (
    ledger_mutation_count >= 0
  ),
  capture_receipt_ref         text UNIQUE,
  capture_digest              text UNIQUE CHECK (
    capture_digest IS NULL OR capture_digest ~ '^[0-9a-f]{64}$'
  ),
  terminal_authority_ref      text,
  created_at                  text NOT NULL,
  terminal_at                 text,
  CHECK (
    (
      terminal_at IS NULL
      AND end_ledger_state_digest IS NULL
      AND end_balance_state_digest IS NULL
      AND capture_receipt_ref IS NULL
      AND capture_digest IS NULL
      AND terminal_authority_ref IS NULL
    )
    OR
    (
      terminal_at IS NOT NULL
      AND end_ledger_state_digest IS NOT NULL
      AND end_balance_state_digest IS NOT NULL
      AND capture_receipt_ref IS NOT NULL
      AND capture_digest IS NOT NULL
      AND terminal_authority_ref IS NOT NULL
    )
  )
);
