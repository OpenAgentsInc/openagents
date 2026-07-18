-- Record the Google Cloud history reconciliation that was originally applied
-- manually after migration 0071 had already entered the migration ledger.
-- Every operation is idempotent so the migration truthfully records the
-- already-converged staging and production state.
ALTER TABLE agent_runs
  DROP CONSTRAINT IF EXISTS agent_runs_backend_check;

UPDATE agent_runs
SET backend = 'retired_pilot'
WHERE backend = 'shc_vm';

ALTER TABLE agent_runs
  ADD CONSTRAINT agent_runs_backend_check
  CHECK (backend IN ('gcloud_vm', 'local_fake', 'retired_pilot'));

ALTER TABLE agent_runs
  DROP CONSTRAINT IF EXISTS agent_runs_retired_pilot_terminal_check;

ALTER TABLE agent_runs
  ADD CONSTRAINT agent_runs_retired_pilot_terminal_check
  CHECK (
    backend <> 'retired_pilot'
    OR status IN ('completed', 'failed', 'canceled')
  );

UPDATE backend_incident_events
SET runtime_name = 'retired_edge_runtime'
WHERE runtime_name = 'cloudflare_workers';
