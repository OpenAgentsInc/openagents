-- The owner reviews the draft before anything is public (rc-review;
-- docs/projects/new-project.md Part 2, step 3). Every field of the draft is
-- the owner's to change: a text, a yes or no, an item of a list (emptied, it
-- leaves the list) or a new item at a list's end. PUBLISH freezes the reviewed
-- draft for the publisher (rc-publish reads drops whose status is 'approved');
-- DISCARD forgets the drop: its files leave the bucket and its draft is gone.
-- Nothing here reaches the relay or a public bucket.
ALTER TABLE drops.drops DROP CONSTRAINT drops_status_check;
ALTER TABLE drops.drops ADD CONSTRAINT drops_status_check CHECK (status IN ('open', 'approved', 'published', 'discarded'));
ALTER TABLE drops.drafts ADD COLUMN original jsonb, ADD COLUMN edits int NOT NULL DEFAULT 0,
  ADD COLUMN edited_ms bigint, ADD COLUMN approved_ms bigint;

CREATE OR REPLACE FUNCTION drops.draft_json(x drops.drafts) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('status', x.status, 'note', x.note, 'model', x.model, 'took_ms', x.took_ms, 'draft', x.draft,
    'edits', x.edits, 'approved', x.approved_ms IS NOT NULL) $$;

-- A new reading starts from the model's answer again.
CREATE OR REPLACE FUNCTION drops.draft_start(pubkey text, drop_ text, bucket text) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE d drops.drops; cur drops.drafts;
BEGIN
  SELECT * INTO d FROM drops.drops WHERE id = drop_ AND owner = pubkey FOR UPDATE;
  IF d.id IS NULL THEN RETURN drops.r(404, '{"error":"not found"}'); END IF;
  IF d.status <> 'open' THEN RETURN drops.r(409, '{"error":"this drop is closed"}'); END IF;
  IF NOT EXISTS (SELECT 1 FROM drops.files WHERE drop_id = d.id AND status = 'scanned') THEN
    RETURN drops.r(409, '{"error":"nothing kept to read yet"}');
  END IF;
  SELECT * INTO cur FROM drops.drafts WHERE drop_id = d.id;
  IF cur.status = 'reading' AND cur.started_ms > drops.now_ms() - 300000 THEN RETURN drops.r(409, '{"error":"already reading"}'); END IF;
  INSERT INTO drops.drafts (drop_id, status, started_ms) VALUES (d.id, 'reading', drops.now_ms())
  ON CONFLICT (drop_id) DO UPDATE SET status = 'reading', note = '', started_ms = EXCLUDED.started_ms, done_ms = NULL,
    original = NULL, edits = 0, edited_ms = NULL;
  RETURN drops.r(200, '{}') || jsonb_build_object('request', drops.draft_request(d.id, bucket));
END $$;

CREATE FUNCTION drops.edit_max() RETURNS int LANGUAGE sql IMMUTABLE AS $$ SELECT 20000 $$;

