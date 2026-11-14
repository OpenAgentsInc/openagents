-- A realm's characters' lore and quests are two content tables of its world (#13526): the world's spec says them as the
-- panes do, a line each ("lore WHO|TEXT", "quest GIVER|ASK|STEP|STEP|STEP|REWARD": verse.edit_panes), after what 0195's
-- verse.realm_world_spec says. metaverse/edit/realm-world.coax makes them the tables `lore` and `quests`; a world edit
-- of a row is carried out by verse.edit_lore or verse.edit_quest in the transaction that saves the world's revision, so
-- the world's undo log takes it back (worlds.c realm_post).
ALTER FUNCTION verse.realm_world_spec(text, text, boolean) RENAME TO realm_world_spec_0195;
CREATE FUNCTION verse.realm_world_spec(s text, k text, site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN out IS NULL THEN NULL ELSE out || coalesce(verse.edit_panes(s, k, site_admin), '') END
  FROM (SELECT verse.realm_world_spec_0195(s, k, site_admin) AS out) x $$;
