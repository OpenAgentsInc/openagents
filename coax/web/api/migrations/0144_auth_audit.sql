-- The append-only audit log (docs/auth/data-and-backups.md#audit-log, #13202): every login, failed
-- login, link, unlink, session end, recovery step, grant change, freeze, admin action, spend step-up
-- and Lightning Address change. A row is the time, the account, the actor (a session or an agent),
-- the action, the result and a small detail with no secrets and no plaintext PII (never an email,
-- a code, a token or an address: an IP only as its country).
--
-- Nobody but the table's owner (the migration role) may change or delete a row: the API's role is
-- granted INSERT and SELECT only, and a trigger refuses UPDATE and DELETE from any other role, so a
-- later blanket grant can't open it. Only auth.audit_age(), run by the owner, rewrites rows: after
-- two years a row loses its account id. A nightly job copies the day's rows to the locked backup
-- bucket (jobs.c, audit-export), so even the owner can't rewrite history after the fact.
CREATE TABLE auth.audit (
  id bigserial PRIMARY KEY,
  at timestamptz NOT NULL DEFAULT now(),
  account text,                          -- NULL for a failure no account is known for, and after two years
  actor text,                            -- the session id (ses_...) or agent account that did it
  action text NOT NULL CHECK (action ~ '^[a-z][a-z0-9_.]{1,47}$'),
  result text NOT NULL CHECK (result IN ('ok', 'refused', 'failed')),
  detail jsonb NOT NULL DEFAULT '{}' CHECK (length(detail::text) <= 2000)
);
CREATE INDEX audit_account ON auth.audit (account, at);
CREATE INDEX audit_at ON auth.audit (at);

CREATE FUNCTION auth.audit_frozen() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF current_user <> (SELECT pg_get_userbyid(c.relowner) FROM pg_class c WHERE c.oid = TG_RELID) THEN
    RAISE EXCEPTION 'auth.audit is append-only (docs/auth/data-and-backups.md)' USING ERRCODE = 'insufficient_privilege';
  END IF;
  RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END $$;
CREATE TRIGGER audit_frozen BEFORE UPDATE OR DELETE ON auth.audit FOR EACH ROW EXECUTE FUNCTION auth.audit_frozen();

REVOKE UPDATE, DELETE, TRUNCATE ON auth.audit FROM PUBLIC;
DO $$
DECLARE r text;
BEGIN
  FOREACH r IN ARRAY ARRAY['oa_api', 'oa_api_staging'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      EXECUTE format('REVOKE UPDATE, DELETE, TRUNCATE ON auth.audit FROM %I', r);
      EXECUTE format('GRANT SELECT, INSERT ON auth.audit TO %I', r);
      EXECUTE format('GRANT USAGE, SELECT ON SEQUENCE auth.audit_id_seq TO %I', r);
    END IF;
  END LOOP;
END $$;

-- One row. detail must already be free of secrets and PII; the API builds it from fixed keys.
CREATE FUNCTION auth.log(acct text, actor_ text, action_ text, result_ text, detail_ jsonb) RETURNS void LANGUAGE sql AS $$
  INSERT INTO auth.audit (account, actor, action, result, detail) VALUES (acct, actor_, action_, result_, coalesce(detail_, '{}'))
$$;

-- After two years a row loses its account id. SECURITY DEFINER: it runs as the owner, the only role
-- the trigger lets change a row; the API may call it, and it can do nothing else.
CREATE FUNCTION auth.audit_age() RETURNS int LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, auth AS $$
DECLARE n int;
BEGIN
  UPDATE auth.audit SET account = NULL WHERE at < now() - interval '2 years' AND account IS NOT NULL;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n;
END $$;

-- One day's rows (Chicago day) as JSON lines, for the nightly copy to the locked bucket.
CREATE FUNCTION auth.audit_day(day date) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg(jsonb_build_object('id', id, 'at', at, 'account', account, 'actor', actor, 'action', action,
                                                'result', result, 'detail', detail)::text, E'\n' ORDER BY id) || E'\n', '')
    FROM auth.audit WHERE (at AT TIME ZONE 'America/Chicago')::date = day
$$;

-- An account's own rows, for its data export.
CREATE FUNCTION auth.audit_of(acct text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object('at', at, 'action', action, 'result', result, 'detail', detail) ORDER BY id), '[]'::jsonb)
    FROM auth.audit WHERE account = acct
$$;
