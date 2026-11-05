-- Admission to the Everglade rooms when they are full (#12796; epic #12718; docs/research/mmo.md
-- sections 4 and 14). POST /api/playtest/room-ticket (player.c) limits asks per player and per address
-- in each API process and answers 429 with a retry time; these keep what must be shared:
--   - room_waiting: the players whose ask found every room full, until they are seated or stop
--     asking for 60 s (the full answer says how many wait, and how long the last one waited);
--   - room_wait_last: the last wait that ended in a seat;
--   - room_admission: refusals counted by reason (full, rate, kicked) and the asks seated after a wait.
-- Times are wall-clock milliseconds. Additive, and safe to run twice.
CREATE TABLE IF NOT EXISTS verse.room_waiting (
  pid text PRIMARY KEY,
  since_ms bigint NOT NULL,
  last_ms bigint NOT NULL
);
CREATE TABLE IF NOT EXISTS verse.room_wait_last (
  one int PRIMARY KEY DEFAULT 1 CHECK (one = 1),
  wait_ms bigint NOT NULL,
  at bigint NOT NULL
);
CREATE TABLE IF NOT EXISTS verse.room_admission (
  reason text PRIMARY KEY,
  n bigint NOT NULL DEFAULT 0
);

CREATE OR REPLACE FUNCTION verse.room_admit_count(reason text, n bigint) RETURNS void LANGUAGE sql AS $$
  INSERT INTO verse.room_admission AS a (reason, n) SELECT reason, n WHERE n > 0
  ON CONFLICT (reason) DO UPDATE SET n = a.n + EXCLUDED.n $$;

-- WHO's ask found every room full: they wait. How many wait now (asked within 60 s), and the last
-- wait that ended in a seat (ms, NULL none in the last hour). RATE_N: the process's 429s since it last
-- wrote, counted as 'rate' with this 'full'.
CREATE OR REPLACE FUNCTION verse.room_wait(who text, rate_n bigint, OUT waiting int, OUT last_wait_ms bigint)
LANGUAGE plpgsql AS $$
DECLARE now bigint := verse.room_now();
BEGIN
  DELETE FROM verse.room_waiting w WHERE w.last_ms < now - 60000;
  INSERT INTO verse.room_waiting AS w (pid, since_ms, last_ms) VALUES (who, now, now)
  ON CONFLICT (pid) DO UPDATE SET last_ms = EXCLUDED.last_ms;
  SELECT count(*)::int INTO waiting FROM verse.room_waiting;
  SELECT l.wait_ms INTO last_wait_ms FROM verse.room_wait_last l WHERE l.at > now - 3600000;
  PERFORM verse.room_admit_count('full', 1);
  PERFORM verse.room_admit_count('rate', rate_n);
END $$;

-- WHO was given a seat: a wait of theirs ends, and is the last wait. RATE_N as room_wait's.
CREATE OR REPLACE FUNCTION verse.room_seated(who text, rate_n bigint) RETURNS void LANGUAGE plpgsql AS $$
DECLARE now bigint := verse.room_now(); since bigint;
BEGIN
  DELETE FROM verse.room_waiting w WHERE w.pid = who RETURNING w.since_ms INTO since;
  IF since IS NOT NULL THEN
    INSERT INTO verse.room_wait_last (one, wait_ms, at) VALUES (1, now - since, now)
    ON CONFLICT (one) DO UPDATE SET wait_ms = EXCLUDED.wait_ms, at = EXCLUDED.at;
    PERFORM verse.room_admit_count('seated after a wait', 1);
  END IF;
  PERFORM verse.room_admit_count('rate', rate_n);
END $$;
