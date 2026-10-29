-- Planetary Villa: reporting and review for what visitors share in Hecate's
-- Cemetery (hc-safety; verse/planetary-villa/README.md). A memorial, a note
-- on one, or a line in the cemetery's chat can be reported by any visitor
-- with a player key. Three people reporting the same note or chat line hold
-- it at once (hidden until it's looked at); a memorial is never hidden by
-- reports alone, so nobody can take down someone's pet by asking friends to
-- report it. An admin (OA_ADMIN_PUBKEYS, signed in) sees the open reports and
-- removes the thing or keeps it. Photos and videos are private to their pet's
-- owner (only its avatar and memorial use them), so they have no report.
--
--   POST /api/villa/report {kind, ref, why}            a visitor reports
--   GET  /api/villa/reports                             an admin: what's open
--   POST /api/villa/reports/<kind>/<ref>/remove|keep    an admin decides

CREATE TABLE villa.reports (
  id bigserial PRIMARY KEY,
  kind text NOT NULL CHECK (kind IN ('memorial', 'note', 'chat')),
  ref text NOT NULL,                 -- a memorial's slug, a note's number, a chat line's number
  player text NOT NULL REFERENCES verse.players (id),
  why text NOT NULL DEFAULT '',
  shown text NOT NULL DEFAULT '',    -- what it said when reported, for the reviewer
  at_ms bigint NOT NULL,
  status text NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'removed', 'kept')),
  decided_by text, decided_ms bigint
);
CREATE INDEX reports_open ON villa.reports (status, kind, ref);
CREATE UNIQUE INDEX reports_once ON villa.reports (kind, ref, player) WHERE status = 'open';   -- one open report a person

CREATE FUNCTION villa.held_text() RETURNS text LANGUAGE sql IMMUTABLE AS $$ SELECT 'This line is waiting to be looked at' $$;
CREATE FUNCTION villa.chat_room() RETURNS text LANGUAGE sql IMMUTABLE AS $$ SELECT 'hecates-cemetery' $$;

