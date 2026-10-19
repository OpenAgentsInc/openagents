-- Turn request keys (#12142). A client names a turn submission with a key;
-- a repeat with the same body returns the original turn, and a repeat with a
-- different body is refused.
ALTER TABLE computers.turns ADD COLUMN request_key text, ADD COLUMN request_digest text;
CREATE UNIQUE INDEX turns_request_key ON computers.turns (owner, computer, request_key)
    WHERE request_key IS NOT NULL;
