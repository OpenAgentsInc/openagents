-- oa-computers (#11624): desired and observed state for every computer.
-- Lives in its own schema of the account database.
CREATE TABLE computers.computers (
    id text PRIMARY KEY,
    -- The account that owns it (opaque; the web's account ID).
    owner text NOT NULL,
    -- The chat it serves, when it is a chat's own computer.
    chat text,
    idempotency_key text,
    driver text NOT NULL,
    -- The immutable image it starts from (an Environment build).
    image text,
    size text NOT NULL,
    -- Desired state, written by the API.
    desired text NOT NULL CHECK (desired IN ('running', 'stopped', 'deleted')),
    -- Observed state, written only by the reconciler through the lease:
    -- the coder_working_computer::Computer record and its phase.
    phase text NOT NULL,
    revision bigint NOT NULL,
    record jsonb NOT NULL,
    -- Deleted and settled: never claimed again.
    done boolean NOT NULL DEFAULT false,
    attempts integer NOT NULL DEFAULT 0,
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    last_error text,
    lease_holder text,
    lease_until timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX computers_owner_chat ON computers.computers (owner, chat)
    WHERE chat IS NOT NULL AND desired <> 'deleted';
CREATE UNIQUE INDEX computers_owner_key ON computers.computers (owner, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
CREATE INDEX computers_due ON computers.computers (next_attempt_at) WHERE NOT done;
CREATE INDEX computers_by_owner ON computers.computers (owner, created_at DESC);
