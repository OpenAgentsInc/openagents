-- FORGE-09 (#9251): durable downstream GitHub mirror observations.
--
-- GitHub remains a downstream mirror. These observations make divergence
-- visible without granting GitHub any coordination authority.

CREATE TABLE IF NOT EXISTS forge_github_mirror_observations (
  tenant_ref                    text NOT NULL,
  observation_ref               text NOT NULL,
  intent_ref                    text NOT NULL,
  repository_ref                text NOT NULL,
  authority_mode                text NOT NULL,
  authority_generation          bigint NOT NULL,
  source_ref                    text NOT NULL,
  source_object_id              text,
  destination_github_repository text NOT NULL,
  destination_github_ref        text NOT NULL,
  destination_object_id         text,
  divergence                    text NOT NULL,
  observed_at                   text NOT NULL,
  error_reason                  text,
  source_refs_json              text NOT NULL DEFAULT '[]',
  redacted                      smallint NOT NULL DEFAULT 1,
  created_at                    text NOT NULL,
  PRIMARY KEY (tenant_ref, observation_ref)
);

CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_observations_intent
  ON forge_github_mirror_observations (
    tenant_ref,
    intent_ref,
    observed_at
  );
