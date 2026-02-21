CREATE SCHEMA IF NOT EXISTS runtime;

CREATE TABLE IF NOT EXISTS runtime.khala_projection_checkpoints (
    projection_name TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    document_id TEXT NOT NULL,
    last_runtime_seq BIGINT NOT NULL DEFAULT 0,
    projection_version TEXT NOT NULL,
    summary_hash TEXT NOT NULL,
    last_projected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (projection_name, entity_id, document_id)
);

CREATE TABLE IF NOT EXISTS runtime.sync_topic_sequences (
    topic TEXT PRIMARY KEY,
    seq BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS runtime.sync_stream_events (
    id BIGSERIAL PRIMARY KEY,
    topic TEXT NOT NULL,
    seq BIGINT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::JSONB,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (topic, seq)
);

CREATE INDEX IF NOT EXISTS idx_sync_stream_events_topic_seq
    ON runtime.sync_stream_events (topic, seq);

CREATE TABLE IF NOT EXISTS runtime.sync_run_summaries (
    run_id TEXT PRIMARY KEY,
    latest_seq BIGINT NOT NULL DEFAULT 0,
    summary JSONB NOT NULL DEFAULT '{}'::JSONB,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS runtime.sync_codex_worker_summaries (
    worker_id TEXT PRIMARY KEY,
    latest_seq BIGINT NOT NULL DEFAULT 0,
    summary JSONB NOT NULL DEFAULT '{}'::JSONB,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
