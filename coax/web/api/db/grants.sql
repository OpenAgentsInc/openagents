-- What each role may do (docs/auth/data-and-backups.md, #13194). Run by the
-- owner role, oa_analytics, in one database, with psql variables:
--   runtime  the API's role there (oa_api, oa_api_staging): rows only, no DDL, no TRUNCATE
--   readers  a comma-separated list of SELECT-only roles (oa_dump, oa_readonly), or ''
-- Idempotent. Default privileges cover what later migrations make.
\set ON_ERROR_STOP 1
SET lock_timeout = '3s';

SELECT format('GRANT USAGE ON SCHEMA %I TO %I', n.nspname, :'runtime')
FROM pg_namespace n WHERE n.nspowner = 'oa_analytics'::regrole OR n.nspname = 'public' \gexec
SELECT format('GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA %I TO %I', n.nspname, :'runtime')
FROM pg_namespace n WHERE n.nspowner = 'oa_analytics'::regrole OR n.nspname = 'public' \gexec
SELECT format('GRANT USAGE, SELECT, UPDATE ON ALL SEQUENCES IN SCHEMA %I TO %I', n.nspname, :'runtime')
FROM pg_namespace n WHERE n.nspowner = 'oa_analytics'::regrole OR n.nspname = 'public' \gexec
SELECT format('ALTER DEFAULT PRIVILEGES FOR ROLE oa_analytics GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO %I', :'runtime') \gexec
SELECT format('ALTER DEFAULT PRIVILEGES FOR ROLE oa_analytics GRANT USAGE, SELECT, UPDATE ON SEQUENCES TO %I', :'runtime') \gexec
SELECT format('ALTER DEFAULT PRIVILEGES FOR ROLE oa_analytics GRANT USAGE ON SCHEMAS TO %I', :'runtime') \gexec

-- Readers: SELECT on every table and sequence, now and later.
SELECT format('GRANT USAGE ON SCHEMA %I TO %I', n.nspname, r)
FROM pg_namespace n, unnest(string_to_array(nullif(:'readers', ''), ',')) r
WHERE n.nspowner = 'oa_analytics'::regrole OR n.nspname = 'public' \gexec
SELECT format('GRANT SELECT ON ALL TABLES IN SCHEMA %I TO %I', n.nspname, r)
FROM pg_namespace n, unnest(string_to_array(nullif(:'readers', ''), ',')) r
WHERE n.nspowner = 'oa_analytics'::regrole OR n.nspname = 'public' \gexec
SELECT format('GRANT SELECT ON ALL SEQUENCES IN SCHEMA %I TO %I', n.nspname, r)
FROM pg_namespace n, unnest(string_to_array(nullif(:'readers', ''), ',')) r
WHERE n.nspowner = 'oa_analytics'::regrole OR n.nspname = 'public' \gexec
SELECT format('ALTER DEFAULT PRIVILEGES FOR ROLE oa_analytics GRANT SELECT ON TABLES TO %I', r)
FROM unnest(string_to_array(nullif(:'readers', ''), ',')) r \gexec
SELECT format('ALTER DEFAULT PRIVILEGES FOR ROLE oa_analytics GRANT SELECT ON SEQUENCES TO %I', r)
FROM unnest(string_to_array(nullif(:'readers', ''), ',')) r \gexec
SELECT format('ALTER DEFAULT PRIVILEGES FOR ROLE oa_analytics GRANT USAGE ON SCHEMAS TO %I', r)
FROM unnest(string_to_array(nullif(:'readers', ''), ',')) r \gexec

-- The TRUNCATE trigger on every existing table, one short transaction each.
SELECT format('SELECT oa_guard.attach(%L::regclass)', c.oid::regclass::text)
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind IN ('r', 'p') AND c.relowner = 'oa_analytics'::regrole
  AND NOT EXISTS (SELECT 1 FROM pg_trigger t WHERE t.tgrelid = c.oid AND t.tgname = 'oa_guard_no_truncate') \gexec
