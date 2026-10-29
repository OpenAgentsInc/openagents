-- /signin also signs in the browser's Verse player: when the phone's approval
-- reaches the page's poll, the page's player key (X-Verse-Player, in who.player)
-- moves to the approved pubkey's account by the rules of POST /api/playtest/signin
-- (verse.api 'signin', migration 0015), and the answer carries {key, id, account}
-- for the page to keep as localStorage verse.player. A page without a player
-- key gets none. The approved challenge is the signature; the browser holds
-- no Nostr key.
ALTER FUNCTION signin.api(text, jsonb, jsonb) RENAME TO api_0019;
CREATE FUNCTION signin.api(op text, body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pk text;
  out jsonb;
  v jsonb;
BEGIN
  IF op = 'poll' AND coalesce(who->>'player', '') <> '' THEN
    SELECT approved_pubkey INTO pk FROM signin.challenges
      WHERE poll_sha256 = projects.sha(coalesce(body->>'poll', '')) AND kind = 'signin' AND done_ms IS NULL
        AND approved_pubkey IS NOT NULL AND expires_ms > projects.now_ms();
  END IF;
  out := signin.api_0019(op, body, who);
  IF pk IS NULL OR out->'body'->>'state' IS DISTINCT FROM 'signed-in' THEN RETURN out; END IF;
  v := verse.api('POST', ARRAY['signin'], '{}'::jsonb,
                 jsonb_build_object('key', who->>'player', 'auto', '', 'npub', pk));
  IF (v->>'status')::int = 200 AND jsonb_typeof(v->'body'->'player') = 'object' THEN
    out := jsonb_set(out, '{body,player}', v->'body'->'player');
  END IF;
  RETURN out;
END $$;
