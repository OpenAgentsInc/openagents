\set ON_ERROR_STOP on

DO $$
DECLARE
  runtime_owner text := :'runtime_owner_role';
  runtime_rw text := :'runtime_rw_role';
  khala_ro text := :'khala_ro_role';
  control_rw text := :'control_rw_role';
  control_schema_exists boolean;
  sync_table record;
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = 'runtime') THEN
    RAISE EXCEPTION 'runtime schema does not exist';
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = runtime_owner) THEN
    EXECUTE format('CREATE ROLE %I NOLOGIN', runtime_owner);
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = runtime_rw) THEN
    EXECUTE format('CREATE ROLE %I NOLOGIN', runtime_rw);
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = khala_ro) THEN
    EXECUTE format('CREATE ROLE %I NOLOGIN', khala_ro);
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = control_rw) THEN
    EXECUTE format('CREATE ROLE %I NOLOGIN', control_rw);
  END IF;

  EXECUTE format('ALTER SCHEMA runtime OWNER TO %I', runtime_owner);

  EXECUTE 'REVOKE ALL ON SCHEMA runtime FROM PUBLIC';
  EXECUTE format('REVOKE ALL ON SCHEMA runtime FROM %I', control_rw);
  EXECUTE format('GRANT USAGE ON SCHEMA runtime TO %I', runtime_rw);
  EXECUTE format('GRANT USAGE ON SCHEMA runtime TO %I', khala_ro);

  EXECUTE 'REVOKE ALL ON ALL TABLES IN SCHEMA runtime FROM PUBLIC';
  EXECUTE format('REVOKE ALL ON ALL TABLES IN SCHEMA runtime FROM %I', control_rw);
  EXECUTE format('REVOKE ALL ON ALL TABLES IN SCHEMA runtime FROM %I', khala_ro);
  EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA runtime TO %I', runtime_rw);

  FOR sync_table IN
    SELECT quote_ident(n.nspname) || '.' || quote_ident(c.relname) AS table_ref
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = 'runtime'
      AND c.relkind = 'r'
      AND (c.relname LIKE 'sync\_%' ESCAPE '\\' OR c.relname = 'khala_projection_checkpoints')
  LOOP
    EXECUTE format('GRANT SELECT ON TABLE %s TO %I', sync_table.table_ref, khala_ro);
  END LOOP;

  EXECUTE 'REVOKE ALL ON ALL SEQUENCES IN SCHEMA runtime FROM PUBLIC';
  EXECUTE format('REVOKE ALL ON ALL SEQUENCES IN SCHEMA runtime FROM %I', control_rw);
  EXECUTE format('REVOKE ALL ON ALL SEQUENCES IN SCHEMA runtime FROM %I', khala_ro);
  EXECUTE format('GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA runtime TO %I', runtime_rw);

  EXECUTE format('ALTER DEFAULT PRIVILEGES FOR ROLE %I IN SCHEMA runtime REVOKE ALL ON TABLES FROM PUBLIC', runtime_owner);
  EXECUTE format('ALTER DEFAULT PRIVILEGES FOR ROLE %I IN SCHEMA runtime REVOKE ALL ON SEQUENCES FROM PUBLIC', runtime_owner);
  EXECUTE format(
    'ALTER DEFAULT PRIVILEGES FOR ROLE %I IN SCHEMA runtime GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO %I',
    runtime_owner,
    runtime_rw
  );
  EXECUTE format(
    'ALTER DEFAULT PRIVILEGES FOR ROLE %I IN SCHEMA runtime GRANT USAGE, SELECT ON SEQUENCES TO %I',
    runtime_owner,
    runtime_rw
  );

  SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = 'control') INTO control_schema_exists;
  IF control_schema_exists THEN
    EXECUTE format('REVOKE ALL ON SCHEMA control FROM %I', runtime_rw);
    EXECUTE format('REVOKE ALL ON ALL TABLES IN SCHEMA control FROM %I', runtime_rw);
    EXECUTE format('REVOKE ALL ON ALL SEQUENCES IN SCHEMA control FROM %I', runtime_rw);

    EXECUTE format('REVOKE ALL ON SCHEMA control FROM %I', khala_ro);
    EXECUTE format('REVOKE ALL ON ALL TABLES IN SCHEMA control FROM %I', khala_ro);
    EXECUTE format('REVOKE ALL ON ALL SEQUENCES IN SCHEMA control FROM %I', khala_ro);
  END IF;
END
$$;
