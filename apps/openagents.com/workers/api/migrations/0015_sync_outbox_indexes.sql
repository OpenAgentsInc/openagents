CREATE INDEX IF NOT EXISTS sync_mutations_scope_idx
  ON sync_mutations(scope, created_at);

CREATE INDEX IF NOT EXISTS sync_changes_collection_idx
  ON sync_changes(scope, collection, entity_id);
