-- Add public-safe recipient attribution and recipient-confirmed state for
-- outbound treasury transactions. Destination material stays redacted/hash-only.
ALTER TABLE treasury_transactions
  ADD COLUMN recipient_ref TEXT;

ALTER TABLE treasury_transactions
  ADD COLUMN redacted_destination_ref TEXT;

ALTER TABLE treasury_transactions
  ADD COLUMN owed_ref TEXT;

ALTER TABLE treasury_transactions
  ADD COLUMN owed_sat INTEGER;

ALTER TABLE treasury_transactions
  ADD COLUMN recipient_confirmation_state TEXT NOT NULL DEFAULT 'unconfirmed'
    CHECK (recipient_confirmation_state IN ('unconfirmed', 'confirmed_received'));

ALTER TABLE treasury_transactions
  ADD COLUMN recipient_confirmation_ref TEXT;

ALTER TABLE treasury_transactions
  ADD COLUMN recipient_confirmed_at TEXT;

CREATE INDEX IF NOT EXISTS idx_treasury_transactions_recipient_ref
  ON treasury_transactions (recipient_ref, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_treasury_transactions_recipient_confirmation
  ON treasury_transactions (recipient_ref, recipient_confirmation_state);
