-- A phone's push gateway delivery grant (#11853). The native token stays
-- with the gateway; this server keeps only the grant it presents.
CREATE TABLE IF NOT EXISTS registry_push_grants (
    account TEXT NOT NULL REFERENCES registry_heads(account) ON DELETE CASCADE,
    device TEXT NOT NULL,
    transport TEXT NOT NULL CHECK (transport IN ('apns','fcm')),
    endpoint_grant TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    prefs JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(account,device)
);
INSERT INTO coder_sync_schema(version) VALUES(6) ON CONFLICT DO NOTHING;
