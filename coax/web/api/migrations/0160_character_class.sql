-- A character's class, kept once (#13259; aethyr/session/seat-class.coax). Everglade II's class is the
-- server's: the API signs it into the seat ticket's EXT (c=CLASS, with l=LEVEL from the stored XP), and
-- the room seats a ticketed player as the ticket says. A character with none (before its first seat
-- after Character Create) sits as its first hello's class, and the room reports that class on its
-- signed vitals line (" c=CLASS", player.c room_vitals_receipt). It is kept only while none is: a
-- later report, from any room, changes nothing. Kept by who the player is, as room_vitals is (0114:
-- the account, else the player), and it follows a guest into their account (vitals_follow, 0119).
-- Additive, and safe to run twice.
ALTER TABLE verse.room_vitals ADD COLUMN IF NOT EXISTS character_class text
  CHECK (character_class IS NULL OR character_class ~ '^srd-[a-z]{3,12}$');

-- Keep p_class as player p_pid's character's class unless it has one: the class kept (NULL: no such
-- player, or a class that is not one).
CREATE OR REPLACE FUNCTION verse.room_class_once(p_pid text, p_class text) RETURNS text
LANGUAGE plpgsql AS $$
DECLARE
  w text;
  kept text;
BEGIN
  IF p_class IS NULL OR p_class !~ '^srd-[a-z]{3,12}$' THEN RETURN NULL; END IF;
  SELECT coalesce(account, id) INTO w FROM verse.players WHERE id = p_pid;
  IF w IS NULL THEN RETURN NULL; END IF;
  INSERT INTO verse.room_vitals AS x (who, character_class, at) VALUES (w, p_class, verse.now_ms())
  ON CONFLICT (who) DO UPDATE SET character_class = coalesce(x.character_class, EXCLUDED.character_class)
  RETURNING x.character_class INTO kept;
  RETURN kept;
END $$;

-- 0119's vitals_follow, carrying the kept class too (the account's own, when it has one).
CREATE OR REPLACE FUNCTION verse.vitals_follow() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  was text := coalesce(OLD.account, OLD.id);
  now_ text := coalesce(NEW.account, NEW.id);
  v verse.room_vitals;
BEGIN
  IF was = now_ THEN RETURN NEW; END IF;
  SELECT * INTO v FROM verse.room_vitals WHERE who = was;
  IF v.who IS NULL THEN RETURN NEW; END IF;
  INSERT INTO verse.room_vitals AS x (who, hp, hp_max, hp_at, wanted, wanted_at, look, class, at, xp, character_class)
  VALUES (now_, v.hp, v.hp_max, v.hp_at, v.wanted, v.wanted_at, v.look, v.class, v.at, v.xp, v.character_class)
  ON CONFLICT (who) DO UPDATE SET
    xp = greatest(x.xp, EXCLUDED.xp),
    hp = CASE WHEN x.hp_at IS NULL OR x.hp_at < EXCLUDED.hp_at THEN EXCLUDED.hp ELSE x.hp END,
    hp_max = CASE WHEN x.hp_at IS NULL OR x.hp_at < EXCLUDED.hp_at THEN EXCLUDED.hp_max ELSE x.hp_max END,
    hp_at = greatest(x.hp_at, EXCLUDED.hp_at),
    look = coalesce(x.look, EXCLUDED.look), class = coalesce(x.class, EXCLUDED.class),
    character_class = coalesce(x.character_class, EXCLUDED.character_class),
    at = greatest(x.at, EXCLUDED.at);
  RETURN NEW;
END $$;
