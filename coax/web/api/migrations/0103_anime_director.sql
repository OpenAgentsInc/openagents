-- #12632: effect-only quota ledger for the versioned Anime Lab director.
-- No words, stage state, model request/answer or signing key are stored here.
-- The C bridge holds advisory transaction lock 3073002 while it observes
-- counts, asks Coax aar_budget, and inserts one admitted lease. Model work
-- runs after COMMIT; aar_lease_ms owns the lease length, not this schema.
CREATE SCHEMA anime;

CREATE TABLE anime.calls (
  id bigserial PRIMARY KEY,
  identity text NOT NULL,
  started timestamptz NOT NULL DEFAULT clock_timestamp(),
  lease_until timestamptz NOT NULL,
  finished boolean NOT NULL DEFAULT false,
  status integer
);

CREATE INDEX anime_calls_identity_started ON anime.calls (identity, started);
CREATE INDEX anime_calls_started ON anime.calls (started);
CREATE INDEX anime_calls_active_lease ON anime.calls (lease_until, identity) WHERE NOT finished;
