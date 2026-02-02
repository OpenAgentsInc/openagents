-- OpenClaw Earn: store the latest public invoice for /openclaw/earn.
-- Apply: npx wrangler d1 migrations apply openagents-api-payments --remote
-- Local: npx wrangler d1 migrations apply openagents-api-payments --local

CREATE TABLE IF NOT EXISTS openclaw_invoices (
  key TEXT PRIMARY KEY,
  payment_request TEXT NOT NULL,
  amount_sats INTEGER NOT NULL,
  description TEXT,
  expires_at TEXT NOT NULL,
  expires_at_ms INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_openclaw_invoices_expires_at ON openclaw_invoices(expires_at_ms);
