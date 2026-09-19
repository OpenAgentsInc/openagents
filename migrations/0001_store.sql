-- Immortal M2 store. Applied atomically by src/store/migration.rs.

CREATE TABLE nostr_event (
    id text COLLATE "C" PRIMARY KEY,
    pubkey text COLLATE "C" NOT NULL,
    created_at bigint NOT NULL,
    kind integer NOT NULL,
    tags jsonb NOT NULL,
    content text NOT NULL,
    sig text COLLATE "C" NOT NULL,
    replacement_identifier text COLLATE "C",
    expires_at bigint,
    ingest_seq bigint GENERATED ALWAYS AS IDENTITY NOT NULL UNIQUE,
    received_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    search_vector tsvector GENERATED ALWAYS AS (
        to_tsvector('simple'::regconfig, content)
    ) STORED,
    CONSTRAINT nostr_event_id_shape CHECK (id ~ '^[0-9a-f]{64}$'),
    CONSTRAINT nostr_event_pubkey_shape CHECK (pubkey ~ '^[0-9a-f]{64}$'),
    CONSTRAINT nostr_event_sig_shape CHECK (sig ~ '^[0-9a-f]{128}$'),
    CONSTRAINT nostr_event_created_at_range CHECK (created_at >= 0),
    CONSTRAINT nostr_event_kind_range CHECK (kind BETWEEN 0 AND 65535),
    CONSTRAINT nostr_event_tags_array CHECK (jsonb_typeof(tags) = 'array'),
    CONSTRAINT nostr_event_expiration_range CHECK (
        expires_at IS NULL OR expires_at >= 0
    ),
    CONSTRAINT nostr_event_no_ephemeral CHECK (
        kind < 20000 OR kind >= 30000
    ),
    CONSTRAINT nostr_event_replacement_identifier CHECK (
        (
            kind IN (0, 3)
            OR kind BETWEEN 10000 AND 19999
            OR kind BETWEEN 30000 AND 39999
        ) = (replacement_identifier IS NOT NULL)
    )
);

CREATE INDEX nostr_event_created_at_idx
    ON nostr_event (created_at DESC, id);
CREATE INDEX nostr_event_pubkey_created_at_idx
    ON nostr_event (pubkey, created_at DESC, id);
CREATE INDEX nostr_event_kind_created_at_idx
    ON nostr_event (kind, created_at DESC, id);
CREATE INDEX nostr_event_pubkey_kind_created_at_idx
    ON nostr_event (pubkey, kind, created_at DESC, id);
CREATE INDEX nostr_event_expiration_idx
    ON nostr_event (expires_at)
    WHERE expires_at IS NOT NULL;
CREATE INDEX nostr_event_search_idx
    ON nostr_event USING gin (search_vector);

CREATE TABLE nostr_indexed_tag (
    event_id text COLLATE "C" NOT NULL
        REFERENCES nostr_event(id) ON DELETE CASCADE,
    tag_name text COLLATE "C" NOT NULL,
    tag_value text COLLATE "C" NOT NULL,
    created_at bigint NOT NULL,
    PRIMARY KEY (event_id, tag_name, tag_value),
    CONSTRAINT nostr_indexed_tag_name CHECK (
        octet_length(tag_name) = 1 AND tag_name ~ '^[A-Za-z]$'
    ),
    CONSTRAINT nostr_indexed_tag_created_at CHECK (created_at >= 0)
);

CREATE INDEX nostr_indexed_tag_lookup_idx
    ON nostr_indexed_tag (tag_name, tag_value, created_at DESC, event_id);

