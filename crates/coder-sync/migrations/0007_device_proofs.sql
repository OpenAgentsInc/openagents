CREATE TABLE IF NOT EXISTS device_host_keys (
    device_id TEXT PRIMARY KEY REFERENCES devices(device_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    public_key TEXT NOT NULL,
    revoked BOOLEAN NOT NULL DEFAULT false
);
CREATE TABLE IF NOT EXISTS device_proof_events (
    event_id TEXT PRIMARY KEY,
    expires_at BIGINT NOT NULL
);
INSERT INTO coder_sync_schema(version) VALUES(7) ON CONFLICT DO NOTHING;
