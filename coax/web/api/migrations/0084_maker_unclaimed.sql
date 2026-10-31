-- A realm nobody has claimed yet is a state, not a missing page: GET /api/playtest/makers/<realm>
-- answers 200 {"realm", "state": "unclaimed"} (a tip page shows it as such), and a claimed one
-- {"realm", "state": "claimed", "name", "pubkey"} as before. A bad slug is still a 404 (makers.c).
CREATE OR REPLACE FUNCTION makers.of(r text, pk text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT jsonb_build_object('status', 200, 'realm', m.realm, 'state', 'claimed', 'name', m.name, 'pubkey', m.pubkey)
                          || CASE WHEN pk = m.pubkey THEN jsonb_build_object('lightning', m.lightning, 'mine', true) ELSE '{}'::jsonb END
                   FROM makers.realms m WHERE m.realm = r),
                  jsonb_build_object('status', 200, 'realm', r, 'state', 'unclaimed'))
$$;
