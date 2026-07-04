-- Latest-row-per-entity index (KS-2.3 #8289; also serves KS-2.2 bootstrap).
--
-- Two hot queries walk the changelog per entity within a scope:
--   * bootstrap's snapshot derivation:
--       DISTINCT ON (entity_type, entity_id) ... WHERE scope = $1
--       ORDER BY entity_type, entity_id, version DESC
--   * compaction's "is this row superseded by a newer row for the same
--     entity" EXISTS probe (compaction preserves each live entity's latest
--     upsert row behind the retained-window watermark).
--
-- The primary key (scope, version, entity_type, entity_id) is ordered for
-- version-range scans, not per-entity lookups, so both of the above would
-- otherwise scan every version of the scope.
CREATE INDEX IF NOT EXISTS khala_sync_changelog_entity_latest_idx
  ON khala_sync_changelog (scope, entity_type, entity_id, version DESC);
