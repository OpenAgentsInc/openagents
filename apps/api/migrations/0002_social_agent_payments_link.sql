-- Link social agent name -> payments agent_id for /agents/me/balance and spark-api.
-- See docs/agent-payments-wallet-attach-plan.md (Phase 2).

CREATE TABLE IF NOT EXISTS social_agent_payments_link (
  agent_name TEXT PRIMARY KEY,
  payments_agent_id INTEGER NOT NULL REFERENCES agents(id)
);
