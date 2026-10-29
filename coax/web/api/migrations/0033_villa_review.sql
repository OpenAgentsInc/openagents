-- Planetary Villa: the review queue as a page (hc-report-review), and hiding
-- that can be undone. 0031 deleted a note and emptied a memorial on REMOVE;
-- now an admin HIDES the thing, which holds it out of sight without losing
-- it, and RESTORE brings it back as it was. Its owner can still delete it for
-- good (a held pet's owner removing it removes it). The page is
-- /admin/reports (admin.coax), served through /api/admin/reports (admin.c),
-- which calls villa.reports_api with the signed-in admin.
--
--   GET  /api/villa/reports                                   {open: [...], decided: [...]}
--   POST /api/villa/reports/<kind>/<ref>/hide|keep|restore    ('remove' still means hide)
--
-- What each kind does lives in villa.report_hide/report_restore, so a new
-- kind (0034's photos and videos) changes only those and report_shown.

ALTER TABLE villa.reports DROP CONSTRAINT reports_status_check;
ALTER TABLE villa.reports ADD CONSTRAINT reports_status_check CHECK (status IN ('open', 'removed', 'kept', 'restored'));
CREATE INDEX reports_decided ON villa.reports (decided_ms DESC) WHERE status <> 'open';

-- A pet an admin hid: 'held' is neither placed (no tombstone, no memorial, no
-- share link) nor removed (its owner still has it, and its plot is kept).
ALTER TABLE villa.pets DROP CONSTRAINT pets_status_check;
ALTER TABLE villa.pets ADD CONSTRAINT pets_status_check CHECK (status IN ('draft', 'placed', 'removed', 'held'));
-- Only RESTORE lifts a hold: PLAY again (or any edit of the status) leaves it
-- held, except the owner removing the pet for good.
CREATE FUNCTION villa.keep_held() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF OLD.status = 'held' AND NEW.status <> 'removed' AND coalesce(current_setting('villa.restoring', true), '') <> 'on' THEN
    NEW.status := 'held';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER pets_keep_held BEFORE UPDATE OF status ON villa.pets FOR EACH ROW EXECUTE FUNCTION villa.keep_held();

-- A note an admin hid: out of the memorial for everyone, its owner too (who
-- could otherwise show it again), until it's restored.
ALTER TABLE villa.notes ADD COLUMN held bool NOT NULL DEFAULT false;
CREATE OR REPLACE FUNCTION villa.notes_of(pet_ text, owner bool) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object('n', n.id, 'name', n.name, 'text', n.text, 'hidden', n.hidden) ORDER BY n.created_ms), '[]')
  FROM villa.notes n WHERE n.pet = pet_ AND NOT n.held AND (owner OR NOT n.hidden) $$;

CREATE FUNCTION villa.removed_text() RETURNS text LANGUAGE sql IMMUTABLE AS $$ SELECT 'This line was removed' $$;

-- Hide one reported thing: false when it isn't there to hide.
CREATE FUNCTION villa.report_hide(kind_ text, ref_ text) RETURNS bool LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  IF kind_ = 'memorial' THEN
    UPDATE villa.pets SET status = 'held', updated_ms = verse.now_ms() WHERE slug = ref_ AND status IN ('placed', 'held');
  ELSIF kind_ = 'note' THEN
    UPDATE villa.notes SET held = true WHERE ref_ ~ '^[0-9]{1,18}$' AND id = ref_::bigint;
  ELSIF kind_ = 'chat' THEN
    UPDATE verse.room_chat SET text = villa.removed_text() WHERE room = villa.chat_room() AND ref_ ~ '^[0-9]{1,9}$' AND room_chat.n = ref_::int;
  ELSE RETURN false;
  END IF;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n > 0;
END $$;

-- Bring a hidden thing back as it was: false when it's gone (its owner deleted it).
CREATE FUNCTION villa.report_restore(kind_ text, ref_ text) RETURNS bool LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  IF kind_ = 'memorial' THEN
    PERFORM set_config('villa.restoring', 'on', true);
    UPDATE villa.pets SET status = 'placed', updated_ms = verse.now_ms() WHERE slug = ref_ AND status = 'held';
    GET DIAGNOSTICS n = ROW_COUNT;
    PERFORM set_config('villa.restoring', '', true);
  ELSIF kind_ = 'note' THEN
    UPDATE villa.notes SET held = false, hidden = false WHERE ref_ ~ '^[0-9]{1,18}$' AND id = ref_::bigint;
    GET DIAGNOSTICS n = ROW_COUNT;
  ELSIF kind_ = 'chat' THEN
    UPDATE verse.room_chat c SET text = substr(r.shown, length(c.name) + 3)
    FROM (SELECT shown FROM villa.reports WHERE kind = 'chat' AND ref = ref_ ORDER BY at_ms LIMIT 1) r
    WHERE c.room = villa.chat_room() AND ref_ ~ '^[0-9]{1,9}$' AND c.n = ref_::int AND c.text IN (villa.removed_text(), villa.held_text());
    GET DIAGNOSTICS n = ROW_COUNT;
  ELSE RETURN false;
  END IF;
  RETURN n > 0;
