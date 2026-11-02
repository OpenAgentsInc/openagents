-- Realms are worlds (2026-11-02). The site serves its realms/ pages at /worlds (coax/web/api/lb.sh: the
-- bucket keeps its realms/ names, and /realms/* moves for good to /worlds/*), so what this database
-- writes for people says world and links to /worlds: made realms' and forks' pages and Markdown
-- (written again on start: builder_page_rev 3), Everglade replies' pages, the drop's first world,
-- link previews' addresses, the errors the routes answer and the voice prompts. Tables, columns,
-- routes (/api/playtest/realms/...), JSON fields and the og table's path column (the site's
-- directory) keep their names. Each function keeps its old body under NAME_0110 and is wrapped.

-- An answer's error, said with world: the exact error strings only.
CREATE FUNCTION verse.worlds_text(t text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(t, '"not a realm"', '"not a world"'), '"this claim link isn''t for this realm"', '"this claim link isn''t for this world"'), '"nobody has claimed this realm yet"', '"nobody has claimed this world yet"'), '"nobody has claimed this realm"', '"nobody has claimed this world"'), '"no such realm"', '"no such world"'), '"the realm is full"', '"the world is full"'), '"this realm has lore for 40 characters"', '"this world has lore for 40 characters"'), '"this realm has 40 quests"', '"this world has 40 quests"'), '"that''s today''s realms"', '"that''s today''s worlds"'), '"only approved creators and admins change who sees a realm"', '"only approved creators and admins change who sees a world"'), '"that realm has no revisions"', '"that world has no revisions"'), '"this realm takes no voice edits"', '"this world takes no voice edits"'), '"this realm takes no voice"', '"this world takes no voice"'), '"this realm can''t be forked"', '"this world can''t be forked"') $$;
CREATE FUNCTION verse.worlds_answer(r jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(r) = 'object' AND r ? 'error' THEN verse.worlds_text(r::text)::jsonb ELSE r END $$;
-- A plpgsql body that names its own parameters (voice_edit_ask.pubkey) is written again under its new name.
CREATE FUNCTION verse.worlds_requalify(f regprocedure, old text) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  EXECUTE replace(pg_get_functiondef(f), old || '.', (SELECT proname FROM pg_proc WHERE oid = f) || '.');
END $$;
-- A page or Markdown the site serves: its links to /realms/... go to /worlds/....
CREATE FUNCTION verse.worlds_page(p text, origin text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT replace(replace(replace(p, origin || '/realms/', origin || '/worlds/'), '"/realms/', '"/worlds/'),
                 'https://openagents.com/realms/', 'https://openagents.com/worlds/') $$;

-- its errors say world
ALTER FUNCTION makers.claim(text, text, text) RENAME TO claim_0110;
CREATE FUNCTION makers.claim(r text, pk text, body text) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(makers.claim_0110(r, pk, body)) $$;
-- its errors say world
ALTER FUNCTION makers.edit(text, text, text) RENAME TO edit_0110;
CREATE FUNCTION makers.edit(r text, pk text, body text) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(makers.edit_0110(r, pk, body)) $$;
-- its errors say world
ALTER FUNCTION makers.gift(text, bigint, bigint) RENAME TO gift_0110;
CREATE FUNCTION makers.gift(r text, n bigint, max_sats bigint) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(makers.gift_0110(r, n, max_sats)) $$;
-- its errors say world
ALTER FUNCTION makers.make_code(text, text, text) RENAME TO make_code_0110;
CREATE FUNCTION makers.make_code(r text, code text, admin text) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(makers.make_code_0110(r, code, admin)) $$;
-- its errors say world
ALTER FUNCTION makers.tip_issue(text, bigint, text, text, bigint, bigint, boolean) RENAME TO tip_issue_0110;
CREATE FUNCTION makers.tip_issue(r text, n bigint, inv text, hash text, amount_msat bigint, expires_unix bigint, is_team boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(makers.tip_issue_0110(r, n, inv, hash, amount_msat, expires_unix, is_team)) $$;
-- its errors say world
ALTER FUNCTION makers.tip_terms(text, text, bigint, bigint) RENAME TO tip_terms_0110;
CREATE FUNCTION makers.tip_terms(r text, body text, min_sats bigint, max_sats bigint) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT verse.worlds_answer(makers.tip_terms_0110(r, body, min_sats, max_sats)) $$;
-- its errors say world
ALTER FUNCTION verse.ask_delete(text, bigint, text, boolean) RENAME TO ask_delete_0110;
CREATE FUNCTION verse.ask_delete(s text, aid bigint, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.ask_delete_0110(s, aid, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.ask_row_delete(text, text, text, boolean) RENAME TO ask_row_delete_0110;
CREATE FUNCTION verse.ask_row_delete(s text, key text, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.ask_row_delete_0110(s, key, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.builder_decide(text, bigint, boolean, text) RENAME TO builder_decide_0110;
CREATE FUNCTION verse.builder_decide(s text, pid bigint, accept boolean, k text) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.builder_decide_0110(s, pid, accept, k)) $$;
-- its errors say world
ALTER FUNCTION verse.builder_describe(text, jsonb, text, boolean) RENAME TO builder_describe_0110;
CREATE FUNCTION verse.builder_describe(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.builder_describe_0110(s, body, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.builder_place(text, jsonb, text) RENAME TO builder_place_0110;
CREATE FUNCTION verse.builder_place(s text, body jsonb, k text) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.builder_place_0110(s, body, k)) $$;
-- its errors say world
ALTER FUNCTION verse.builder_settings(text, jsonb, text) RENAME TO builder_settings_0110;
CREATE FUNCTION verse.builder_settings(s text, body jsonb, k text) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.builder_settings_0110(s, body, k)) $$;
-- its errors say world
ALTER FUNCTION verse.edit_lore(text, jsonb, text, boolean) RENAME TO edit_lore_0110;
CREATE FUNCTION verse.edit_lore(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.edit_lore_0110(s, body, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.edit_place(text, jsonb, text, boolean) RENAME TO edit_place_0110;
CREATE FUNCTION verse.edit_place(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.edit_place_0110(s, body, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.edit_quest(text, jsonb, text, boolean) RENAME TO edit_quest_0110;
CREATE FUNCTION verse.edit_quest(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.edit_quest_0110(s, body, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.edit_settings(text, jsonb, text, boolean) RENAME TO edit_settings_0110;
CREATE FUNCTION verse.edit_settings(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.edit_settings_0110(s, body, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.edit_switch(text, jsonb, text, boolean) RENAME TO edit_switch_0110;
CREATE FUNCTION verse.edit_switch(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.edit_switch_0110(s, body, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.fork_publish(text, jsonb, text, boolean) RENAME TO fork_publish_0110;
CREATE FUNCTION verse.fork_publish(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.fork_publish_0110(s, body, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.made_publish(text, jsonb, text, boolean) RENAME TO made_publish_0110;
CREATE FUNCTION verse.made_publish(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.made_publish_0110(s, body, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.npc_ask(jsonb, int, text) RENAME TO npc_ask_0110;
CREATE FUNCTION verse.npc_ask(body jsonb, cap int, k text) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.npc_ask_0110(body, cap, k)) $$;
-- its errors say world
ALTER FUNCTION verse.realm_doc_of(text, text, boolean) RENAME TO realm_doc_of_0110;
CREATE FUNCTION verse.realm_doc_of(s text, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT verse.worlds_answer(verse.realm_doc_of_0110(s, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.realm_edit_begin(text, jsonb, text, boolean, text) RENAME TO realm_edit_begin_0110;
CREATE FUNCTION verse.realm_edit_begin(s text, body jsonb, k text, site_admin boolean, op text) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.realm_edit_begin_0110(s, body, k, site_admin, op)) $$;
-- its errors say world
ALTER FUNCTION verse.realm_edit_finish(jsonb, int, text, text, int, text) RENAME TO realm_edit_finish_0110;
CREATE FUNCTION verse.realm_edit_finish(asked jsonb, http int, response text, model text, taken int, op text) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.realm_edit_finish_0110(asked, http, response, model, taken, op)) $$;
-- its errors say world
ALTER FUNCTION verse.realm_save(text, jsonb, text, boolean) RENAME TO realm_save_0110;
CREATE FUNCTION verse.realm_save(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.realm_save_0110(s, body, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.realm_set_visibility(text, text, text, boolean) RENAME TO realm_set_visibility_0110;
CREATE FUNCTION verse.realm_set_visibility(s text, v text, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.realm_set_visibility_0110(s, v, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.realm_undo(text, jsonb, text, boolean) RENAME TO realm_undo_0110;
CREATE FUNCTION verse.realm_undo(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.realm_undo_0110(s, body, k, site_admin)) $$;
-- its errors say world
ALTER FUNCTION verse.voice_edit_ask(text, jsonb, text, int) RENAME TO voice_edit_ask_0110;
SELECT verse.worlds_requalify('verse.voice_edit_ask_0110(text, jsonb, text, int)'::regprocedure, 'voice_edit_ask');
CREATE FUNCTION verse.voice_edit_ask(realm text, body jsonb, pubkey text, cap int) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.worlds_answer(verse.voice_edit_ask_0110(realm, body, pubkey, cap)) $$;

-- a made realm's, an Everglade fork's and a reply's paths are /worlds/...
ALTER FUNCTION verse.builder_make(jsonb, text, int) RENAME TO builder_make_0110;
CREATE FUNCTION verse.builder_make(body jsonb, k text, per_day int) RETURNS jsonb LANGUAGE sql AS $$
  SELECT replace(verse.worlds_answer(verse.builder_make_0110(body, k, per_day))::text, '"/realms/', '"/worlds/')::jsonb $$;
ALTER FUNCTION verse.fork_make(text, jsonb, text, int) RENAME TO fork_make_0110;
CREATE FUNCTION verse.fork_make(s text, body jsonb, k text, per_day int) RETURNS jsonb LANGUAGE sql AS $$
  SELECT replace(verse.worlds_answer(verse.fork_make_0110(s, body, k, per_day))::text, '"/realms/', '"/worlds/')::jsonb $$;
ALTER FUNCTION verse.reply_make(jsonb, text, int) RENAME TO reply_make_0110;
CREATE FUNCTION verse.reply_make(b jsonb, k text, per_day int) RETURNS jsonb LANGUAGE sql AS $$
  SELECT replace(verse.worlds_answer(verse.reply_make_0110(b, k, per_day))::text, '"/realms/', '"/worlds/')::jsonb $$;
-- its errors say world, and the fork's path is /worlds/<slug>
ALTER FUNCTION verse.made_fork(text, jsonb, text, int) RENAME TO made_fork_0110;
CREATE FUNCTION verse.made_fork(s text, body jsonb, k text, per_day int) RETURNS jsonb LANGUAGE sql AS $$
  SELECT replace(verse.worlds_answer(verse.made_fork_0110(s, body, k, per_day))::text, '"/realms/', '"/worlds/')::jsonb $$;
-- its errors and its prompt say world
ALTER FUNCTION verse.voice_look_ask(text, jsonb, text, int) RENAME TO voice_look_ask_0110;
SELECT verse.worlds_requalify('verse.voice_look_ask_0110(text, jsonb, text, int)'::regprocedure, 'voice_look_ask');
CREATE FUNCTION verse.voice_look_ask(realm text, body jsonb, pubkey text, cap int) RETURNS jsonb LANGUAGE sql AS $$
  SELECT replace(verse.worlds_answer(verse.voice_look_ask_0110(realm, body, pubkey, cap))::text, ', a realm in the Verse: a small 3D world drawn live', ', a world in the Verse: a small 3D scene drawn live')::jsonb $$;

-- a made realm's page links to /worlds
ALTER FUNCTION verse.builder_page(text, text) RENAME TO builder_page_0110;
CREATE FUNCTION verse.builder_page(s text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT verse.worlds_page(verse.builder_page_0110(s, origin), origin) $$;
-- a fork's page links to /worlds
ALTER FUNCTION verse.fork_page(text, text) RENAME TO fork_page_0110;
CREATE FUNCTION verse.fork_page(s text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT verse.worlds_page(verse.fork_page_0110(s, origin), origin) $$;
-- a reply's page links to /worlds
ALTER FUNCTION verse.reply_page(text, text) RENAME TO reply_page_0110;
CREATE FUNCTION verse.reply_page(i text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT verse.worlds_page(verse.reply_page_0110(i, origin), origin) $$;
-- Markdown links to /worlds
ALTER FUNCTION verse.builder_md(text) RENAME TO builder_md_0110;
CREATE FUNCTION verse.builder_md(s text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT verse.worlds_page(verse.builder_md_0110(s), 'https://openagents.com') $$;
-- Markdown links to /worlds
ALTER FUNCTION verse.fork_md(text) RENAME TO fork_md_0110;
CREATE FUNCTION verse.fork_md(s text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT verse.worlds_page(verse.fork_md_0110(s), 'https://openagents.com') $$;
-- Markdown links to /worlds
ALTER FUNCTION verse.reply_md(text) RENAME TO reply_md_0110;
CREATE FUNCTION verse.reply_md(i text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT verse.worlds_page(verse.reply_md_0110(i), 'https://openagents.com') $$;
-- the picture's address (its object stays realms/<slug>/card.jpg)
ALTER FUNCTION verse.builder_card_path(verse.made_realms) RENAME TO builder_card_path_0110;
CREATE FUNCTION verse.builder_card_path(m verse.made_realms) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT regexp_replace(verse.builder_card_path_0110(m), '^/realms/', '/worlds/') $$;
-- the picture's address
ALTER FUNCTION verse.reply_card_path(verse.replies) RENAME TO reply_card_path_0110;
CREATE FUNCTION verse.reply_card_path(r verse.replies) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT regexp_replace(verse.reply_card_path_0110(r), '^/realms/', '/worlds/') $$;
-- /og's rows: the address is /worlds/<slug> (the path column stays the site's directory)
ALTER FUNCTION verse.builder_og_rows(int) RENAME TO builder_og_rows_0110;
CREATE FUNCTION verse.builder_og_rows(n int) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT replace(verse.builder_og_rows_0110(n), '|https://openagents.com/realms/', '|https://openagents.com/worlds/') $$;
-- a made realm's line
ALTER FUNCTION verse.builder_line_of(verse.made_realms) RENAME TO builder_line_of_0110;
CREATE FUNCTION verse.builder_line_of(m verse.made_realms) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT replace(verse.builder_line_of_0110(m), ' A realm built on the Verse.', ' A world built on the Verse.') $$;
-- a reply's line
ALTER FUNCTION verse.reply_line() RENAME TO reply_line_0110;
CREATE FUNCTION verse.reply_line() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT replace(verse.reply_line_0110(), 'reply with a realm of your own.', 'reply with a world of your own.') $$;
-- a drop's first world
ALTER FUNCTION drops.realm_page(text, text) RENAME TO realm_page_0110;
CREATE FUNCTION drops.realm_page(slug_ text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT replace(replace(drops.realm_page_0110(slug_, origin), 'first realm', 'first world'), '"/realms/drop"', '"/worlds/drop"') $$;
-- a drop's first world
ALTER FUNCTION drops.realm_md(text) RENAME TO realm_md_0110;
CREATE FUNCTION drops.realm_md(slug_ text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT replace(drops.realm_md_0110(slug_), 'first realm', 'first world') $$;
-- a drop's first world
ALTER FUNCTION drops.publish_md(text) RENAME TO publish_md_0110;
CREATE FUNCTION drops.publish_md(slug_ text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT replace(drops.publish_md_0110(slug_), 'first realm', 'first world') $$;
-- the voice prompt says world, so its replies do
ALTER FUNCTION verse.voice_edit_prompt(text) RENAME TO voice_edit_prompt_0110;
CREATE FUNCTION verse.voice_edit_prompt(realm text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT replace(replace(replace(replace(verse.voice_edit_prompt_0110(realm),
    ' is a realm in the Verse (a small 3D world) and they stand in it. ', ' is a world in the Verse (a small 3D scene) and they stand in it. '),
    'what is in this realm comes from the ', 'what is in this world comes from the '),
    E'\nThe realm''s switches', E'\nThe world''s switches'),
    'what''s in this realm? => none', 'what''s in this world? => none') $$;
-- the voice's verbs say world
ALTER FUNCTION verse.voice_edit_generic() RENAME TO voice_edit_generic_0110;
CREATE FUNCTION verse.voice_edit_generic() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT replace(replace(verse.voice_edit_generic_0110()::text, ' realm''s ', ' world''s '), 'the realm changed', 'the world changed')::jsonb $$;
-- the hand editor's prompt says world
ALTER FUNCTION verse.edit_hand_prompt(text) RENAME TO edit_hand_prompt_0110;
CREATE FUNCTION verse.edit_hand_prompt(s text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT replace(replace(verse.edit_hand_prompt_0110(s), 'You edit a realm of the Verse, a 3D world in the browser, for its owner. The realm is built by hand; ', 'You edit a world of the Verse, a 3D scene in the browser, for its owner. The world is built by hand; '), 'this realm', 'this world') $$;

-- Every made realm's page is written again with these (realms_refresh_pages).
CREATE OR REPLACE FUNCTION verse.builder_page_rev() RETURNS int LANGUAGE sql IMMUTABLE AS $$ SELECT 3 $$;
