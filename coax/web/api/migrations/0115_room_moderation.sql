-- Everglade rooms' moderation (#12714, epic #12718; spec sections 4.3, 6.5 and 6.8):
--   - pvp: a new row on Everglade's board ("Players can hurt each other"), off until its owner or an
--     admin switches it on. oa-room reads it (GET /api/playtest/room-control) and its rules let a
--     harmful spell or blow reach another player only while it is on.
--   - verse.name_ok: the names everyone sees (a ticket's, coax/web/api/player.c room_ticket):
--     3 to 20 letters, digits, spaces, '-' and '''', no links, nothing on the short block list.
--   - verse.room_kicks: an admin's kick (POST /api/playtest/room-kick) keeps a player out for 24
--     hours: no ticket is signed for them, and oa-room closes their socket within seconds.
--   - a report of a player in a room (POST /api/villa/report {kind: "player", ref: their id's 16 hex})
--     goes into the review queue at /admin/reports; HIDE there kicks them for 24 hours, RESTORE
--     lets them back.
-- Additive.

UPDATE verse.realm_rights SET rows = rows || '[{"id": "pvp", "label": "Players can hurt each other", "says": "spells and blows hurt other players too; off, they pass through players and only heals reach them"}]'::jsonb
WHERE slug = 'everglade' AND NOT rows @> '[{"id": "pvp"}]';
INSERT INTO verse.realm_edit (slug, switched, ms) VALUES ('everglade', '{"pvp": false}'::jsonb, verse.now_ms())
ON CONFLICT (slug) DO UPDATE SET switched = verse.realm_edit.switched || '{"pvp": false}'::jsonb
WHERE NOT verse.realm_edit.switched ? 'pvp';

CREATE OR REPLACE FUNCTION verse.name_ok(n text) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT n IS NOT NULL AND length(n) BETWEEN 3 AND 20 AND n ~ '^[A-Za-z0-9][A-Za-z0-9 ''-]*$' AND n !~ '  '
    AND lower(n) !~ '(https?|www|\.com|\.net|\.org|discord|onlyfans)'
    AND NOT EXISTS (SELECT 1 FROM unnest(ARRAY['fuck', 'shit', 'cunt', 'nigg', 'fagg', 'retard', 'whore', 'slut', 'bitch', 'nazi', 'hitler', 'porn']) w
                    WHERE regexp_replace(lower(n), '[^a-z]', '', 'g') LIKE '%' || w || '%') $$;

CREATE TABLE IF NOT EXISTS verse.room_kicks (
  player text PRIMARY KEY REFERENCES verse.players (id),
  by_who text NOT NULL,
  until_ms bigint NOT NULL,
  at_ms bigint NOT NULL
);

-- Reports of a player (0031, 0033, 0034 keep the others): what it shows, HIDE (a 24-hour kick), RESTORE.
ALTER TABLE villa.reports DROP CONSTRAINT reports_kind_check;
ALTER TABLE villa.reports ADD CONSTRAINT reports_kind_check CHECK (kind IN ('memorial', 'note', 'chat', 'file', 'player'));
CREATE OR REPLACE FUNCTION villa.report_kinds() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$ SELECT ARRAY['memorial', 'note', 'chat', 'file', 'player'] $$;
ALTER FUNCTION villa.report_shown(text, text) RENAME TO report_shown_0034;
ALTER FUNCTION villa.report_hide(text, text) RENAME TO report_hide_0034;
ALTER FUNCTION villa.report_restore(text, text) RENAME TO report_restore_0034;
CREATE FUNCTION villa.report_shown(kind_ text, ref_ text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN kind_ = 'player' THEN coalesce((
      SELECT 'An Everglade player, p-' || ref_ || coalesce(' (' || nullif(a.name, '') || ')', '')
      FROM verse.players p LEFT JOIN verse.accounts a ON a.id = p.account WHERE ref_ ~ '^[0-9a-f]{16}$' AND p.id = 'p-' || ref_), '')
    ELSE villa.report_shown_0034(kind_, ref_) END $$;
CREATE FUNCTION villa.report_hide(kind_ text, ref_ text) RETURNS bool LANGUAGE plpgsql AS $$
BEGIN
  IF kind_ <> 'player' THEN RETURN villa.report_hide_0034(kind_, ref_); END IF;
  IF ref_ !~ '^[0-9a-f]{16}$' OR NOT EXISTS (SELECT 1 FROM verse.players WHERE id = 'p-' || ref_) THEN RETURN false; END IF;
  INSERT INTO verse.room_kicks (player, by_who, until_ms, at_ms) VALUES ('p-' || ref_, 'report', verse.now_ms() + 86400000, verse.now_ms())
  ON CONFLICT (player) DO UPDATE SET by_who = 'report', until_ms = EXCLUDED.until_ms, at_ms = EXCLUDED.at_ms;
  RETURN true;
END $$;
CREATE FUNCTION villa.report_restore(kind_ text, ref_ text) RETURNS bool LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  IF kind_ <> 'player' THEN RETURN villa.report_restore_0034(kind_, ref_); END IF;
  DELETE FROM verse.room_kicks WHERE ref_ ~ '^[0-9a-f]{16}$' AND player = 'p-' || ref_;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n > 0;
END $$;

-- A guest's name (spec 4.1; the owner's default): two words from fixed, reviewed lists, the same for
-- the same player every time (from its id), so a guest's nameplate is never blank.
CREATE OR REPLACE FUNCTION verse.guest_name(pid text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT (ARRAY['Mossy', 'Amber', 'Quiet', 'Swift', 'Misty', 'Golden', 'Silver', 'Rusty', 'Brave', 'Gentle', 'Hollow', 'Bright',
                'Dusky', 'Fern', 'Willow', 'Cedar', 'Copper', 'Frosty', 'Sunny', 'Stormy', 'Mellow', 'Nimble', 'Wild', 'Lucky',
                'Sleepy', 'Clever', 'Dapper', 'Rowan', 'Hazel', 'Briar', 'Pebble', 'River'])[1 + (('x' || substr(md5(pid), 1, 2))::bit(8)::int % 32)]
      || ' ' ||
         (ARRAY['Heron', 'Otter', 'Badger', 'Wren', 'Finch', 'Fox', 'Hare', 'Owl', 'Robin', 'Marten', 'Beaver', 'Lark',
                'Newt', 'Stoat', 'Vole', 'Crane', 'Egret', 'Kestrel', 'Plover', 'Turtle', 'Salmon', 'Toad', 'Moth', 'Sparrow',
                'Thrush', 'Swallow', 'Magpie', 'Starling', 'Dormouse', 'Hedgehog', 'Kingfisher', 'Mallard'])[1 + (('x' || substr(md5(pid), 3, 2))::bit(8)::int % 32)] $$;
