-- Private projects keep the lighter parts of Scrum (svc-sprints,
-- docs/private-projects.md): sprints, an ordered backlog with stretch and
-- carried-over items, done with evidence, a limit on work in progress, and a
-- log of priority changes. All optional in openagents.private-project.v1:
--   sprints: [{id, goal, start, end, status: planned|active|closed, agreed_by: [names], review, retro}]
--   backlog: [{id, text, status: todo|doing|done, evidence, stretch, carried_from, proposed_by}], in priority order
--   wip_limit: how many backlog items may be doing at once
--   priority_changes: [{at, what, why, proposed_by, agreed_by: [names]}]
-- An agent may add backlog items it proposes (proposed_by), move an item's
-- status with evidence, and propose priority changes (no agreed_by); it may
-- not change a sprint, the order or text of the backlog, or a change members
-- agreed. The project's agent (runner) may not change a sprint's goal.
CREATE OR REPLACE FUNCTION projects.invalid_sprints(doc jsonb) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  names text[] := (SELECT coalesce(array_agg(m->>'name'), '{}') FROM jsonb_array_elements(doc->'members') m);
  x jsonb;
  who text[];
  active int := 0;
BEGIN
  FOREACH x IN ARRAY ARRAY[coalesce(doc->'sprints', '[]'), coalesce(doc->'backlog', '[]'), coalesce(doc->'priority_changes', '[]')] LOOP
    IF jsonb_typeof(x) <> 'array' THEN RETURN 'sprints, backlog and priority_changes are lists'; END IF;
  END LOOP;
  IF doc ? 'wip_limit' AND (jsonb_typeof(doc->'wip_limit') <> 'number' OR (doc->>'wip_limit')::numeric NOT IN (1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20)) THEN
    RETURN 'wip_limit is a whole number from 1 to 20'; END IF;
  IF (SELECT count(DISTINCT s->>'id') <> count(*) FROM jsonb_array_elements(coalesce(doc->'sprints', '[]')) s) THEN RETURN 'a sprint id is listed twice'; END IF;
  FOR x IN SELECT * FROM jsonb_array_elements(coalesce(doc->'sprints', '[]')) LOOP
    IF coalesce(x->>'id', '') = '' OR coalesce(x->>'goal', '') = '' THEN RETURN 'every sprint needs an id and one goal'; END IF;
    IF coalesce(x->>'start', '') !~ '^\d{4}-\d{2}-\d{2}$' OR coalesce(x->>'end', '') !~ '^\d{4}-\d{2}-\d{2}$' OR x->>'end' < x->>'start' THEN
      RETURN 'a sprint has start and end dates, YYYY-MM-DD, end not before start'; END IF;
    IF coalesce(x->>'status', '') NOT IN ('planned', 'active', 'closed') THEN RETURN 'a sprint is planned, active or closed'; END IF;
    IF x->>'status' = 'active' THEN active := active + 1; END IF;
    who := (SELECT coalesce(array_agg(a), '{}') FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(x->'agreed_by') = 'array' THEN x->'agreed_by' ELSE '[]' END) a);
    IF cardinality(who) = 0 OR NOT names @> who THEN RETURN 'a sprint''s goal names who agreed in agreed_by, members only'; END IF;
  END LOOP;
  IF active > 1 THEN RETURN 'one sprint is active at a time'; END IF;
  IF (SELECT count(DISTINCT b->>'id') <> count(*) FROM jsonb_array_elements(coalesce(doc->'backlog', '[]')) b) THEN RETURN 'a backlog id is listed twice'; END IF;
  FOR x IN SELECT * FROM jsonb_array_elements(coalesce(doc->'backlog', '[]')) LOOP
    IF coalesce(x->>'id', '') = '' OR coalesce(x->>'text', '') = '' THEN RETURN 'every backlog item needs an id and text'; END IF;
    IF coalesce(x->>'status', '') NOT IN ('todo', 'doing', 'done') THEN RETURN 'a backlog item is todo, doing or done'; END IF;
    IF x->>'status' = 'done' AND coalesce(x->>'evidence', '') = '' THEN RETURN 'a done backlog item says what shows it is done (evidence)'; END IF;
    IF x ? 'stretch' AND jsonb_typeof(x->'stretch') <> 'boolean' THEN RETURN 'stretch is true or false'; END IF;
    IF x ? 'carried_from' AND coalesce(x->>'carried_from', '') !~ '^\d{4}-\d{2}-\d{2}$' THEN RETURN 'carried_from is a date, YYYY-MM-DD'; END IF;
  END LOOP;
  IF doc ? 'wip_limit' AND (SELECT count(*) FROM jsonb_array_elements(coalesce(doc->'backlog', '[]')) b WHERE b->>'status' = 'doing') > (doc->>'wip_limit')::int THEN
    RETURN 'more backlog items are doing than wip_limit allows'; END IF;
  FOR x IN SELECT * FROM jsonb_array_elements(coalesce(doc->'priority_changes', '[]')) LOOP
    IF coalesce(x->>'at', '') !~ '^\d{4}-\d{2}-\d{2}' OR coalesce(x->>'what', '') = '' OR coalesce(x->>'why', '') = '' OR coalesce(x->>'proposed_by', '') = '' THEN
      RETURN 'a priority change says when (at), what, why and who proposed it'; END IF;
    who := (SELECT coalesce(array_agg(a), '{}') FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(x->'agreed_by') = 'array' THEN x->'agreed_by' ELSE '[]' END) a);
    IF NOT names @> who THEN RETURN 'a priority change is agreed by members only'; END IF;
  END LOOP;
  RETURN NULL;
