-- One name for a player everywhere (#12714; spec 4.1): the seat ticket's nameplate, the room's chat and
-- presence, the inbox, Fairy's course times and the villa's notes all read verse.player_name:
--   - the account's name when it passes verse.name_ok (0115),
--   - else the name the guest chose (POST /api/playtest/player/name {name}, kept in verse.players.name
--     when it passes the same filter),
--   - else verse.guest_name: two words from the player id (0115), the one generator.
-- "Guest" and four letters of the id is gone.
ALTER TABLE verse.players ADD COLUMN IF NOT EXISTS name text;

CREATE OR REPLACE FUNCTION verse.player_name(pid text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN verse.name_ok(a.name) THEN a.name WHEN verse.name_ok(p.name) THEN p.name ELSE verse.guest_name(pid) END
  FROM (SELECT pid AS id) q LEFT JOIN verse.players p ON p.id = q.id LEFT JOIN verse.accounts a ON a.id = p.account
$$;
CREATE OR REPLACE FUNCTION villa.player_name(p verse.players) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT verse.player_name(p.id) $$;

-- POST /api/playtest/player/name {name}: a guest's own name (an account's name is the account's).
-- {name} back: the name everyone now sees; 400 when it doesn't pass the filter.
CREATE OR REPLACE FUNCTION verse.player_set_name(k text, body jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  me verse.players;
  want text := CASE WHEN jsonb_typeof(body->'name') = 'string' THEN btrim(body->>'name') END;
BEGIN
  SELECT * INTO me FROM verse.players WHERE k ~ '^[A-Za-z0-9_-]{20,128}$' AND key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
  IF me.id IS NULL THEN RETURN verse.r(403, '{"error":"unknown player key"}'); END IF;
  IF want IS NULL OR want = '' THEN
    UPDATE verse.players SET name = NULL WHERE id = me.id;   -- back to the generated name
  ELSIF NOT verse.name_ok(want) THEN
    RETURN verse.r(400, '{"error":"pick another name: 3 to 20 letters, digits, spaces, hyphens"}');
  ELSE
    UPDATE verse.players SET name = want WHERE id = me.id;
  END IF;
  RETURN verse.r(200, jsonb_build_object('name', verse.player_name(me.id), 'account', me.account IS NOT NULL));
END $$;
