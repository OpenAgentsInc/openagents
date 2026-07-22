-- IDE-13 #9041: carry destination readiness through phase completion.
--
-- This receipt is public-safe. It contains stable refs and readiness facts.
-- It does not contain credentials, host paths, process IDs, or handles.

ALTER TABLE khala_sync_portable_phase_operations
  ADD COLUMN IF NOT EXISTS result_destination_activation_receipt_json jsonb;

ALTER TABLE khala_sync_portable_phase_operations
  ADD CONSTRAINT khala_sync_portable_phase_destination_activation_receipt_shape
  CHECK (
    (state = 'completed' AND kind = 'destination-activate'
      AND result_destination_activation_receipt_json IS NOT NULL)
    OR
    (NOT (state = 'completed' AND kind = 'destination-activate')
      AND result_destination_activation_receipt_json IS NULL)
  ) NOT VALID;
