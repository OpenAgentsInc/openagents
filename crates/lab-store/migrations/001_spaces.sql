-- One conditional index update publishes all changes in a space together.
-- Large version and artifact objects live in the private bucket.
CREATE TABLE IF NOT EXISTS lab_space_indexes (
    space TEXT PRIMARY KEY CHECK (space ~ '^[a-f0-9]{64}$'),
    generation BIGINT NOT NULL CHECK (generation > 0),
    state JSONB NOT NULL CHECK (jsonb_typeof(state) = 'object')
);
