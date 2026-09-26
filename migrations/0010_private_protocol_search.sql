-- Rebuild the index for existing rows as well as future private records.
-- Access predicates also guard history, counting, and live delivery.
DROP INDEX nostr_event_search_idx;
ALTER TABLE nostr_event DROP COLUMN search_vector;
ALTER TABLE nostr_event ADD COLUMN search_vector tsvector GENERATED ALWAYS AS (
    CASE
        WHEN kind IN (78, 1059, 3187, 3188, 21059, 30078, 30174, 30175,
                      30178, 30179, 30186, 30300, 30350, 30622, 44200)
          OR (kind = 30181 AND tags @> '[["t","oa:cap-policy:private:v1"]]'::jsonb)
        THEN NULL::tsvector
        ELSE to_tsvector('simple'::regconfig, content)
    END
) STORED;
CREATE INDEX nostr_event_search_idx ON nostr_event USING gin (search_vector);

CREATE TABLE relay_query_authorization (
    event_id text COLLATE "C" PRIMARY KEY,
    expires_at bigint NOT NULL
);
CREATE INDEX relay_query_authorization_expiry ON relay_query_authorization (expires_at);
