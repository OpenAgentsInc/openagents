-- Gift-wrap ciphertext is recipient-private and must never enter full-text
-- search. This keeps the access-gated Block exclusions from migration 5 and
-- adds the generic NIP-17 gift-wrap kind.
DROP INDEX nostr_event_search_idx;
ALTER TABLE nostr_event DROP COLUMN search_vector;
ALTER TABLE nostr_event ADD COLUMN search_vector tsvector GENERATED ALWAYS AS (
    CASE
        WHEN kind = 1059
          OR kind IN (30078, 30174, 30175, 30178, 30300, 30350, 30622, 44200)
        THEN NULL::tsvector
        ELSE to_tsvector('simple'::regconfig, content)
    END
) STORED;
CREATE INDEX nostr_event_search_idx
    ON nostr_event USING gin (search_vector);
