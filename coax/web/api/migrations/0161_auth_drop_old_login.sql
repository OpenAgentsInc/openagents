-- The old login's tables and functions go (#13207, docs/auth/build-order.md "Code to delete"): the
-- phone sign-in and words account's signin schema, the groundwork's auth.identities, auth.v0_sessions
-- and auth.workos_states (WorkOS, decision D7), the admin page's old analytics.admin_sessions, and the
-- unverified email the old /api/playtest/join kept on verse.accounts. Every login is auth.c's now
-- (auth.accounts, auth.nostr_keys, auth.sessions); verse.accounts stays, the Verse's own account ids.
-- A dump of the database is taken before this runs in production (coax/web/api/backup/deploy.sh).

-- A key the old groundwork linked (auth.identities, kind nostr) that no account of ours holds yet
-- joins the account that adopted its old account id, or adopts it itself (auth.account_for_key),
-- so a linked key keeps its realms and saves.
DO $$
DECLARE r record; a text;
BEGIN
  FOR r IN SELECT i.subject, i.account FROM auth.identities i
           WHERE i.kind = 'nostr' AND i.subject ~ '^[0-9a-f]{64}$' AND NOT EXISTS (SELECT 1 FROM auth.nostr_keys k WHERE k.pubkey = i.subject)
           ORDER BY i.linked_ms, i.subject LOOP
    SELECT id INTO a FROM auth.accounts WHERE verse_account = r.account AND deleted_at IS NULL;
    IF a IS NOT NULL THEN
      INSERT INTO auth.nostr_keys (account, pubkey, label) VALUES (a, r.subject, 'linked') ON CONFLICT DO NOTHING;
    ELSE
      PERFORM auth.account_for_key(r.subject, 'linked');
    END IF;
  END LOOP;
END $$;

-- The account a key logs in to, made on first login: its old Verse account (n-<key>) when nobody has
-- adopted it, else a fresh one.
CREATE OR REPLACE FUNCTION auth.account_for_key(pk text, label_ text, OUT acct text, OUT made boolean)
 RETURNS record
 LANGUAGE plpgsql
AS $function$
DECLARE old text;
BEGIN
  made := false;
  IF coalesce(pk, '') !~ '^[0-9a-f]{64}$' THEN RAISE EXCEPTION 'auth: not a key' USING ERRCODE = 'invalid_parameter_value'; END IF;
  SELECT account INTO acct FROM auth.nostr_keys WHERE pubkey = pk;
  IF acct IS NOT NULL THEN
    UPDATE auth.nostr_keys SET last_used_at = now() WHERE pubkey = pk;
    RETURN;
  END IF;
  old := (SELECT a.id FROM verse.accounts a WHERE a.id = 'n-' || pk);
  -- An old account already adopted by another key keeps its one owner: this key starts fresh.
  IF old IS NOT NULL AND EXISTS (SELECT 1 FROM auth.accounts WHERE verse_account = old) THEN old := NULL; END IF;
  acct := auth.new_person(old);
  INSERT INTO auth.nostr_keys (account, pubkey, label, last_used_at) VALUES (acct, pk, nullif(left(label_, 80), ''), now());
  UPDATE auth.accounts SET team = auth.is_team(acct) WHERE id = acct;
  made := true;
END $function$;

-- A caller's Verse account: an account id's, or a key's account's, else the key's own n-<key>.
CREATE OR REPLACE FUNCTION auth.account_of(pk text)
 RETURNS text
 LANGUAGE sql
 STABLE
AS $function$
  SELECT CASE
    WHEN pk ~ '^acct_[a-z2-7]{26}$' THEN (SELECT a.verse_account FROM auth.accounts a WHERE a.id = pk AND a.deleted_at IS NULL)
    WHEN coalesce(pk, '') !~ '^[0-9a-f]{64}$' THEN NULL
    ELSE coalesce((SELECT a.verse_account FROM auth.nostr_keys k JOIN auth.accounts a ON a.id = k.account WHERE k.pubkey = pk),
                  'n-' || pk) END
