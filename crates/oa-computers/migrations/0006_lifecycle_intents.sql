-- Distinguish a new wake or queued command from an older running intent.
ALTER TABLE computers.computers
    ADD COLUMN intent_generation bigint NOT NULL DEFAULT 0 CHECK (intent_generation >= 0);
