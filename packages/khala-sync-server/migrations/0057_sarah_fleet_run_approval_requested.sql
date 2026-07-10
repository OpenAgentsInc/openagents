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

-- Approval refs are global identities while their immutable post-images live
-- in per-run scopes. The authority path resolves the latest binding by ref
-- before admitting a new request. Keep that lookup proportional to one
-- approval's history rather than to the global changelog.
CREATE INDEX IF NOT EXISTS khala_sync_changelog_fleet_approval_latest_idx
  ON khala_sync_changelog (entity_id, committed_at DESC, version DESC)
  WHERE entity_type = 'fleet_approval' AND op = 'upsert';
