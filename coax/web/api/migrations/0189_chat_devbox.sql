-- /chat's devbox conversations (GitHub #13474, stage 4; docs/cloud/boat-devbox.md): a conversation
-- may name one of our boat dev boxes. Its turns then run on that box: the runner hands such a
-- turn to the devbox engine (coax/web/agent/devbox.coax), which brings the box up under the dev
-- box policy (coax/lib/boat/policy.coax), runs our Claude engine there as a detached command and
-- follows its output into chat.events, as any turn's events. The box's id is all we keep; whether
-- it is up, and what it costs, come from boat each turn.
--   POST /api/chat/devbox {c, box}     name the box ("" for none): an admin, on their own conversation
--   the runner's next                  a turn of such a conversation carries "devbox": the box's id
ALTER TABLE chat.conversations ADD COLUMN devbox text NOT NULL DEFAULT ''
  CONSTRAINT conversations_devbox_ok CHECK (devbox = '' OR devbox ~ '^bx_[23456789abcdefghjkmnpqrstuvwxyz]{8}$');

CREATE OR REPLACE FUNCTION chat.api(op text, body jsonb, who jsonb) RETURNS jsonb
    LANGUAGE plpgsql
    AS $_$
DECLARE
  pk text := nullif(who->>'pubkey', '');
  may boolean := coalesce((who->>'may')::boolean, false);
  b jsonb := CASE WHEN jsonb_typeof(body) = 'object' THEN body ELSE '{}' END;
  cid bigint := CASE WHEN coalesce(b->>'c', '') ~ '^[0-9]{1,18}$' THEN (b->>'c')::bigint END;
  box text := coalesce(b->>'box', '');
  res jsonb;
  at int;
  dv text;
BEGIN
  IF op IN ('tab', 'record', 'devbox') THEN
    IF pk IS NULL OR NOT may THEN RETURN chat.r(403, '{"error":"admins only"}'); END IF;
    IF cid IS NULL OR NOT EXISTS (SELECT 1 FROM chat.conversations WHERE id = cid AND owner = pk) THEN
      RETURN chat.r(404, '{"error":"no such conversation"}');
    END IF;
    IF op = 'record' THEN RETURN chat.r(200, to_jsonb(chat.record(cid))); END IF;
    IF op = 'devbox' THEN
      IF box <> '' AND box !~ '^bx_[23456789abcdefghjkmnpqrstuvwxyz]{8}$' THEN
        RETURN chat.r(400, '{"error":"box is a boat sandbox id (bx_ and 8 letters or digits), or empty for none"}');
      END IF;
      IF EXISTS (SELECT 1 FROM chat.turns WHERE conv = cid AND status IN ('queued', 'running')) THEN
        RETURN chat.r(409, '{"error":"the agent is still answering; wait or press STOP"}');
      END IF;
      UPDATE chat.conversations SET devbox = box WHERE id = cid;
      RETURN chat.r(200, jsonb_build_object('c', cid, 'devbox', box));
    END IF;
    IF coalesce(b->>'open', 'true') = 'false' THEN
      PERFORM chat.tab_close(pk, cid);
      RETURN chat.r(200, jsonb_build_object('c', cid, 'tab', NULL));
    END IF;
    at := chat.tab_open(pk, cid);
    RETURN chat.r(200, jsonb_build_object('c', cid, 'tab', at));
  END IF;
  res := chat.api_v1(op, body, who);
  IF op IN ('new', 'send') AND (res->>'status')::int = 200 AND coalesce(res->'body'->>'c', '') ~ '^[0-9]{1,18}$' THEN
    PERFORM chat.tab_open(pk, (res->'body'->>'c')::bigint);
  END IF;
  -- the runner's next: a turn of a devbox conversation names its box
  IF op = 'next' AND (res->>'status')::int = 200 AND jsonb_typeof(res->'body'->'turn') = 'object' THEN
    SELECT devbox INTO dv FROM chat.conversations WHERE id = (res->'body'->'turn'->>'conv')::bigint;
    IF coalesce(dv, '') <> '' THEN res := jsonb_set(res, '{body,turn,devbox}', to_jsonb(dv)); END IF;
  END IF;
  RETURN res;
END $_$;
