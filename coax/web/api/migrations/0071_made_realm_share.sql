-- A made realm's link preview (docs/web/og-tags.md): its own line and its own picture, never the
-- Haunted House's. Additive: three columns, a trigger and new or replaced functions.
--
--   description   the owner's own line (POST /api/playtest/realms {description} or .../<slug>/describe);
--                 NULL: the line is written here from what is in the realm (verse.builder_line_of)
--   card_ms       when its card (/realms/<slug>/card.jpg, 1200x630 JPEG, in the site's bucket) was kept;
--                 the page names it with ?v=card_ms. NULL: the branded card until one is taken
--   card_kb       its size
--   edited_ms     the last change to what is in it (the trigger): a card older than this is taken again
--   page_rev      the page's template it was written with; the API rewrites older pages when it starts
--
--   GET  /api/playtest/realms/<slug>/card   -> text "want" (take a picture and send it) or "no"
--   POST /api/playtest/realms/<slug>/card   a 1200x630 JPEG under 300 KB, its owner or a site admin
--   POST /api/playtest/realms/<slug>/describe {description}   its owner or a site admin
--   GET  /api/playtest/realms/made          the newest made realms as /og's rows (og/pages.txt's columns)
ALTER TABLE verse.made_realms ADD COLUMN description text;
ALTER TABLE verse.made_realms ADD COLUMN card_ms bigint;
ALTER TABLE verse.made_realms ADD COLUMN card_kb int;
ALTER TABLE verse.made_realms ADD COLUMN edited_ms bigint;
ALTER TABLE verse.made_realms ADD COLUMN page_rev int NOT NULL DEFAULT 0;
CREATE INDEX made_realms_made ON verse.made_realms (made_ms DESC);

CREATE FUNCTION verse.made_realms_edited() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF (NEW.name, NEW.style, NEW.ground, NEW.time, NEW.camera, NEW.features, NEW.placed, NEW.off, NEW.description)
     IS DISTINCT FROM (OLD.name, OLD.style, OLD.ground, OLD.time, OLD.camera, OLD.features, OLD.placed, OLD.off, OLD.description) THEN
    NEW.edited_ms := verse.now_ms();
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER made_realms_edited BEFORE UPDATE ON verse.made_realms FOR EACH ROW EXECUTE FUNCTION verse.made_realms_edited();

-- The page's template: a page written with an older one is written again (realms.c, on start).
CREATE FUNCTION verse.builder_page_rev() RETURNS int LANGUAGE sql IMMUTABLE AS $$ SELECT 1 $$;

