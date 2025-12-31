-- Autopilot agents table for Durable Object registry

CREATE TABLE IF NOT EXISTS agents (
    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    agent_id INTEGER NOT NULL,
    name TEXT,
    nostr_public_key TEXT,
    nostr_npub TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT,
    deleted_at TEXT,
    PRIMARY KEY (user_id, agent_id)
);

CREATE INDEX IF NOT EXISTS idx_agents_user ON agents(user_id);
CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);
