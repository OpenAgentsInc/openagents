-- Site admin keys held in the database, next to the root admins in admins.conf
-- (docs/cli/2026-11-01-openagents-cli-v1.md, "Admin keys"; INVARIANTS.md, "Only a root admin
-- changes who is an admin").
--
-- A root admin is a pubkey in admins.conf (OA_ADMIN_PUBKEYS): changed only by a deploy. A database
-- admin is a row here. Both are site admins for every admin check (admin.c, is_admin). Only a root
-- admin adds or removes a database admin; a database admin can add or remove no one, and no request
-- can remove a root (a root is not a row, and naming one is refused). Every change is a row in
-- access.admin_audit, kept.
--
--   GET  /api/admin/access/admins                 rows A|npub|hex|root and A|npub|hex|db|added by npub|at|note
--   POST /api/admin/access/admins {pubkey, note?} a root admin: 200 {pubkey, admin}; 400, 403, 409 a root already
--   POST /api/admin/access/admins/<hex>/remove    a root admin: 200 {pubkey, admin: false}; 403, 404, 409 a root
--
-- Additive: two new tables and new functions.
CREATE TABLE access.admin_keys (
  pubkey text PRIMARY KEY CHECK (pubkey ~ '^[0-9a-f]{64}$'),
  note text NOT NULL DEFAULT '',
  added_by text NOT NULL,        -- the root admin's pubkey
  added_ms bigint NOT NULL
);
CREATE TABLE access.admin_audit (
  id bigserial PRIMARY KEY,
  at_ms bigint NOT NULL,
  by text NOT NULL,              -- the admin who asked
  action text NOT NULL CHECK (action IN ('add', 'remove')),
  pubkey text NOT NULL,
  note text NOT NULL DEFAULT ''
);

CREATE FUNCTION access.is_db_admin(pk text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM access.admin_keys WHERE pubkey = lower(coalesce(pk, ''))) $$;

-- The rule: may `by` change the admin list? Only a root (roots: admins.conf, from admin.c).
CREATE FUNCTION access.admin_may(by_ text, roots text[]) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(by_ ~ '^[0-9a-f]{64}$' AND by_ = ANY (roots), false) $$;

CREATE FUNCTION access.admin_add(body jsonb, by_ text, roots text[]) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pk text := lower(coalesce(body->>'pubkey', ''));
  note_ text := verse.clean_text(body->'note', 120);
BEGIN
  IF NOT access.admin_may(by_, roots) THEN RETURN access.err(403, 'only a root admin (admins.conf) changes the admins'); END IF;
  IF pk !~ '^[0-9a-f]{64}$' THEN RETURN access.err(400, 'a pubkey (64 hex)'); END IF;
  IF pk = ANY (roots) THEN RETURN access.err(409, 'that key is a root admin already'); END IF;
  INSERT INTO access.admin_keys VALUES (pk, note_, by_, projects.now_ms()) ON CONFLICT (pubkey) DO NOTHING;
  IF FOUND THEN INSERT INTO access.admin_audit (at_ms, by, action, pubkey, note) VALUES (projects.now_ms(), by_, 'add', pk, note_); END IF;
  RETURN access.r(200, jsonb_build_object('pubkey', pk, 'admin', true), format(E'admin %s\n', pk));
END $$;

CREATE FUNCTION access.admin_remove(pk_ text, by_ text, roots text[]) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE pk text := lower(coalesce(pk_, ''));
BEGIN
  IF NOT access.admin_may(by_, roots) THEN RETURN access.err(403, 'only a root admin (admins.conf) changes the admins'); END IF;
  IF pk = ANY (roots) THEN RETURN access.err(409, 'a root admin is in admins.conf: only a deploy removes it'); END IF;
  DELETE FROM access.admin_keys WHERE pubkey = pk;
  IF NOT FOUND THEN RETURN access.err(404, 'not a database admin'); END IF;
  INSERT INTO access.admin_audit (at_ms, by, action, pubkey) VALUES (projects.now_ms(), by_, 'remove', pk);
  RETURN access.r(200, jsonb_build_object('pubkey', pk, 'admin', false), format(E'removed %s\n', pk));
END $$;

-- Every admin: the roots first (as admins.conf lists them), then the database's, oldest first.
CREATE FUNCTION access.admin_list(roots text[]) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT string_agg(format(E'A|%s|%s|root\n', signin.bech32('npub', r), r), '' ORDER BY o)
                   FROM unnest(roots) WITH ORDINALITY AS t(r, o) WHERE r ~ '^[0-9a-f]{64}$'), '')
      || coalesce((SELECT string_agg(format(E'A|%s|%s|db|%s|%s|%s\n', signin.bech32('npub', k.pubkey), k.pubkey,
                                            CASE WHEN k.added_by ~ '^[0-9a-f]{64}$' THEN signin.bech32('npub', k.added_by) ELSE access.line(k.added_by) END,
                                            k.added_ms, access.line(k.note)), '' ORDER BY k.added_ms, k.pubkey)
                   FROM access.admin_keys k WHERE NOT k.pubkey = ANY (roots)), '') $$;