END $$;

-- KEEP: what three reports held comes back; nothing else changes.
CREATE FUNCTION villa.report_keep(kind_ text, ref_ text) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  IF kind_ = 'note' THEN UPDATE villa.notes SET hidden = false WHERE ref_ ~ '^[0-9]{1,18}$' AND id = ref_::bigint AND NOT held; END IF;
  IF kind_ = 'chat' THEN
    UPDATE verse.room_chat c SET text = substr(r.shown, length(c.name) + 3)
    FROM (SELECT shown FROM villa.reports WHERE kind = 'chat' AND ref = ref_ ORDER BY at_ms LIMIT 1) r
    WHERE c.room = villa.chat_room() AND ref_ ~ '^[0-9]{1,9}$' AND c.n = ref_::int AND c.text = villa.held_text();
  END IF;
END $$;

-- The last decision on a thing, and whether RESTORE applies to it now.
CREATE FUNCTION villa.report_last(kind_ text, ref_ text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT status FROM villa.reports WHERE kind = kind_ AND ref = ref_ AND status <> 'open' ORDER BY decided_ms DESC, id DESC LIMIT 1 $$;

CREATE OR REPLACE FUNCTION villa.reports_api(method text, parts text[], body jsonb, k text, admin text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE me verse.players := villa.player(k); kind_ text; ref_ text; shown_ text; cnt int; by_ int; act text; open_ bool;
BEGIN
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' THEN body := '{}'; END IF;
  -- a visitor reports
  IF parts[1] = 'report' AND cardinality(parts) = 1 THEN
    IF method <> 'POST' THEN RETURN villa.err(405, 'POST'); END IF;
    IF me.id IS NULL THEN RETURN villa.err(403, 'send a player key (POST /api/playtest/player)'); END IF;
    kind_ := body->>'kind'; ref_ := body->>'ref';
    IF kind_ IS NULL OR NOT EXISTS (SELECT 1 FROM unnest(villa.report_kinds()) x WHERE x = kind_) OR ref_ IS NULL OR ref_ !~ '^[A-Za-z0-9]{1,18}$' THEN
      RETURN villa.err(400, 'what is it?');
    END IF;
    shown_ := villa.report_shown(kind_, ref_);
    IF shown_ = '' THEN RETURN villa.err(404, 'it isn''t there any more'); END IF;
    SELECT count(*) INTO cnt FROM villa.reports WHERE player = me.id AND at_ms > verse.now_ms() - 3600000;
    IF cnt >= 20 THEN RETURN villa.err(429, 'a little slower'); END IF;
    INSERT INTO villa.reports (kind, ref, player, why, shown, at_ms) VALUES (kind_, ref_, me.id, villa.clean(body->'why', 300), shown_, verse.now_ms())
    ON CONFLICT (kind, ref, player) WHERE status = 'open' DO NOTHING;
    SELECT count(*) INTO by_ FROM villa.reports WHERE kind = kind_ AND ref = ref_ AND status = 'open';
    IF by_ >= 3 AND kind_ = 'note' THEN UPDATE villa.notes SET hidden = true WHERE id = ref_::bigint; END IF;
    IF by_ >= 3 AND kind_ = 'chat' THEN UPDATE verse.room_chat SET text = villa.held_text() WHERE room = villa.chat_room() AND room_chat.n = ref_::int AND text <> villa.removed_text(); END IF;
    RETURN villa.r(200, '{"reported":true}');
  END IF;
  -- an admin
  IF parts[1] <> 'reports' THEN RETURN villa.err(404, 'unknown'); END IF;
  IF admin IS NULL OR admin = '' THEN RETURN villa.err(401, 'admins only'); END IF;
  IF cardinality(parts) = 1 AND method = 'GET' THEN
    RETURN villa.r(200, jsonb_build_object(
      'open', coalesce((
        SELECT jsonb_agg(jsonb_build_object('kind', r.kind, 'ref', r.ref, 'reports', r.c, 'first_ms', r.first_ms,
                 'shown', r.shown, 'why', r.whys) ORDER BY r.c DESC, r.first_ms)
        FROM (SELECT kind, ref, count(*) AS c, min(at_ms) AS first_ms, (array_agg(shown ORDER BY at_ms))[1] AS shown,
                     coalesce(jsonb_agg(why) FILTER (WHERE why <> ''), '[]') AS whys
              FROM villa.reports WHERE status = 'open' GROUP BY kind, ref) r), '[]'),
      -- the last forty decisions, newest first; restore: it was hidden, is still hidden, and nobody has reported it since
      'decided', coalesce((
        SELECT jsonb_agg(jsonb_build_object('kind', d.kind, 'ref', d.ref, 'decision', CASE d.status WHEN 'removed' THEN 'hidden' ELSE d.status END,
                 'reports', d.c, 'decided_ms', d.decided_ms, 'by', d.decided_by, 'shown', d.shown,
                 'restore', d.status = 'removed' AND villa.report_last(d.kind, d.ref) = 'removed'
                            AND NOT EXISTS (SELECT 1 FROM villa.reports o WHERE o.kind = d.kind AND o.ref = d.ref AND o.status = 'open'))
                 ORDER BY d.decided_ms DESC)
        FROM (SELECT kind, ref, status, decided_ms, min(decided_by) AS decided_by, count(*) AS c, (array_agg(shown ORDER BY at_ms))[1] AS shown
              FROM villa.reports WHERE status <> 'open' GROUP BY kind, ref, status, decided_ms ORDER BY decided_ms DESC LIMIT 40) d), '[]')));
  END IF;
  IF cardinality(parts) = 4 AND method = 'POST' AND parts[4] IN ('hide', 'remove', 'keep', 'restore') THEN
    kind_ := parts[2]; ref_ := parts[3]; act := CASE parts[4] WHEN 'remove' THEN 'hide' ELSE parts[4] END;
    open_ := EXISTS (SELECT 1 FROM villa.reports WHERE kind = kind_ AND ref = ref_ AND status = 'open');
    IF act = 'restore' THEN
      IF open_ OR villa.report_last(kind_, ref_) IS DISTINCT FROM 'removed' THEN RETURN villa.err(404, 'nothing hidden to restore'); END IF;
      IF NOT villa.report_restore(kind_, ref_) THEN RETURN villa.err(410, 'its owner has deleted it'); END IF;
      UPDATE villa.reports SET status = 'restored', decided_by = admin, decided_ms = verse.now_ms()
      WHERE id = (SELECT id FROM villa.reports WHERE kind = kind_ AND ref = ref_ AND status = 'removed' ORDER BY decided_ms DESC, id DESC LIMIT 1);
      RETURN villa.r(200, jsonb_build_object('decided', 'restore'));
    END IF;
    IF NOT open_ THEN RETURN villa.err(404, 'nothing open'); END IF;
    IF act = 'hide' THEN PERFORM villa.report_hide(kind_, ref_); ELSE PERFORM villa.report_keep(kind_, ref_); END IF;
    UPDATE villa.reports SET status = CASE act WHEN 'hide' THEN 'removed' ELSE 'kept' END, decided_by = admin, decided_ms = verse.now_ms()
    WHERE kind = kind_ AND ref = ref_ AND status = 'open';
    RETURN villa.r(200, jsonb_build_object('decided', act));
  END IF;
  RETURN villa.err(404, 'unknown');
END $$;

-- The kinds a visitor can report (0034 adds one).
CREATE FUNCTION villa.report_kinds() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$ SELECT ARRAY['memorial', 'note', 'chat'] $$;

-- The queue as rows for admin.coax (admin.c's format): "#TITLE|FLAGS|COLUMN|..." then "value|..." a row.
-- FLAGS "a": a row's first two values are its kind and ref, and it has HIDE and KEEP; "r": RESTORE
-- on a row whose last value is "yes". Times are Chicago's.
CREATE FUNCTION villa.reports_rows(admin text) RETURNS text LANGUAGE plpgsql AS $$
DECLARE q jsonb; t text; x jsonb;
BEGIN
  q := villa.reports_api('GET', ARRAY['reports'], '{}', NULL, admin)->'body';
  IF q ? 'error' THEN RETURN NULL; END IF;
  t := '#OPEN REPORTS|aw|KIND|WHICH|REPORTS|FIRST|WHAT IT SAYS|WHY' || chr(10);
  FOR x IN SELECT * FROM jsonb_array_elements(q->'open') LOOP
    t := t || concat_ws('|', x->>'kind', x->>'ref', x->>'reports',
      to_char(to_timestamp((x->>'first_ms')::bigint / 1000.0) AT TIME ZONE 'America/Chicago', 'MM-DD HH24:MI'),
      villa.row_value(x->>'shown'), villa.row_value((SELECT string_agg(w, '; ') FROM jsonb_array_elements_text(x->'why') w))) || chr(10);
  END LOOP;
  t := t || '#DECIDED|rw|KIND|WHICH|DECISION|WHEN|BY|WHAT IT SAID|RESTORE' || chr(10);
  FOR x IN SELECT * FROM jsonb_array_elements(q->'decided') LOOP
    t := t || concat_ws('|', x->>'kind', x->>'ref', upper(x->>'decision'),
      to_char(to_timestamp((x->>'decided_ms')::bigint / 1000.0) AT TIME ZONE 'America/Chicago', 'MM-DD HH24:MI'),
      left(coalesce(x->>'by', ''), 8), villa.row_value(x->>'shown'), CASE WHEN (x->>'restore')::bool THEN 'yes' ELSE '' END) || chr(10);
  END LOOP;
  RETURN t;
END $$;
-- A value for a row: '|' and line breaks would split it.
CREATE FUNCTION villa.row_value(s text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT left(regexp_replace(coalesce(s, ''), '[|\r\n\t]+', ' ', 'g'), 400) $$;
