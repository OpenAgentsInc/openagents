-- #8388: watermark support for the runtime.* control-intent dispatch
-- consumer (Pylon-side poller). khala_sync_runtime_control_intents
-- (migration 0029) was created with a client-minted text primary key
-- (intent_id) and no monotonic ordering column, so a Pylon-side enforcement
-- loop had nothing to persist as a resumable watermark — the
-- khala_sync_fleet_intents equivalent already has one (`id bigint
-- GENERATED ALWAYS AS IDENTITY`). This adds the same convention here
-- without touching any existing column, row, or constraint.

ALTER TABLE khala_sync_runtime_control_intents
  ADD COLUMN IF NOT EXISTS seq bigint GENERATED ALWAYS AS IDENTITY;

CREATE UNIQUE INDEX IF NOT EXISTS
  khala_sync_runtime_control_intents_seq_idx
  ON khala_sync_runtime_control_intents(seq);
