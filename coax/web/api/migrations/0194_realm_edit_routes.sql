-- The realm edit layer's routes /switch, /settings and /place go (#13514, after #13506): a row switched, the tools
-- visitors get and a thing placed are world.edit.v1 edits of the realm's world (/api/worlds/<slug>, coax/web/api/worlds.c;
-- the realm's code is metaverse/edit/realm-world.coax), sent by the editor shell, the edit layer's voice, a made
-- realm's page and the CLI (realm world-switch, world-tools, world-place). The world still carries a switch out with
-- verse.edit_switch and verse.edit_settings; a place's rows are 0193's.
--
-- What goes: the place route's SQL (verse.edit_place and verse.builder_place, with the 0110 bodies they wrap), and the
-- three arms of verse.realm_edit_call that sent the routes there. The other arms are as they were: no rule changes here,
-- so they move to a *-policy.coax file when one of them next changes (coax/web/api/COAX.md).
CREATE OR REPLACE FUNCTION verse.realm_edit_call(s text, body jsonb, k text, site_admin boolean, op text, id text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  gate jsonb := verse.realm_edit_begin(s,body,k,site_admin,op);
  result jsonb;
BEGIN
  IF gate->>'status' <> '200' THEN RETURN gate; END IF;
  CASE op
    WHEN 'lore' THEN result := verse.edit_lore(s,body,k,site_admin);
    WHEN 'quest' THEN result := verse.edit_quest(s,body,k,site_admin);
    WHEN 'visibility' THEN result := verse.realm_visibility(s,body,k,site_admin);
    WHEN 'publish' THEN
      IF (body ? 'published' AND jsonb_typeof(body->'published') IS DISTINCT FROM 'boolean')
         OR (body ? 'listed' AND jsonb_typeof(body->'listed') IS DISTINCT FROM 'boolean') THEN
        RETURN '{"status":400,"error":"published and listed are booleans"}';
      END IF;
      result := verse.fork_publish(s,body,k,site_admin);
    WHEN 'ask-delete' THEN result := verse.ask_delete(s,id::bigint,k,site_admin);
    WHEN 'row-delete' THEN result := verse.ask_row_delete(s,id,k,site_admin);
    WHEN 'ask-done' THEN result := verse.ask_done(s,id::bigint,body,site_admin);
    WHEN 'accept' THEN result := verse.edit_decide(s,id::bigint,true,k,site_admin);
    WHEN 'decline' THEN result := verse.edit_decide(s,id::bigint,false,k,site_admin);
    ELSE RETURN '{"status":400,"error":"unknown edit"}';
  END CASE;
  RETURN verse.realm_edit_result(s,gate || jsonb_build_object('op',op),result);
END $$;

DROP FUNCTION verse.edit_place(text, jsonb, text, boolean);
DROP FUNCTION verse.edit_place_0110(text, jsonb, text, boolean);
DROP FUNCTION verse.builder_place(text, jsonb, text);
DROP FUNCTION verse.builder_place_0110(text, jsonb, text);