-- What is in it, a phrase a feature, for the line: "a small pond", "9 pine trees", "Mara".
CREATE FUNCTION verse.builder_phrase(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE f->>'feature'
    WHEN 'water' THEN 'a ' || coalesce(f->>'size', 'small') || ' pond'
    WHEN 'trees' THEN (f->>'count') || ' ' || CASE f->>'variant' WHEN 'mixed' THEN 'trees and bushes' ELSE coalesce(f->>'variant', 'round') || ' trees' END
    WHEN 'rocks' THEN (f->>'count') || ' rocks'
    WHEN 'plants' THEN (f->>'count') || ' ferns'
    WHEN 'lanterns' THEN (f->>'count') || ' ' || coalesce(f->>'colour', 'gold') || ' lanterns'
    WHEN 'flock' THEN 'a flock of ' || coalesce(f->>'variant', 'crow') || 's'
    WHEN 'props' THEN CASE f->>'variant' WHEN 'crates' THEN 'crates' WHEN 'barrels' THEN 'barrels' WHEN 'wagon' THEN 'a wagon'
                                         WHEN 'fence' THEN 'a fence' ELSE 'a camp' END
  END $$;

-- Its line when the owner wrote none, from what is in it, at most 200 characters:
-- "A forest at dusk with a small pond, 9 pine trees and a flock of crows, where Mara lives."
CREATE FUNCTION verse.builder_line_of(m verse.made_realms) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  place text := CASE m.ground WHEN 'forest' THEN 'A forest' WHEN 'desert' THEN 'A desert' WHEN 'snow' THEN 'A snowfield'
                              WHEN 'stone' THEN 'A stony plain' WHEN 'beach' THEN 'A beach' ELSE 'A meadow' END
                || CASE m.time WHEN 'dawn' THEN ' at dawn' WHEN 'dusk' THEN ' at dusk' WHEN 'night' THEN ' at night' ELSE ' by day' END;
  things text[] := ARRAY(SELECT p FROM (SELECT verse.builder_phrase(f) p, o FROM jsonb_array_elements(m.features) WITH ORDINALITY x(f, o)) y
                         WHERE p IS NOT NULL ORDER BY o);
  people text[] := ARRAY(SELECT f->>'name' FROM jsonb_array_elements(m.features) WITH ORDINALITY x(f, o)
                         WHERE f->>'feature' = 'character' AND coalesce(f->>'name', '') <> '' ORDER BY o);
  out text;
  list text;
  who text;
BEGIN
  LOOP
    list := CASE coalesce(array_length(things, 1), 0) WHEN 0 THEN NULL WHEN 1 THEN things[1]
            ELSE array_to_string(things[1:array_length(things, 1) - 1], ', ') || ' and ' || things[array_length(things, 1)] END;
    who := CASE coalesce(array_length(people, 1), 0) WHEN 0 THEN NULL WHEN 1 THEN people[1] || ' lives'
           ELSE array_to_string(people[1:array_length(people, 1) - 1], ', ') || ' and ' || people[array_length(people, 1)] || ' live' END;
    out := place || coalesce(' with ' || list, '') || coalesce(', where ' || who, '') || '.';
    EXIT WHEN length(out) <= 160 OR (coalesce(array_length(things, 1), 0) <= 1 AND coalesce(array_length(people, 1), 0) <= 1);
    IF coalesce(array_length(things, 1), 0) > 1 THEN things := things[1:array_length(things, 1) - 1];
    ELSE people := people[1:array_length(people, 1) - 1]; END IF;
  END LOOP;
  out := left(out, 160);
  -- a short line says what it is too (a preview's line wants 50 characters or more)
  RETURN out || ' A realm built on the Verse.';
END $$;
CREATE FUNCTION verse.builder_share_line(m verse.made_realms) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(nullif(btrim(m.description), ''), verse.builder_line_of(m)) $$;
-- The picture a preview shows: its own card, else the branded one (never another realm's).
CREATE FUNCTION verse.builder_card_path(m verse.made_realms) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN m.card_ms IS NOT NULL THEN '/realms/' || m.slug || '/card.jpg?v=' || m.card_ms ELSE '/og/default.jpg' END $$;

-- The page at /realms/<slug>, now with every preview tag: its line, its own picture, Twitter's.
CREATE OR REPLACE FUNCTION verse.builder_page(s text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>' || verse.builder_html(m.name) || ' · a realm on the Verse · OpenAgents</title>
<meta name="description" content="' || verse.builder_html(verse.builder_share_line(m)) || '">
<meta property="og:type" content="website">
<meta property="og:site_name" content="OpenAgents">
<meta property="og:title" content="' || verse.builder_html(m.name) || '">
<meta property="og:description" content="' || verse.builder_html(verse.builder_share_line(m)) || '">
<meta property="og:url" content="' || verse.builder_html(origin) || '/realms/' || m.slug || '">
<meta property="og:image" content="' || verse.builder_html(origin) || verse.builder_html(verse.builder_card_path(m)) || '">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="' || verse.builder_html(m.name) || ', a realm on the Verse">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="' || verse.builder_html(m.name) || '">
<meta name="twitter:description" content="' || verse.builder_html(verse.builder_share_line(m)) || '">
<meta name="twitter:image" content="' || verse.builder_html(origin) || verse.builder_html(verse.builder_card_path(m)) || '">
<meta name="theme-color" content="#000000">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<link rel="alternate" type="text/markdown" href="/realms/' || m.slug || '/index.md">
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
<h1>' || verse.builder_html(m.name) || '</h1>
<p><a href="/realms/' || m.slug || '/index.md">' || verse.builder_html(m.name) || '</a></p>
</main>
<script type="module" data-made="' || m.slug || '" data-style-of="' || m.style || '">
const me = document.querySelector("script[data-made]"), slug = me.dataset.made;
const t = await (await fetch("/realms/built", { cache: "no-store" })).text();
const from = new DOMParser().parseFromString(t, "text/html").querySelector("script[data-app]");
const s = document.createElement("script");
for (const a of from.attributes) s.setAttribute(a.name, a.value);
s.dataset.realm = "made"; s.dataset.slot = "made-" + slug; s.dataset.style = me.dataset.styleOf;
s.dataset.text = slug; s.dataset.card = "/api/playtest/realms/" + slug + "/card";
document.body.append(s);
</script>
</body>
</html>
' FROM verse.made_realms m WHERE m.slug = s $$;

-- The realm's text, with its line.
CREATE OR REPLACE FUNCTION verse.builder_md(s text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '# ' || m.name || E'\n\n' || verse.builder_share_line(m)
      || E'\n\nA realm on the Verse at https://openagents.com/realms/' || m.slug
      || ', made by a visitor from one prompt (https://openagents.com/realms/new). '
      || CASE m.camera WHEN 'first' THEN '3D, first person' WHEN 'top' THEN '3D, top-down' WHEN 'side' THEN '2D, side-on' ELSE '3D, third person' END
      || '. Style: ' || m.style || '. Ground: ' || m.ground
      || '. Time of day: ' || m.time || E'.\n\n## What is here\n\n'
      || coalesce((SELECT string_agg('- ' || (f->>'label'), E'\n' ORDER BY o) FROM jsonb_array_elements(m.features) WITH ORDINALITY AS x(f, o)), '- Nothing yet: open ground')
      || E'\n\n## What visitors may do\n\n- Walk'
      || coalesce(E'\n' || (SELECT string_agg('- ' || initcap(t), E'\n' ORDER BY o) FROM unnest(verse.builder_tools()) WITH ORDINALITY AS x(t, o)
                            WHERE coalesce((m.tools->>t)::boolean, false)), '')
      || E'\n\nThe owner decides what visitors get, and accepts or declines what others build here.\n'
  FROM verse.made_realms m WHERE m.slug = s $$;

-- One line of plain words: no line breaks, bars or runs of spaces, at most n characters.
CREATE FUNCTION verse.builder_one_line(v jsonb, n int) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT left(btrim(regexp_replace(CASE WHEN jsonb_typeof(v) = 'string' THEN v #>> '{}' ELSE '' END, '[[:cntrl:][:space:]|]+', ' ', 'g')), n) $$;

-- The owner's own line: one line of plain words, at most 200 characters; "" goes back to the
-- written one. -> {status, description} | {status, error}
CREATE FUNCTION verse.builder_describe(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  line text := CASE WHEN jsonb_typeof(body) = 'object' AND jsonb_typeof(body->'description') = 'string' THEN verse.builder_one_line(body->'description', 200) END;
  m verse.made_realms;
BEGIN
  IF NOT verse.edit_made(s) THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF NOT verse.edit_may(s, k, site_admin) THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF line IS NULL THEN RETURN '{"status":400,"error":"a description"}'; END IF;
  UPDATE verse.made_realms SET description = nullif(line, '') WHERE slug = s RETURNING * INTO m;
  RETURN jsonb_build_object('status', 200, 'description', verse.builder_share_line(m));
END $$;

-- GET .../card: "want" when this visitor should take its picture: its owner when it has none or it
-- changed since; a site admin only when it has none.
CREATE FUNCTION verse.builder_card_want(s text, k text, site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN m.card_ms IS NULL AND (verse.builder_owns(k, s) OR site_admin) THEN 'want'
              WHEN verse.builder_owns(k, s) AND coalesce(m.edited_ms, 0) > m.card_ms THEN 'want'
              ELSE 'no' END
  FROM verse.made_realms m WHERE m.slug = s $$;
-- May this visitor keep its card? -> 200, 403 or 404
CREATE FUNCTION verse.builder_card_may(s text, k text, site_admin boolean) RETURNS int LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN NOT verse.edit_made(s) THEN 404 WHEN verse.edit_may(s, k, site_admin) THEN 200 ELSE 403 END $$;
CREATE FUNCTION verse.builder_card_kept(s text, kb int) RETURNS bigint LANGUAGE sql AS $$
  UPDATE verse.made_realms SET card_ms = verse.now_ms(), card_kb = kb WHERE slug = s RETURNING card_ms $$;

-- The next page to write again with the current template (one at a time, any instance).
CREATE FUNCTION verse.builder_page_stale() RETURNS text LANGUAGE sql AS $$
  UPDATE verse.made_realms SET page_rev = verse.builder_page_rev()
  WHERE slug = (SELECT slug FROM verse.made_realms WHERE page_rev < verse.builder_page_rev() ORDER BY made_ms DESC LIMIT 1 FOR UPDATE SKIP LOCKED)
  RETURNING slug $$;
CREATE FUNCTION verse.builder_page_written(s text) RETURNS boolean LANGUAGE sql AS $$
  UPDATE verse.made_realms SET page_rev = verse.builder_page_rev() WHERE slug = s RETURNING true $$;

-- /og's rows for the newest made realms (og/pages.txt's columns, coax/web/og.ts):
-- path|kind|noindex|title|description|url|image|alt|source|width|height|kb|errors|warnings|progress
CREATE FUNCTION verse.builder_og_rows(n int) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg(
    '/realms/' || m.slug || '|made||' || verse.builder_line(to_jsonb(m.name), 120) || '|' || verse.builder_line(to_jsonb(verse.builder_share_line(m)), 240)
    || '|https://openagents.com/realms/' || m.slug || '|' || verse.builder_card_path(m) || '|' || verse.builder_line(to_jsonb(m.name), 120)
    || '|' || CASE WHEN m.card_ms IS NULL THEN 'default' ELSE 'made' END || '|1200|630|' || coalesce(m.card_kb, 0)
    || '|' || CASE WHEN m.card_ms IS NULL THEN 'no picture of its own yet (its owner''s next visit takes one)' ELSE '' END
    || '|' || CASE WHEN m.description IS NULL THEN 'line written from what is in it' ELSE '' END || '|', E'\n' ORDER BY m.made_ms DESC) || E'\n', '')
  FROM (SELECT * FROM verse.made_realms ORDER BY made_ms DESC LIMIT greatest(least(n, 200), 1)) m $$;
