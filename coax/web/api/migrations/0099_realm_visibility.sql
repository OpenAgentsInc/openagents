-- Who sees a made realm: private, unlisted or public (docs/cli/2026-11-01-openagents-cli-v1.md,
-- "Who sees a realm"; INVARIANTS.md, "A private realm answers only its owner and site admins").
--
--   private   only its owner (its player key, the key's account, or a NIP-98 signer's account) and
--             site admins see it. Everyone else gets 404 from every /api/playtest/realms/<slug>
--             route (realms.c asks verse.realm_hidden first), its page is not in the site's bucket,
--             and it is off /og's rows and realms/made-slugs.txt. Nobody else can fork it.
--   unlisted  anyone with its address sees it; it is off /og's rows and /realms, and its page says
--             noindex.
--   public    on /og's rows and /realms (the made rows), indexed.
--
-- A new made realm and a new fork start private. Every realm made before this migration keeps what
-- visitors had: listed is public, unlisted is unlisted (an unpublished fork of a built-in realm,
-- reachable only by its address, is unlisted). Built-in realms are not made realms and stay public.
--
-- Who changes it: the realm's owner when it is an approved creator (access.gate: an approved
-- creator, or realm_invites off), and site admins for any realm.
--
--   POST /api/playtest/realms/<slug>/visibility {visibility}   -> 200 {slug, visibility, was}; 400, 403, 404
--   GET  /api/admin/access/realms[?visibility=V]               every made realm, for site admins
-- The old fields still work, mapped onto this one: publish's `listed` true is public and false is
-- unlisted (a private realm stays private); a built-in fork's `published` true is public, false
-- unlisted. Each is a visibility change and needs the same right. `unlisted` stays, kept equal to
-- visibility <> 'public' by a trigger, so every older reader of it still answers right.
--
-- Additive: one column, a trigger, new functions, renamed functions behind dispatchers; realms_mine
-- (0098) replaced to add the visibility.

ALTER TABLE verse.made_realms ADD COLUMN visibility text NOT NULL DEFAULT 'private'
  CHECK (visibility IN ('private', 'unlisted', 'public'));
UPDATE verse.made_realms SET visibility = CASE WHEN unlisted THEN 'unlisted'
                                               WHEN base IS NOT NULL AND published_ms IS NULL THEN 'unlisted'
                                               ELSE 'public' END;

-- unlisted follows visibility. An older path that sets unlisted moves public to unlisted, or
-- unlisted back to public; it never makes a private realm visible.
CREATE FUNCTION verse.made_realms_seen() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF TG_OP = 'UPDATE' AND NEW.visibility = OLD.visibility AND NEW.unlisted IS DISTINCT FROM OLD.unlisted THEN
    NEW.visibility := CASE WHEN NEW.unlisted AND OLD.visibility = 'public' THEN 'unlisted'
                           WHEN NOT NEW.unlisted AND OLD.visibility = 'unlisted' THEN 'public'
                           ELSE OLD.visibility END;
  END IF;
  NEW.unlisted := NEW.visibility <> 'public';
  RETURN NEW;
END $$;
CREATE TRIGGER made_realms_seen BEFORE INSERT OR UPDATE ON verse.made_realms FOR EACH ROW EXECUTE FUNCTION verse.made_realms_seen();

