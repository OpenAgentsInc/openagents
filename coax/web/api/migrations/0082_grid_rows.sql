-- The Grid now runs the homepage's module walked (apps/openagents-web/newland-walk.coax): it has no
-- robots, so its board's robots row switched nothing. Its rows are what that module draws and
-- honours: the other visitors walking it and the projects' names in the air (the names row's id
-- stays, so a switch already made holds). The robots switch goes with its row: nothing dangles.
UPDATE verse.realm_rights SET rows =
  '[{"id": "walkers", "label": "Other visitors walking the Grid", "says": "everyone else walking the Grid now, where they are"}, {"id": "names", "label": "Project names in the air", "says": "each project''s name and how much of it is done, over its tree"}]'::jsonb
WHERE slug = 'the-grid';
UPDATE verse.realm_edit SET switched = switched - 'robots' WHERE slug = 'the-grid';
