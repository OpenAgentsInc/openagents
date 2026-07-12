-- FC-4 (#8636): retain the authenticated FleetRun execution target on durable
-- attempts. Existing owner-local rows remain valid. Managed cloud is an
-- explicit Codex-only capacity class; `auto` is a routing preference and must
-- never be persisted as capacity custody.

ALTER TABLE sarah_fleet_run_attempts
  DROP CONSTRAINT sarah_fleet_run_attempts_capacity_class_check;

ALTER TABLE sarah_fleet_run_attempts
  ADD CONSTRAINT sarah_fleet_run_attempts_capacity_class_check
  CHECK (
    capacity_class = 'owner_local'
    OR (capacity_class = 'managed_cloud' AND worker_kind = 'codex')
  );
