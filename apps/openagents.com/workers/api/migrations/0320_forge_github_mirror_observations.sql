-- FORGE-09 (#9251): durable GitHub mirror observations.
--
-- Each row records one downstream observation. The bare repository and the
-- applied internal receipt stay authoritative. GitHub does not supply
-- coordination authority.

CREATE TABLE IF NOT EXISTS forge_github_mirror_observations (
  tenant_ref TEXT NOT NULL,
  observation_ref TEXT NOT NULL,
  intent_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  authority_mode TEXT NOT NULL CHECK (
    authority_mode IN ('github_authoritative', 'openagents_git_authoritative')
  ),
  authority_generation INTEGER NOT NULL,
  source_ref TEXT NOT NULL,
  source_object_id TEXT,
  destination_github_repository TEXT NOT NULL,
  destination_github_ref TEXT NOT NULL,
  destination_object_id TEXT,
  divergence TEXT NOT NULL CHECK (
    divergence IN (
      'not_applicable',
      'in_sync',
      'source_ahead',
      'destination_ahead',
      'diverged',
      'destination_missing',
      'unknown'
    )
  ),
  observed_at TEXT NOT NULL,
  error_reason TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  redacted INTEGER NOT NULL DEFAULT 1 CHECK (redacted = 1),
  created_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, observation_ref)
);

CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_observations_intent
  ON forge_github_mirror_observations (
    tenant_ref,
    intent_ref,
    observed_at
  );
