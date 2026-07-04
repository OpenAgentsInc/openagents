-- Khala Sync fleet cockpit scope (KS-6.1, #8302).
-- Spec: docs/khala-sync/SPEC.md §2.1 (scope.fleet_run.<id>), §7 invariant 9.
--
-- khala_sync_scope_owners: per-scope ownership for owner-gated scopes
-- (v1: fleet_run scopes). The row is written on the scope's FIRST
-- projection append (or first operator mutation) and never migrates —
-- ownership claims are first-writer-wins by primary key. The v1 read gate
-- and the fleet operator mutators both consult this table.
--
-- khala_sync_fleet_intents: operator intents (set_desired_slots / pause /
-- resume) recorded by the fleet mutators. HONEST V1: intents are recorded
-- and the updated fleet_run post-image is projected in the same
-- transaction, but supervisor ENFORCEMENT (the Pylon-side supervisor
-- consuming these intents) is a follow-up lane — an intent row is a
-- durable request, not proof the fleet changed behavior.

CREATE TABLE IF NOT EXISTS khala_sync_scope_owners (
  scope         text        PRIMARY KEY,
  owner_user_id text        NOT NULL,
  created_at    timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT khala_sync_scope_owners_scope_shape
    CHECK (scope ~ '^scope\.[a-z_]+\.[A-Za-z0-9._:-]+$'),
  CONSTRAINT khala_sync_scope_owners_owner_nonempty
    CHECK (length(owner_user_id) > 0)
);

CREATE TABLE IF NOT EXISTS khala_sync_fleet_intents (
  id                   bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  scope                text        NOT NULL,
  run_id               text        NOT NULL,
  intent               text        NOT NULL
    CHECK (intent IN ('set_desired_slots', 'pause', 'resume')),
  desired_slots        integer
    CHECK (desired_slots IS NULL OR (desired_slots >= 0 AND desired_slots <= 1024)),
  requested_by_user_id text        NOT NULL,
  mutation_ref         text        NOT NULL,
  created_at           timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT khala_sync_fleet_intents_scope_shape
    CHECK (scope ~ '^scope\.fleet_run\.[A-Za-z0-9._:-]+$'),
  -- set_desired_slots carries a slot count; pause/resume never do.
  CONSTRAINT khala_sync_fleet_intents_slots_shape
    CHECK ((intent = 'set_desired_slots') = (desired_slots IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS khala_sync_fleet_intents_scope_idx
  ON khala_sync_fleet_intents (scope, created_at);
