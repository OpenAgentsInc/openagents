-- Analytics events, as the playtest records them (aethyr-cpp tools/analytics.py,
-- docs/verse/playtest-port.md section 1), plus what the load balancer knows
-- about the visit. Append-only: no route updates or deletes an event.

-- One row per event. ms is the playtest's "t" (server time, ms); ct is the
-- page's own clock. space: 'web' for openagents.com's pages, 'kiki' and
-- 'dev' for events imported from the playtest's two namespaces.
CREATE TABLE analytics.events (
  id bigserial PRIMARY KEY,
  ms bigint NOT NULL,
  sid text NOT NULL,
  acct text,                 -- an account id (the playtest's), when a sign-in tied the session
  npub text,                 -- the Nostr pubkey (hex) of a NIP-98 signed request
  ev text NOT NULL,
  page text NOT NULL DEFAULT '',
  ct bigint,
  p jsonb NOT NULL DEFAULT '{}',
  auto text,                 -- the automated tool that sent it, as the playtest marks it
  space text NOT NULL DEFAULT 'web',
  host text,                 -- openagents.com or staging.openagents.com
  import_key text UNIQUE     -- a stable key for imported events, so an import runs twice harmlessly
);
CREATE INDEX events_sid ON analytics.events (sid, ms);
CREATE INDEX events_ms ON analytics.events (ms);
CREATE INDEX events_ev ON analytics.events (ev, ms);

-- One row per session: what the request said about the visitor, first and
-- last seen. The address, agent and geography are the load balancer's.
CREATE TABLE analytics.sessions (
  sid text PRIMARY KEY,
  first_ms bigint NOT NULL,
  last_ms bigint NOT NULL,
  space text NOT NULL DEFAULT 'web',
  host text,
  ip text,
  ua text,
  agent text,                -- a robot or headless User-Agent's short name
  region text, subdivision text, city text, latlong text,
  rtt_ms int,
  tls text,
  device text,
  npub text,
  events int NOT NULL DEFAULT 0
);
CREATE INDEX sessions_first ON analytics.sessions (first_ms);

-- The playtest's props rules (clean_props): at most 16 plain values with
-- short names; strings cut to 80; "text" kept (cut to 500) only when the
-- event says consent: true; floats rounded to 3 places.
CREATE FUNCTION analytics.clean_props(p jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  out jsonb := '{}';
  k text; v jsonb; n int := 0;
  consent boolean := jsonb_typeof(p) = 'object' AND p->'consent' = 'true'::jsonb;
BEGIN
  IF jsonb_typeof(p) IS DISTINCT FROM 'object' THEN RETURN out; END IF;
  FOR k, v IN SELECT key, value FROM jsonb_each(p) LOOP
    EXIT WHEN n >= 16;
    CONTINUE WHEN k !~ '^[A-Za-z][A-Za-z0-9_]{0,23}$';
    IF k = 'text' THEN
      IF consent AND jsonb_typeof(v) = 'string' THEN out := out || jsonb_build_object(k, left(v #>> '{}', 500)); n := n + 1; END IF;
      CONTINUE;
    END IF;
    CASE jsonb_typeof(v)
      WHEN 'boolean', 'null' THEN out := out || jsonb_build_object(k, v);
      WHEN 'number' THEN
        out := out || jsonb_build_object(k, CASE WHEN v::text ~ '[.eE]' THEN to_jsonb(round((v::text)::numeric, 3)) ELSE v END);
      WHEN 'string' THEN out := out || jsonb_build_object(k, left(v #>> '{}', 80));
      ELSE CONTINUE;
    END CASE;
    n := n + 1;
  END LOOP;
  RETURN out;
END $$;

-- The playtest's bot_agent(): a short name for a headless or robot
-- User-Agent, or NULL for a browser (the same pattern, case-insensitive).
CREATE FUNCTION analytics.bot_agent(ua text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT left(regexp_replace(lower(m[1]), '[^a-z0-9-]', '', 'g'), 32)
  FROM (SELECT regexp_match(coalesce(ua, ''),
    '(headlesschrome|headless|phantomjs|puppeteer|playwright|selenium|webdriver|lighthouse|'
    'facebookexternalhit|facebot|meta-externalagent|twitterbot|slackbot|discordbot|linkedinbot|'
    'telegrambot|googlebot|google-inspectiontool|adsbot|bingbot|applebot|yandexbot|'
    'baiduspider|duckduckbot|petalbot|bytespider|gptbot|claudebot|ccbot|crawler|spider|'
    'python-requests|python-urllib|curl/|wget/|go-http-client|okhttp|\ybot\y)', 'i') AS m) x
$$;

-- A page's batch, {"sid", "events": [{"ev", "page", "ct", "p"}], "auto"},
-- with what the server knows (ctx: ms, host, ip, ua, agent, auto, npub and the
-- load balancer's region, subdivision, city, latlong, rtt, tls, device).
-- Keeps what the playtest's clean_event keeps: at most 50 events a batch and
-- 5,000 a session; a robot agent noted on the session-start. Returns how
-- many were kept, or -1 when the batch is not one.
CREATE FUNCTION analytics.ingest(batch jsonb, ctx jsonb) RETURNS int LANGUAGE plpgsql AS $$
DECLARE
  s text := batch->>'sid';
  now_ms bigint := (ctx->>'ms')::bigint;
  auto text := coalesce(nullif(ctx->>'auto', ''), CASE WHEN batch->>'auto' ~ '^[a-z][a-z0-9-]{0,31}$' THEN batch->>'auto' END);
  agent text := coalesce(nullif(ctx->>'agent', ''), analytics.bot_agent(ctx->>'ua'));
  have int;
  kept int;
BEGIN
  IF jsonb_typeof(batch) IS DISTINCT FROM 'object' OR s IS NULL OR s !~ '^[A-Za-z0-9_:-]{8,80}$'
     OR jsonb_typeof(batch->'events') IS DISTINCT FROM 'array' THEN
    RETURN -1;
  END IF;
  SELECT events INTO have FROM analytics.sessions WHERE sid = s;
  have := coalesce(have, 0);
  WITH raw AS (
    SELECT e, ord FROM jsonb_array_elements(batch->'events') WITH ORDINALITY AS a(e, ord) WHERE ord <= 50
  ), clean AS (
    SELECT ord, e->>'ev' AS ev,
           CASE WHEN jsonb_typeof(e->'page') = 'string' THEN left(e->>'page', 32) ELSE '' END AS page,
           CASE WHEN jsonb_typeof(e->'ct') = 'number' THEN floor((e->>'ct')::numeric)::bigint END AS ct,
           analytics.clean_props(e->'p') AS p
    FROM raw WHERE jsonb_typeof(e) = 'object' AND e->>'ev' ~ '^[a-z][a-z0-9-]{0,31}$'
  ), capped AS (
    SELECT * FROM clean ORDER BY ord LIMIT greatest(0, 5000 - have)
  )
  INSERT INTO analytics.events (ms, sid, npub, ev, page, ct, p, auto, space, host)
  SELECT now_ms, s, nullif(ctx->>'npub', ''), ev, page, ct,
         CASE WHEN ev = 'session-start' AND agent IS NOT NULL THEN p || jsonb_build_object('agent', agent) ELSE p END,
         auto, 'web', nullif(ctx->>'host', '')
  FROM capped ORDER BY ord;
  GET DIAGNOSTICS kept = ROW_COUNT;
  INSERT INTO analytics.sessions AS x (sid, first_ms, last_ms, host, ip, ua, agent, region, subdivision, city, latlong, rtt_ms, tls, device, npub, events)
  VALUES (s, now_ms, now_ms, nullif(ctx->>'host', ''), nullif(ctx->>'ip', ''), left(nullif(ctx->>'ua', ''), 400), agent,
          nullif(ctx->>'region', ''), nullif(ctx->>'subdivision', ''), nullif(ctx->>'city', ''), nullif(ctx->>'latlong', ''),
          CASE WHEN ctx->>'rtt' ~ '^[0-9]{1,9}$' THEN (ctx->>'rtt')::int END, nullif(ctx->>'tls', ''), nullif(ctx->>'device', ''),
          nullif(ctx->>'npub', ''), kept)
  ON CONFLICT (sid) DO UPDATE SET last_ms = greatest(x.last_ms, EXCLUDED.last_ms), events = x.events + EXCLUDED.events,
    npub = coalesce(x.npub, EXCLUDED.npub), ip = coalesce(x.ip, EXCLUDED.ip), ua = coalesce(x.ua, EXCLUDED.ua),
    agent = coalesce(x.agent, EXCLUDED.agent), region = coalesce(x.region, EXCLUDED.region),
    subdivision = coalesce(x.subdivision, EXCLUDED.subdivision), city = coalesce(x.city, EXCLUDED.city),
    latlong = coalesce(x.latlong, EXCLUDED.latlong), rtt_ms = coalesce(x.rtt_ms, EXCLUDED.rtt_ms),
    tls = coalesce(x.tls, EXCLUDED.tls), device = coalesce(x.device, EXCLUDED.device);
  RETURN kept;
END $$;

-- The playtest's feedback records, kept as they were written: joins
-- (playtesters/), one-question survey answers (survey-answers/) and the
-- form sent once (surveys/). New ones from openagents.com come with
-- pt-feedback; the import fills them from the demo's bucket.
CREATE TABLE analytics.playtesters (
  id text PRIMARY KEY, at double precision NOT NULL, email text NOT NULL, player text, sid text, how text, auto text,
  space text NOT NULL DEFAULT 'web'
);
CREATE TABLE analytics.survey_answers (
  id text PRIMARY KEY, at double precision NOT NULL, rid text NOT NULL, q text NOT NULL, a jsonb, skip boolean,
  n int, "of" int, player text, sid text, "user" text, joined_as text, auto text, space text NOT NULL DEFAULT 'web'
);
CREATE TABLE analytics.surveys (
  id text PRIMARY KEY, at double precision NOT NULL, answers jsonb NOT NULL, auto text, space text NOT NULL DEFAULT 'web'
);