CREATE TABLE replaceable_head (
    kind integer NOT NULL,
    pubkey text COLLATE "C" NOT NULL,
    identifier text COLLATE "C" NOT NULL,
    event_id text COLLATE "C" NOT NULL UNIQUE
        REFERENCES nostr_event(id) ON DELETE CASCADE,
    created_at bigint NOT NULL,
    PRIMARY KEY (kind, pubkey, identifier),
    CONSTRAINT replaceable_head_kind CHECK (
        kind IN (0, 3)
        OR kind BETWEEN 10000 AND 19999
        OR kind BETWEEN 30000 AND 39999
    ),
    CONSTRAINT replaceable_head_identifier CHECK (
        (kind BETWEEN 30000 AND 39999) OR identifier = ''
    ),
    CONSTRAINT replaceable_head_pubkey_shape CHECK (
        pubkey ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT replaceable_head_created_at CHECK (created_at >= 0)
);

CREATE TABLE deletion_tombstone (
    tombstone_type text NOT NULL,
    event_id text COLLATE "C",
    kind integer,
    author_pubkey text COLLATE "C" NOT NULL,
    identifier text COLLATE "C",
    deleted_through bigint,
    deletion_event_id text COLLATE "C" NOT NULL
        REFERENCES nostr_event(id) ON DELETE RESTRICT,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT deletion_tombstone_type CHECK (
        tombstone_type IN ('event', 'address')
    ),
    CONSTRAINT deletion_tombstone_author_shape CHECK (
        author_pubkey ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT deletion_tombstone_shape CHECK (
        (
            tombstone_type = 'event'
            AND event_id IS NOT NULL
            AND kind IS NULL
            AND identifier IS NULL
            AND deleted_through IS NULL
        ) OR (
            tombstone_type = 'address'
            AND event_id IS NULL
            AND kind IS NOT NULL
            AND identifier IS NOT NULL
            AND deleted_through IS NOT NULL
            AND deleted_through >= 0
        )
    )
);

CREATE UNIQUE INDEX deletion_tombstone_event_unique_idx
    ON deletion_tombstone (event_id, author_pubkey)
    WHERE tombstone_type = 'event';
CREATE UNIQUE INDEX deletion_tombstone_address_unique_idx
    ON deletion_tombstone (kind, author_pubkey, identifier)
    WHERE tombstone_type = 'address';
CREATE INDEX deletion_tombstone_deletion_event_idx
    ON deletion_tombstone (deletion_event_id);

CREATE TABLE relay_policy (
    singleton boolean PRIMARY KEY DEFAULT TRUE,
    closed_membership boolean NOT NULL DEFAULT FALSE,
    max_content_bytes bigint NOT NULL DEFAULT 131072,
    max_tags integer NOT NULL DEFAULT 256,
    max_future_seconds bigint NOT NULL DEFAULT 900,
    max_past_seconds bigint NOT NULL DEFAULT 0,
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT relay_policy_singleton CHECK (singleton),
    CONSTRAINT relay_policy_content_bound CHECK (max_content_bytes > 0),
    CONSTRAINT relay_policy_tag_bound CHECK (max_tags >= 0),
    CONSTRAINT relay_policy_future_bound CHECK (max_future_seconds >= 0),
    CONSTRAINT relay_policy_past_bound CHECK (max_past_seconds >= 0)
);

INSERT INTO relay_policy (singleton) VALUES (TRUE);

CREATE TABLE relay_allowed_pubkey (
    pubkey text COLLATE "C" PRIMARY KEY,
    reason text NOT NULL DEFAULT '',
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT relay_allowed_pubkey_shape CHECK (
        pubkey ~ '^[0-9a-f]{64}$'
    )
);

CREATE TABLE relay_allowed_kind (
    kind integer PRIMARY KEY,
    reason text NOT NULL DEFAULT '',
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT relay_allowed_kind_range CHECK (kind BETWEEN 0 AND 65535)
);

CREATE TABLE relay_member_pubkey (
    pubkey text COLLATE "C" PRIMARY KEY,
    note text NOT NULL DEFAULT '',
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT relay_member_pubkey_shape CHECK (
        pubkey ~ '^[0-9a-f]{64}$'
    )
);

CREATE TABLE relay_blocked_pubkey (
    pubkey text COLLATE "C" PRIMARY KEY,
    reason text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT relay_blocked_pubkey_shape CHECK (
        pubkey ~ '^[0-9a-f]{64}$'
    )
);

CREATE TABLE relay_blocked_kind (
    kind integer PRIMARY KEY,
    reason text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT relay_blocked_kind_range CHECK (kind BETWEEN 0 AND 65535)
);
