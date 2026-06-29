INSERT INTO team_projects (
  id,
  team_id,
  slug,
  name,
  description,
  status,
  metadata_json,
  created_at,
  updated_at
)
VALUES (
  'project_adjutant',
  'team_openagents_core',
  'adjutant',
  'Autopilot',
  'OpenAgents Sites fulfillment supervisor.',
  'active',
  '{"program":"adjutant","surface":"openagents-core-team","agent":{"id":"agent_adjutant","name":"Autopilot","status":"active","scope":"project","runtime":"Autopilot","backend":"SHC","repository":"autopilot-omega","focus":"Sites"}}',
  '2026-06-05T00:00:00.000Z',
  '2026-06-05T00:00:00.000Z'
)
ON CONFLICT(id) DO UPDATE SET
  team_id = excluded.team_id,
  slug = excluded.slug,
  name = excluded.name,
  description = excluded.description,
  status = excluded.status,
  metadata_json = excluded.metadata_json,
  archived_at = NULL,
  updated_at = excluded.updated_at;
