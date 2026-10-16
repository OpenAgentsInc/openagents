CREATE TABLE IF NOT EXISTS coder_sync_schema (version INTEGER PRIMARY KEY);
CREATE TABLE IF NOT EXISTS chats (
    chat_id TEXT PRIMARY KEY,
    account TEXT NOT NULL,
    head BIGINT NOT NULL DEFAULT 0 CHECK (head >= 0),
    UNIQUE (account, chat_id)
);
CREATE TABLE IF NOT EXISTS chat_rows (
    chat_id TEXT NOT NULL REFERENCES chats(chat_id) ON DELETE CASCADE,
    seq BIGINT NOT NULL CHECK (seq > 0),
    batch_id TEXT NOT NULL,
    bytes BYTEA NOT NULL,
    at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (chat_id, seq),
    UNIQUE (chat_id, batch_id)
);
CREATE TABLE IF NOT EXISTS chat_checkpoints (
    chat_id TEXT PRIMARY KEY REFERENCES chats(chat_id) ON DELETE CASCADE,
    seq BIGINT NOT NULL CHECK (seq >= 0),
    object TEXT NOT NULL,
    size BIGINT NOT NULL CHECK (size >= 0),
    frontier TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS registry_heads (
    account TEXT PRIMARY KEY,
    seq BIGINT NOT NULL DEFAULT 0 CHECK (seq >= 0)
);
CREATE TABLE IF NOT EXISTS registry_rows (
    account TEXT NOT NULL REFERENCES registry_heads(account) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    id TEXT NOT NULL,
    seq BIGINT NOT NULL CHECK (seq > 0),
    clock BIGINT NOT NULL,
    writer TEXT NOT NULL,
    deleted BOOLEAN NOT NULL DEFAULT false,
    value JSONB NOT NULL,
    PRIMARY KEY (account, kind, id)
);
CREATE INDEX IF NOT EXISTS registry_since ON registry_rows(account, seq);
CREATE TABLE IF NOT EXISTS devices (
    device_id TEXT PRIMARY KEY,
    account TEXT NOT NULL,
    sidecars JSONB NOT NULL DEFAULT '{}'
);
CREATE TABLE IF NOT EXISTS device_sessions (
    device_id TEXT NOT NULL REFERENCES devices(device_id) ON DELETE CASCADE,
    connection_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('host', 'client')),
    connected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (device_id, connection_id)
);
CREATE TABLE IF NOT EXISTS blobs (
    account TEXT NOT NULL,
    id TEXT NOT NULL,
    chat_id TEXT REFERENCES chats(chat_id) ON DELETE CASCADE,
    object TEXT NOT NULL UNIQUE,
    size BIGINT NOT NULL CHECK (size >= 0),
    content_type TEXT NOT NULL,
    etag TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (account, id)
);
INSERT INTO coder_sync_schema(version) VALUES (1) ON CONFLICT DO NOTHING;
