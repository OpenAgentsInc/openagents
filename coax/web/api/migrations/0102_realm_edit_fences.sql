-- Optional compare-and-write for the CLI edit surface (#12635). Old bodies without rev keep
-- their old results and behavior. A fenced request locks the made realm through its mutation;
-- a model request carries the admitted revision and checks it again when the answer arrives.
-- This orders side-table edits too; it does not extend the v1 document's existing undo coverage.
-- Only server-created completion metadata crosses the model boundary, never model/client fields.

-- The edit-v1 route family requires a revision; the legacy family keeps it optional. This
-- check only establishes presence: realm_edit_begin validates and compares under the row lock.
CREATE FUNCTION verse.realm_edit_required(body jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(body)='object' AND body ? 'rev' THEN '{"status":200}'::jsonb
              ELSE '{"status":400,"error":"a revision"}'::jsonb END $$;

CREATE FUNCTION verse.realm_edit_begin(s text, body jsonb, k text, site_admin boolean, op text)
RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  role text;
  expected numeric;
  actual bigint;
BEGIN
  IF NOT (body ? 'rev') THEN RETURN '{"status":200}'; END IF;
  role := verse.edit_role(s, k, site_admin);
  IF role IS NULL OR verse.realm_hidden(s, k, site_admin) THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF (op = 'ask-done' AND NOT coalesce(site_admin, false))
     OR (op NOT IN ('place', 'ask-done') AND role NOT IN ('owner', 'admin')) THEN
    RETURN '{"status":403,"error":"only its owner and admins"}';
  END IF;
  IF jsonb_typeof(body->'rev') IS DISTINCT FROM 'number' THEN RETURN '{"status":400,"error":"a revision"}'; END IF;
  expected := (body->>'rev')::numeric;
  IF expected < 1 OR expected > 9223372036854775807 OR expected <> trunc(expected) THEN
    RETURN '{"status":400,"error":"a revision"}';
  END IF;
  SELECT rev INTO actual FROM verse.made_realms WHERE slug = s FOR UPDATE;
  IF actual IS NULL THEN RETURN '{"status":400,"error":"that realm has no revisions"}'; END IF;
  IF expected <> actual THEN RETURN jsonb_build_object('status',409,'error','changed since','rev',actual); END IF;
  RETURN jsonb_build_object('status',200,'rev',actual);
END $$;

-- Existing made_realms triggers advance document writes. Side-table-only fenced operations
-- consume a revision as well, so a second writer cannot reuse their prior observation.
CREATE FUNCTION verse.realm_edit_result(s text, gate jsonb, result jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  current_rev bigint;
  restoring text;
  decline_step boolean := gate->>'op' = 'decline'
    AND EXISTS (SELECT 1 FROM verse.made_realms WHERE slug=s AND base IS NULL);
BEGIN
  IF NOT (gate ? 'rev') OR result->>'status' <> '200' THEN RETURN result; END IF;
  -- 0094 already pushed a made realm's declined card as an undo step. Give it a revision
  -- without also pushing a second copy of that step through the document trigger.
  IF decline_step THEN
    restoring := current_setting('verse.restoring',true);
    PERFORM set_config('verse.restoring','on',true);
  END IF;
  -- Publication freezes this revision; it is not a document edit that invents another one.
  IF gate->>'op' IS DISTINCT FROM 'publish' THEN
    UPDATE verse.made_realms SET rev = rev + 1 WHERE slug = s AND rev = (gate->>'rev')::bigint;
  END IF;
  IF decline_step THEN PERFORM set_config('verse.restoring',coalesce(restoring,''),true); END IF;
  SELECT rev INTO current_rev FROM verse.made_realms WHERE slug = s;
  result := result || jsonb_build_object('rev',current_rev);
  IF result ? 'lines' THEN result := jsonb_set(result,'{lines}',to_jsonb(coalesce(result->>'lines','') || E'rev ' || current_rev || E'\n')); END IF;
  RETURN result;
END $$;

-- The C host forwards the typed route and body; every admission/mutation decision stays SQL's.
CREATE FUNCTION verse.realm_edit_call(s text, body jsonb, k text, site_admin boolean, op text, id text)
RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  gate jsonb := verse.realm_edit_begin(s,body,k,site_admin,op);
  result jsonb;
BEGIN
  IF gate->>'status' <> '200' THEN RETURN gate; END IF;
  CASE op
    WHEN 'switch' THEN result := verse.edit_switch(s,body,k,site_admin);
    WHEN 'settings' THEN result := verse.edit_settings(s,body,k,site_admin);
    WHEN 'place' THEN result := verse.edit_place(s,body,k,site_admin);
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

CREATE FUNCTION verse.realm_edit_start(s text, body jsonb, cap int, k text, site_admin boolean, op text)
RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  gate jsonb := verse.realm_edit_begin(s,body,k,site_admin,op);
  result jsonb;
BEGIN
  IF gate->>'status' <> '200' THEN RETURN gate; END IF;
  CASE op
    WHEN 'revise' THEN result := verse.edit_revise_ask(s,body,cap,k,site_admin);
    WHEN 'ask' THEN result := verse.ask_add(s,body,cap,k,site_admin);
    WHEN 'ask-doc' THEN result := verse.ask_doc_ask(s,body,cap,k,site_admin);
    ELSE RETURN '{"status":400,"error":"unknown edit"}';
  END CASE;
  IF NOT (gate ? 'rev') OR result->>'status' <> '200' THEN RETURN result; END IF;
  IF NOT (result ? 'request') THEN RETURN verse.realm_edit_result(s,gate,result); END IF;
  RETURN result || jsonb_build_object('edit_fence',jsonb_build_object('slug',s,'rev',gate->'rev'));
END $$;

CREATE FUNCTION verse.realm_edit_finish(asked jsonb, http int, response text, model text, taken int, op text)
RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  s text;
  expected bigint;
  actual bigint;
  gate jsonb := '{"status":200}';
  result jsonb;
BEGIN
  IF asked ? 'edit_fence' THEN
    SELECT slug INTO s FROM verse.realm_plans WHERE id = (asked->>'id')::bigint;
    IF s IS NULL OR s IS DISTINCT FROM asked #>> '{edit_fence,slug}' THEN RETURN '{"status":400,"error":"no such request"}'; END IF;
    expected := (asked #>> '{edit_fence,rev}')::bigint;
    SELECT rev INTO actual FROM verse.made_realms WHERE slug = s FOR UPDATE;
    IF actual IS NULL THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
    IF actual <> expected THEN RETURN jsonb_build_object('status',409,'error','changed since','rev',actual); END IF;
    gate := jsonb_build_object('status',200,'rev',actual);
  END IF;
  CASE op
    WHEN 'revise' THEN result := verse.fork_revise_answer(asked,http,response,model,taken);
    WHEN 'ask' THEN result := verse.ask_answer(asked,http,response,model,taken);
    WHEN 'ask-doc' THEN result := verse.ask_doc_answer(asked,http,response,model,taken);
    ELSE RETURN '{"status":400,"error":"unknown edit"}';
  END CASE;
  RETURN verse.realm_edit_result(s,gate,result);
END $$;

-- Only the new C transport calls this reader. Old instances in a rolling deployment keep
-- calling edit_spec and therefore cannot advertise a fence they do not forward.
CREATE FUNCTION verse.realm_edit_spec(s text,k text,site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT verse.edit_spec(s,k,site_admin)
    || CASE WHEN EXISTS (SELECT 1 FROM verse.made_realms WHERE slug=s) THEN E'edit_fence 1\n' ELSE '' END $$;
