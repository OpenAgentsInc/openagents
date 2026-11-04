-- Room vitals count as saved only when the API accepts them (#12793), and WANTED is the room
-- server's, not the page's (#12794). Epic #12718; docs/verse/2026-11-04-aethyr-verse-mmo-gap-analysis.md G5.
--
-- oa-room posts "t MS\no SHARD GEN BATCH\nPID HP MAX XP WANTED REV AT\n..." (coax/web/room/room-vitals.coax).
-- Each line's operation ID is (SHARD, GEN, PID, REV): the shard, the room process's generation (its
-- start in wall ms), the player and the revision of that player's values in that process.
-- verse.room_vital_apply applies a line once and says what happened, one letter for the receipt:
--   a applied   d already applied (a retry of one kept here)   s stale (a newer revision is kept)
--   u no such player
-- Revisions are ordered within one process (SHARD, GEN). Across processes the merge stays as it was
-- (0114, 0118): the newer HP by AT wins, and XP only rises. A later owner fence (#12797) can refuse a
-- replaced process's lines here, by GEN, with a new letter (f).
--
-- WANTED: room_vitals.wanted_by names the room process that wrote it ("SHARD.GEN"). A ticket brings
-- WANTED back only when a room wrote it, so a level a page saved before this migration is never
-- brought back; nothing is deleted. POST /api/playtest/room-state no longer writes WANTED.
-- Additive, and safe to run twice. The table grows by one row a player a room process; it is small.
CREATE TABLE IF NOT EXISTS verse.room_vital_ops (
  shard int NOT NULL,
  gen bigint NOT NULL,
  pid text NOT NULL,
  rev bigint NOT NULL,          -- the newest revision applied
  at bigint NOT NULL,           -- when it was applied (wall ms, the server's)
  applied int NOT NULL DEFAULT 0,
  dups int NOT NULL DEFAULT 0,  -- lines that came again after they were applied (retries)
  PRIMARY KEY (shard, gen, pid)
);
ALTER TABLE verse.room_vitals ADD COLUMN IF NOT EXISTS wanted_by text;

CREATE OR REPLACE FUNCTION verse.room_vital_apply(p_shard int, p_gen bigint, p_pid text, p_hp int, p_max int, p_xp int,
                                                  p_wanted int, p_rev bigint, p_at bigint) RETURNS text
LANGUAGE plpgsql AS $$
DECLARE
  who_ text;
  kept bigint;
BEGIN
  SELECT coalesce(account, id) INTO who_ FROM verse.players WHERE id = p_pid;
  IF who_ IS NULL THEN RETURN 'u'; END IF;
  INSERT INTO verse.room_vital_ops AS o (shard, gen, pid, rev, at, applied)
  VALUES (p_shard, p_gen, p_pid, p_rev, verse.now_ms(), 1)
  ON CONFLICT (shard, gen, pid) DO UPDATE SET rev = EXCLUDED.rev, at = EXCLUDED.at, applied = o.applied + 1
    WHERE o.rev < EXCLUDED.rev
  RETURNING o.rev INTO kept;
  IF kept IS NULL THEN
    SELECT o.rev INTO kept FROM verse.room_vital_ops o WHERE o.shard = p_shard AND o.gen = p_gen AND o.pid = p_pid;
    IF kept = p_rev THEN
      UPDATE verse.room_vital_ops SET dups = dups + 1 WHERE shard = p_shard AND gen = p_gen AND pid = p_pid;
      RETURN 'd';
    END IF;
    RETURN 's';
  END IF;
  INSERT INTO verse.room_vitals AS rv (who, hp, hp_max, hp_at, at, xp, wanted, wanted_at, wanted_by)
  VALUES (who_, p_hp, p_max, p_at, p_at, p_xp, p_wanted, p_at, p_shard || '.' || p_gen)
  ON CONFLICT (who) DO UPDATE SET
    hp = CASE WHEN rv.hp_at IS NULL OR rv.hp_at <= EXCLUDED.hp_at THEN EXCLUDED.hp ELSE rv.hp END,
    hp_max = CASE WHEN rv.hp_at IS NULL OR rv.hp_at <= EXCLUDED.hp_at THEN EXCLUDED.hp_max ELSE rv.hp_max END,
    hp_at = greatest(rv.hp_at, EXCLUDED.hp_at),
    at = greatest(rv.at, EXCLUDED.at),
    xp = greatest(coalesce(rv.xp, 0), EXCLUDED.xp),
    wanted = CASE WHEN rv.wanted_by IS NULL OR rv.wanted_at IS NULL OR rv.wanted_at <= EXCLUDED.wanted_at THEN EXCLUDED.wanted ELSE rv.wanted END,
    wanted_by = CASE WHEN rv.wanted_by IS NULL OR rv.wanted_at IS NULL OR rv.wanted_at <= EXCLUDED.wanted_at THEN EXCLUDED.wanted_by ELSE rv.wanted_by END,
    wanted_at = CASE WHEN rv.wanted_by IS NULL OR rv.wanted_at IS NULL OR rv.wanted_at <= EXCLUDED.wanted_at THEN EXCLUDED.wanted_at ELSE rv.wanted_at END;
  RETURN 'a';
END $$;
