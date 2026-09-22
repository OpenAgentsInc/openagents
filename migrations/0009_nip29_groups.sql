-- NIP-29 private, hidden, and subgroup columns. Existing groups stay
-- public-read and restricted-write, which is the policy they were admitted under.

ALTER TABLE relay_group
    ADD COLUMN banner text NOT NULL DEFAULT '',
    ADD COLUMN private boolean NOT NULL DEFAULT FALSE,
    ADD COLUMN hidden boolean NOT NULL DEFAULT FALSE,
    ADD COLUMN restricted boolean NOT NULL DEFAULT TRUE,
    ADD COLUMN parent text,
    ADD COLUMN children text[] NOT NULL DEFAULT '{}',
    ADD COLUMN livekit text NOT NULL DEFAULT '';

ALTER TABLE relay_group
    ADD CONSTRAINT relay_group_parent_bound CHECK (
        parent IS NULL OR octet_length(parent) BETWEEN 1 AND 128
    ),
    ADD CONSTRAINT relay_group_parent_not_self CHECK (parent IS DISTINCT FROM id);
