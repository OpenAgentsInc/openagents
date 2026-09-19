-- Immortal M6 protocol expansion. Applied atomically after 0001_store.sql.

-- NIP-09 tombstones outlive an expiring deletion event. Keep the signed
-- source ID in the tombstone after NIP-40 physically removes the event row.
ALTER TABLE deletion_tombstone
    DROP CONSTRAINT deletion_tombstone_deletion_event_id_fkey;

CREATE TABLE relay_group (
    id text COLLATE "C" PRIMARY KEY,
    name text NOT NULL DEFAULT '',
    about text NOT NULL DEFAULT '',
    picture text NOT NULL DEFAULT '',
    closed boolean NOT NULL DEFAULT FALSE,
    supported_kinds integer[],
    pins jsonb NOT NULL DEFAULT '[]'::jsonb,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT relay_group_id_bound CHECK (
        octet_length(id) BETWEEN 1 AND 128
    ),
    CONSTRAINT relay_group_supported_kind_range CHECK (
        supported_kinds IS NULL OR
        (0 <= ALL(supported_kinds) AND 65535 >= ALL(supported_kinds))
    ),
    CONSTRAINT relay_group_pins_array CHECK (jsonb_typeof(pins) = 'array')
);

CREATE TABLE relay_group_member (
    group_id text COLLATE "C" NOT NULL
        REFERENCES relay_group(id) ON DELETE CASCADE,
    pubkey text COLLATE "C" NOT NULL,
    roles text[] NOT NULL DEFAULT '{}',
    joined_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (group_id, pubkey),
    CONSTRAINT relay_group_member_pubkey_shape CHECK (
        pubkey ~ '^[0-9a-f]{64}$'
    )
);

CREATE INDEX relay_group_member_pubkey_idx
    ON relay_group_member (pubkey, group_id);

CREATE TABLE relay_group_invite (
    group_id text COLLATE "C" NOT NULL
        REFERENCES relay_group(id) ON DELETE CASCADE,
    code text COLLATE "C" NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (group_id, code),
    CONSTRAINT relay_group_invite_code_bound CHECK (
        octet_length(code) BETWEEN 1 AND 256
    )
);

CREATE TABLE management_request (
    event_id text COLLATE "C" PRIMARY KEY,
    pubkey text COLLATE "C" NOT NULL,
    accepted_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT management_request_event_shape CHECK (
        event_id ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT management_request_pubkey_shape CHECK (
        pubkey ~ '^[0-9a-f]{64}$'
    )
);
