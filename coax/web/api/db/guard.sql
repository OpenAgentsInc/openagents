-- The DROP and TRUNCATE guard (docs/auth/data-and-backups.md, #13194).
-- Run once per database by a cloudsqlsuperuser (roles.sh, as oa_admin):
-- event triggers need it. Idempotent.
--
-- Outside a migration, DROP of any object and TRUNCATE of any table fail,
-- whoever asks, including the owner role. A migration sets
-- oa.allow_ddl = '<migration file>' in its own transaction (api.c, migrate()),
-- which lets it through. Cloud SQL's own maintenance roles pass.
CREATE SCHEMA IF NOT EXISTS oa_guard;
REVOKE ALL ON SCHEMA oa_guard FROM PUBLIC;
GRANT USAGE ON SCHEMA oa_guard TO PUBLIC;

CREATE OR REPLACE FUNCTION oa_guard.allowed() RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT coalesce(current_setting('oa.allow_ddl', true), '') <> ''
      OR session_user IN ('cloudsqladmin', 'cloudsqlagent')
$$;

CREATE OR REPLACE FUNCTION oa_guard.refuse_drop() RETURNS event_trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NOT oa_guard.allowed() THEN
    RAISE EXCEPTION 'oa_guard: % refused outside a migration (docs/auth/data-and-backups.md)', tg_tag
      USING ERRCODE = 'insufficient_privilege';
  END IF;
END $$;

CREATE OR REPLACE FUNCTION oa_guard.refuse_truncate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NOT oa_guard.allowed() THEN
    RAISE EXCEPTION 'oa_guard: TRUNCATE %.% refused outside a migration (docs/auth/data-and-backups.md)', TG_TABLE_SCHEMA, TG_TABLE_NAME
      USING ERRCODE = 'insufficient_privilege';
  END IF;
  RETURN NULL;
END $$;

-- Puts the TRUNCATE trigger on one table; runs as whoever owns it.
CREATE OR REPLACE FUNCTION oa_guard.attach(t regclass) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgrelid = t AND tgname = 'oa_guard_no_truncate') THEN
    EXECUTE format('CREATE TRIGGER oa_guard_no_truncate BEFORE TRUNCATE ON %s FOR EACH STATEMENT EXECUTE FUNCTION oa_guard.refuse_truncate()', t);
  END IF;
END $$;

-- Every table a migration makes gets the TRUNCATE trigger as it's made.
CREATE OR REPLACE FUNCTION oa_guard.guard_new_tables() RETURNS event_trigger LANGUAGE plpgsql AS $$
DECLARE o record;
BEGIN
  FOR o IN SELECT objid FROM pg_event_trigger_ddl_commands()
           WHERE object_type = 'table' AND schema_name NOT LIKE 'pg_temp%' LOOP
    PERFORM oa_guard.attach(o.objid::regclass);
  END LOOP;
END $$;

DROP EVENT TRIGGER IF EXISTS oa_guard_refuse_drop;
CREATE EVENT TRIGGER oa_guard_refuse_drop ON ddl_command_start
  WHEN TAG IN ('DROP TABLE', 'DROP SCHEMA', 'DROP VIEW', 'DROP MATERIALIZED VIEW', 'DROP SEQUENCE',
               'DROP FUNCTION', 'DROP PROCEDURE', 'DROP ROUTINE', 'DROP TYPE', 'DROP DOMAIN',
               'DROP INDEX', 'DROP TRIGGER', 'DROP RULE', 'DROP POLICY', 'DROP OWNED', 'DROP EXTENSION',
               'DROP FOREIGN TABLE', 'DROP AGGREGATE', 'DROP OPERATOR')
  EXECUTE FUNCTION oa_guard.refuse_drop();

DROP EVENT TRIGGER IF EXISTS oa_guard_new_tables;
CREATE EVENT TRIGGER oa_guard_new_tables ON ddl_command_end
  WHEN TAG IN ('CREATE TABLE', 'CREATE TABLE AS', 'SELECT INTO')
  EXECUTE FUNCTION oa_guard.guard_new_tables();
