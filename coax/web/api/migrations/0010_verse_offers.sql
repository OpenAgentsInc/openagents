-- Offered changes to someone's world and the edit log (pt-change-offers; the
-- demo's /api/change/* and /api/log/<id>, aethyr-cpp tools/serve.py at
-- demo/kiki-2026-10-28d). Anyone the server knows (a player key or an
-- account) offers a change to a world; its owner (verse.owns, or the site
-- for a built-in world) accepts it, which publishes the world with the change
-- made, or declines it, which changes nothing. The edit log lists every
-- publish, reset and offer of a world, newest first. No settlement: the
-- price an offer names is kept as the demo kept it, and nothing is paid.
--
-- An offer carries the world with its change made (doc) when the page that
-- made it has one; an accept without a {"doc"} publishes that. The demo's
-- offers, carried over in verse.demo_files (0009), have only their delta:
-- verse.offers_from_demo() copies them here, keyed on the file and the
-- offer's id, so it runs again after every import and adds only new ones.
CREATE TABLE verse.offers (
  world text NOT NULL,
  id text NOT NULL,              -- 12 hex, as the demo's
  who text NOT NULL,
  summary text NOT NULL DEFAULT '',
  at double precision NOT NULL,  -- seconds, as the demo answers it
  amount int NOT NULL DEFAULT 0,
  state text NOT NULL CHECK (state IN ('waiting', 'accepted', 'declined')),
  delta jsonb NOT NULL DEFAULT '{}',
  doc jsonb,
  by_account text, by_player text,
  guest boolean NOT NULL DEFAULT true,
  decided_at double precision,
  decided_by text,
  version int,                   -- the version an accept published
  source text,
  import_key text UNIQUE,
  PRIMARY KEY (world, id)
);
CREATE INDEX offers_world ON verse.offers (world, at);

CREATE FUNCTION verse.offer_json(o verse.offers) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_strip_nulls(jsonb_build_object('id', o.id, 'who', o.who, 'summary', o.summary, 'at', o.at,
    'amount', o.amount, 'unit', 'sats', 'state', o.state, 'delta', o.delta, 'guest', o.guest,
    'decidedAt', o.decided_at, 'decidedBy', o.decided_by, 'version', o.version))
  || CASE WHEN o.by_account IS NOT NULL OR o.by_player IS NOT NULL
     THEN jsonb_build_object('by', jsonb_build_object('account', o.by_account, 'player', o.by_player)) ELSE '{}' END
$$;

CREATE FUNCTION verse.offers_from_demo() RETURNS int LANGUAGE sql AS $$
  WITH e AS (
    SELECT f.path, substring(f.path FROM '/log-([A-Za-z0-9_-]{1,64})\.json$') AS world, x.v
    FROM verse.demo_files f, jsonb_array_elements(CASE WHEN jsonb_typeof(f.doc) = 'array' THEN f.doc ELSE '[]' END) AS x(v)
    WHERE f.path ~ '/log-[A-Za-z0-9_-]{1,64}\.json$'
  ), n AS (
    INSERT INTO verse.offers (world, id, who, summary, at, amount, state, delta, by_account, by_player, guest,
                              decided_at, decided_by, version, source, import_key)
    SELECT world, v->>'id', left(coalesce(v->>'who', ''), 40), left(coalesce(v->>'summary', ''), 300), (v->>'at')::double precision,
           CASE WHEN jsonb_typeof(v->'amount') = 'number' THEN (v->>'amount')::numeric::int ELSE 0 END, v->>'state',
           CASE WHEN jsonb_typeof(v->'delta') = 'object' THEN v->'delta' ELSE '{}' END,
           NULL, v->'by'->>'player', coalesce((v->>'guest')::boolean, true),
           CASE WHEN jsonb_typeof(v->'decidedAt') = 'number' THEN (v->>'decidedAt')::double precision END,
           left(v->>'decidedBy', 40), CASE WHEN jsonb_typeof(v->'version') = 'number' THEN (v->>'version')::int END,
           path, path || '#' || (v->>'id')
    FROM e
    WHERE world IS NOT NULL AND v->>'id' ~ '^[A-Za-z0-9_-]{1,64}$' AND v->>'state' IN ('waiting', 'accepted', 'declined')
      AND jsonb_typeof(v->'at') = 'number'
    ORDER BY path, (v->>'at')::double precision
    ON CONFLICT DO NOTHING RETURNING 1)
  SELECT count(*)::int FROM n
