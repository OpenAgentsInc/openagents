-- Private projects: an agreed goal names who agreed (one or more members),
-- not necessarily every member: a goal one member set before the others
-- joined stays agreed by that member, and the page shows who. Agents still
-- never change or remove an agreed goal (projects.fixed).
CREATE OR REPLACE FUNCTION projects.invalid(doc jsonb, want text) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  names text[];
  agreed text[];
  x jsonb; y jsonb;
BEGIN
  IF jsonb_typeof(doc) <> 'object' THEN RETURN 'the document is not an object'; END IF;
  IF doc->>'v' IS DISTINCT FROM 'openagents.private-project.v1' THEN RETURN 'v must be openagents.private-project.v1'; END IF;
  IF doc->>'slug' IS DISTINCT FROM want THEN RETURN 'slug must be this project''s'; END IF;
  IF coalesce(doc->>'title', '') = '' THEN RETURN 'a title is needed'; END IF;
  IF jsonb_typeof(doc->'members') <> 'array' OR jsonb_array_length(doc->'members') = 0 THEN RETURN 'members must list someone'; END IF;
  FOR x IN SELECT * FROM jsonb_array_elements(doc->'members') LOOP
    IF coalesce(x->>'name', '') = '' THEN RETURN 'every member needs a name'; END IF;
    IF coalesce(x->>'pubkey', '') !~ '^[0-9a-f]{64}$' THEN RETURN 'every member needs a pubkey (64 lowercase hex)'; END IF;
  END LOOP;
  IF (SELECT count(DISTINCT m->>'pubkey') FROM jsonb_array_elements(doc->'members') m) <> jsonb_array_length(doc->'members') THEN
    RETURN 'a pubkey is listed twice'; END IF;
  SELECT array_agg(m->>'name') INTO names FROM jsonb_array_elements(doc->'members') m;
  FOREACH x IN ARRAY ARRAY[coalesce(doc->'rules', '[]'), coalesce(doc->'goals', '[]'), coalesce(doc->'days', '[]'),
                           coalesce(doc->'checklists', '[]'), coalesce(doc->'links', '[]')] LOOP
    IF jsonb_typeof(x) <> 'array' THEN RETURN 'rules, goals, days, checklists and links are lists'; END IF;
  END LOOP;
  FOR x IN SELECT * FROM jsonb_array_elements(coalesce(doc->'goals', '[]')) LOOP
    IF coalesce(x->>'id', '') = '' OR coalesce(x->>'text', '') = '' THEN RETURN 'every goal needs an id and text'; END IF;
    IF x->>'status' NOT IN ('agreed', 'proposed') THEN RETURN 'a goal is agreed or proposed'; END IF;
    IF x->>'status' = 'agreed' THEN
      agreed := (SELECT coalesce(array_agg(a), '{}') FROM jsonb_array_elements_text(
                   CASE WHEN jsonb_typeof(x->'agreed_by') = 'array' THEN x->'agreed_by' ELSE '[]' END) a);
      IF cardinality(agreed) = 0 OR NOT names @> agreed THEN
        RETURN 'an agreed goal names who agreed in agreed_by, members only'; END IF;
    END IF;
  END LOOP;
  FOR x IN SELECT * FROM jsonb_array_elements(coalesce(doc->'days', '[]')) LOOP
    IF coalesce(x->>'date', '') !~ '^\d{4}-\d{2}-\d{2}$' THEN RETURN 'a day''s date is YYYY-MM-DD'; END IF;
    IF jsonb_typeof(coalesce(x->'milestones', '[]')) <> 'array' THEN RETURN 'milestones are a list'; END IF;
    FOR y IN SELECT * FROM jsonb_array_elements(coalesce(x->'milestones', '[]')) LOOP
      IF coalesce(y->>'id', '') = '' OR coalesce(y->>'text', '') = '' THEN RETURN 'every milestone needs an id and text'; END IF;
      IF y->>'status' NOT IN ('todo', 'doing', 'done') THEN RETURN 'a milestone is todo, doing or done'; END IF;
    END LOOP;
  END LOOP;
  FOR x IN SELECT * FROM jsonb_array_elements(coalesce(doc->'checklists', '[]')) LOOP
    IF coalesce(x->>'id', '') = '' OR jsonb_typeof(coalesce(x->'items', '[]')) <> 'array' THEN RETURN 'every checklist needs an id and items'; END IF;
    FOR y IN SELECT * FROM jsonb_array_elements(coalesce(x->'items', '[]')) LOOP
      IF coalesce(y->>'id', '') = '' OR coalesce(y->>'text', '') = '' THEN RETURN 'every item needs an id and text'; END IF;
      IF y->>'status' NOT IN ('todo', 'doing', 'done') THEN RETURN 'an item is todo, doing or done'; END IF;
    END LOOP;
  END LOOP;
  RETURN NULL;
END $$;
