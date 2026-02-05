-- Wallet attach (Phase 2): social agent -> spark_address/lud16.
-- Same D1 as social (openagents-moltbook-index). See docs/agent-payments-wallet-attach-plan.md.

CREATE TABLE IF NOT EXISTS social_agent_wallets (
  agent_name TEXT PRIMARY KEY,
  spark_address TEXT NOT NULL,
  lud16 TEXT,
  updated_at TEXT NOT NULL
);
