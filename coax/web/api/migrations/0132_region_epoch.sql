-- The store's region epoch (#12853; the contract is aethyr/session/region.coax, the spec
-- docs/verse/region-recovery.md). One region is the primary at a time. When a replica in another
-- region is promoted, the arbiter gives it the next epoch (verse.region_promote), and owner
-- generations there start above anything the old region could have issued: every character's newest
-- generation is raised to at least EPOCH << 20, and a new character's first one is past that too. So a
-- seat ticket or a room's vitals line or item write from the old region carries a lower generation and
-- is refused at every fence (room_vital_apply's f, 0127; item_apply, 0130; the checkpoint receiver, 0131).
-- A store that was never promoted is at epoch 1, and its generations start at 1 as before.
-- Additive, and safe to run twice.
CREATE TABLE IF NOT EXISTS verse.region (
  one boolean PRIMARY KEY DEFAULT true CHECK (one),
  epoch int NOT NULL DEFAULT 1,
  at bigint NOT NULL DEFAULT 0          -- when this store took its epoch (wall ms; 0: at its start)
);
INSERT INTO verse.region (one) VALUES (true) ON CONFLICT DO NOTHING;

-- The store's epoch.
CREATE OR REPLACE FUNCTION verse.region_epoch() RETURNS int
LANGUAGE sql STABLE AS $$ SELECT coalesce((SELECT epoch FROM verse.region), 1) $$;

-- The lowest owner generation this store may issue next, less one: 0 at epoch 1.
CREATE OR REPLACE FUNCTION verse.region_gen_floor() RETURNS bigint
LANGUAGE sql STABLE AS $$ SELECT CASE WHEN verse.region_epoch() <= 1 THEN 0 ELSE verse.region_epoch()::bigint << 20 END $$;

-- The arbiter's promotion of this store to epoch p_epoch: refused (the epoch it holds) unless p_epoch
-- is newer, and below 2048 (so EPOCH << 20 stays a 31-bit generation). Raises every character's newest
-- owner generation to the new floor in the same transaction.
CREATE OR REPLACE FUNCTION verse.region_promote(p_epoch int) RETURNS int
LANGUAGE plpgsql AS $$
DECLARE
  was int;
BEGIN
  SELECT epoch INTO was FROM verse.region FOR UPDATE;
  IF p_epoch IS NULL OR p_epoch <= was OR p_epoch >= 2048 THEN RETURN was; END IF;
  UPDATE verse.region SET epoch = p_epoch, at = verse.now_ms();
  UPDATE verse.room_owners SET gen = p_epoch::bigint << 20, at = verse.now_ms() WHERE gen < p_epoch::bigint << 20;
  RETURN p_epoch;
END $$;

-- The next owner generation of player p_pid's character (0127), never below this store's floor.
CREATE OR REPLACE FUNCTION verse.room_owner_next(p_pid text) RETURNS bigint
LANGUAGE plpgsql AS $$
DECLARE
  who_ text;
  g bigint;
  floor_ bigint := verse.region_gen_floor();
BEGIN
  SELECT coalesce(account, id) INTO who_ FROM verse.players WHERE id = p_pid;
  IF who_ IS NULL THEN RETURN NULL; END IF;
  INSERT INTO verse.room_owners AS o (who, gen, at) VALUES (who_, floor_ + 1, verse.now_ms())
  ON CONFLICT (who) DO UPDATE SET gen = greatest(o.gen, floor_) + 1, at = EXCLUDED.at WHERE greatest(o.gen, floor_) < 2147483647
  RETURNING o.gen INTO g;
  RETURN g;
END $$;
