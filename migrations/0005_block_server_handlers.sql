-- Server-side state for the pinned Block NIP lane.

CREATE TABLE block_command (
    event_id text COLLATE "C" PRIMARY KEY,
    pubkey text COLLATE "C" NOT NULL,
    kind integer NOT NULL,
    accepted_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT block_command_event_shape CHECK (event_id ~ '^[0-9a-f]{64}$'),
    CONSTRAINT block_command_pubkey_shape CHECK (pubkey ~ '^[0-9a-f]{64}$'),
    CONSTRAINT block_command_kind_range CHECK (kind BETWEEN 0 AND 65535)
);

CREATE TABLE workspace_profile (
    singleton boolean PRIMARY KEY DEFAULT TRUE,
    icon text,
    command_event_id text COLLATE "C",
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT workspace_profile_singleton CHECK (singleton),
    CONSTRAINT workspace_profile_icon_bound CHECK (
        icon IS NULL OR octet_length(icon) <= 98304
    )
);
INSERT INTO workspace_profile (singleton) VALUES (TRUE);

CREATE TABLE archived_identity (
    pubkey text COLLATE "C" PRIMARY KEY,
    reason text,
    replaced_by text COLLATE "C",
    consent text NOT NULL,
    actor_pubkey text COLLATE "C" NOT NULL,
    request_event_id text COLLATE "C" NOT NULL,
    archived_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT archived_identity_pubkey_shape CHECK (pubkey ~ '^[0-9a-f]{64}$'),
    CONSTRAINT archived_identity_replacement_shape CHECK (
        replaced_by IS NULL OR replaced_by ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT archived_identity_actor_shape CHECK (actor_pubkey ~ '^[0-9a-f]{64}$'),
    CONSTRAINT archived_identity_request_shape CHECK (request_event_id ~ '^[0-9a-f]{64}$'),
    CONSTRAINT archived_identity_consent CHECK (consent IN ('self', 'owner', 'admin', 'relay'))
);

CREATE TABLE dm_hidden (
    viewer_pubkey text COLLATE "C" NOT NULL,
    group_id text COLLATE "C" NOT NULL,
    hidden_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (viewer_pubkey, group_id),
    CONSTRAINT dm_hidden_viewer_shape CHECK (viewer_pubkey ~ '^[0-9a-f]{64}$'),
    CONSTRAINT dm_hidden_group_bound CHECK (octet_length(group_id) BETWEEN 1 AND 128)
);

-- Every encrypted or access-gated Block kind stays out of FTS. Persona/team
-- heads are also excluded because an unshared head is author-only and a
-- generated expression cannot safely encode the authenticated reader.
DROP INDEX nostr_event_search_idx;
ALTER TABLE nostr_event DROP COLUMN search_vector;
ALTER TABLE nostr_event ADD COLUMN search_vector tsvector GENERATED ALWAYS AS (
    CASE
        WHEN kind IN (30078, 30174, 30175, 30178, 30300, 30350, 30622, 44200)
        THEN NULL::tsvector
        ELSE to_tsvector('simple'::regconfig, content)
    END
) STORED;
CREATE INDEX nostr_event_search_idx
    ON nostr_event USING gin (search_vector);
