CREATE TABLE IF NOT EXISTS site_mdk_account_bindings (
  id TEXT PRIMARY KEY NOT NULL,
  binding_ref TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  site_id TEXT NOT NULL,
  site_version_id TEXT,
  customer_ref TEXT,
  order_ref TEXT,
  requested_provider_mode TEXT NOT NULL CHECK (
    requested_provider_mode IN ('customer_owned_mdk')
  ),
  environment TEXT NOT NULL CHECK (environment IN ('production', 'sandbox')),
  review_status TEXT NOT NULL CHECK (
    review_status IN ('approved', 'blocked', 'pending_review', 'revoked')
  ),
  secret_binding_refs_json TEXT NOT NULL DEFAULT '[]',
  allowed_catalog_refs_json TEXT NOT NULL DEFAULT '[]',
  allowed_product_refs_json TEXT NOT NULL DEFAULT '[]',
  allowed_action_refs_json TEXT NOT NULL DEFAULT '[]',
  reviewer_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (site_id, binding_ref)
);

CREATE INDEX IF NOT EXISTS site_mdk_account_bindings_site_status_idx
  ON site_mdk_account_bindings(site_id, review_status, updated_at DESC)
  WHERE archived_at IS NULL;
