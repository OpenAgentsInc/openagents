-- Room vitals for the admins' room view (#12791; docs/research/mmo.md section 15): each oa-room
-- shard's report every 10 s (POST /api/playtest/room-stats, signed with the seat-ticket key,
-- coax/web/room/room-stats.h), kept a day. A report is lines: "t MS", then "NAME=VALUE" for the
-- shard's numbers (its tick by workload, queues, bytes by type, join bursts, rejoins, refusals,
-- input to acknowledgement) and "seat|ROOM|SEAT|..." for each seat. GET /api/admin/rooms
-- (coax/web/api/admin.c) draws the last 10 minutes of them. Additive, and safe to run twice.
CREATE TABLE IF NOT EXISTS verse.room_stats (
  id bigserial PRIMARY KEY,
  shard int NOT NULL CHECK (shard BETWEEN 1 AND 99),
  at timestamptz NOT NULL DEFAULT now(),
  body text NOT NULL
);
CREATE INDEX IF NOT EXISTS room_stats_shard_at ON verse.room_stats (shard, at);

-- A report kept: its shard from its "shard=N" line, refused (NULL) when it isn't one or its t is not
-- within two minutes of now (a replayed report). Reports older than a day go.
CREATE OR REPLACE FUNCTION verse.room_stats_add(b text) RETURNS int LANGUAGE plpgsql AS $$
DECLARE
  t bigint := nullif(substring(b FROM '^t ([0-9]{1,15})\n'), '')::bigint;
  s int := nullif(substring(b FROM '\nshard=([0-9]{1,2})\n'), '')::int;
BEGIN
  IF t IS NULL OR s IS NULL OR s < 1 OR s > 99 OR abs(t - (extract(epoch FROM now()) * 1000)::bigint) > 120000 THEN RETURN NULL; END IF;
  INSERT INTO verse.room_stats (shard, body) VALUES (s, b);
  DELETE FROM verse.room_stats WHERE at < now() - interval '1 day';
  RETURN s;
END $$;
