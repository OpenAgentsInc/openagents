-- Public commands extend the existing turn queue and lifecycle owner.
-- Older workers select only queued/running; they must skip public commands.
ALTER TABLE computers.turns DROP CONSTRAINT turns_state_check;
ALTER TABLE computers.turns ADD CONSTRAINT turns_state_check
    CHECK(state IN ('queued','running','command_queued','command_running','done','failed'));
CREATE TABLE computers.commands (
    turn text PRIMARY KEY REFERENCES computers.turns(id),
    owner text NOT NULL,
    computer text NOT NULL REFERENCES computers.computers(id),
    request_key text NOT NULL,
    digest text NOT NULL,
    options jsonb NOT NULL,
    status text NOT NULL CHECK(status IN ('queued','dispatching','running','succeeded','failed','cancelled','timed_out','unknown')),
    provider_resource text,
    provider_reference text,
    dispatched_at timestamptz,
    cancel_requested boolean NOT NULL DEFAULT false,
    stdout bytea NOT NULL DEFAULT '',
    stderr bytea NOT NULL DEFAULT '',
    stdout_truncated boolean NOT NULL DEFAULT false,
    stderr_truncated boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    expires_at timestamptz NOT NULL DEFAULT now() + interval '7 days',
    UNIQUE(owner, computer, request_key),
    CHECK(octet_length(stdout) <= 262144 AND octet_length(stderr) <= 262144)
);
CREATE INDEX commands_expiry ON computers.commands(expires_at);
