-- Additive durable character profile 2. The receiver locks identity and owner
-- through synchronous COMMIT; v1 keeps its published contract and table.
CREATE TABLE IF NOT EXISTS verse.character_checkpoints_v2 (
  who text PRIMARY KEY,
  owner bigint NOT NULL CHECK (owner BETWEEN 1 AND 2147483647),
  process bigint NOT NULL CHECK (process BETWEEN 1 AND 9007199254740991),
  rev bigint NOT NULL CHECK (rev BETWEEN 1 AND 9007199254740991),
  body jsonb NOT NULL CHECK (jsonb_typeof(body)='object' AND body->>'v'='2'
    AND jsonb_typeof(body->'state')='array' AND jsonb_array_length(body->'state')=27),
  accepted_at bigint NOT NULL CHECK (accepted_at>0)
);
-- Sticky from ticket issuance, even before the first successful capture.
CREATE TABLE IF NOT EXISTS verse.character_profiles_v2(who text PRIMARY KEY);
INSERT INTO verse.character_profiles_v2 SELECT who FROM verse.character_checkpoints_v2 ON CONFLICT DO NOTHING;

-- Close the legacy check/write race with ticket issuance. Identity is locked
-- first, then the same owner row used by both checkpoint versions. Keep the
-- legacy implementation and receipt meanings intact behind this lock adapter.
DO $$ BEGIN
  IF to_regprocedure('verse.room_vital_apply_before_checkpoints(integer,bigint,text,integer,integer,integer,integer,bigint,bigint,bigint)') IS NULL THEN
    ALTER FUNCTION verse.room_vital_apply(int,bigint,text,int,int,int,int,bigint,bigint,bigint)
      RENAME TO room_vital_apply_before_checkpoints;
  END IF;
END $$;
CREATE OR REPLACE FUNCTION verse.room_vital_apply(p_shard int,p_gen bigint,p_pid text,p_hp int,p_max int,p_xp int,
  p_wanted int,p_rev bigint,p_at bigint,p_owner bigint) RETURNS text LANGUAGE plpgsql AS $$
DECLARE w text; g bigint;
BEGIN
  SELECT coalesce(account,id) INTO w FROM verse.players WHERE id=p_pid FOR SHARE;
  SELECT gen INTO g FROM verse.room_owners WHERE who=w FOR UPDATE;
  IF EXISTS (SELECT 1 FROM verse.character_profiles_v2 WHERE who=w)
     AND (p_owner IS NULL OR p_owner<>g) THEN RETURN 'f'; END IF;
  RETURN verse.room_vital_apply_before_checkpoints(p_shard,p_gen,p_pid,p_hp,p_max,p_xp,p_wanted,p_rev,p_at,p_owner);
END $$;

-- Account linking locks both owner directories in lexical order. A room's
-- identity SHARE lock makes linking wait for an in-progress checkpoint commit.
-- The later accepted complete body wins; XP remains the legacy merged maximum.
CREATE OR REPLACE FUNCTION verse.checkpoints_follow() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  was text:=coalesce(OLD.account,OLD.id); now_ text:=coalesce(NEW.account,NEW.id);
  floor_ bigint; v verse.character_checkpoints_v2;
BEGIN
  IF was=now_ THEN RETURN NEW; END IF;
  IF NOT EXISTS(SELECT 1 FROM verse.character_profiles_v2 WHERE who IN(was,now_)) THEN RETURN NEW; END IF;
  INSERT INTO verse.character_profiles_v2 VALUES(now_) ON CONFLICT DO NOTHING;
  INSERT INTO verse.room_owners(who,gen,at) SELECT x,1,verse.now_ms() FROM unnest(ARRAY[was,now_]) x ORDER BY x ON CONFLICT(who) DO NOTHING;
  PERFORM 1 FROM verse.room_owners WHERE who IN(was,now_) ORDER BY who FOR UPDATE;
  SELECT max(gen)+1 INTO floor_ FROM verse.room_owners WHERE who IN(was,now_);
  IF floor_>2147483647 THEN RAISE EXCEPTION 'character owner exhausted'; END IF;
  UPDATE verse.room_owners SET gen=floor_,at=verse.now_ms() WHERE who IN(was,now_);
  SELECT * INTO v FROM verse.character_checkpoints_v2 WHERE who IN(was,now_) ORDER BY accepted_at DESC,who LIMIT 1;
  IF v.who IS NULL THEN RETURN NEW; END IF;
  INSERT INTO verse.character_checkpoints_v2(who,owner,process,rev,body,accepted_at)
  VALUES(now_,v.owner,v.process,v.rev,v.body,v.accepted_at)
  ON CONFLICT(who) DO UPDATE SET owner=EXCLUDED.owner,process=EXCLUDED.process,rev=EXCLUDED.rev,body=EXCLUDED.body,accepted_at=EXCLUDED.accepted_at;
  RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS checkpoints_follow ON verse.players;
CREATE TRIGGER checkpoints_follow AFTER UPDATE OF account ON verse.players FOR EACH ROW EXECUTE FUNCTION verse.checkpoints_follow();

-- The unversioned report has no owner to authenticate. Serialize with first
-- activation and refuse it for every durable-profile character.
CREATE OR REPLACE FUNCTION verse.room_vital_legacy(p_pid text,p_hp int,p_max int,p_xp int,p_at bigint)
RETURNS int LANGUAGE plpgsql AS $$
DECLARE w text; kept int;
BEGIN
  SELECT coalesce(account,id) INTO w FROM verse.players WHERE id=p_pid FOR SHARE;
  IF w IS NULL THEN RETURN 0; END IF;
  PERFORM 1 FROM verse.room_owners WHERE who=w FOR UPDATE;
  IF EXISTS(SELECT 1 FROM verse.character_profiles_v2 WHERE who=w) THEN RETURN 0; END IF;
  INSERT INTO verse.room_vitals(who,hp,hp_max,hp_at,at,xp)
  VALUES(w,p_hp,p_max,p_at,p_at,nullif(greatest(p_xp,-1),-1))
  ON CONFLICT(who) DO UPDATE SET
    hp=CASE WHEN verse.room_vitals.hp_at IS NULL OR verse.room_vitals.hp_at<=EXCLUDED.hp_at THEN EXCLUDED.hp ELSE verse.room_vitals.hp END,
    hp_max=CASE WHEN verse.room_vitals.hp_at IS NULL OR verse.room_vitals.hp_at<=EXCLUDED.hp_at THEN EXCLUDED.hp_max ELSE verse.room_vitals.hp_max END,
    hp_at=greatest(verse.room_vitals.hp_at,EXCLUDED.hp_at),at=greatest(verse.room_vitals.at,EXCLUDED.at),xp=greatest(verse.room_vitals.xp,EXCLUDED.xp)
  WHERE verse.room_vitals.hp_at IS NULL OR verse.room_vitals.hp_at<=EXCLUDED.hp_at OR EXCLUDED.xp>coalesce(verse.room_vitals.xp,-1);
  GET DIAGNOSTICS kept=ROW_COUNT;RETURN kept;
END $$;
