CREATE TABLE IF NOT EXISTS device_grants (
    device_id TEXT NOT NULL REFERENCES devices(device_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    PRIMARY KEY(device_id,account)
);
CREATE TABLE IF NOT EXISTS device_nudges (
    account TEXT NOT NULL,
    device_id TEXT NOT NULL,
    chat_id TEXT NOT NULL,
    token TEXT NOT NULL,
    queued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(account,device_id,chat_id)
);
CREATE TABLE IF NOT EXISTS device_reconcile (
    account TEXT NOT NULL,
    device_id TEXT NOT NULL,
    token TEXT NOT NULL,
    PRIMARY KEY(account,device_id)
);
INSERT INTO coder_sync_schema(version) VALUES(4) ON CONFLICT DO NOTHING;
