-- IDE-13: retain the exact destination runner-session reservation from stage.
--
-- This value is an opaque public-safe reference. It is not a credential and it
-- does not prove that a helper authenticated the reserved session.

ALTER TABLE khala_sync_portable_phase_operations
  ADD COLUMN IF NOT EXISTS result_destination_runner_session_reservation_ref text;

ALTER TABLE khala_sync_portable_phase_operations
  ADD CONSTRAINT khala_sync_portable_phase_runner_session_reservation_shape
  CHECK (
    (state = 'completed' AND kind = 'checkpoint-stage'
      AND result_destination_runner_session_reservation_ref IS NOT NULL
      AND result_destination_runner_session_reservation_ref ~
        '^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$')
    OR
    (NOT (state = 'completed' AND kind = 'checkpoint-stage')
      AND result_destination_runner_session_reservation_ref IS NULL)
  ) NOT VALID;
