-- Persist public-safe failure classification for payout attempts that fail
-- before a durable MDK payment id exists.
ALTER TABLE treasury_transactions
  ADD COLUMN failure_reason_ref TEXT;
