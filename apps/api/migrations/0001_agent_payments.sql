-- Agent Payments API: agents and wallet registry.
-- Apply: npx wrangler d1 migrations apply openagents-api-payments --remote
-- Local: npx wrangler d1 migrations apply openagents-api-payments --local

-- Agents (id from API; name optional)
CREATE TABLE IF NOT EXISTS agents (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Wallet per agent: public payment coordinates only (no secrets).
-- Actual wallet state for Spark ops lives in spark-api (KV or separate store).
CREATE TABLE IF NOT EXISTS agent_wallets (
  agent_id INTEGER PRIMARY KEY REFERENCES agents(id) ON DELETE CASCADE,
  spark_address TEXT NOT NULL,
  lud16 TEXT,
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_agent_wallets_agent_id ON agent_wallets(agent_id);