-- What a reported thing says now ('' when it's gone).
CREATE FUNCTION villa.report_shown(kind_ text, ref_ text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(CASE kind_
    WHEN 'memorial' THEN (SELECT left(concat_ws(' · ', name, line, story, traits, achievements), 2000) FROM villa.pets WHERE slug = ref_ AND status = 'placed')
    WHEN 'note' THEN (SELECT name || ': ' || text FROM villa.notes WHERE ref_ ~ '^[0-9]{1,18}$' AND id = ref_::bigint)
    WHEN 'chat' THEN (SELECT name || ': ' || text FROM verse.room_chat WHERE room = villa.chat_room() AND ref_ ~ '^[0-9]{1,9}$' AND n = ref_::int)
  END, '') $$;

CREATE FUNCTION villa.reports_api(method text, parts text[], body jsonb, k text, admin text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE me verse.players := villa.player(k); kind_ text; ref_ text; shown_ text; cnt int; by_ int;
BEGIN
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' THEN body := '{}'; END IF;
  -- a visitor reports
  IF parts[1] = 'report' AND cardinality(parts) = 1 THEN
    IF method <> 'POST' THEN RETURN villa.err(405, 'POST'); END IF;
    IF me.id IS NULL THEN RETURN villa.err(403, 'send a player key (POST /api/playtest/player)'); END IF;
    kind_ := body->>'kind'; ref_ := body->>'ref';
    IF kind_ IS NULL OR kind_ NOT IN ('memorial', 'note', 'chat') OR ref_ IS NULL OR ref_ !~ '^[A-Za-z0-9]{1,18}$' THEN RETURN villa.err(400, 'what is it?'); END IF;
    shown_ := villa.report_shown(kind_, ref_);
    IF shown_ = '' THEN RETURN villa.err(404, 'it isn''t there any more'); END IF;
    SELECT count(*) INTO cnt FROM villa.reports WHERE player = me.id AND at_ms > verse.now_ms() - 3600000;
    IF cnt >= 20 THEN RETURN villa.err(429, 'a little slower'); END IF;
    INSERT INTO villa.reports (kind, ref, player, why, shown, at_ms) VALUES (kind_, ref_, me.id, villa.clean(body->'why', 300), shown_, verse.now_ms())
    ON CONFLICT (kind, ref, player) WHERE status = 'open' DO NOTHING;
    SELECT count(*) INTO by_ FROM villa.reports WHERE kind = kind_ AND ref = ref_ AND status = 'open';
    IF by_ >= 3 AND kind_ = 'note' THEN UPDATE villa.notes SET hidden = true WHERE id = ref_::bigint; END IF;
    IF by_ >= 3 AND kind_ = 'chat' THEN UPDATE verse.room_chat SET text = villa.held_text() WHERE room = villa.chat_room() AND room_chat.n = ref_::int; END IF;
    RETURN villa.r(200, '{"reported":true}');
  END IF;
  -- an admin
  IF parts[1] <> 'reports' THEN RETURN villa.err(404, 'unknown'); END IF;
  IF admin IS NULL OR admin = '' THEN RETURN villa.err(401, 'admins only'); END IF;
  IF cardinality(parts) = 1 AND method = 'GET' THEN
    RETURN villa.r(200, jsonb_build_object('open', coalesce((
      SELECT jsonb_agg(jsonb_build_object('kind', r.kind, 'ref', r.ref, 'reports', r.c, 'first_ms', r.first_ms,
               'shown', r.shown, 'why', r.whys) ORDER BY r.c DESC, r.first_ms)
      FROM (SELECT kind, ref, count(*) AS c, min(at_ms) AS first_ms, (array_agg(shown ORDER BY at_ms))[1] AS shown,
                   coalesce(jsonb_agg(why) FILTER (WHERE why <> ''), '[]') AS whys
            FROM villa.reports WHERE status = 'open' GROUP BY kind, ref) r), '[]')));
  END IF;
  IF cardinality(parts) = 4 AND method = 'POST' AND parts[4] IN ('remove', 'keep') THEN
    kind_ := parts[2]; ref_ := parts[3];
    IF NOT EXISTS (SELECT 1 FROM villa.reports WHERE kind = kind_ AND ref = ref_ AND status = 'open') THEN RETURN villa.err(404, 'nothing open'); END IF;
    IF parts[4] = 'remove' THEN
      IF kind_ = 'memorial' THEN
        UPDATE villa.pets SET status = 'removed', plot = NULL, story = '', traits = '', achievements = '', updated_ms = verse.now_ms() WHERE slug = ref_;
      ELSIF kind_ = 'note' THEN
        DELETE FROM villa.notes WHERE id = ref_::bigint;
      ELSE
        UPDATE verse.room_chat SET text = 'This line was removed' WHERE room = villa.chat_room() AND room_chat.n = ref_::int;
      END IF;
    ELSE   -- kept: a held note or line comes back as it was
      IF kind_ = 'note' THEN UPDATE villa.notes SET hidden = false WHERE id = ref_::bigint; END IF;
      IF kind_ = 'chat' THEN
        UPDATE verse.room_chat c SET text = substr(r.shown, length(c.name) + 3)
        FROM (SELECT shown FROM villa.reports WHERE kind = 'chat' AND ref = ref_ ORDER BY at_ms LIMIT 1) r
        WHERE c.room = villa.chat_room() AND c.n = ref_::int AND c.text = villa.held_text();
      END IF;
    END IF;
    UPDATE villa.reports SET status = CASE parts[4] WHEN 'remove' THEN 'removed' ELSE 'kept' END, decided_by = admin, decided_ms = verse.now_ms()
    WHERE kind = kind_ AND ref = ref_ AND status = 'open';
    RETURN villa.r(200, jsonb_build_object('decided', parts[4]));
  END IF;
  RETURN villa.err(404, 'unknown');
END $$;
