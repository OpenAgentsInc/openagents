-- Free public SystemOne demo accounting. Limits and admission live in
-- mesh-policy.coax. Hosts hold one transaction advisory lock while reading
-- these observations and recording the admitted lease, across all revisions.
CREATE SCHEMA IF NOT EXISTS mesh;
CREATE TABLE mesh.calls (
  id bigserial PRIMARY KEY,
  pubkey text NOT NULL,
  identity text NOT NULL,
  started timestamptz NOT NULL DEFAULT clock_timestamp(),
  lease_until timestamptz NOT NULL DEFAULT clock_timestamp() + interval '60 seconds',
  finished boolean NOT NULL DEFAULT false,
  status int NOT NULL DEFAULT 0
);
CREATE INDEX mesh_calls_started ON mesh.calls(started);
CREATE INDEX mesh_calls_account ON mesh.calls(pubkey, started);