-- A visibility as asked: one of the three words, else NULL.
CREATE FUNCTION verse.visibility_of(v jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(v) = 'string' AND (v #>> '{}') IN ('private', 'unlisted', 'public') THEN v #>> '{}' END $$;

-- The one rule for who sees a private realm: true when the realm is private and this caller is
-- neither its owner nor a site admin. realms.c answers 404 to every route of a hidden realm.
CREATE FUNCTION verse.realm_hidden(s text, k text, site_admin boolean) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM verse.made_realms m WHERE m.slug = s AND m.visibility = 'private')
     AND NOT coalesce(site_admin, false) AND NOT verse.builder_owns(k, s) $$;

-- May this caller change who sees the realm? 200, 403, or 404 (none, or hidden from it).
CREATE FUNCTION verse.realm_visibility_may(s text, k text, site_admin boolean) RETURNS int LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN NOT verse.edit_made(s) OR verse.realm_hidden(s, k, site_admin) THEN 404
              WHEN coalesce(site_admin, false) THEN 200
              WHEN verse.builder_owns(k, s) AND access.gate(k, NULL, false) = 200 THEN 200
              ELSE 403 END $$;

-- Sets it. A built-in realm's fork keeps 0075's published in step (public is published).
CREATE FUNCTION verse.realm_set_visibility(s text, v text, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  may int := verse.realm_visibility_may(s, k, site_admin);
  was text;
BEGIN
  IF may = 404 THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  IF v IS NULL THEN RETURN '{"status":400,"error":"visibility is private, unlisted or public"}'; END IF;
  IF may <> 200 THEN RETURN '{"status":403,"error":"only approved creators and admins change who sees a realm"}'; END IF;
  SELECT visibility INTO was FROM verse.made_realms WHERE slug = s FOR UPDATE;
  UPDATE verse.made_realms SET visibility = v,
         published_ms = CASE WHEN base IS NULL THEN published_ms WHEN v = 'public' THEN coalesce(published_ms, verse.now_ms()) ELSE NULL END
  WHERE slug = s;
  RETURN jsonb_build_object('status', 200, 'slug', s, 'visibility', v, 'was', was);
END $$;
CREATE FUNCTION verse.realm_visibility(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.realm_set_visibility(s, verse.visibility_of(body->'visibility'), k, site_admin) $$;

-- The visibility a publish asks for: listed (a made realm) or published (a built-in fork) mapped
-- onto it; NULL when it asks for no change.
CREATE FUNCTION verse.publish_wants(m verse.made_realms, body jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN up IS NULL THEN NULL
              WHEN up THEN 'public'
              WHEN m.visibility = 'public' THEN 'unlisted'
              ELSE m.visibility END
  FROM (SELECT CASE WHEN m.base IS NULL AND jsonb_typeof(body->'listed') = 'boolean' THEN (body->>'listed')::boolean
                    WHEN m.base IS NOT NULL THEN CASE WHEN jsonb_typeof(body->'published') = 'boolean' THEN (body->>'published')::boolean ELSE true END
               END AS up) x $$;

-- PUBLISH (0093's made_publish, 0075's fork publish): the same, and its listed or published is a
-- visibility change with the same right as POST .../visibility.
ALTER FUNCTION verse.fork_publish(text, jsonb, text, boolean) RENAME TO fork_publish_0093;
CREATE FUNCTION verse.fork_publish(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m verse.made_realms;
  want text;
  out jsonb;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s;
  IF m.slug IS NULL THEN RETURN verse.fork_publish_0093(s, body, k, site_admin); END IF;
  IF verse.realm_hidden(s, k, site_admin) THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
  want := verse.publish_wants(m, body);
  IF want IS NOT NULL AND want <> m.visibility AND verse.edit_may(s, k, site_admin) AND verse.realm_visibility_may(s, k, site_admin) <> 200 THEN
    RETURN '{"status":403,"error":"only approved creators and admins change who sees a realm"}';
  END IF;
  out := verse.fork_publish_0093(s, body, k, site_admin);
  IF (out->>'status')::int = 200 AND want IS NOT NULL THEN
    UPDATE verse.made_realms SET visibility = want WHERE slug = s;
    out := out || jsonb_build_object('visibility', want, 'listed', want = 'public');
  END IF;
  RETURN out;
END $$;

-- The spec gains "visibility private|unlisted|public" for a made realm.
ALTER FUNCTION verse.edit_spec(text, text, boolean) RENAME TO edit_spec_0094;
CREATE FUNCTION verse.edit_spec(s text, k text, site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN out IS NULL THEN NULL
              ELSE out || coalesce((SELECT 'visibility ' || m.visibility || E'\n' FROM verse.made_realms m WHERE m.slug = s), '') END
  FROM (SELECT verse.edit_spec_0094(s, k, site_admin) AS out) x $$;

-- Lists keep to what each says: /og's rows public only (0093's, with visibility), the bucket's
-- list of pages everything but private.
CREATE OR REPLACE FUNCTION verse.builder_og_rows(n int) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg(
    '/realms/' || m.slug || '|made||' || verse.builder_line(to_jsonb(m.name), 120) || '|' || verse.builder_line(to_jsonb(verse.builder_share_line(m)), 240)
    || '|https://openagents.com/realms/' || m.slug || '|' || verse.builder_card_path(m) || '|' || verse.builder_line(to_jsonb(m.name), 120)
    || '|' || CASE WHEN m.card_ms IS NULL THEN 'default' ELSE 'made' END || '|1200|630|' || coalesce(m.card_kb, 0)
    || '|' || CASE WHEN m.card_ms IS NULL THEN 'no picture of its own yet (its owner''s next visit takes one)' ELSE '' END
    || '|' || CASE WHEN m.description IS NULL THEN 'line written from what is in it' ELSE '' END || '|', E'\n' ORDER BY m.made_ms DESC) || E'\n', '')
  FROM (SELECT * FROM verse.made_realms WHERE visibility = 'public' AND (base IS NULL OR published_ms IS NOT NULL)
        ORDER BY made_ms DESC LIMIT greatest(least(n, 200), 1)) m $$;
CREATE OR REPLACE FUNCTION verse.builder_slugs() RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg(slug, E'\n' ORDER BY made_ms, slug) || E'\n', '') FROM verse.made_realms WHERE visibility <> 'private' $$;

-- What realms.c does with a realm's page: 'write' it, or 'remove' it from the bucket (private).
CREATE FUNCTION verse.realm_page_action(s text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN visibility = 'private' THEN 'remove' ELSE 'write' END FROM verse.made_realms WHERE slug = s $$;
-- A private realm keeps no picture in the bucket: removed, it is asked for again when it is shown.
CREATE FUNCTION verse.realm_card_gone(s text) RETURNS boolean LANGUAGE sql AS $$
  UPDATE verse.made_realms SET card_ms = NULL, card_kb = NULL WHERE slug = s AND visibility = 'private' RETURNING true $$;

-- Its picture: not taken or kept while it is private.
ALTER FUNCTION verse.builder_card_want(text, text, boolean) RENAME TO builder_card_want_0099;
CREATE FUNCTION verse.builder_card_want(s text, k text, site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s AND visibility = 'private') THEN 'no'
              ELSE verse.builder_card_want_0099(s, k, site_admin) END $$;
ALTER FUNCTION verse.builder_card_may(text, text, boolean) RENAME TO builder_card_may_0099;
CREATE FUNCTION verse.builder_card_may(s text, k text, site_admin boolean) RETURNS int LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s AND visibility = 'private') AND verse.builder_card_may_0099(s, k, site_admin) = 200 THEN 409
              ELSE verse.builder_card_may_0099(s, k, site_admin) END $$;

-- GET /api/admin/access/realms[?visibility=V]: every made realm, newest first, for site admins.
-- {lines: "realm SLUG|VISIBILITY|REV|PUBLISHED|OWNER|NAME" a line, body: {realms: [...]}}; OWNER is
-- the account, else the guest player.
CREATE FUNCTION verse.realm_admin_rows(v text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object(
    'lines', coalesce(string_agg(format(E'realm %s|%s|%s|%s|%s|%s\n', x.slug, x.visibility, x.rev, x.published, access.line(x.owner), access.line(x.name)), '' ORDER BY x.made_ms DESC, x.slug), ''),
    'body', jsonb_build_object('realms', coalesce(jsonb_agg(jsonb_build_object('slug', x.slug, 'visibility', x.visibility, 'rev', x.rev, 'published', x.published,
                                                                               'owner', x.owner, 'name', x.name) ORDER BY x.made_ms DESC, x.slug), '[]')))
  FROM (SELECT m.slug, m.visibility, m.rev, coalesce(m.published_rev, 0) AS published, coalesce(w.owner_account, p.account, w.owner_player, '') AS owner, m.name, m.made_ms
        FROM verse.made_realms m JOIN verse.worlds w ON w.id = m.world LEFT JOIN verse.players p ON p.id = w.owner_player
        WHERE v IS NULL OR m.visibility = v) x $$;

-- GET /api/playtest/realms/mine (0098) says who sees each: "realm SLUG|REV|PUBLISHED|LISTED|NAME|VISIBILITY"
-- (the field after the name, last, so a reader of the first five is unchanged) and visibility in JSON.
CREATE OR REPLACE FUNCTION verse.realms_mine(k text) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE
  rec verse.players := verse.builder_player(k);
  out jsonb;
BEGIN
  SELECT jsonb_build_object(
      'lines', coalesce(string_agg(format(E'realm %s|%s|%s|%s|%s|%s\n', x.slug, x.rev, x.published, CASE WHEN x.listed THEN 1 ELSE 0 END, access.line(x.name), x.visibility), '' ORDER BY x.made_ms DESC, x.slug), ''),
      'body', jsonb_build_object('realms', coalesce(jsonb_agg(jsonb_build_object('slug', x.slug, 'name', x.name, 'rev', x.rev, 'published', x.published, 'listed', x.listed,
                                                                                 'visibility', x.visibility)
                                                             ORDER BY x.made_ms DESC, x.slug), '[]')))
    INTO out
    FROM (SELECT m.slug, m.name, m.rev, coalesce(m.published_rev, 0) AS published, NOT m.unlisted AS listed, m.visibility, m.made_ms
          FROM verse.made_realms m WHERE rec.id IS NOT NULL AND verse.owns(rec, NULL, m.world)) x;
  RETURN out;
END $$;
