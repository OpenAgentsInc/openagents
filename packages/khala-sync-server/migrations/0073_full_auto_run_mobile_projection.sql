-- FA-RUN-05 (#8981): one owner-scoped row holding the latest published
-- `full_auto_run.mobile_projection.v1` projection (Desktop-published,
-- mobile-consumed via GET /api/full-auto-runs).
--
-- This is a LIVE PROJECTION store, not an append-only authority ledger like
-- `sarah_fleet_run_requests` -- Desktop is the single source of truth
-- (`apps/openagents-desktop/src/full-auto-run-registry.ts`, FA-RUN-01
-- #8969) and this row is always "last write wins" for one owner. Primary key
-- on `owner_user_id` alone both enforces the v1 "one active run per owner"
-- product policy and makes cross-owner isolation trivial: every query is
-- `WHERE owner_user_id = $1` against the authenticated caller's own id, so
-- there is no `run_ref`-keyed lookup path that could leak another owner's row.
--
-- Every column is exactly the public-safe field set in
-- packages/khala-sync/src/full-auto-run-client-projection.ts
-- (`FullAutoRunClientRunProjection`) -- no raw prompts, tool output, local
-- file paths, or credentials. `workspace_label` is a short derived label
-- ONLY, never the raw local `workspaceRef` filesystem path.

CREATE TABLE IF NOT EXISTS desktop_full_auto_run_projections (
  owner_user_id         text PRIMARY KEY,
  run_ref                text NOT NULL,
  thread_ref              text,
  objective               text NOT NULL,
  done_condition          text NOT NULL,
  lifecycle_state         text NOT NULL
    CHECK (lifecycle_state IN (
      'draft', 'running', 'pausing', 'paused', 'retrying',
      'stalled', 'completed', 'failed', 'stopped', 'cap_reached'
    )),
  workspace_label          text,
  started_at               text,
  updated_at                text NOT NULL,
  last_transition_actor     text NOT NULL,
  last_transition_at        text NOT NULL,
  published_at               text NOT NULL,
  CONSTRAINT desktop_full_auto_run_projections_owner_shape
    CHECK (owner_user_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{2,159}$'),
  CONSTRAINT desktop_full_auto_run_projections_run_ref_shape
    CHECK (run_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{0,179}$'),
  CONSTRAINT desktop_full_auto_run_projections_thread_ref_shape
    CHECK (thread_ref IS NULL OR thread_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{0,179}$'),
  CONSTRAINT desktop_full_auto_run_projections_workspace_label_shape
    CHECK (workspace_label IS NULL OR (
      length(workspace_label) BETWEEN 1 AND 200
      AND workspace_label !~ '[/\\]'
    ))
);

CREATE INDEX IF NOT EXISTS desktop_full_auto_run_projections_updated_idx
  ON desktop_full_auto_run_projections (owner_user_id, updated_at DESC);
