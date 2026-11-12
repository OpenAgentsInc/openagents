-- The Character aggregate's store (#13290, AAA P4.1; docs/protocol/everglade-character-v1.md): one
-- record for each character, profile everglade-character-v1, kept by who the player is (the account,
-- else the player: coalesce(account, id), as room_vitals and the item ledger keep them). Its rules are
-- Coax's (everglade/model/character.coax, through coax/web/api/character-policy.coax): player.c's seat
-- ticket keeps the record and signs its class and level, and an accepted checkpoint raises its XP.
-- SQL stores the record and checks only its shape.
--
-- The fill: every character the server already knows (a room_vitals row or an accepted checkpoint)
-- gets its record from the facts kept today: the class kept once (0160), the greatest XP any of them
-- holds, level floor 1, and nothing else. Its bag and inventory version are the ledger's, filled at its
-- next seat ticket. A who that is not a record's identity is skipped (none is, today). Additive, and
-- safe to run twice: a record already kept is never changed here.
CREATE TABLE IF NOT EXISTS verse.characters (
  who text PRIMARY KEY,
  body jsonb NOT NULL,
  at_ms bigint NOT NULL CHECK (at_ms > 0),
  CONSTRAINT characters_who CHECK (who ~ '^[a-z0-9_-]{1,80}$'),
  CONSTRAINT characters_body CHECK (jsonb_typeof(body) = 'object' AND body->>'v' = '1' AND body->>'who' = who
    AND jsonb_typeof(body->'known') = 'array' AND jsonb_typeof(body->'bag') = 'array'
    AND jsonb_typeof(body->'worn') = 'array' AND jsonb_array_length(body->'worn') = 10
    AND jsonb_typeof(body->'quests') = 'array')
);

-- A checkpoint body's XP, when it is a whole number the record can hold; else 0.
CREATE OR REPLACE FUNCTION verse.character_xp_of(b jsonb) RETURNS bigint LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN b->>'xp' ~ '^[0-9]{1,7}$' THEN (b->>'xp')::bigint ELSE 0 END $$;

INSERT INTO verse.characters (who, body, at_ms)
SELECT s.who,
       jsonb_build_object('v', 1, 'who', s.who, 'class', coalesce(rv.character_class, ''),
         'xp', least(greatest(coalesce(rv.xp, 0), coalesce(c1.xp, 0), coalesce(c2.xp, 0)), 9999999),
         'floor', 1, 'known', '[]'::jsonb, 'bag', '[]'::jsonb, 'inv', 0,
         'worn', '[0,0,0,0,0,0,0,0,0,0]'::jsonb, 'coin', 0, 'quests', '[]'::jsonb),
       verse.now_ms()
FROM (SELECT who FROM verse.room_vitals
      UNION SELECT who FROM verse.character_checkpoints_v1
      UNION SELECT who FROM verse.character_checkpoints_v2) s
LEFT JOIN verse.room_vitals rv ON rv.who = s.who
LEFT JOIN (SELECT who, verse.character_xp_of(body) AS xp FROM verse.character_checkpoints_v1) c1 ON c1.who = s.who
LEFT JOIN (SELECT who, verse.character_xp_of(body) AS xp FROM verse.character_checkpoints_v2) c2 ON c2.who = s.who
WHERE s.who ~ '^[a-z0-9_-]{1,80}$'
ON CONFLICT (who) DO NOTHING;
