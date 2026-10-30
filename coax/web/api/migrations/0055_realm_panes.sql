-- The panes edit mode brings up one at a time (ff-panes; verse/fairy-forest/README.md "Making the
-- world: panes"; docs/verse/realm-edit.md): a character's lore, saved with the character, and a
-- quest's form (who gives it, what is asked, the steps, what you get), whose save makes the quest.
-- Any realm, made or hand-built; its owner and admins write (0050's verse.edit_role decides), and
-- everyone reads, so the characters know their lore and players are given the quests.
--
--   GET  /api/playtest/realms/<slug>/panes            text/plain, a line each:
--          lore WHO|TEXT
--          quest GIVER|ASK|STEP|STEP|STEP|REWARD
--   POST /api/playtest/realms/<slug>/lore {who, text}  -> 200 {who, saved}; "" text forgets it
--   POST /api/playtest/realms/<slug>/quest {giver, ask, steps, reward}  -> 200 {giver, saved}; "" ask
--        takes the quest away
-- A character's key is its realm's own short name (a-z, 0-9, -). One quest a giver. What is written
-- stays in these tables only: analytics never sees it.

CREATE TABLE verse.realm_lore (
  slug text NOT NULL,
  who text NOT NULL,
  text text NOT NULL,
  ms bigint NOT NULL,
  by_player text,
  PRIMARY KEY (slug, who)
);
CREATE TABLE verse.realm_quests (
  slug text NOT NULL,
  giver text NOT NULL,
  ask text NOT NULL,
  steps text[] NOT NULL DEFAULT '{}',
  reward text NOT NULL DEFAULT '',
  ms bigint NOT NULL,
  by_player text,
  PRIMARY KEY (slug, giver)
);

-- One field of a pane: a string, control bytes and the line's bar (|) made spaces, trimmed, capped.
CREATE FUNCTION verse.pane_text(v jsonb, cap int) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT left(btrim(regexp_replace(coalesce(CASE WHEN jsonb_typeof(v) = 'string' THEN v #>> '{}' END, ''), '[\x00-\x1f\x7f|]+', ' ', 'g')), cap)
$$;
CREATE FUNCTION verse.pane_key_ok(k text) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT k ~ '^[a-z0-9][a-z0-9-]{0,31}$'
$$;

-- What the panes hold, for anyone; NULL for no such realm.
CREATE FUNCTION verse.edit_panes(s text, k text, site_admin boolean) RETURNS text LANGUAGE plpgsql STABLE AS $$
BEGIN
  IF verse.edit_role(s, k, site_admin) IS NULL THEN RETURN NULL; END IF;
  RETURN coalesce((SELECT string_agg('lore ' || who || '|' || text, E'\n' ORDER BY who) || E'\n' FROM verse.realm_lore WHERE slug = s), '')
      || coalesce((SELECT string_agg('quest ' || giver || '|' || ask || '|' || coalesce(steps[1], '') || '|' || coalesce(steps[2], '') || '|'
                                     || coalesce(steps[3], '') || '|' || reward, E'\n' ORDER BY giver) || E'\n'
                   FROM verse.realm_quests WHERE slug = s), '');
END $$;

-- A character's lore: up to 400 characters; "" forgets it. 40 characters a realm.
CREATE FUNCTION verse.edit_lore(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  v_who text := verse.pane_text(body->'who', 40);
  t text := verse.pane_text(body->'text', 400);
BEGIN
  IF role IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF role NOT IN ('owner', 'admin') THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' OR NOT verse.pane_key_ok(v_who) THEN RETURN '{"status":400,"error":"say whose lore"}'; END IF;
  IF t = '' THEN
    DELETE FROM verse.realm_lore l WHERE l.slug = s AND l.who = v_who;
    RETURN jsonb_build_object('status', 200, 'who', v_who, 'saved', false);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM verse.realm_lore l WHERE l.slug = s AND l.who = v_who)
     AND (SELECT count(*) FROM verse.realm_lore l WHERE l.slug = s) >= 40 THEN
    RETURN '{"status":409,"error":"this realm has lore for 40 characters"}';
  END IF;
  INSERT INTO verse.realm_lore (slug, who, text, ms, by_player) VALUES (s, v_who, t, verse.now_ms(), (verse.builder_player(k)).id)
  ON CONFLICT (slug, who) DO UPDATE SET text = excluded.text, ms = excluded.ms, by_player = excluded.by_player;
  RETURN jsonb_build_object('status', 200, 'who', v_who, 'saved', true);
END $$;

-- A quest: its giver, what is asked (up to 160), three steps (in place, blank or not) and a reward
-- (100 each); ""
-- ask takes it away. One a giver, 40 a realm.
CREATE FUNCTION verse.edit_quest(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text := verse.edit_role(s, k, site_admin);
  v_giver text := verse.pane_text(body->'giver', 40);
  v_ask text := verse.pane_text(body->'ask', 160);
  v_reward text := verse.pane_text(body->'reward', 100);
  v_steps text[];
BEGIN
  IF role IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF role NOT IN ('owner', 'admin') THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' OR NOT verse.pane_key_ok(v_giver) THEN RETURN '{"status":400,"error":"say who gives it"}'; END IF;
  IF v_ask = '' THEN
    DELETE FROM verse.realm_quests q WHERE q.slug = s AND q.giver = v_giver;
    RETURN jsonb_build_object('status', 200, 'giver', v_giver, 'saved', false);
  END IF;
  v_steps := coalesce((SELECT array_agg(t ORDER BY i) FROM (
             SELECT verse.pane_text(x, 100) t, i FROM jsonb_array_elements(CASE WHEN jsonb_typeof(body->'steps') = 'array' THEN body->'steps' ELSE '[]' END)
             WITH ORDINALITY a(x, i) ORDER BY i LIMIT 3) z), '{}');
  IF NOT EXISTS (SELECT 1 FROM verse.realm_quests q WHERE q.slug = s AND q.giver = v_giver)
     AND (SELECT count(*) FROM verse.realm_quests q WHERE q.slug = s) >= 40 THEN
    RETURN '{"status":409,"error":"this realm has 40 quests"}';
  END IF;
  INSERT INTO verse.realm_quests (slug, giver, ask, steps, reward, ms, by_player)
  VALUES (s, v_giver, v_ask, v_steps, v_reward, verse.now_ms(), (verse.builder_player(k)).id)
  ON CONFLICT (slug, giver) DO UPDATE SET ask = excluded.ask, steps = excluded.steps, reward = excluded.reward, ms = excluded.ms, by_player = excluded.by_player;
  RETURN jsonb_build_object('status', 200, 'giver', v_giver, 'saved', true);
END $$;
