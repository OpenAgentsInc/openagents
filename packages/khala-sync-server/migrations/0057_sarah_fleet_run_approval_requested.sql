-- FC-3 (#8639): permit the additive v2 approval-request execution event.
-- Approval state itself remains migration-free in the Sync changelog; this
-- constraint change is required because the immutable execution ledger has a
-- closed event_kind check from migration 0053.

ALTER TABLE sarah_fleet_run_execution_events
  DROP CONSTRAINT IF EXISTS sarah_fleet_run_execution_events_event_kind_check;

ALTER TABLE sarah_fleet_run_execution_events
  ADD CONSTRAINT sarah_fleet_run_execution_events_event_kind_check
  CHECK (event_kind IN (
    'run_started',
    'work_progress',
    'approval_requested',
    'work_terminal',
    'run_terminal'
  ));
