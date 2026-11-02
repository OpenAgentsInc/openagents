-- The voice switches a realm's features and opens its editing screen (after 0056).
-- "Turn off the guards" and "open the edit panel" used to come back as chat ("I can't ...").
-- Two new edits:
--   switch {row, on}: one of the realm's rows (verse.realm_rights.rows, the board's list) on or off;
--                     the page sends it as the board's tap does (POST /switch), so the owner's check holds.
--   show {panel}:     the editing screen's panes: edit (the board), settings, ideas (visitors' cards), close.
-- The rows come from the realm's own rights, so a realm that gains a row can be switched by voice
-- with no new migration. The prompt and schema read that table now, so they are STABLE.
-- Everglade gains two rows: guards (the watch gives chase; off, nobody comes after you) and
-- invincible (off unless switched on: the realm's owner and admins take no harm; visitors still do).

UPDATE verse.realm_rights SET rows = rows || '[{"id": "guards", "label": "Guards who chase you", "says": "the town watch gives chase when you break things or attack; off, nobody comes after you"}]'::jsonb
WHERE slug = 'everglade' AND NOT rows @> '[{"id": "guards"}]';
UPDATE verse.realm_rights SET rows = rows || '[{"id": "invincible", "label": "Owner can''t be hurt", "says": "the realm''s owner and admins take no harm from blows or spells; visitors still do"}]'::jsonb
WHERE slug = 'everglade' AND NOT rows @> '[{"id": "invincible"}]';
-- off until the owner switches it on
INSERT INTO verse.realm_edit (slug, switched, ms) VALUES ('everglade', '{"invincible": false}'::jsonb, verse.now_ms())
ON CONFLICT (slug) DO UPDATE SET switched = verse.realm_edit.switched || '{"invincible": false}'::jsonb
WHERE NOT verse.realm_edit.switched ? 'invincible';

CREATE OR REPLACE FUNCTION verse.voice_edit_rows(realm text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT jsonb_agg(r) FROM verse.realm_rights x, jsonb_array_elements(x.rows) r
                   WHERE x.slug = realm AND jsonb_typeof(r->'id') = 'string'), '[]'::jsonb) $$;
CREATE OR REPLACE FUNCTION verse.voice_edit_panels() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT '["edit", "settings", "ideas", "close"]'::jsonb $$;

CREATE OR REPLACE FUNCTION verse.voice_edit_order() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT ARRAY['place', 'remove', 'move', 'resize', 'rotate', 'restore', 'list', 'undo', 'switch', 'show'] $$;

