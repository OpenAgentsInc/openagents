-- Khala Sync Client View Records (KS-7.2, #8306).
-- Design: docs/khala-sync/CVR_DESIGN.md. Reference spec: Replicache
-- row-version strategy. Flag-gated (KHALA_SYNC_CVR=1): unflagged
-- deployments never write or read this table.
--
-- One row per (client_group_id, scope, cvr_version): the exact row set —
-- entity key → per-scope changelog version — one client group's durable
-- state was reconciled to by one CVR pull, taken at `snapshot_cursor`.
-- The next diff pull loads the referenced CVR, computes the current
-- authorized row set at a fresh snapshot, and set-diffs: puts (new/changed
-- rows) and dels (rows that LEFT the set — deleted, compacted-away, or no
-- longer authorized) fall out structurally, without tombstone retention
-- and without a full re-bootstrap.
--
-- `entries` is a single jsonb object {"<entity_type>/<entity_id>": version}
-- (entity types match ^[a-z][a-z0-9_]*$, so the FIRST '/' is an
-- unambiguous separator). One-object-per-CVR is deliberate for our scope
-- sizes (scopes are bounded — fleet runs / threads / personal workrooms in
-- the tens-to-thousands of entities; the service caps the row set and
-- refuses beyond it); a side table keyed per entry only starts winning
-- when row sets grow past ~10^5 entries or when partial CVR updates are
-- needed — see CVR_DESIGN.md §3 for the full justification.
--
-- Retention: pulls prune versions older than the newest few (the service
-- keeps CVR_RETAINED_VERSIONS per (group, scope)); a pull referencing a
-- pruned/unknown cvr_version degrades safely to a reset-mode response.

CREATE TABLE IF NOT EXISTS khala_sync_cvrs (
  client_group_id text        NOT NULL,
  scope           text        NOT NULL,
  cvr_version     bigint      NOT NULL CHECK (cvr_version >= 1),
  snapshot_cursor bigint      NOT NULL CHECK (snapshot_cursor >= 0),
  entries         jsonb       NOT NULL,
  created_at      timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (client_group_id, scope, cvr_version),
  CONSTRAINT khala_sync_cvrs_scope_shape
    CHECK (scope ~ '^scope\.[a-z_]+\.[A-Za-z0-9._:-]+$'),
  CONSTRAINT khala_sync_cvrs_client_group_nonempty
    CHECK (length(client_group_id) > 0)
);

-- For a future age-based janitor (per-pull pruning already bounds growth
-- per (group, scope); this covers abandoned client groups).
CREATE INDEX IF NOT EXISTS khala_sync_cvrs_created_at_idx
  ON khala_sync_cvrs (created_at);
