CREATE TABLE IF NOT EXISTS site_payment_catalog_items (
  id TEXT PRIMARY KEY NOT NULL,
  catalog_ref TEXT NOT NULL UNIQUE,
  item_kind TEXT NOT NULL CHECK (item_kind IN ('product', 'paid_action')),
  site_id TEXT NOT NULL,
  site_version_id TEXT NOT NULL,
  deployment_id TEXT,
  order_ref TEXT,
  workroom_ref TEXT,
  manifest_ref TEXT,
  source_manifest_digest TEXT,
  product_id TEXT,
  action_id TEXT,
  action_ref TEXT,
  method TEXT CHECK (
    method IS NULL OR method IN ('GET', 'POST', 'PUT', 'PATCH', 'DELETE')
  ),
  path TEXT,
  display_ref TEXT NOT NULL,
  checkout_path TEXT NOT NULL,
  price_asset TEXT NOT NULL CHECK (price_asset IN ('bitcoin', 'credits', 'usd')),
  price_denomination TEXT NOT NULL,
  price_amount_minor_units INTEGER NOT NULL CHECK (price_amount_minor_units >= 0),
  entitlement_scope TEXT NOT NULL CHECK (
    entitlement_scope IN ('account', 'action', 'path', 'product', 'site')
  ),
  settlement_mode TEXT NOT NULL CHECK (
    settlement_mode IN ('accepted_work_linked', 'checkout_only', 'deferred')
  ),
  public_projection_state TEXT NOT NULL CHECK (
    public_projection_state IN ('hidden', 'listed', 'proof_only', 'redacted')
  ),
  sandbox INTEGER NOT NULL CHECK (sandbox IN (0, 1)),
  agent_readable INTEGER NOT NULL CHECK (agent_readable IN (0, 1)),
  status TEXT NOT NULL CHECK (status IN ('active', 'archived', 'draft', 'retired')),
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  customer_data_requirements_json TEXT NOT NULL DEFAULT '[]',
  paid_endpoint_product_json TEXT NOT NULL DEFAULT '{}',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  CHECK (
    (item_kind = 'product' AND product_id IS NOT NULL AND action_id IS NULL AND action_ref IS NULL AND method IS NULL AND path IS NULL) OR
    (item_kind = 'paid_action' AND product_id IS NULL AND action_id IS NOT NULL AND action_ref IS NOT NULL AND method IS NOT NULL AND path IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS site_payment_catalog_items_site_version_idx
  ON site_payment_catalog_items(site_id, site_version_id, item_kind, status);

CREATE INDEX IF NOT EXISTS site_payment_catalog_items_deployment_idx
  ON site_payment_catalog_items(deployment_id, item_kind, status)
  WHERE deployment_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS site_payment_catalog_items_order_idx
  ON site_payment_catalog_items(order_ref, workroom_ref, updated_at DESC)
  WHERE order_ref IS NOT NULL OR workroom_ref IS NOT NULL;

CREATE INDEX IF NOT EXISTS site_payment_catalog_items_product_idx
  ON site_payment_catalog_items(product_id, status)
  WHERE product_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS site_payment_catalog_items_action_idx
  ON site_payment_catalog_items(action_id, action_ref, status)
  WHERE action_id IS NOT NULL;
