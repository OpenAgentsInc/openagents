UPDATE team_projects
SET
  metadata_json = json_set(
    COALESCE(NULLIF(metadata_json, ''), '{}'),
    '$.program',
    'artanis',
    '$.surface',
    'openagents-core-team',
    '$.agent',
    json_object(
      'id',
      'agent_artanis',
      'name',
      'Artanis',
      'status',
      'active',
      'scope',
      'project',
      'runtime',
      'Autopilot',
      'backend',
      'SHC',
      'repository',
      'autopilot-omega',
      'focus',
      'Pylon'
    )
  ),
  updated_at = '2026-06-04T00:55:00.000Z'
WHERE id = 'project_artanis'
  AND team_id = 'team_openagents_core'
  AND archived_at IS NULL;
