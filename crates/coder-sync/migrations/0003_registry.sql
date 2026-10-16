ALTER TABLE registry_heads ADD COLUMN IF NOT EXISTS gc_floor BIGINT NOT NULL DEFAULT 0 CHECK (gc_floor>=0 AND gc_floor<=seq);
CREATE TABLE IF NOT EXISTS registry_push_targets (
    account TEXT NOT NULL REFERENCES registry_heads(account) ON DELETE CASCADE,
    device TEXT NOT NULL,
    token TEXT NOT NULL,
    environment TEXT NOT NULL CHECK (environment IN ('sandbox','production')),
    prefs JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(account,device)
);
INSERT INTO coder_sync_schema(version) VALUES(3) ON CONFLICT DO NOTHING;
