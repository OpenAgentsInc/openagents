-- FC-3 (#8639): terminal receipts for queued Pylon steering follow-ups.
--
-- The original outcome remains the immutable delivery receipt. These columns
-- add only a body-free, content-bound terminal completion and the server clock
-- that projected it. Private steer bodies and failure text never enter this
-- table or the Sarah Sync scope.

ALTER TABLE sarah_fleet_run_steering_outcomes
  ADD COLUMN completion_outcome text,
  ADD COLUMN completion_ref text UNIQUE,
  ADD COLUMN completed_at text,
  ADD COLUMN completion_recorded_at text,
  ADD CONSTRAINT sarah_fleet_run_steering_completion_outcome_check
    CHECK (
      completion_outcome IS NULL OR
      completion_outcome IN ('applied', 'failed', 'skipped_stale')
    ),
  ADD CONSTRAINT sarah_fleet_run_steering_completion_tuple_check
    CHECK (
      (
        completion_outcome IS NULL AND
        completion_ref IS NULL AND
        completed_at IS NULL AND
        completion_recorded_at IS NULL
      ) OR (
        completion_outcome IS NOT NULL AND
        completion_ref IS NOT NULL AND
        completed_at IS NOT NULL AND
        completion_recorded_at IS NOT NULL
      )
    ),
  ADD CONSTRAINT sarah_fleet_run_steering_completion_queued_only_check
    CHECK (completion_ref IS NULL OR outcome = 'queued_follow_up'),
  ADD CONSTRAINT sarah_fleet_run_steering_completion_ref_shape
    CHECK (
      completion_ref IS NULL OR
      completion_ref ~ '^completion\.pylon\.fleet_steering\.[a-f0-9]{24}$'
    );