END $$;

-- The checks before (0008), then the sprint parts.
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace WHERE n.nspname = 'projects' AND p.proname = 'invalid_core') THEN
    ALTER FUNCTION projects.invalid(jsonb, text) RENAME TO invalid_core;
  END IF;
END $$;
CREATE OR REPLACE FUNCTION projects.invalid(doc jsonb, want text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(projects.invalid_core(doc, want), projects.invalid_sprints(doc))
$$;

-- What an agent may not change (0007), now with the sprint parts: every
-- sprint; the backlog's members' items, in order, but for status and evidence;
-- the priority changes members agreed.
CREATE OR REPLACE FUNCTION projects.fixed(doc jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object(
    'rest', doc - 'days' - 'checklists' - 'goals' - 'backlog' - 'priority_changes',
    'goals', (SELECT coalesce(jsonb_agg(g ORDER BY i), '[]') FROM jsonb_array_elements(coalesce(doc->'goals', '[]')) WITH ORDINALITY AS z(g, i)
              WHERE g->>'status' = 'agreed'),
    'days', (SELECT coalesce(jsonb_agg(jsonb_build_object('day', d - 'milestones', 'milestones',
               (SELECT coalesce(jsonb_agg(m - 'status' - 'note' ORDER BY o), '[]') FROM jsonb_array_elements(coalesce(d->'milestones', '[]')) WITH ORDINALITY AS x(m, o)))
             ORDER BY i), '[]') FROM jsonb_array_elements(coalesce(doc->'days', '[]')) WITH ORDINALITY AS y(d, i)),
    'checklists', (SELECT coalesce(jsonb_agg(jsonb_build_object('list', c - 'items', 'items',
               (SELECT coalesce(jsonb_agg(t - 'status' - 'evidence' ORDER BY o), '[]') FROM jsonb_array_elements(coalesce(c->'items', '[]')) WITH ORDINALITY AS x(t, o)))
             ORDER BY i), '[]') FROM jsonb_array_elements(coalesce(doc->'checklists', '[]')) WITH ORDINALITY AS y(c, i)),
    'backlog', (SELECT coalesce(jsonb_agg(b - 'status' - 'evidence' ORDER BY i), '[]') FROM jsonb_array_elements(coalesce(doc->'backlog', '[]')) WITH ORDINALITY AS z(b, i)
                WHERE coalesce(b->>'proposed_by', '') = ''),
    'priority_changes', (SELECT coalesce(jsonb_agg(c ORDER BY i), '[]') FROM jsonb_array_elements(coalesce(doc->'priority_changes', '[]')) WITH ORDINALITY AS z(c, i)
                         WHERE jsonb_typeof(c->'agreed_by') = 'array' AND jsonb_array_length(c->'agreed_by') > 0))
$$;

-- What the project's agent may not change (0013), now with each sprint's goal and who agreed to it.
CREATE OR REPLACE FUNCTION projects.kept(doc jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('v', doc->'v', 'slug', doc->'slug', 'title', doc->'title', 'members', doc->'members',
    'goals', (SELECT coalesce(jsonb_agg(g ORDER BY i), '[]') FROM jsonb_array_elements(coalesce(doc->'goals', '[]')) WITH ORDINALITY AS z(g, i)
              WHERE g->>'status' = 'agreed'),
    'sprint_goals', (SELECT coalesce(jsonb_agg(jsonb_build_object('id', s->'id', 'goal', s->'goal', 'agreed_by', s->'agreed_by') ORDER BY i), '[]')
                     FROM jsonb_array_elements(coalesce(doc->'sprints', '[]')) WITH ORDINALITY AS z(s, i)))
$$;
