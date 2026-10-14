-- oa-computers (#11624): turns run on a computer. No secret is ever kept
-- here: a turn's credentials live only in the process's memory until the
-- one command that uses them starts; only their names are recorded.
CREATE TABLE computers.turns (
    id text PRIMARY KEY,
    computer text NOT NULL REFERENCES computers.computers (id),
    owner text NOT NULL,
    state text NOT NULL CHECK (state IN ('queued', 'running', 'done', 'failed')),
    generation bigint,
    command text NOT NULL,
    cwd text,
    timeout_seconds integer NOT NULL,
    env_names text[] NOT NULL DEFAULT '{}',
    exit_code bigint,
    stdout text,
    stderr text,
    error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    started_at timestamptz,
    finished_at timestamptz
);
CREATE INDEX turns_by_computer ON computers.turns (computer, created_at);