CREATE OR REPLACE FUNCTION verse.voice_edit_prompt(realm text) RETURNS text LANGUAGE sql STABLE AS $$
  WITH c AS (SELECT verse.voice_edit_catalogue(realm) AS c)
  SELECT 'You turn what an admin said out loud into one edit of ' || initcap(realm) || ', a realm in the Verse (a small 3D world). '
    || 'They stand in the realm and speak one sentence to change it. Choose the one edit below that does what they asked, '
    || 'and fill its fields from what they said; every field the edit does not use gets its "not said" value. '
    || 'Speech recognition may mishear a word; pick what they most likely meant. When the sentence is unclear, still pick the edit it most likely means. '
    || 'Answer edit "none" only when it is not an edit at all (a question, small talk, thanks). '
    || 'Never answer a thing they did not ask for: when what they named is not in these lists, the item is something and name is what they said. '
    || 'reply is "" for every edit. For none, reply answers them the way a friendly builder standing beside them would, out loud: '
    || 'one to three short plain sentences, no lists or markdown. Answer the question itself when you can (what is in this realm and what you '
    || 'can do come from the lists below; the current time is given); when you cannot know, say so briefly. '
    || E'\nThe things that can be put down (width x depth x height in metres at size 1):\n'
    || (SELECT string_agg(i || ' ' || (c.c->'sizes'->>(o - 1)::int), '; ' ORDER BY o) FROM jsonb_array_elements_text(c.c->'items') WITH ORDINALITY AS t(i, o))
    || E'\nThings with a top that others can go onto: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c.c->'onto') k)
    || E'\nColours: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c.c->'colours') k)
    || E'\nThe city''s buildings: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c.c->'buildings') k)
    || E'\nThe city''s people and cats: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c.c->'who') k)
    || E'\nThe realm''s switches (its features, each on or off for everyone):\n'
    || coalesce((SELECT string_agg('- ' || (r->>'id') || ': ' || coalesce(r->>'label', '') || ' (' || coalesce(r->>'says', '') || ')', E'\n')
                 FROM jsonb_array_elements(verse.voice_edit_rows(realm)) r), '(none)')
    || E'\nThe edits:\n'
    || (SELECT string_agg('- ' || n || ': ' || (e->>'says')
         || E'\n  for example: ' || (SELECT string_agg(k, '; ') FROM jsonb_array_elements_text(e->'examples') k), E'\n' ORDER BY o)
        FROM unnest(verse.voice_edit_order()) WITH ORDINALITY AS t(n, o), LATERAL (SELECT c.c->'edits'->n AS e) x WHERE e IS NOT NULL)
    || E'\n- switch: turn one of the realm''s switches on or off: row is its id, on is true to turn it on, false to turn it off, disable, stop or get rid of it'
    || E'\n  for example: turn off the guards => switch, row guards, on false; stop the guards chasing me => switch, row guards, on false; '
    || 'make me invincible => switch, row invincible, on true; I want to be able to die again => switch, row invincible, on false; '
    || 'no more ducks => switch, row floats, on false; keep it daytime => switch, row daycycle, on false'
    || E'\n- show: open or close the editing screen: panel edit (the edit panel, the board, the list of features, edit mode), '
    || 'settings (the realm''s settings, tools visitors get), ideas (visitors'' ideas, proposals, cards), close (close it, done editing)'
    || E'\n  for example: open the edit panel => show, panel edit; edit mode => show, panel edit; show me the settings => show, panel settings; '
    || 'what have visitors suggested => show, panel ideas; close that => show, panel close; I''m done editing => show, panel close'
    || E'\n- none: not an edit; reply answers them. for example: what time is it => none, reply with the time; '
    || 'is this working? => none, reply yes, I can hear you; hello => none, reply with a greeting and what they can ask for; '
    || 'who lives here? => none, reply naming a few of the city''s people'
  FROM c WHERE c.c IS NOT NULL $$;

