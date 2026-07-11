-- #8687: conversation commands keep one durable semantic identity across
-- retries and reconnects. Expiry is a terminal projected result in the same
-- runtime-control-intent ledger, never a parallel recovery queue.

ALTER TABLE khala_sync_runtime_control_intents
  DROP CONSTRAINT IF EXISTS khala_sync_runtime_control_intents_status_check;

ALTER TABLE khala_sync_runtime_control_intents
  ADD CONSTRAINT khala_sync_runtime_control_intents_status_check
  CHECK (status IN ('accepted', 'settled', 'expired'));
