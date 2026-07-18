-- MOB-FA-02 (#8994): additive columns on `desktop_full_auto_run_projections`
-- (0073) for the run-state fields the mobile Full Auto thread surface needs
-- to render without a second fetch: current provider lane/account, the
-- continuations-vs-cap counters, the typed same-pass rotation count, and a
-- bounded run-report summary once the run reaches a terminal state.
--
-- Every new column mirrors a field already added to
-- `FullAutoRunClientRunProjection`
-- (packages/khala-sync/src/full-auto-run-client-projection.ts). Counters
-- default to 0 / a cap default so a projection published by an older Desktop
-- build (pre-#8994) still round-trips through `ON CONFLICT ... DO UPDATE`
-- without a NOT NULL violation; `receipt_summary` is a nullable jsonb blob
-- (already-bounded/public-safe per that schema, never raw report internals).

ALTER TABLE desktop_full_auto_run_projections
  ADD COLUMN IF NOT EXISTS lane_ref text,
  ADD COLUMN IF NOT EXISTS account_ref text,
  ADD COLUMN IF NOT EXISTS turn_cap integer NOT NULL DEFAULT 20,
  ADD COLUMN IF NOT EXISTS successful_attempts integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS failed_attempts integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS rotation_count integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS receipt_summary jsonb;

ALTER TABLE desktop_full_auto_run_projections
  ADD CONSTRAINT desktop_full_auto_run_projections_lane_ref_shape
    CHECK (lane_ref IS NULL OR length(lane_ref) BETWEEN 1 AND 80),
  ADD CONSTRAINT desktop_full_auto_run_projections_account_ref_shape
    CHECK (account_ref IS NULL OR length(account_ref) BETWEEN 1 AND 80),
  ADD CONSTRAINT desktop_full_auto_run_projections_turn_cap_nonneg
    CHECK (turn_cap >= 0),
  ADD CONSTRAINT desktop_full_auto_run_projections_successful_attempts_nonneg
    CHECK (successful_attempts >= 0),
  ADD CONSTRAINT desktop_full_auto_run_projections_failed_attempts_nonneg
    CHECK (failed_attempts >= 0),
  ADD CONSTRAINT desktop_full_auto_run_projections_rotation_count_nonneg
    CHECK (rotation_count >= 0);
