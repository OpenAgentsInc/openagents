UPDATE team_projects
SET
  name = 'Autopilot',
  metadata_json = '{"program":"adjutant","surface":"openagents-core-team","agent":{"id":"agent_adjutant","name":"Autopilot","status":"active","scope":"project","runtime":"Autopilot","backend":"SHC","repository":"autopilot-omega","focus":"Sites"}}',
  updated_at = '2026-06-05T00:00:00.000Z'
WHERE id = 'project_adjutant';