-- 0056's schema with row, on and panel before the reply.
CREATE OR REPLACE FUNCTION verse.voice_edit_schema(realm text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  WITH c AS (SELECT verse.voice_edit_catalogue(realm) AS c)
  SELECT jsonb_build_object('type', 'object',
    'required', '["edit", "item", "name", "count", "arrange", "onto", "colour", "size", "direction", "metres", "turn", "all", "who", "building", "row", "on", "panel", "reply"]'::jsonb,
    'propertyOrdering', '["edit", "item", "name", "count", "arrange", "onto", "colour", "size", "direction", "metres", "turn", "all", "who", "building", "row", "on", "panel", "reply"]'::jsonb,
    'properties', jsonb_build_object(
    'edit', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.voice_edit_order() || ARRAY['none'])),
    'item', jsonb_build_object('type', 'string', 'description', 'the kind of thing they named (a person, cat or building of the city by its kind); something when they named no kind ("that", "it", "this") or a thing not in the list',
                               'enum', (c.c->'items') || (c.c->'kinds') || '["something"]'::jsonb),
    'name', jsonb_build_object('type', 'string', 'description', 'only when they named a thing, a person or a building that is NOT in the lists: what they said, as said ("dragon", "Bob"); else ""'),
    'count', jsonb_build_object('type', 'integer', 'description', 'for place: how many (1 to 40; "two" is 2, "a" is 1, a plural with no number is 3); else 1'),
    'arrange', jsonb_build_object('type', 'string', 'description', 'for place: front (the usual), row, ring (all around, around me, in a circle), scatter (spread about), fill (fill the field with)',
                                  'enum', c.c->'arrange'),
    'onto', jsonb_build_object('type', 'string', 'description', 'for place: the thing they said to put it ON; ground when they did not say on',
                               'enum', '["ground"]'::jsonb || (c.c->'onto')),
    'colour', jsonb_build_object('type', 'string', 'description', 'for place: the colour they said; unsaid when none', 'enum', (c.c->'colours') || '["unsaid"]'::jsonb),
    'size', jsonb_build_object('type', 'number', 'description', 'for place: big 1.5, huge 2.5, small 0.6, tiny 0.4, else 1; for resize: the factor (bigger 1.5, smaller 0.67, twice 2, half 0.5)'),
    'direction', jsonb_build_object('type', 'string', 'description', 'for move: which way; here (to in front of the speaker) when not said', 'enum', c.c->'directions'),
    'metres', jsonb_build_object('type', 'number', 'description', 'for move: how far, when they said; else 0'),
    'turn', jsonb_build_object('type', 'number', 'description', 'for rotate: degrees, positive to the right, around is 180; else 0'),
    'all', jsonb_build_object('type', 'boolean', 'description', 'for remove, move, resize, rotate: true when they mean more than one (these, those, a plural, all, clear), else false'),
    'who', jsonb_build_object('type', 'string', 'description', 'for remove or restore: the person or cat of the city they said by name; nobody when they said no such name',
                              'enum', '["nobody"]'::jsonb || (c.c->'who')),
    'building', jsonb_build_object('type', 'string', 'description', 'for remove or restore: the city''s building they named; unnamed when none',
                                   'enum', '["unnamed"]'::jsonb || (c.c->'buildings')),
    'row', jsonb_build_object('type', 'string', 'description', 'for switch: the switch they mean; unnamed otherwise',
                              'enum', '["unnamed"]'::jsonb || coalesce((SELECT jsonb_agg(r->'id') FROM jsonb_array_elements(verse.voice_edit_rows(realm)) r), '[]'::jsonb)),
    'on', jsonb_build_object('type', 'boolean', 'description', 'for switch: true to turn it on, false to turn it off; else false'),
    'panel', jsonb_build_object('type', 'string', 'description', 'for show: the pane to open, or close; unsaid otherwise',
                                'enum', '["unsaid"]'::jsonb || verse.voice_edit_panels()),
    'reply', jsonb_build_object('type', 'string', 'description', 'for none: what to say back to them, one to three short spoken sentences; for an edit: ""')))
  FROM c WHERE c.c IS NOT NULL $$;

-- 0051's hold, with switch and show held to the realm's rows and the panes; the rest as before.
CREATE OR REPLACE FUNCTION verse.voice_edit_hold(realm text, d jsonb) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE
  c jsonb := verse.voice_edit_catalogue(realm);
  verb text := CASE WHEN jsonb_typeof(d) = 'object' AND jsonb_typeof(d->'edit') = 'string' THEN d->>'edit' END;
  said text := CASE WHEN jsonb_typeof(d->'name') = 'string' THEN left(btrim(d->>'name'), 60) ELSE '' END;
  item text := CASE WHEN jsonb_typeof(d->'item') = 'string' THEN d->>'item' END;
  placed bool;
  out jsonb;
  x numeric;