-- One change: {"path": ["card", "name"], "value": "..."}.
CREATE FUNCTION drops.draft_edit(pubkey text, drop_ text, req jsonb) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE d drops.drops; x drops.drafts; p text[]; v text; parent jsonb; target jsonb; nd jsonb; why text; n int;
BEGIN
  SELECT * INTO d FROM drops.drops WHERE id = drop_ AND owner = pubkey FOR UPDATE;
  IF d.id IS NULL THEN RETURN drops.r(404, '{"error":"not found"}'); END IF;
  IF d.status <> 'open' THEN RETURN drops.r(409, '{"error":"this drop is closed"}'); END IF;
  SELECT * INTO x FROM drops.drafts WHERE drop_id = d.id FOR UPDATE;
  IF x.status IS DISTINCT FROM 'ready' THEN RETURN drops.r(409, '{"error":"no draft to change"}'); END IF;
  IF jsonb_typeof(req->'path') <> 'array' OR jsonb_typeof(req->'value') <> 'string' THEN
    RETURN drops.r(400, '{"error":"a path and a value"}');
  END IF;
  SELECT array_agg(e ORDER BY i) INTO p FROM jsonb_array_elements_text(req->'path') WITH ORDINALITY AS z(e, i);
  v := req->>'value';
  n := coalesce(array_length(p, 1), 0);
  IF n < 2 OR n > 8 OR p[1] NOT IN ('card', 'mechanics', 'design', 'roadmap', 'assets', 'to_make', 'engine_needs') THEN
    RETURN drops.r(400, '{"error":"not a field of the draft"}');
  END IF;
  IF length(v) > drops.edit_max() THEN RETURN drops.r(400, '{"error":"too long"}'); END IF;
  v := btrim(v);
  target := x.draft #> p;
  parent := x.draft #> p[1:n - 1];
  IF target IS NULL AND jsonb_typeof(parent) = 'array' AND p[n] ~ '^[0-9]+$' AND p[n]::int = jsonb_array_length(parent)
     AND NOT EXISTS (SELECT 1 FROM jsonb_array_elements(parent) e WHERE jsonb_typeof(e) <> 'string') THEN
    IF v = '' THEN RETURN drops.r(400, '{"error":"nothing to add"}'); END IF;
    nd := jsonb_set(x.draft, p[1:n - 1], parent || to_jsonb(v));
  ELSIF jsonb_typeof(target) = 'boolean' THEN
    IF lower(v) NOT IN ('yes', 'no', 'true', 'false') THEN RETURN drops.r(400, '{"error":"yes or no"}'); END IF;
    nd := jsonb_set(x.draft, p, to_jsonb(lower(v) IN ('yes', 'true')));
  ELSIF jsonb_typeof(target) = 'string' THEN
    IF v = '' AND jsonb_typeof(parent) = 'array' THEN
      nd := x.draft #- p;
    ELSE
      IF p[n] = 'size' AND v NOT IN ('S', 'M', 'L') THEN RETURN drops.r(400, '{"error":"a size is S, M or L"}'); END IF;
      IF p[1:2] = ARRAY['card', 'slug'] THEN
        v := drops.slug(v);
        IF v = '' THEN RETURN drops.r(400, '{"error":"a slug needs a letter or a digit"}'); END IF;
      END IF;
      nd := jsonb_set(x.draft, p, to_jsonb(v));
    END IF;
  ELSE
    RETURN drops.r(400, '{"error":"not a field of the draft"}');
  END IF;
  why := drops.draft_invalid(nd);
  IF why IS NOT NULL THEN RETURN drops.r(400, jsonb_build_object('error', 'that would leave ' || why)); END IF;
  UPDATE drops.drafts SET original = coalesce(original, draft), draft = nd, edits = edits + 1, edited_ms = drops.now_ms()
  WHERE drop_id = d.id RETURNING * INTO x;
  RETURN drops.r(200, drops.draft_json(x));
END $$;

-- PUBLISH: the reviewed draft, frozen for the publisher.
CREATE FUNCTION drops.draft_approve(pubkey text, drop_ text) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE d drops.drops; x drops.drafts;
BEGIN
  SELECT * INTO d FROM drops.drops WHERE id = drop_ AND owner = pubkey FOR UPDATE;
  IF d.id IS NULL THEN RETURN drops.r(404, '{"error":"not found"}'); END IF;
  IF d.status <> 'open' THEN RETURN drops.r(409, '{"error":"this drop is closed"}'); END IF;
  SELECT * INTO x FROM drops.drafts WHERE drop_id = d.id FOR UPDATE;
  IF x.status IS DISTINCT FROM 'ready' OR drops.draft_invalid(x.draft) IS NOT NULL THEN
    RETURN drops.r(409, '{"error":"no draft to publish"}');
  END IF;
  IF EXISTS (SELECT 1 FROM drops.files WHERE drop_id = d.id AND status IN ('waiting', 'uploading')) THEN
    RETURN drops.r(409, '{"error":"files are still uploading"}');
  END IF;
  UPDATE drops.drops SET status = 'approved' WHERE id = d.id;
  UPDATE drops.drafts SET approved_ms = drops.now_ms() WHERE drop_id = d.id RETURNING * INTO x;
  RETURN drops.r(200, drops.draft_json(x) || '{"drop":"approved"}');
END $$;

-- DISCARD: the drop is forgotten. The objects to take out of the bucket go back to drops.c.
CREATE FUNCTION drops.discard(pubkey text, drop_ text) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE d drops.drops; gone jsonb;
BEGIN
  SELECT * INTO d FROM drops.drops WHERE id = drop_ AND owner = pubkey FOR UPDATE;
  IF d.id IS NULL THEN RETURN drops.r(404, '{"error":"not found"}'); END IF;
  IF d.status NOT IN ('open', 'approved') THEN RETURN drops.r(409, '{"error":"this drop is closed"}'); END IF;
  IF EXISTS (SELECT 1 FROM drops.files WHERE drop_id = d.id AND status IN ('waiting', 'uploading') AND created_ms > drops.now_ms() - 3600000) THEN
    RETURN drops.r(409, '{"error":"files are still uploading"}');
  END IF;
  SELECT coalesce(jsonb_agg(object), '[]') INTO gone FROM drops.files WHERE drop_id = d.id AND status <> 'refused';
  DELETE FROM drops.drafts WHERE drop_id = d.id;
  DELETE FROM drops.files WHERE drop_id = d.id;
  UPDATE drops.drops SET status = 'discarded' WHERE id = d.id;
  RETURN drops.r(200, '{"drop":"discarded"}') || jsonb_build_object('objects', gone);
END $$;
