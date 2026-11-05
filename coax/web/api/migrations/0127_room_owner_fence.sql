-- Who may save a character's room state, fenced by an owner generation (#12834; the contract is
-- aethyr/session/control.coax). The API is the one directory: each room ticket it signs for a
-- player (POST /api/playtest/room-ticket, player.c) takes the next generation of that player's
-- character (who: the account, else the player), and the ticket carries it ("g" in its HP field,
-- coax/web/room/room-ticket.coax). The room sends it with each vitals line it reports for that
-- seat. verse.room_vital_apply refuses a line whose generation is below the character's newest
-- (receipt letter f): a room the player left for another room or device, or a paused room process
-- that resumes, can't overwrite what the newer owner saved. The newest ticket wins (a second tab
-- takes over); a client never names a generation, since the ticket is signed.
-- A line without a generation (a room from before this, or a ticket without one) is applied as before.
-- Generations only rise; one that would pass 2147483647 is refused, never wrapped, as in the contract.
-- Additive, and safe to run twice.
CREATE TABLE IF NOT EXISTS verse.room_owners (
  who text PRIMARY KEY,
  gen bigint NOT NULL,
  at bigint NOT NULL            -- when the newest generation was issued (wall ms)
);

-- The next owner generation of player p_pid's character (NULL: no such player, or none left).
CREATE OR REPLACE FUNCTION verse.room_owner_next(p_pid text) RETURNS bigint
LANGUAGE plpgsql AS $$
DECLARE
  who_ text;
  g bigint;
BEGIN
  SELECT coalesce(account, id) INTO who_ FROM verse.players WHERE id = p_pid;
  IF who_ IS NULL THEN RETURN NULL; END IF;
  INSERT INTO verse.room_owners AS o (who, gen, at) VALUES (who_, 1, verse.now_ms())
  ON CONFLICT (who) DO UPDATE SET gen = o.gen + 1, at = EXCLUDED.at WHERE o.gen < 2147483647
  RETURNING o.gen INTO g;
  RETURN g;
END $$;

-- room_vital_apply with the line's owner generation (p_owner; NULL: none). Below the character's
-- newest: f, and nothing is written; otherwise as the 9-argument form (0124).
CREATE OR REPLACE FUNCTION verse.room_vital_apply(p_shard int, p_gen bigint, p_pid text, p_hp int, p_max int, p_xp int,
                                                  p_wanted int, p_rev bigint, p_at bigint, p_owner bigint) RETURNS text
LANGUAGE plpgsql AS $$
DECLARE
  newest bigint;
BEGIN
  IF p_owner IS NOT NULL THEN
    SELECT o.gen INTO newest FROM verse.room_owners o JOIN verse.players p ON o.who = coalesce(p.account, p.id) WHERE p.id = p_pid;
    IF newest IS NOT NULL AND p_owner < newest THEN RETURN 'f'; END IF;
  END IF;
  RETURN verse.room_vital_apply(p_shard, p_gen, p_pid, p_hp, p_max, p_xp, p_wanted, p_rev, p_at);
END $$;
