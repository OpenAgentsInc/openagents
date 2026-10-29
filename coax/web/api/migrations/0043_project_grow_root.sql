-- A comment's root scope is one tag: the project's address (A) alone; its parent is the address and the
-- 30621 event's id (a and e), as the relay takes a comment on an addressable event (rc-later-drops).

CREATE OR REPLACE FUNCTION drops.grow_next(drop_ text) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE d drops.drops; i drops.grow_items; h record; relay text := drops.repo()->>'relay'; pc text; rc text; tags text[]; id_of jsonb;
        a jsonb; old jsonb; top text; k int := 0; name_ text;
BEGIN
  SELECT * INTO d FROM drops.drops WHERE id = drop_;
  SELECT * INTO i FROM drops.grow_items WHERE drop_id = drop_ AND kind > 0 AND event_id IS NULL ORDER BY seq LIMIT 1;
  IF i.drop_id IS NULL THEN RETURN '{}'; END IF;
  SELECT * INTO h FROM drops.home(d.project, d.project_pubkey) LIMIT 1;
  name_ := coalesce(h.name, d.project);
  pc := '30621:' || d.project_pubkey || ':' || d.project;
  rc := '30617:' || d.project_pubkey || ':' || (drops.repo()->>'d');
  SELECT coalesce(jsonb_object_agg(item, event_id), '{}') INTO id_of FROM drops.grow_items WHERE drop_id = drop_ AND event_id IS NOT NULL;
  SELECT body::jsonb INTO a FROM drops.grow_items WHERE drop_id = drop_ AND item = 'a';
  IF i.kind = 1621 THEN
    IF jsonb_array_length(coalesce(a, '[]')) > 0 THEN
      SELECT array_agg(drops.tag(VARIADIC ARRAY(SELECT jsonb_array_elements_text(t))) ORDER BY o) INTO tags FROM jsonb_array_elements(a) WITH ORDINALITY AS z(t, o);
    ELSE
      tags := ARRAY[drops.tag('a', rc, relay), drops.tag('a', pc, relay)];
    END IF;
    tags := tags || drops.tag('p', d.project_pubkey) || drops.tag('slug', i.item) || drops.tag('subject', i.title);
    IF i.item_kind <> 'task' THEN tags := tags || drops.tag('t', i.item_kind); END IF;
    tags := tags || coalesce((SELECT array_agg(drops.tag('t', l)) FROM unnest(i.labels) l), '{}');
    IF i.size <> '' THEN tags := tags || drops.tag('size', i.size); END IF;
    IF coalesce(i.parent_id, id_of->>i.parent) IS NOT NULL THEN tags := tags || drops.tag('parent', coalesce(i.parent_id, id_of->>i.parent)); END IF;
    tags := tags || drops.tag('alt', 'A ' || i.item_kind || ' in the ' || name_ || ' project: ' || i.title);
  ELSIF i.kind = 1111 THEN   -- NIP-22: a comment on the project itself
    tags := ARRAY[drops.tag('A', pc, relay)]
      || ARRAY[drops.tag('K', '30621'), drops.tag('P', d.project_pubkey), drops.tag('a', pc, relay)]
      || CASE WHEN i.parent_id IS NOT NULL THEN ARRAY[drops.tag('e', i.parent_id, relay, d.project_pubkey)] ELSE '{}' END
      || ARRAY[drops.tag('k', '30621'), drops.tag('p', d.project_pubkey), drops.tag('t', 'design'),
               drops.tag('alt', 'Design notes for the ' || name_ || ' project')];
  ELSE                        -- the order as it was, then the new issues after its last rank
    old := CASE WHEN i.body = '' THEN NULL ELSE i.body::jsonb END;
    IF old IS NULL THEN
      tags := ARRAY[drops.tag('d', d.project), drops.tag('a', pc, relay)];
    ELSE
      SELECT array_agg(drops.tag(VARIADIC ARRAY(SELECT jsonb_array_elements_text(t))) ORDER BY o) INTO tags
      FROM jsonb_array_elements(old) WITH ORDINALITY AS z(t, o) WHERE t->>0 <> 'alt';
      SELECT max((t->>2) COLLATE "C") INTO top FROM jsonb_array_elements(old) t WHERE t->>0 = 'item';
    END IF;
    FOR i IN SELECT * FROM drops.grow_items WHERE drop_id = drop_ AND kind = 1621 ORDER BY seq LOOP
      tags := tags || drops.tag('item', id_of->>i.item, coalesce(top, '1')
        || substr('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz', 2 + k / 61, 1)
        || substr('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz', 2 + k % 61, 1));
      k := k + 1;
    END LOOP;
    tags := tags || drops.tag('alt', 'The ' || name_ || ' project''s task order');
    SELECT * INTO i FROM drops.grow_items WHERE drop_id = drop_ AND item = 'order';
  END IF;
  RETURN jsonb_build_object('seq', i.seq, 'kind', i.kind, 'tags', '[' || array_to_string(tags, ',') || ']',
    'content', CASE WHEN i.kind = 30623 THEN '""' ELSE drops.j(i.body) END);
END $$;
