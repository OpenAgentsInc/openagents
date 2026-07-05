-- #8379: remove the write-dead forum trust tables. The Wave 1 D1 sweep
-- found no production readers or writers; only sqlite test fixtures kept the
-- names alive.

DROP TABLE IF EXISTS forum_actor_forum_trust;
DROP TABLE IF EXISTS forum_trust_edges;
