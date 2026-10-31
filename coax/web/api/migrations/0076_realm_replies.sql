-- Reply with a realm (ev-reply-realm, docs/verse/replies.md): a player pastes a post's link or
-- types a line in Everglade, a house on the road in takes the words as its sign, a meteor shower
-- smashes it, and the reply's link is shared back: its page at /realms/everglade/reply/<id>
-- replays the smash from the same seed, and its card is the moment of impact.
--
-- Only what the player typed or pasted is kept: the line, or the link as text. A link is never
-- fetched, unfurled or followed here or on the page, and nothing of a post's author is kept.
-- Every word is one line of plain characters (no control characters, no bars), at most 140
-- characters for a line and 280 for a link, and escaped wherever a page shows it.
-- Additive: one table and its functions.
--
--   POST /api/playtest/replies {realm, text | link, seed}   -> 200 {id, path, page}; 400, 401, 429
--   GET  /api/playtest/replies/<id>                         -> text/plain: id=, realm=, house=, seed=, kind=, text=
--   GET  /api/playtest/replies/<id>/card                    -> text/plain "want" (its maker, no card yet) or "no"
--   POST /api/playtest/replies/<id>/card  (image/jpeg)      -> 200 {image, page}: its maker only; a 1200x630 JPEG under 300 KB

CREATE TABLE verse.replies (
  id text PRIMARY KEY CHECK (id ~ '^[a-z0-9]{12}$'),
  realm text NOT NULL CHECK (realm IN ('everglade')),
  house int NOT NULL CHECK (house BETWEEN 0 AND 15),   -- the building whose sign it is (verse/everglade/plan.coax EG_BUILDING)
  kind text NOT NULL CHECK (kind IN ('text', 'link')),
  words text NOT NULL CHECK (length(words) BETWEEN 1 AND 280 AND words !~ '[[:cntrl:]|]'),
  seed bigint NOT NULL CHECK (seed BETWEEN 1 AND 2147483647),   -- the meteor shower's dice (verse/rules/srd5.coax srd_dice)
  player text NOT NULL REFERENCES verse.players (id),           -- its maker: only they keep its card
  made_ms bigint NOT NULL,
  card_ms bigint,
  card_kb int
);
CREATE INDEX replies_player ON verse.replies (player, made_ms);

