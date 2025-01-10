CREATE TABLE events (
    id TEXT PRIMARY KEY,
    pubkey TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    kind INTEGER NOT NULL, 
    content TEXT NOT NULL,
    sig TEXT NOT NULL,
    tags JSONB NOT NULL,
    CONSTRAINT valid_id CHECK (length(id) = 64),
    CONSTRAINT valid_pubkey CHECK (length(pubkey) = 64),
    CONSTRAINT valid_sig CHECK (length(sig) = 128)
);

CREATE INDEX idx_events_pubkey ON events(pubkey);
CREATE INDEX idx_events_created_at ON events(created_at);
CREATE INDEX idx_events_kind ON events(kind);
CREATE INDEX idx_events_tags ON events USING gin(tags);
