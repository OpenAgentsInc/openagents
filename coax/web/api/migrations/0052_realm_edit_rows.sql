-- Every realm edits the way Build your own realm does (docs/verse/realm-edit.md): a board offers only
-- what its realm's code really switches. Dead Gears' support engineer is part of the fight's rules
-- (verse/deadgears/rules.coax: the repairs, the cover, the waves all reach for it), so its row goes.
UPDATE verse.realm_rights
   SET rows = COALESCE((SELECT jsonb_agg(r ORDER BY o) FROM jsonb_array_elements(rows) WITH ORDINALITY t(r, o) WHERE r->>'id' <> 'support'), '[]'::jsonb)
 WHERE slug = 'dead-gears';
UPDATE verse.realm_edit SET switched = switched - 'support' WHERE slug = 'dead-gears' AND switched ? 'support';
