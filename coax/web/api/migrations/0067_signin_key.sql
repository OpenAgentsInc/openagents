-- /api/auth/key: the words account's login (docs/web-log-in.md). private.c has
-- checked the NIP-98 event (kind 27235, u this route, method POST, payload the
-- body {"kind":"login"}, within 60 s, each event id once) and passes its pubkey
-- in who.pubkey. The answer is the poll's: a site session (oa_session, 30 days)
-- bound to that pubkey, and the page's Verse player (who.player) moved to it by
-- the rule of migration 0022 (verse.api 'signin').
ALTER FUNCTION signin.api(text, jsonb, jsonb) RENAME TO api_0026;
CREATE FUNCTION signin.api(op text, body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pk text;
  t text;
  out jsonb;
  v jsonb;
BEGIN
  IF op <> 'key' THEN RETURN signin.api_0026(op, body, who); END IF;
  pk := coalesce(who->>'pubkey', '');
  IF pk !~ '^[0-9a-f]{64}$' OR body->>'kind' IS DISTINCT FROM 'login' THEN RETURN projects.no(); END IF;
  t := projects.token();
  INSERT INTO signin.sessions VALUES (projects.sha(t), pk, projects.now_ms(), projects.now_ms() + 30 * 86400000::bigint);
  out := projects.r(200, jsonb_build_object('state', 'signed-in', 'npub', signin.bech32('npub', pk))) || jsonb_build_object('cookie', t);
  IF coalesce(who->>'player', '') <> '' THEN
    v := verse.api('POST', ARRAY['signin'], '{}'::jsonb, jsonb_build_object('key', who->>'player', 'auto', '', 'npub', pk));
    IF (v->>'status')::int = 200 AND jsonb_typeof(v->'body'->'player') = 'object' THEN
      out := jsonb_set(out, '{body,player}', v->'body'->'player');
    END IF;
  END IF;
  RETURN out;
END $$;
