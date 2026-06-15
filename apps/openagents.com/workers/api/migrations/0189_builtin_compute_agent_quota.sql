-- Free-tier quota ledger for the built-in hosted-compute (Gemini) agent.
--
-- A no-key user's built-in agent may be granted a hosted-Gemini broker grant
-- without ever seeing the shared hosted key. Each grant must stay within a
-- conservative per-user daily budget. Quota events are keyed by actor_user_id
-- (the agent's owner/user), reset daily (UTC start-of-day), and carry only safe
-- refs and bucketed counts. They never store raw prompts, completions, provider
-- payloads, API keys, mnemonics, or any secret material.

CREATE TABLE IF NOT EXISTS builtin_compute_agent_quota_events (
  id TEXT PRIMARY KEY,
  actor_user_id TEXT NOT NULL,
  grant_ref TEXT NOT NULL UNIQUE,
  provider TEXT NOT NULL CHECK (provider IN ('google_gemini')),
  budget_class TEXT NOT NULL CHECK (budget_class IN ('free_tier')),
  session_units INTEGER NOT NULL CHECK (session_units >= 0),
  session_budget_seconds INTEGER NOT NULL CHECK (session_budget_seconds >= 0),
  token_ceiling INTEGER NOT NULL CHECK (token_ceiling >= 0),
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS builtin_compute_agent_quota_actor_created_idx
  ON builtin_compute_agent_quota_events(actor_user_id, created_at DESC);
