-- Block NIP-OA/NIP-AA identity plus NIP-AM private turn metrics.

CREATE TABLE agent_owner (
    agent_pubkey text COLLATE "C" PRIMARY KEY,
    owner_pubkey text COLLATE "C" NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT agent_owner_agent_shape CHECK (
        agent_pubkey ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT agent_owner_owner_shape CHECK (
        owner_pubkey ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT agent_owner_distinct_keys CHECK (
        agent_pubkey <> owner_pubkey
    )
);

CREATE INDEX agent_owner_owner_idx
    ON agent_owner (owner_pubkey, agent_pubkey);

-- NIP-AM content is encrypted and MUST NOT enter full-text search. A
-- generated expression cannot be changed in place, so the additive migration
-- replaces the generated column and its dependent index.
DROP INDEX nostr_event_search_idx;
ALTER TABLE nostr_event DROP COLUMN search_vector;
ALTER TABLE nostr_event ADD COLUMN search_vector tsvector GENERATED ALWAYS AS (
    CASE
        WHEN kind = 44200 THEN NULL::tsvector
        ELSE to_tsvector('simple'::regconfig, content)
    END
) STORED;
CREATE INDEX nostr_event_search_idx
    ON nostr_event USING gin (search_vector);
