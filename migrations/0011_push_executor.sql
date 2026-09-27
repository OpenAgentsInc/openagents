-- NIP-PL executor state. Lease rows commit in the same transaction as the
-- accepted kind 30350 event; the endpoint itself stays in that encrypted
-- event and only its SHA-256 is stored here.
CREATE TABLE push_lease (
    origin text COLLATE "C" NOT NULL,
    author text COLLATE "C" NOT NULL,
    installation text COLLATE "C" NOT NULL,
    event_id text COLLATE "C" NOT NULL,
    created_at bigint NOT NULL,
    expires_at bigint NOT NULL,
    generation bigint NOT NULL,
    active boolean NOT NULL,
    app_profile text COLLATE "C",
    transport text COLLATE "C",
    endpoint_hash text COLLATE "C",
    subscriptions jsonb,
    endpoint_invalid_at bigint,
    retain_until bigint NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (origin, author, installation),
    CONSTRAINT push_lease_generation CHECK (generation >= 1),
    CONSTRAINT push_lease_active_shape CHECK (
        (active AND app_profile IS NOT NULL AND transport IS NOT NULL
            AND endpoint_hash ~ '^[0-9a-f]{64}$' AND subscriptions IS NOT NULL)
        OR (NOT active AND endpoint_hash IS NULL AND subscriptions IS NULL
            AND endpoint_invalid_at IS NULL)
    )
);
CREATE UNIQUE INDEX push_lease_active_endpoint
    ON push_lease (origin, author, app_profile, transport, endpoint_hash)
    WHERE active;
CREATE INDEX push_lease_active_origin ON push_lease (origin, expires_at) WHERE active;
CREATE INDEX push_lease_retention ON push_lease (retain_until) WHERE NOT active;

-- One durable wake job per (origin, app profile, transport, endpoint hash,
-- event id). A job is claimed by at most one worker at a time; an expired
-- claim returns it to the queue.
CREATE TABLE push_delivery_job (
    job_id text COLLATE "C" PRIMARY KEY,
    origin text COLLATE "C" NOT NULL,
    author text COLLATE "C" NOT NULL,
    installation text COLLATE "C" NOT NULL,
    generation bigint NOT NULL,
    app_profile text COLLATE "C" NOT NULL,
    transport text COLLATE "C" NOT NULL,
    endpoint_hash text COLLATE "C" NOT NULL,
    event_id text COLLATE "C" NOT NULL,
    state text COLLATE "C" NOT NULL,
    attempts integer NOT NULL DEFAULT 0,
    next_attempt_at bigint NOT NULL,
    claim_token text COLLATE "C",
    claimed_until bigint,
    last_error text COLLATE "C",
    created_at bigint NOT NULL,
    expires_at bigint NOT NULL,
    finished_at bigint,
    CONSTRAINT push_delivery_job_state CHECK (
        state IN ('pending', 'claimed', 'delivered', 'suppressed', 'dead')
    ),
    CONSTRAINT push_delivery_job_claim CHECK (
        (state = 'claimed') = (claim_token IS NOT NULL AND claimed_until IS NOT NULL)
    ),
    CONSTRAINT push_delivery_job_id_shape CHECK (
        job_id ~ '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'
    ),
    UNIQUE (origin, app_profile, transport, endpoint_hash, event_id)
);
CREATE INDEX push_delivery_job_ready
    ON push_delivery_job (origin, next_attempt_at)
    WHERE state IN ('pending', 'claimed');
CREATE INDEX push_delivery_job_lease
    ON push_delivery_job (origin, author, installation)
    WHERE state IN ('pending', 'claimed');
CREATE INDEX push_delivery_job_finished
    ON push_delivery_job (finished_at)
    WHERE finished_at IS NOT NULL;

-- The matcher's durable position in the ingest sequence, per origin.
CREATE TABLE push_match_cursor (
    origin text COLLATE "C" PRIMARY KEY,
    ingest_seq bigint NOT NULL
);
