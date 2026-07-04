-- Khala Sync core substrate (Cloud SQL Postgres).
-- Spec: docs/khala-sync/SPEC.md §4. Issue: KS-2.
--
-- Version allocation happens INSIDE the mutator transaction under the
-- scope-counter row lock, so per-scope versions are dense, monotonic, and
-- commit-ordered by construction (no outbox sequence-gap trap).

CREATE TABLE IF NOT EXISTS khala_sync_scopes (
  scope                 text PRIMARY KEY,
  last_version          bigint NOT NULL DEFAULT 0,
  retained_from_version bigint NOT NULL DEFAULT 1,
  created_at            timestamptz NOT NULL DEFAULT now(),
  updated_at            timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT khala_sync_scopes_scope_shape
    CHECK (scope ~ '^scope\.[a-z_]+\.[A-Za-z0-9._:-]+$'),
  CONSTRAINT khala_sync_scopes_retention
    CHECK (retained_from_version >= 1
       AND retained_from_version <= last_version + 1)
);

CREATE TABLE IF NOT EXISTS khala_sync_changelog (
  scope           text        NOT NULL,
  version         bigint      NOT NULL,
  entity_type     text        NOT NULL,
  entity_id       text        NOT NULL,
  op              text        NOT NULL CHECK (op IN ('upsert', 'delete')),
  post_image_json jsonb,
  mutation_ref    text,
  committed_at    timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (scope, version, entity_type, entity_id),
  CONSTRAINT khala_sync_changelog_post_image_shape
    CHECK ((op = 'delete') = (post_image_json IS NULL))
);

-- Capture tails (scope, version) ranges; committed_at supports compaction.
CREATE INDEX IF NOT EXISTS khala_sync_changelog_committed_at_idx
  ON khala_sync_changelog (committed_at);

CREATE TABLE IF NOT EXISTS khala_sync_mutations (
  client_group_id text        NOT NULL,
  client_id       text        NOT NULL,
  mutation_id     bigint      NOT NULL CHECK (mutation_id >= 1),
  name            text        NOT NULL,
  status          text        NOT NULL
    CHECK (status IN ('applied', 'rejected', 'duplicate')),
  error_code      text,
  result_json     jsonb,
  scope           text,
  committed_at    timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (client_group_id, client_id, mutation_id)
);

CREATE TABLE IF NOT EXISTS khala_sync_client_state (
  client_group_id text        PRIMARY KEY,
  user_id         text        NOT NULL,
  schema_version  bigint      NOT NULL CHECK (schema_version >= 1),
  created_at      timestamptz NOT NULL DEFAULT now(),
  last_seen_at    timestamptz NOT NULL DEFAULT now()
);

-- Wake channel for the capture worker (LISTEN khala_sync_changelog_append).
-- NOTIFY is a wake signal only, never the data channel (SPEC §4).
CREATE OR REPLACE FUNCTION khala_sync_notify_append() RETURNS trigger AS $$
BEGIN
  PERFORM pg_notify('khala_sync_changelog_append', NEW.scope);
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS khala_sync_changelog_append_notify
  ON khala_sync_changelog;
CREATE TRIGGER khala_sync_changelog_append_notify
  AFTER INSERT ON khala_sync_changelog
  FOR EACH ROW EXECUTE FUNCTION khala_sync_notify_append();
