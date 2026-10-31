-- A design document dropped on a realm's feature list or on the builder (realm-docs; realms.c's
-- doc(): the text taken out of a .md, .txt or .docx, or a PDF the model reads itself). It goes the
-- way an ADD goes (0078): the model answers the realm's board under its schema, and also names every
-- feature or change the document asks for ("asked", each with the feature or row that makes it, or
-- none). Each becomes its own line on the list: APPLIED with what it changed (so DELETE undoes just
-- that), or NOT APPLIED, waiting for code. No words are matched here: the model reads them.

-- {name, text | pdf} -> as edit_revise_ask, with {doc: NAME}.
CREATE FUNCTION verse.ask_doc_ask(s text, d jsonb, cap int, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  nm text := coalesce(nullif(verse.builder_line(d->'name', 80), ''), 'document');
  x jsonb := verse.edit_revise_ask(s, jsonb_build_object('prompt', 'the design document ' || nm), cap, k, site_admin);
  made boolean := verse.edit_made(s) AND (verse.fork_of(s)).slug IS NULL;
  t text;
  sch jsonb;
  ids jsonb;
  parts jsonb;
BEGIN
  IF (x->>'status')::int <> 200 OR NOT x ? 'request' THEN RETURN x; END IF;
  IF coalesce(d->>'text', '') = '' AND coalesce(d->>'pdf', '') = '' THEN RETURN '{"status":400,"error":"that document has no words in it"}'; END IF;
  t := split_part(x #>> '{request,contents,0,parts,0,text}', E'\nThey now say: ', 1)
    || E'\nThey dropped their design document "' || nm || '". In "asked", list every feature or change it asks for, in its order, '
    || 'each in a few of its own words (at most 40 of them), with the feature that makes it'
    || CASE WHEN made THEN ', setup for how it looks (its ground, time of day or colours),' ELSE '' END
    || ' or none when nothing here does. Then answer the board with every one a feature makes on, and keep everything else as it is.';
  IF coalesce(d->>'text', '') <> '' THEN t := t || E'\nThe document:\n' || left(d->>'text', 60000); END IF;
  parts := jsonb_build_array(jsonb_build_object('text', t));
  IF coalesce(d->>'pdf', '') <> '' THEN
    parts := parts || jsonb_build_array(jsonb_build_object('inlineData', jsonb_build_object('mimeType', 'application/pdf', 'data', d->>'pdf')));
  END IF;
  x := jsonb_set(x, '{request,contents,0,parts}', parts);
  sch := x #> '{request,generationConfig,responseSchema}';
  ids := CASE WHEN made THEN to_jsonb(verse.builder_order() || ARRAY['setup', 'none'])
              ELSE coalesce(sch #> '{properties,features,items,properties,id,enum}', '[]') || '["none"]' END;
  sch := jsonb_set(sch, '{properties,asked}', jsonb_build_object('type', 'array', 'items', jsonb_build_object('type', 'object',
    'required', jsonb_build_array('thing', 'feature'), 'propertyOrdering', jsonb_build_array('thing', 'feature'),
    'properties', jsonb_build_object('thing', jsonb_build_object('type', 'string'), 'feature', jsonb_build_object('type', 'string', 'enum', ids)))));
  sch := jsonb_set(sch, '{required}', '["asked"]'::jsonb || (coalesce(sch->'required', '[]') - 'asked'));
  IF sch ? 'propertyOrdering' THEN sch := jsonb_set(sch, '{propertyOrdering}', '["asked"]'::jsonb || ((sch->'propertyOrdering') - 'asked')); END IF;
  x := jsonb_set(x, '{request,generationConfig,responseSchema}', sch);
  x := jsonb_set(x, '{request,generationConfig,maxOutputTokens}', '8000');
  RETURN x || jsonb_build_object('doc', nm);
END $$;

-- The row a key of the board holds, before or after (a made realm's feature, a hand-built row's {on, raw}).
CREATE FUNCTION verse.ask_doc_row(dd jsonb, key text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(nullif(dd->'after'->key, 'null'::jsonb), dd->'before'->key) $$;

-- Is what an item names in the realm now: a made realm's feature (or a fork's row) on, a hand-built row on.
CREATE FUNCTION verse.ask_doc_there(st jsonb, f text) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN coalesce((st->>'made')::boolean, false)
    THEN EXISTS (SELECT 1 FROM jsonb_each(coalesce(st->'rows', '{}')) r
                 WHERE (r.value->>'feature' = f OR (r.value->>'feature' = 'row' AND r.value->>'variant' = f)) AND coalesce((r.value->>'on')::boolean, true))
    ELSE coalesce((st->'rows'->f->>'on')::boolean, false) END $$;

-- The model's answer: the board as an ADD's is, then a line on the list for each thing it named.
-- asked: {id, doc}. -> {status, applied, waiting, lines: "doc N A applied W waiting\n" and the rows}
CREATE FUNCTION verse.ask_doc_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pid bigint := (asked->>'id')::bigint;
  s text := (SELECT slug FROM verse.realm_plans WHERE id = pid);
  who text := (SELECT player FROM verse.realm_plans WHERE id = pid);
  nm text := coalesce(nullif(asked->>'doc', ''), 'document');
  before jsonb := verse.ask_state(s);
  now_st jsonb;
  res jsonb;
  dd jsonb;
  said jsonb;
  it jsonb;
  w text;
  f text;
  keys text[];
  claimed text[] := '{}';
  did jsonb;
  st text;
  applied int := 0;
  waiting int := 0;
  rest text[];
BEGIN
  res := verse.fork_revise_answer(asked - 'doc', http, response, model, taken);
  IF (res->>'status')::int <> 200 THEN RETURN res; END IF;
  now_st := verse.ask_state(s);
  dd := verse.ask_diff(before, now_st);
  -- a hand-built realm's answer names every row; the ones it didn't change keep what they held (as 0078's)
  IF NOT coalesce((before->>'made')::boolean, false) THEN
    UPDATE verse.realm_edit SET
      switched = (switched - ARRAY(SELECT kk FROM jsonb_object_keys(before->'rows') kk WHERE NOT coalesce(dd->'after', '{}') ? kk))
              || coalesce((SELECT jsonb_object_agg(kk, v->'raw') FROM jsonb_each(before->'rows') x(kk, v)
                           WHERE NOT coalesce(dd->'after', '{}') ? kk AND jsonb_typeof(v->'raw') = 'boolean'), '{}')
    WHERE slug = s;
  END IF;
  BEGIN said := verse.builder_said(response)::jsonb; EXCEPTION WHEN others THEN said := NULL; END;
  FOR it IN SELECT e FROM jsonb_array_elements(CASE WHEN jsonb_typeof(said->'asked') = 'array' THEN said->'asked' ELSE '[]' END) WITH ORDINALITY t(e, o)
            WHERE jsonb_typeof(e) = 'object' ORDER BY o LIMIT 40 LOOP
    w := verse.builder_line(it->'thing', 200);
    CONTINUE WHEN w = '';
    f := coalesce(verse.builder_line(it->'feature', 60), 'none');
    keys := ARRAY(SELECT kk FROM jsonb_object_keys(coalesce(dd->'after', '{}')) kk
                  WHERE f <> 'none' AND NOT kk = ANY (claimed)
                    AND (kk = f OR verse.ask_doc_row(dd, kk)->>'feature' = f
                         OR (verse.ask_doc_row(dd, kk)->>'feature' = 'row' AND verse.ask_doc_row(dd, kk)->>'variant' = f)));
    claimed := claimed || keys;
    did := jsonb_build_object('before', coalesce((SELECT jsonb_object_agg(kk, dd->'before'->kk) FROM unnest(keys) kk), '{}'),
                              'after', coalesce((SELECT jsonb_object_agg(kk, dd->'after'->kk) FROM unnest(keys) kk), '{}'));
    IF f = 'setup' THEN did := did || (dd - 'before' - 'after'); dd := dd - 'ground' - 'time' - 'palette'; END IF;
    st := CASE WHEN verse.ask_changed(did) OR (f NOT IN ('none', 'setup') AND verse.ask_doc_there(now_st, f)) THEN 'applied' ELSE 'waiting' END;
    INSERT INTO verse.realm_asks (slug, ms, player, words, state, plan, did)
    VALUES (s, verse.now_ms(), who, w, st, pid, CASE WHEN st = 'applied' AND verse.ask_changed(did) THEN did ELSE '{}' END);
    IF st = 'applied' THEN applied := applied + 1; ELSE waiting := waiting + 1; END IF;
  END LOOP;
  -- what it changed that no line claimed: one more line holds it, so DELETE can take it back too
  rest := ARRAY(SELECT kk FROM jsonb_object_keys(coalesce(dd->'after', '{}')) kk WHERE NOT kk = ANY (claimed));
  did := jsonb_build_object('before', coalesce((SELECT jsonb_object_agg(kk, dd->'before'->kk) FROM unnest(rest) kk), '{}'),
                            'after', coalesce((SELECT jsonb_object_agg(kk, dd->'after'->kk) FROM unnest(rest) kk), '{}')) || (dd - 'before' - 'after');
  IF verse.ask_changed(did) THEN
    INSERT INTO verse.realm_asks (slug, ms, player, words, state, plan, did)
    VALUES (s, verse.now_ms(), who, left('The rest of ' || nm, 400), 'applied', pid, did);
    applied := applied + 1;
  END IF;
  -- nothing named and nothing changed: the document itself waits, never dropped
  IF applied + waiting = 0 THEN
    INSERT INTO verse.realm_asks (slug, ms, player, words, state, plan) VALUES (s, verse.now_ms(), who, left('What ' || nm || ' asks for', 400), 'waiting', pid);
    waiting := 1;
  END IF;
  RETURN jsonb_build_object('status', 200, 'applied', applied, 'waiting', waiting,
    'lines', 'doc ' || (applied + waiting) || ' ' || applied || ' applied ' || waiting || E' waiting\n' || coalesce(res->>'lines', verse.edit_rows(s)));
END $$;

-- The builder: a board planned from the document, as from a prompt (its setup kept). -> as builder_ask
CREATE FUNCTION verse.builder_doc_ask(d jsonb, cap int, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  nm text := coalesce(nullif(verse.builder_line(d->'name', 80), ''), 'document');
  said text := 'the design document ' || nm;
  x jsonb;
  t text;
  parts jsonb;
BEGIN
  IF coalesce(d->>'text', '') = '' AND coalesce(d->>'pdf', '') = '' THEN RETURN '{"status":400,"error":"that document has no words in it"}'; END IF;
  x := verse.builder_ask(jsonb_build_object('prompt', said) || CASE WHEN jsonb_typeof(d->'setup') = 'object' THEN jsonb_build_object('setup', d->'setup') ELSE '{}' END, cap, k);
  IF (x->>'status')::int <> 200 OR NOT x ? 'request' THEN RETURN x; END IF;
  t := replace(x #>> '{request,contents,0,parts,0,text}', said,
         'They dropped their design document "' || nm || '": plan the realm it describes. In "asked", name every thing it asks for, '
         || 'in its order (at most 40), each with the feature that makes it or none.'
         || CASE WHEN coalesce(d->>'text', '') <> '' THEN E'\nThe document:\n' || left(d->>'text', 60000) ELSE '' END);
  parts := jsonb_build_array(jsonb_build_object('text', t));
  IF coalesce(d->>'pdf', '') <> '' THEN
    parts := parts || jsonb_build_array(jsonb_build_object('inlineData', jsonb_build_object('mimeType', 'application/pdf', 'data', d->>'pdf')));
  END IF;
  x := jsonb_set(x, '{request,contents,0,parts}', parts);
  RETURN jsonb_set(x, '{request,generationConfig,maxOutputTokens}', '8000');
END $$;
