-- KS-8.10/#8379: the D1 source tables `forum_trust_edges` and
-- `forum_actor_forum_trust` are write-dead and dropped from the Worker
-- database in apps/openagents.com migration 0300. Keep any already-applied
-- forum remainder Postgres twin schema aligned by dropping the corresponding
-- derived mirrors. `forum_score_snapshots` remains live.

DROP TABLE IF EXISTS forum_actor_forum_trust;
DROP TABLE IF EXISTS forum_trust_edges;
