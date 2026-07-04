-- Khala Sync capture checkpoints (KS-4.1, #8294).
-- Spec: docs/khala-sync/SPEC.md §4 (capture).
--
-- One row per scope: the highest changelog version the capture worker has
-- durably pushed to the scope's hub. Delivery is at-least-once (the hub
-- dedupes appends by version), so a checkpoint may lag a successful push
-- (crash between 2xx and the UPDATE) but must never lead one — capture
-- advances it ONLY after the hub acknowledged the batch.
--
-- pushed_through_version = 0 means "nothing pushed yet" (the watermark
-- convention from KS-2.2: 0 is the scope-start watermark, real versions
-- start at 1). Rows are created lazily on a scope's first successful push;
-- scope discovery joins against khala_sync_scopes, so absent rows behave
-- as checkpoint 0.

CREATE TABLE IF NOT EXISTS khala_sync_capture_checkpoints (
  scope                  text        PRIMARY KEY,
  pushed_through_version bigint      NOT NULL DEFAULT 0,
  updated_at             timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT khala_sync_capture_checkpoints_scope_shape
    CHECK (scope ~ '^scope\.[a-z_]+\.[A-Za-z0-9._:-]+$'),
  CONSTRAINT khala_sync_capture_checkpoints_watermark
    CHECK (pushed_through_version >= 0)
);
