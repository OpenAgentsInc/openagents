-- Joining a party member's room (#12911): a page's seat ticket for the realm can name one member of
-- the asker's party (?party=PID). The ticket then goes to the live room that member sits in, while it
-- has a seat; when it is full, "party_full" names it and the page is placed as usual. The roster
-- (verse.party_view, 0129) stays health only: no room is in it, and only a member of the same party,
-- neither blocking the other and not hidden, is followed.
-- Additive, and safe to run twice.

-- The live room in REALM that MEMBER, of ME's party, sits in, and whether it is full (is_full).
-- No row's code when MEMBER isn't in ME's party, is hidden, blocks or is blocked by ME, or sits in
-- no live room there.
CREATE OR REPLACE FUNCTION verse.party_member_room(realm text, me text, member text, seats int, OUT code text, OUT is_full boolean)
LANGUAGE sql STABLE AS $$
  WITH clk AS (SELECT verse.room_now() AS t),
  mate AS (SELECT m.pid FROM verse.party_members m
           WHERE m.pid = member AND m.pid <> me AND NOT m.hidden
             AND m.party = (SELECT party FROM verse.party_members WHERE pid = me)
             AND NOT verse.party_blocked(m.pid, me)),
  rooms AS (SELECT i.code, verse.room_taken(i, clk.t) AS taken FROM verse.room_instances i, clk
            WHERE i.realm = party_member_room.realm AND verse.room_live(i, clk.t) AND (SELECT pid FROM mate) = ANY (i.pids)
            ORDER BY i.code LIMIT 1)
  SELECT (SELECT r.code FROM rooms r), coalesce((SELECT r.taken >= seats FROM rooms r), false) $$;
