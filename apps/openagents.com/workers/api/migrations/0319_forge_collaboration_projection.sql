-- FORGE-06 (#9248): admitted collaboration event read model.
--
-- This table stores only events that passed the Forge signature, membership,
-- repository, and object gates. Raw relay input and purgatory rows are not
-- readable collaboration state.

CREATE TABLE IF NOT EXISTS forge_git_projected_events (
  tenant_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  event_id TEXT NOT NULL,
  kind INTEGER NOT NULL CHECK (
    kind IN (1111, 1617, 1618, 1619, 1621, 1630, 1631, 1632, 1633, 30617, 30618)
  ),
  author_pubkey TEXT NOT NULL,
  actor_binding_ref TEXT NOT NULL,
  event_json TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  projected_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, event_id)
);
CREATE INDEX IF NOT EXISTS idx_forge_git_projected_events_repository
  ON forge_git_projected_events(tenant_ref, repository_ref, observed_at, event_id);
