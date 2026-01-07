-- OpenAgents Nexus Relay - D1 Schema
-- Events table for persistent storage

CREATE TABLE IF NOT EXISTS events (
    id TEXT PRIMARY KEY,
    pubkey TEXT NOT NULL,
    kind INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    content TEXT NOT NULL,
    tags TEXT NOT NULL,  -- JSON array
    sig TEXT NOT NULL
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_events_pubkey ON events(pubkey);
CREATE INDEX IF NOT EXISTS idx_events_kind ON events(kind);
CREATE INDEX IF NOT EXISTS idx_events_created_at ON events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_events_kind_created ON events(kind, created_at DESC);

-- Composite index for author + kind queries (common for NIP-90)
CREATE INDEX IF NOT EXISTS idx_events_pubkey_kind ON events(pubkey, kind);
