-- A player's progress saves by revision, not by the device's clock (#12719; Aethyr book chapter 81).
-- Every kept save of a slot is a new revision (`rev`; an empty slot is revision 0). A save names the
-- revision it was made from, and coax/web/api/progress-policy.coax (#authority) decides whether it
-- is kept: player.c reads the facts here under the slot's lock (verse.progress_facts), asks the
-- policy, and then keeps the save (verse.progress_keep) or refuses it with 409 and what is there
-- now (verse.progress_refuse). A kept save's time is the server's clock.
--   GET  /api/playtest/progress/<world>/<name>        -> 200 {rev, current, previous}
--   POST /api/playtest/progress/<world>/<name>        {bytes, rev, at?} -> 200 {rev, current, previous};
--        400 (not a good save, or rev not a whole number), 403, 404,
--        409 {error: "changed since", rev, current, previous}
--   POST /api/playtest/progress/<world>/<name>/erase  -> 200 {erased}; a new revision with no save
-- The transition rule (progress-policy.coax): a save that names no revision comes from a page built
-- before this, and is kept only while no page with revisions has saved the slot (`marked`).
-- Existing saves are kept as revision 1, unmarked; a time a fast clock put past now comes back to
-- now. Additive: no save's bytes change.
ALTER TABLE verse.progress ADD COLUMN IF NOT EXISTS rev bigint NOT NULL DEFAULT 0;
ALTER TABLE verse.progress ADD COLUMN IF NOT EXISTS marked boolean NOT NULL DEFAULT false;
UPDATE verse.progress SET rev = 1 WHERE rev = 0 AND cur IS NOT NULL;
UPDATE verse.progress SET cur_at = CASE WHEN cur_at > verse.now_ms() THEN verse.now_ms() ELSE cur_at END,
  prev_at = CASE WHEN prev_at > verse.now_ms() THEN verse.now_ms() ELSE prev_at END
WHERE cur_at > verse.now_ms() OR prev_at > verse.now_ms();

-- A change that names no revision of its own (a sign-in moving a guest's saves into an account,
-- 0015; an erase) makes a revision no device has seen: at least the next one, and at least the
-- server's clock in ms, so it never equals one a device read from the guest's slot. A marked slot
-- stays marked.
CREATE OR REPLACE FUNCTION verse.progress_count() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF TG_OP = 'INSERT' THEN
    IF NEW.rev = 0 THEN NEW.rev := verse.now_ms(); END IF;
  ELSE
    IF NEW.rev = OLD.rev AND (NEW.cur IS DISTINCT FROM OLD.cur OR NEW.prev IS DISTINCT FROM OLD.prev) THEN
      NEW.rev := greatest(OLD.rev + 1, verse.now_ms());
    END IF;
    NEW.marked := NEW.marked OR OLD.marked;
  END IF;
  RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS progress_count ON verse.progress;
CREATE TRIGGER progress_count BEFORE INSERT OR UPDATE ON verse.progress FOR EACH ROW EXECUTE FUNCTION verse.progress_count();

-- The account a request's player key or NIP-98 key is (as verse.api has it, 0004).
CREATE OR REPLACE FUNCTION verse.progress_account(who jsonb) RETURNS text LANGUAGE plpgsql AS $$
DECLARE
  k text := who->>'key';
  npub text := nullif(who->>'npub', '');
  rec verse.players;
  acct text;
BEGIN
  IF k ~ '^[A-Za-z0-9_-]{20,128}$' THEN
    SELECT * INTO rec FROM verse.players WHERE key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
  END IF;
  IF npub IS NULL THEN RETURN rec.account; END IF;
  INSERT INTO verse.accounts (id, npub, created_ms) VALUES ('n-' || npub, npub, verse.now_ms()) ON CONFLICT DO NOTHING;
  acct := 'n-' || npub;
  IF rec.id IS NOT NULL AND rec.account IS NULL THEN UPDATE verse.players SET account = acct WHERE id = rec.id; END IF;
  RETURN acct;
END $$;

-- A slot as the API shows it: its revision and its two saves.
CREATE OR REPLACE FUNCTION verse.progress_view(acct text, slot_ text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object(
    'rev', coalesce(p.rev, 0),
    'current', CASE WHEN p.cur IS NOT NULL THEN jsonb_build_object('bytes', verse.b64(p.cur), 'at', p.cur_at) END,
    'previous', CASE WHEN p.prev IS NOT NULL THEN jsonb_build_object('bytes', verse.b64(p.prev), 'at', p.prev_at) END)
  FROM (SELECT 1) one LEFT JOIN verse.progress p ON p.account = acct AND p.slot = slot_
$$;

-- The route's slot and account: {status: 0, acct, slot}, or a refusal {status, body}.
CREATE OR REPLACE FUNCTION verse.progress_route(parts text[], who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  safe text := '^[A-Za-z0-9_-]{1,64}$';
  acct text;
BEGIN
  IF parts[2] !~ safe OR parts[3] !~ safe THEN RETURN verse.r(404, '{"error":"unknown"}'); END IF;
  acct := verse.progress_account(who);
  IF acct IS NULL THEN RETURN verse.r(403, '{"error":"join with your email to keep your progress on the server"}'); END IF;
  RETURN jsonb_build_object('status', 0, 'acct', acct, 'slot', parts[2] || '/' || parts[3]);
END $$;

-- What progress-policy.coax decides a save on, with the slot locked until the request's transaction
-- ends: {status: 0, acct, slot, rev, marked, sent_rev}, or a refusal {status, body}. sent_rev is
-- the body's rev: -1 when it names none (an old page), -2 when it is not a whole number of at least 0.
CREATE OR REPLACE FUNCTION verse.progress_facts(parts text[], body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  b jsonb := CASE WHEN jsonb_typeof(body) = 'object' THEN body ELSE '{}' END;
  route jsonb := verse.progress_route(parts, who);
  p verse.progress;
  sent bigint;
BEGIN
  IF (route->>'status')::int <> 0 THEN RETURN route; END IF;
  IF NOT coalesce((who->>'good')::boolean, false) THEN RETURN verse.r(400, '{"error":"that is not a good progress save"}'); END IF;
  sent := CASE WHEN b->'rev' IS NULL OR jsonb_typeof(b->'rev') = 'null' THEN -1
               WHEN jsonb_typeof(b->'rev') = 'number' AND (b->>'rev')::numeric >= 0 AND (b->>'rev')::numeric < 9007199254740992
                    AND (b->>'rev')::numeric = trunc((b->>'rev')::numeric) THEN (b->>'rev')::numeric::bigint
               ELSE -2 END;
  PERFORM pg_advisory_xact_lock(hashtextextended('verse.progress ' || (route->>'acct') || ' ' || (route->>'slot'), 0));
  SELECT * INTO p FROM verse.progress WHERE account = route->>'acct' AND slot = route->>'slot' FOR UPDATE;
  RETURN route || jsonb_build_object('rev', coalesce(p.rev, 0), 'marked', coalesce(p.marked, false), 'sent_rev', sent);
END $$;

-- A save progress-policy.coax keeps: the current one becomes the previous, at revision rev_.
CREATE OR REPLACE FUNCTION verse.progress_keep(acct text, slot_ text, bytes text, rev_ bigint, marked_ boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
BEGIN
  INSERT INTO verse.progress AS x (account, slot, cur, cur_at, rev, marked) VALUES (acct, slot_, decode(bytes, 'base64'), verse.now_ms(), rev_, marked_)
  ON CONFLICT (account, slot) DO UPDATE SET prev = x.cur, prev_at = x.cur_at, cur = EXCLUDED.cur, cur_at = EXCLUDED.cur_at,
    rev = EXCLUDED.rev, marked = EXCLUDED.marked;
  RETURN verse.r(200, verse.progress_view(acct, slot_));
END $$;

-- A save it refuses: 409 with the revision and the saves there now; any other status with its words.
CREATE OR REPLACE FUNCTION verse.progress_refuse(acct text, slot_ text, status int, words text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN status = 409 THEN verse.r(409, jsonb_build_object('error', words) || verse.progress_view(acct, slot_))
              ELSE verse.r(status, jsonb_build_object('error', words)) END
$$;

ALTER FUNCTION verse.api(text, text[], jsonb, jsonb) RENAME TO api_0057;

-- The progress routes, which 0004 had: reading and erasing here, saving through player.c and
-- progress-policy.coax. A good save that reaches here comes from an API binary built before 0119
-- (a rollback): it keeps 0004's rule, and the trigger above still makes it a new revision.
CREATE FUNCTION verse.api(method text, parts text[], body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  n int := coalesce(array_length(parts, 1), 0);
  route jsonb;
BEGIN
  IF parts[1] IS DISTINCT FROM 'progress' OR n NOT IN (3, 4) THEN RETURN verse.api_0057(method, parts, body, who); END IF;
  IF n = 4 AND (parts[4] <> 'erase' OR method <> 'POST') THEN RETURN verse.r(404, '{"error":"unknown"}'); END IF;
  route := verse.progress_route(parts, who);
  IF (route->>'status')::int <> 0 THEN RETURN route; END IF;
  IF n = 4 THEN
    UPDATE verse.progress SET cur = NULL, cur_at = NULL, prev = NULL, prev_at = NULL
    WHERE account = route->>'acct' AND slot = route->>'slot';
    RETURN verse.r(200, '{"erased":true}');
  END IF;
  IF method = 'POST' THEN
    IF NOT coalesce((who->>'good')::boolean, false) THEN RETURN verse.r(400, '{"error":"that is not a good progress save"}'); END IF;
    RETURN verse.api_0057(method, parts, body, who);
  END IF;
  RETURN verse.r(200, verse.progress_view(route->>'acct', route->>'slot'));
END $$;

-- XP and the rest of a player's room state follow them into an account. verse.room_vitals is kept by
-- who a player is (0114: the account, else the player), and oa-room writes it (XP since 0118, which
-- only rises). When a guest joins with an email, or a sign-in moves a key to a pubkey's account, the
-- row under the old who is merged into the new one, XP the larger of the two, so nothing a room
-- server wrote is left behind under a name no ticket reads any more. No page writes XP or HP.
CREATE OR REPLACE FUNCTION verse.vitals_follow() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  was text := coalesce(OLD.account, OLD.id);
  now_ text := coalesce(NEW.account, NEW.id);
  v verse.room_vitals;
BEGIN
  IF was = now_ THEN RETURN NEW; END IF;
  SELECT * INTO v FROM verse.room_vitals WHERE who = was;
  IF v.who IS NULL THEN RETURN NEW; END IF;
  INSERT INTO verse.room_vitals AS x (who, hp, hp_max, hp_at, wanted, wanted_at, look, class, at, xp)
  VALUES (now_, v.hp, v.hp_max, v.hp_at, v.wanted, v.wanted_at, v.look, v.class, v.at, v.xp)
  ON CONFLICT (who) DO UPDATE SET
    xp = greatest(x.xp, EXCLUDED.xp),
    hp = CASE WHEN x.hp_at IS NULL OR x.hp_at < EXCLUDED.hp_at THEN EXCLUDED.hp ELSE x.hp END,
    hp_max = CASE WHEN x.hp_at IS NULL OR x.hp_at < EXCLUDED.hp_at THEN EXCLUDED.hp_max ELSE x.hp_max END,
    hp_at = greatest(x.hp_at, EXCLUDED.hp_at),
    look = coalesce(x.look, EXCLUDED.look), class = coalesce(x.class, EXCLUDED.class),
    at = greatest(x.at, EXCLUDED.at);
  RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS vitals_follow ON verse.players;
CREATE TRIGGER vitals_follow AFTER UPDATE OF account ON verse.players FOR EACH ROW EXECUTE FUNCTION verse.vitals_follow();
