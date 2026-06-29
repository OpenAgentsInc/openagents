CREATE TABLE IF NOT EXISTS site_commerce_products (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL REFERENCES site_projects(id) ON DELETE CASCADE,
  site_version_id TEXT REFERENCES site_versions(id) ON DELETE SET NULL,
  product_key TEXT NOT NULL CHECK (length(product_key) > 0),
  name TEXT NOT NULL CHECK (length(name) > 0),
  asset TEXT NOT NULL CHECK (asset IN ('usd', 'sats', 'credits')),
  amount INTEGER NOT NULL CHECK (amount > 0),
  checkout_path TEXT NOT NULL CHECK (
    length(checkout_path) > 0
    AND substr(checkout_path, 1, 1) = '/'
    AND instr(checkout_path, '?') = 0
    AND instr(checkout_path, '#') = 0
  ),
  entitlement_scope TEXT NOT NULL CHECK (
    entitlement_scope IN ('site', 'product', 'path', 'action', 'account')
  ),
  agent_readable INTEGER NOT NULL CHECK (agent_readable IN (0, 1)),
  settlement_mode TEXT NOT NULL CHECK (
    settlement_mode IN ('checkout_only', 'deferred', 'accepted_work_linked')
  ),
  customer_data_requirements_json TEXT NOT NULL DEFAULT '[]',
  public_projection_state TEXT NOT NULL CHECK (
    public_projection_state IN ('hidden', 'listed', 'redacted', 'proof_only')
  ),
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS site_commerce_products_active_key_idx
  ON site_commerce_products(site_id, product_key)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS site_commerce_products_site_version_idx
  ON site_commerce_products(site_id, site_version_id, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS site_commerce_paid_actions (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL REFERENCES site_projects(id) ON DELETE CASCADE,
  site_version_id TEXT REFERENCES site_versions(id) ON DELETE SET NULL,
  action_key TEXT NOT NULL CHECK (length(action_key) > 0),
  name TEXT NOT NULL CHECK (length(name) > 0),
  method TEXT NOT NULL CHECK (method IN ('GET', 'POST')),
  path TEXT NOT NULL CHECK (
    length(path) > 0
    AND substr(path, 1, 1) = '/'
    AND instr(path, '?') = 0
    AND instr(path, '#') = 0
  ),
  asset TEXT NOT NULL CHECK (asset IN ('usd', 'sats', 'credits')),
  amount INTEGER NOT NULL CHECK (amount > 0),
  checkout_path TEXT NOT NULL CHECK (
    length(checkout_path) > 0
    AND substr(checkout_path, 1, 1) = '/'
    AND instr(checkout_path, '?') = 0
    AND instr(checkout_path, '#') = 0
  ),
  entitlement_scope TEXT NOT NULL CHECK (
    entitlement_scope IN ('site', 'product', 'path', 'action', 'account')
  ),
  agent_readable INTEGER NOT NULL CHECK (agent_readable IN (0, 1)),
  settlement_mode TEXT NOT NULL CHECK (
    settlement_mode IN ('checkout_only', 'deferred', 'accepted_work_linked')
  ),
  customer_data_requirements_json TEXT NOT NULL DEFAULT '[]',
  public_projection_state TEXT NOT NULL CHECK (
    public_projection_state IN ('hidden', 'listed', 'redacted', 'proof_only')
  ),
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS site_commerce_paid_actions_active_key_idx
  ON site_commerce_paid_actions(site_id, action_key)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS site_commerce_paid_actions_site_version_idx
  ON site_commerce_paid_actions(site_id, site_version_id, updated_at DESC)
  WHERE archived_at IS NULL;
