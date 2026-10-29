-- /api/auth/me also names the session's key in hex (H|) and the member keys it
-- speaks for (M|, a linked phone's member), so every page knows the same
-- viewer: host.js sends these lines to each app (VIEWER_KIND).
ALTER FUNCTION signin.api(text, jsonb, jsonb) RENAME TO api_0024;
CREATE FUNCTION signin.api(op text, body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  out jsonb;
  pk text;
BEGIN
  out := signin.api_0024(op, body, who);
  IF op <> 'me' OR (out->>'status')::int IS DISTINCT FROM 200 THEN RETURN out; END IF;
  pk := signin.session(nullif(who->>'site', ''));
  RETURN jsonb_set(out, '{body}', to_jsonb((out->>'body') || 'H|' || pk || E'\n' || coalesce((
    SELECT string_agg(DISTINCT 'M|' || k.member, E'\n') FROM projects.member_keys k WHERE k.pubkey = pk), '') || E'\n'));
END $$;
