ALTER TABLE chat_rows ADD COLUMN IF NOT EXISTS device TEXT NOT NULL DEFAULT '';
CREATE TABLE IF NOT EXISTS chat_batches (
    chat_id TEXT NOT NULL REFERENCES chats(chat_id) ON DELETE CASCADE,
    batch_id TEXT NOT NULL,
    seq BIGINT NOT NULL CHECK (seq > 0),
    PRIMARY KEY (chat_id, batch_id),
    UNIQUE (chat_id, seq)
);
INSERT INTO chat_batches(chat_id,batch_id,seq)
SELECT chat_id,batch_id,seq FROM chat_rows ON CONFLICT DO NOTHING;
INSERT INTO coder_sync_schema(version) VALUES (2) ON CONFLICT DO NOTHING;