-- The words as kept: one line of plain characters, runs of space as one, at most n characters.
CREATE FUNCTION verse.reply_words(v jsonb, n int) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(v) = 'string' AND length(btrim(regexp_replace(v #>> '{}', '[[:cntrl:][:space:]|]+', ' ', 'g'))) BETWEEN 1 AND n
              THEN btrim(regexp_replace(v #>> '{}', '[[:cntrl:][:space:]|]+', ' ', 'g')) END $$;

-- A reply made: {status, id, path} or {status, error}.
CREATE FUNCTION verse.reply_make(b jsonb, k text, per_day int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  p verse.players := verse.builder_player(k);
  words text; kind text; seed bigint; nid text;
BEGIN
  IF p.id IS NULL THEN RETURN '{"status":401,"error":"a player key"}'; END IF;
  IF jsonb_typeof(b) IS DISTINCT FROM 'object' THEN RETURN '{"status":400,"error":"not json"}'; END IF;
  IF b->>'realm' IS DISTINCT FROM 'everglade' OR jsonb_typeof(b->'realm') <> 'string' THEN RETURN '{"status":400,"error":"realm is everglade"}'; END IF;
  IF (b ? 'text') = (b ? 'link') THEN RETURN '{"status":400,"error":"a text or a link, one of them"}'; END IF;
  IF b ? 'text' THEN
    kind := 'text'; words := verse.reply_words(b->'text', 140);
    IF words IS NULL THEN
      RETURN CASE WHEN jsonb_typeof(b->'text') = 'string' AND btrim(b->>'text') <> '' THEN '{"status":400,"error":"a line is 140 characters at most"}'
                  ELSE '{"status":400,"error":"a line to put on the sign"}' END::jsonb;
    END IF;
  ELSE
    kind := 'link'; words := verse.reply_words(b->'link', 280);
    -- a post's address, kept as the text it is: never fetched
    IF words IS NULL OR words !~ '^https?://[^[:space:]<>"''`\\]+$' THEN RETURN '{"status":400,"error":"a link is an http or https address, 280 characters at most"}'; END IF;
  END IF;
  IF jsonb_typeof(b->'seed') IS DISTINCT FROM 'number' OR (b->>'seed') !~ '^[1-9][0-9]{0,9}$' OR (b->>'seed')::numeric > 2147483647 THEN
    RETURN '{"status":400,"error":"seed is a whole number from 1 to 2147483647"}';
  END IF;
  seed := (b->>'seed')::bigint;
  IF (SELECT count(*) FROM verse.replies WHERE player = p.id AND made_ms > verse.now_ms() - 86400000) >= per_day THEN
    RETURN jsonb_build_object('status', 429, 'error', format('%s replies a day', per_day));
  END IF;
  LOOP
    nid := substr(md5(random()::text || clock_timestamp()::text || p.id), 1, 12);
    EXIT WHEN NOT EXISTS (SELECT 1 FROM verse.replies WHERE id = nid);
  END LOOP;
  INSERT INTO verse.replies (id, realm, house, kind, words, seed, player, made_ms)
  VALUES (nid, 'everglade', 12, kind, words, seed, p.id, verse.now_ms());
  RETURN jsonb_build_object('status', 200, 'id', nid, 'path', '/realms/everglade/reply/' || nid);
END $$;

-- What the realm reads to replay it, a line each (the words last: they never hold a line break).
CREATE FUNCTION verse.reply_lines(i text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT 'id=' || r.id || E'\nrealm=' || r.realm || E'\nhouse=' || r.house || E'\nseed=' || r.seed
      || E'\nkind=' || r.kind || E'\ntext=' || r.words || E'\n'
  FROM verse.replies r WHERE r.id = i $$;

CREATE FUNCTION verse.reply_card_path(r verse.replies) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN r.card_ms IS NOT NULL THEN '/realms/everglade/reply/' || r.id || '/card.jpg?v=' || r.card_ms ELSE '/realms/everglade/og.jpg' END $$;
-- The preview's title: the words in quotes, cut at 90 characters.
CREATE FUNCTION verse.reply_title(r verse.replies) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT '“' || CASE WHEN length(r.words) > 90 THEN rtrim(left(r.words, 89)) || '…' ELSE r.words END || '”' $$;
CREATE FUNCTION verse.reply_line() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT 'A reply from Everglade: these words went up on a house and a meteor shower came down on it. Watch it again, then reply with a realm of your own.' $$;

-- The page at /realms/everglade/reply/<id>: every preview tag, every word escaped, and the realm's
-- own page's module (the build names its files) opened with the reply.
CREATE FUNCTION verse.reply_page(i text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>' || verse.builder_html(verse.reply_title(r)) || ' · a reply from Everglade · OpenAgents</title>
<meta name="description" content="' || verse.builder_html(verse.reply_line()) || '">
<meta property="og:type" content="website">
<meta property="og:site_name" content="OpenAgents">
<meta property="og:title" content="' || verse.builder_html(verse.reply_title(r)) || '">
<meta property="og:description" content="' || verse.builder_html(verse.reply_line()) || '">
<meta property="og:url" content="' || verse.builder_html(origin) || '/realms/everglade/reply/' || r.id || '">
<meta property="og:image" content="' || verse.builder_html(origin) || verse.builder_html(verse.reply_card_path(r)) || '">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="A meteor shower comes down on a house in Everglade with these words on its sign: ' || verse.builder_html(r.words) || '">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="' || verse.builder_html(verse.reply_title(r)) || '">
<meta name="twitter:description" content="' || verse.builder_html(verse.reply_line()) || '">
<meta name="twitter:image" content="' || verse.builder_html(origin) || verse.builder_html(verse.reply_card_path(r)) || '">
<meta name="theme-color" content="#000000">
<meta name="robots" content="noindex">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<link rel="alternate" type="text/markdown" href="/realms/everglade/reply/' || r.id || '/index.md">
<script>if ("gpu" in navigator) document.documentElement.classList.add("gpu")</script>
<style>
html,body{margin:0;height:100%;background:#000;color:#ededed}
body{overflow:hidden;font-family:ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;font-size:14px;line-height:1.5}
canvas{position:fixed;inset:0;width:100%;height:100%;touch-action:none;user-select:none;-webkit-user-select:none;-webkit-touch-callout:none}
canvas[data-cursor=pointer]{cursor:pointer}
main{position:relative;max-width:640px;margin:0 auto;padding:24px 16px}
html:not(.gpu) body{overflow:auto}
a{color:#ededed}
.gpu main{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;padding:0}
</style>
</head>
<body>
<canvas id="coaxial" aria-hidden="true"></canvas>
<main>
<h1>A reply from Everglade</h1>
<p><a href="/realms/everglade/reply/' || r.id || '/index.md">A reply from Everglade</a></p>
</main>
<script type="module" data-reply="' || r.id || '">
const id = document.querySelector("script[data-reply]").dataset.reply;
const t = await (await fetch("/realms/everglade", { cache: "no-store" })).text();
const from = new DOMParser().parseFromString(t, "text/html").querySelector("script[data-app]");
const s = document.createElement("script");
for (const a of from.attributes) s.setAttribute(a.name, a.value);
s.dataset.slot = "everglade-reply"; s.dataset.clock = "fixed:0.4";
s.dataset.lines = "620:/api/playtest/replies/" + id;
s.dataset.card = "/api/playtest/replies/" + id + "/card"; s.dataset.cardOn = "impact";
document.body.append(s);
</script>
</body>
</html>
' FROM verse.replies r WHERE r.id = i $$;

-- Its text: the words as a quote (a link is its text, never a link here), and what happened.
CREATE FUNCTION verse.reply_md(i text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT E'# A reply from Everglade\n\n> ' || replace(replace(replace(r.words, '\', '\\'), '<', '\<'), '>', '\>')
      || E'\n\n' || verse.reply_line()
      || E'\n\nA visitor ' || CASE r.kind WHEN 'link' THEN 'pasted a post''s address' ELSE 'typed a line' END
      || ' in Everglade (https://openagents.com/realms/everglade), a house on the road in by the South Gate took it as its sign, '
      || E'and a Meteor Swarm (SRD 5e) came down on it. The page replays it from the same dice.\n'
  FROM verse.replies r WHERE r.id = i $$;

-- GET .../card: "want" for its maker when it has no card yet.
CREATE FUNCTION verse.reply_card_want(i text, k text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN r.card_ms IS NULL AND r.player = (verse.builder_player(k)).id THEN 'want' ELSE 'no' END
  FROM verse.replies r WHERE r.id = i $$;
-- May this visitor keep its card? -> 200, 403 or 404
CREATE FUNCTION verse.reply_card_may(i text, k text) RETURNS int LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT CASE WHEN r.player = (verse.builder_player(k)).id THEN 200 ELSE 403 END FROM verse.replies r WHERE r.id = i), 404) $$;
CREATE FUNCTION verse.reply_card_kept(i text, kb int) RETURNS bigint LANGUAGE sql AS $$
  UPDATE verse.replies SET card_ms = verse.now_ms(), card_kb = kb WHERE id = i RETURNING card_ms $$;