BEGIN
  IF c IS NULL OR verb IS NULL OR NOT verb = ANY (verse.voice_edit_order()) THEN RETURN NULL; END IF;
  placed := coalesce(c->'items' ? item, false);
  out := jsonb_build_object('edit', verb);
  IF verb IN ('list', 'undo') THEN RETURN out; END IF;
  IF verb = 'switch' THEN   -- one of the realm's own rows, by its id
    IF NOT verse.voice_edit_rows(realm) @> jsonb_build_array(jsonb_build_object('id', coalesce(d->>'row', ''))) THEN RETURN NULL; END IF;
    RETURN out || jsonb_build_object('row', d->>'row', 'on', jsonb_typeof(d->'on') = 'boolean' AND (d->>'on')::boolean);
  END IF;
  IF verb = 'show' THEN     -- one of the editing screen's panes, or close
    IF NOT verse.voice_edit_panels() ? coalesce(d->>'panel', '') THEN RETURN NULL; END IF;
    RETURN out || jsonb_build_object('panel', d->>'panel');
  END IF;

  IF verb = 'place' THEN
    IF NOT placed THEN   -- never another thing instead: what they named is missing, or it's no edit
      RETURN CASE WHEN said <> '' THEN jsonb_build_object('edit', 'missing', 'name', said) END;
    END IF;
    x := CASE WHEN jsonb_typeof(d->'count') = 'number' THEN round((d->>'count')::numeric) END;
    out := out || jsonb_build_object('item', item, 'count', CASE WHEN x IS NULL OR x < 1 THEN 1 ELSE least(x, 40)::int END);
    IF c->'arrange' ? coalesce(d->>'arrange', '') AND d->>'arrange' <> 'front' THEN out := out || jsonb_build_object('arrange', d->>'arrange'); END IF;
    IF c->'onto' ? coalesce(d->>'onto', '') THEN out := out || jsonb_build_object('onto', d->>'onto'); END IF;
    IF c->'colours' ? coalesce(d->>'colour', '') THEN out := out || jsonb_build_object('colour', d->>'colour'); END IF;
    x := CASE WHEN jsonb_typeof(d->'size') = 'number' THEN round((d->>'size')::numeric, 2) END;
    IF x > 0 AND x <> 1 THEN out := out || jsonb_build_object('size', trim_scale(greatest(0.25, least(4, x)))); END IF;
    RETURN out;
  END IF;

  IF verb = 'restore' THEN
    IF c->'buildings' ? coalesce(d->>'building', '') THEN out := out || jsonb_build_object('building', d->>'building');
    ELSIF c->'who' ? coalesce(d->>'who', '') THEN out := out || jsonb_build_object('who', d->>'who');
    ELSIF said <> '' THEN RETURN jsonb_build_object('edit', 'missing', 'name', said);
    END IF;
    RETURN out;
  END IF;

  -- remove, move, resize, rotate: what (a placed kind; remove also the city's people, cats and buildings), and how many
  IF placed OR (verb = 'remove' AND c->'kinds' ? coalesce(item, '')) THEN out := out || jsonb_build_object('item', item); END IF;
  IF jsonb_typeof(d->'all') = 'boolean' AND (d->>'all')::boolean THEN out := out || '{"all": true}'::jsonb; END IF;
  IF verb = 'remove' THEN
    -- a name only for one person: not with a placed thing's kind, nor with many
    IF c->'who' ? coalesce(d->>'who', '') AND NOT out ? 'all' AND coalesce(out->>'item', 'person') IN ('person', 'cat') THEN
      out := out || jsonb_build_object('who', d->>'who');
    END IF;
    -- a building of the city only as the building it is
    IF c->'buildings' ? coalesce(d->>'building', '') AND coalesce(out->>'item', 'building') = 'building' AND NOT out ? 'who' THEN
      out := out || jsonb_build_object('item', 'building', 'building', d->>'building');
    END IF;
  END IF;
  -- they named someone or something not here, and nothing that is
  IF said <> '' AND NOT placed AND NOT out ? 'who' AND NOT out ? 'building' THEN
    RETURN jsonb_build_object('edit', 'missing', 'name', said);
  END IF;
  IF verb = 'move' THEN
    out := out || jsonb_build_object('direction', CASE WHEN c->'directions' ? coalesce(d->>'direction', '') THEN d->>'direction' ELSE 'here' END);
    x := CASE WHEN jsonb_typeof(d->'metres') = 'number' THEN round((d->>'metres')::numeric, 1) END;
    IF x > 0 THEN out := out || jsonb_build_object('metres', trim_scale(greatest(0.5, least(30, x)))); END IF;
  ELSIF verb = 'resize' THEN
    x := CASE WHEN jsonb_typeof(d->'size') = 'number' THEN round((d->>'size')::numeric, 2) END;
    out := out || jsonb_build_object('size', CASE WHEN x IS NULL OR x <= 0 OR x = 1 THEN 1.5 ELSE trim_scale(greatest(0.25, least(4, x))) END);
  ELSIF verb = 'rotate' THEN
    x := CASE WHEN jsonb_typeof(d->'turn') = 'number' THEN round((d->>'turn')::numeric) END;
    out := out || jsonb_build_object('turn', CASE WHEN x IS NULL OR x = 0 THEN 90 ELSE greatest(-360, least(360, x))::int END);
  END IF;
  RETURN out;
END $$;
