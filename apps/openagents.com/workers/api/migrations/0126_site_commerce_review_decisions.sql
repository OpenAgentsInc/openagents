CREATE TABLE IF NOT EXISTS site_commerce_review_decisions (
  id TEXT PRIMARY KEY NOT NULL,
  decision_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  site_id TEXT NOT NULL,
  site_version_id TEXT NOT NULL,
  catalog_ref TEXT NOT NULL,
  review_status TEXT NOT NULL CHECK (
    review_status IN (
      'accepted',
      'held',
      'needs_customer_input',
      'rejected'
    )
  ),
  reason_refs_json TEXT NOT NULL DEFAULT '[]',
  customer_input_requirement_refs_json TEXT NOT NULL DEFAULT '[]',
  actor_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (site_id, site_version_id, catalog_ref)
);

CREATE INDEX IF NOT EXISTS site_commerce_review_decisions_site_idx
  ON site_commerce_review_decisions(site_id, site_version_id, updated_at DESC)
  WHERE archived_at IS NULL;
