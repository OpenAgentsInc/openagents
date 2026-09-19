-- Idempotency ledger for an explicitly enabled migration from the legacy
-- nostr-effect public.events table. The source table remains untouched.

CREATE TABLE nostr_effect_import_ledger (
    event_id text COLLATE "C" PRIMARY KEY,
    outcome text COLLATE "C" NOT NULL,
    processed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT nostr_effect_import_outcome CHECK (
        outcome IN ('stored', 'duplicate', 'ephemeral', 'rejected')
    )
);
