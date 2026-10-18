CREATE TABLE owner_notification_grants (
    account TEXT NOT NULL,
    agent TEXT NOT NULL,
    enabled BOOLEAN NOT NULL,
    PRIMARY KEY (account, agent)
);
CREATE TABLE owner_notifications (
    sequence BIGSERIAL NOT NULL,
    account TEXT NOT NULL,
    request_id TEXT NOT NULL,
    agent TEXT NOT NULL,
    note JSONB NOT NULL,
    created_at BIGINT NOT NULL,
    PRIMARY KEY (account, request_id)
);
CREATE INDEX owner_notifications_rate ON owner_notifications(account, agent, created_at);
CREATE TABLE owner_notification_deliveries (
    account TEXT NOT NULL,
    request_id TEXT NOT NULL,
    device TEXT NOT NULL,
    delivered BOOLEAN NOT NULL DEFAULT FALSE,
    read BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (account, request_id, device),
    FOREIGN KEY (account, request_id) REFERENCES owner_notifications(account, request_id) ON DELETE CASCADE
);
INSERT INTO coder_sync_schema(version) VALUES(8);