$function$;

-- The groundwork's functions (access.c's /api/auth/v2, gone): its own me, links, device approval,
-- unlink and WorkOS. auth.unlink(acct, kind, id), the device routes agents.c calls
-- (device_start, device_poll, device_look(jsonb)) and device_link stay.
DROP FUNCTION auth.me(text);
DROP FUNCTION auth.link_nostr(text, text);
DROP FUNCTION auth.device_approve(text, jsonb);
DROP FUNCTION auth.unlink(text, jsonb);
DROP FUNCTION auth.workos_link(jsonb, text);
DROP FUNCTION auth.workos_spend(text);
DROP FUNCTION auth.workos_state(text);
DROP FUNCTION auth.session(text);

DROP TABLE auth.identities;
DROP TABLE auth.v0_sessions;
DROP TABLE auth.workos_states;
DROP TABLE analytics.admin_sessions;   -- the admin page logs in with a session of ours (admin.c, auth_login_key_redirect)
DROP SCHEMA signin CASCADE;            -- its two tables (challenges, sessions) and its functions; nothing else reads it (0158_auth_viewer_lines)

-- The old /api/playtest/join's email on verse.accounts (never verified): gone, with the column.
CREATE OR REPLACE FUNCTION auth.purge(acct text)
 RETURNS jsonb
 LANGUAGE plpgsql
AS $function$
DECLARE
  r record;
  vals text[];
  m bigint;
  counts jsonb := '{}';
  agent text;
  vacct text;
BEGIN
  SELECT verse_account INTO vacct FROM auth.accounts WHERE id = acct AND deleted_at IS NULL FOR UPDATE;
  IF vacct IS NULL THEN RETURN NULL; END IF;
  FOR agent IN SELECT id FROM auth.accounts WHERE owner = acct AND deleted_at IS NULL LOOP
    counts := counts || jsonb_build_object('agent:' || agent, auth.purge(agent));
  END LOOP;
  PERFORM auth.sessions_end_all(acct);
  -- A tombstone from here on: no session, and keep_a_way_in (0142) lets its last way in go.
  UPDATE auth.accounts SET deleted_at = now(), display_name = NULL, team = false WHERE id = acct;
  -- Values first (the keys, players and realms), then the rows: deleting the keys' rows last.
  FOR r IN SELECT l.* FROM auth.life_columns l
            WHERE l.on_purge <> 'keep' AND l.tbl <> 'auth.accounts' AND to_regclass(l.tbl) IS NOT NULL
            ORDER BY CASE l.holds WHEN 'world' THEN 0 WHEN 'player' THEN 1 WHEN 'pubkey' THEN 2 WHEN 'verse' THEN 3 ELSE 4 END,
                     CASE WHEN l.tbl = 'auth.nostr_keys' THEN 1 ELSE 0 END, l.tbl, l.col LOOP
    vals := auth.life_values(acct, r.holds);
    IF vals IS NULL OR cardinality(vals) = 0 THEN CONTINUE; END IF;
    IF r.on_purge = 'delete' THEN
      m := auth.life_delete(r.tbl::regclass, r.col, vals, 0);
    ELSE
      EXECUTE format('UPDATE %s SET %I = ''deleted'' WHERE %I::text = ANY($1)', r.tbl, r.col, r.col) USING vals;
      GET DIAGNOSTICS m = ROW_COUNT;
    END IF;
    IF m > 0 THEN counts := counts || jsonb_build_object(r.tbl || '.' || r.col, m); END IF;
  END LOOP;
  UPDATE verse.accounts SET npub = NULL, name = NULL WHERE id = vacct;
  counts := counts || jsonb_build_object('audit_forgotten', auth.audit_forget(acct));
  RETURN counts;
END $function$;
ALTER TABLE verse.accounts DROP COLUMN email;
