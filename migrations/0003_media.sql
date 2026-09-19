-- Immortal M7 media storage metadata. Blob bytes remain in the configured
-- filesystem backend; Postgres owns visibility, ownership, quotas, and
-- one-use NIP-98 authorization state.

CREATE TABLE media_blob (
    sha256 text COLLATE "C" PRIMARY KEY,
    storage_key text COLLATE "C" NOT NULL,
    size bigint NOT NULL,
    media_type text COLLATE "C" NOT NULL,
    uploaded_at bigint NOT NULL,
    ready boolean NOT NULL DEFAULT FALSE,
    CONSTRAINT media_blob_sha256_shape CHECK (
        sha256 ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT media_blob_storage_key_shape CHECK (
        storage_key ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT media_blob_size_range CHECK (
        size >= 0 AND size <= 1073741824
    ),
    CONSTRAINT media_blob_type_bound CHECK (
        octet_length(media_type) BETWEEN 1 AND 127 AND
        media_type ~ '^[a-z0-9!#$&^_.+-]+/[a-z0-9!#$&^_.+-]+$'
    ),
    CONSTRAINT media_blob_uploaded_at_range CHECK (uploaded_at >= 0)
);

CREATE TABLE media_owner (
    sha256 text COLLATE "C" NOT NULL
        REFERENCES media_blob(sha256) ON DELETE CASCADE,
    pubkey text COLLATE "C" NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (sha256, pubkey),
    CONSTRAINT media_owner_pubkey_shape CHECK (
        pubkey ~ '^[0-9a-f]{64}$'
    )
);

CREATE INDEX media_owner_pubkey_idx ON media_owner (pubkey, sha256);

CREATE TABLE media_auth_request (
    event_id text COLLATE "C" PRIMARY KEY,
    pubkey text COLLATE "C" NOT NULL,
    action text COLLATE "C" NOT NULL,
    accepted_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT media_auth_request_event_shape CHECK (
        event_id ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT media_auth_request_pubkey_shape CHECK (
        pubkey ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT media_auth_request_action CHECK (
        action IN ('upload', 'delete')
    )
);