$$;
SELECT verse.offers_from_demo();

-- The routes before this one, kept as they were; verse.api answers the new
-- ones and hands every other request to it.
ALTER FUNCTION verse.api(text, text[], jsonb, jsonb) RENAME TO api_0004;

CREATE FUNCTION verse.api(method text, parts text[], body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
#variable_conflict use_column
DECLARE
  k text := who->>'key';
  site boolean := coalesce((who->>'site')::boolean, false);
  npub text := nullif(who->>'npub', '');
  now_s double precision := extract(epoch FROM clock_timestamp());
  n int := coalesce(array_length(parts, 1), 0);
  world text := CASE WHEN n >= 2 THEN parts[2] END;
  rec verse.players;
  acct text;
  b jsonb := CASE WHEN jsonb_typeof(body) = 'object' THEN body ELSE '{}' END;
  given text;
  who_name text;
  o verse.offers;
  price numeric;
  ver int;
  d jsonb;
BEGIN
  IF NOT (parts[1] IN ('change', 'log') AND n >= 2) THEN
    RETURN verse.api_0004(method, parts, body, who);
  END IF;
  IF world !~ '^[A-Za-z0-9_-]{1,64}$' THEN RETURN verse.r(404, '{"error":"unknown"}'); END IF;

  IF parts[1] = 'log' AND n = 2 AND method = 'GET' THEN
    RETURN verse.r(200, coalesce((SELECT jsonb_agg(e ORDER BY at DESC, o2 DESC) FROM (
      SELECT * FROM (
        SELECT l.at, l.id AS o2, jsonb_build_object('version', l.version, 'who', l.who, 'summary', l.summary, 'at', l.at, 'amount', 0,
               'unit', 'sats', 'state', l.state) || CASE WHEN l.by_account IS NOT NULL OR l.by_player IS NOT NULL
               THEN jsonb_build_object('by', jsonb_build_object('account', l.by_account, 'player', l.by_player)) ELSE '{}' END AS e
        FROM verse.log l WHERE l.world = parts[2]
        UNION ALL
        SELECT x.at, 0, verse.offer_json(x) FROM verse.offers x WHERE x.world = parts[2]) u
      ORDER BY at DESC, o2 DESC LIMIT 200) q), '[]'));
  END IF;
  IF parts[1] = 'log' THEN RETURN verse.r(404, '{"error":"unknown"}'); END IF;

  -- who asks: the player key, and the account a Nostr key or the key gives
  IF k ~ '^[A-Za-z0-9_-]{20,128}$' THEN
    SELECT * INTO rec FROM verse.players WHERE key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
  END IF;
  IF npub IS NOT NULL THEN
    INSERT INTO verse.accounts (id, npub, created_ms) VALUES ('n-' || npub, npub, verse.now_ms()) ON CONFLICT DO NOTHING;
    acct := 'n-' || npub;
  ELSE
    acct := rec.account;
  END IF;
  given := CASE WHEN jsonb_typeof(b->'user') = 'object' THEN nullif(btrim(b->'user'->>'name'), '') END;

  IF method <> 'POST' THEN RETURN verse.r(405, '{"error":"POST a change"}'); END IF;

  IF n = 2 THEN
    IF rec.id IS NULL AND acct IS NULL THEN RETURN verse.r(403, '{"error":"log in to offer a change"}'); END IF;
    d := b->'delta';
    IF jsonb_typeof(d) IS DISTINCT FROM 'object' OR NOT EXISTS (
         SELECT 1 FROM unnest(ARRAY['added', 'changed', 'removed']) AS f(k)
         WHERE jsonb_typeof(d->f.k) = 'array' AND jsonb_array_length(d->f.k) > 0) THEN
      RETURN verse.r(400, '{"error":"there is no change to offer"}');
    END IF;
    IF b ? 'doc' AND jsonb_typeof(b->'doc') NOT IN ('object', 'null') THEN
      RETURN verse.r(400, '{"error":"the world with the change made must be a world record"}');
    END IF;
    IF b ? 'price' AND jsonb_typeof(b->'price') <> 'number' THEN
      RETURN verse.r(400, '{"error":"the price must be a whole number of sats"}');
    END IF;
    price := coalesce((b->>'price')::numeric, 0);
    IF price <> trunc(price) THEN RETURN verse.r(400, '{"error":"the price must be a whole number of sats"}'); END IF;
    IF price < 0 OR price > 1000000 THEN RETURN verse.r(400, '{"error":"the price must be between 0 and 1,000,000 sats"}'); END IF;
    who_name := left(coalesce((SELECT a.name FROM verse.accounts a WHERE a.id = acct), given, 'Guest'), 40);
    INSERT INTO verse.offers (world, id, who, summary, at, amount, state, delta, doc, by_account, by_player, guest)
    VALUES (world, verse.hex(12), who_name, left(coalesce(b->>'summary', ''), 300), now_s, price::int, 'waiting', d,
            CASE WHEN jsonb_typeof(b->'doc') = 'object' THEN b->'doc' END, acct, rec.id, acct IS NULL)
    RETURNING * INTO o;
    RETURN verse.r(200, jsonb_build_object('id', o.id, 'state', 'waiting'));
  END IF;

  IF n = 4 AND parts[3] ~ '^[A-Za-z0-9_-]{1,64}$' AND parts[4] IN ('accept', 'decline') THEN
    IF NOT (site OR verse.owns(rec, acct, world)) THEN
      RETURN verse.r(403, '{"error":"only the owner can accept or decline changes"}');
    END IF;
    PERFORM pg_advisory_xact_lock(7731004, hashtext(world));
    SELECT * INTO o FROM verse.offers x WHERE x.world = parts[2] AND x.id = parts[3] FOR UPDATE;
    IF o.id IS NULL THEN RETURN verse.r(404, '{"error":"no such change"}'); END IF;
    IF o.state <> 'waiting' THEN RETURN verse.r(409, jsonb_build_object('error', 'that change was already ' || o.state)); END IF;
    who_name := left(coalesce((SELECT a.name FROM verse.accounts a WHERE a.id = acct), CASE WHEN site THEN 'site owner' END, given, 'owner'), 40);
    IF parts[4] = 'decline' THEN
      UPDATE verse.offers x SET state = 'declined', decided_at = now_s, decided_by = who_name WHERE x.world = o.world AND x.id = o.id;
      RETURN verse.r(200, '{"state":"declined"}');
    END IF;
    d := CASE WHEN jsonb_typeof(b->'doc') = 'object' THEN b->'doc' ELSE o.doc END;
    IF d IS NULL THEN RETURN verse.r(400, '{"error":"accepting needs the world with the change made"}'); END IF;
    SELECT coalesce(max(version), 0) + 1 INTO ver FROM verse.published p WHERE p.world = o.world;
    INSERT INTO verse.published (world, doc, version, published_by, published_at) VALUES (o.world, d, ver, o.who, now_s)
    ON CONFLICT (world) DO UPDATE SET doc = EXCLUDED.doc, version = EXCLUDED.version, published_by = EXCLUDED.published_by,
      published_at = EXCLUDED.published_at, source = NULL;
    UPDATE verse.offers x SET state = 'accepted', decided_at = now_s, decided_by = who_name, version = ver, doc = d
    WHERE x.world = o.world AND x.id = o.id;
    RETURN verse.r(200, jsonb_build_object('state', 'accepted', 'version', ver));
  END IF;
  RETURN verse.r(404, '{"error":"unknown"}');
END $$;
