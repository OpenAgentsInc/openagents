-- The shard that holds a character's newest owner generation (#13260, docs/deployment/room-keys.md).
-- room_owners (0127) is the one directory of who may save a character: each seat ticket takes the
-- next generation. It now also notes the shard that ticket seats (the directory's pick, rooms.c
-- room_pick; shard 1 for a ticket without one, as a v2 ticket is seated). An item or checkpoint
-- write signed with a shard's own write key (room-write-v1) for that generation or a newer one is
-- refused (403, player.c room_write_owner) unless that shard holds it. A write for an older
-- generation is fenced as before (stale, fenced): travel, a drain's re-seat and a failover take a
-- new ticket, so the old shard's late writes keep their fence and the new shard writes.
-- shard NULL (a generation issued before this, or by the one-argument form): not bound, as before.
-- region_promote (0132) and an account link (0135) raise generations without a ticket; the shard
-- stays the last ticket's until the player's next ticket.
-- Additive, and safe to run twice.
ALTER TABLE verse.room_owners ADD COLUMN IF NOT EXISTS shard int CHECK (shard >= 1);

-- room_owner_next (0132) for a ticket that shard p_shard seats: the generation, and the shard noted
-- with it in the same row (NULL: no such player, or none left; nothing is noted).
CREATE OR REPLACE FUNCTION verse.room_owner_next(p_pid text, p_shard int) RETURNS bigint
LANGUAGE plpgsql AS $$
DECLARE
  g bigint := verse.room_owner_next(p_pid);
BEGIN
  IF g IS NOT NULL THEN
    UPDATE verse.room_owners o SET shard = p_shard FROM verse.players p
     WHERE p.id = p_pid AND o.who = coalesce(p.account, p.id);
  END IF;
  RETURN g;
END $$;

-- Whether shard p_shard may write player p_pid's state at owner generation p_gen: false only when
-- the character's newest generation is noted with another shard and p_gen is not older than it.
CREATE OR REPLACE FUNCTION verse.room_owner_shard_ok(p_pid text, p_gen bigint, p_shard int) RETURNS boolean
LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT o.shard IS NULL OR o.shard = p_shard OR p_gen IS NULL OR p_gen < o.gen
                     FROM verse.room_owners o JOIN verse.players p ON o.who = coalesce(p.account, p.id)
                    WHERE p.id = p_pid), true)
$$;
