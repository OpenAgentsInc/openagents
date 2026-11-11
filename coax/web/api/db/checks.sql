-- roles.coax's catalog check (docs/auth/data-and-backups.md, #13194): one line for each thing
-- that is wrong, none when the runtime role (psql variable runtime) can't run DDL or TRUNCATE,
-- owns nothing, can read and write every table the owner made, and the guard is in place.
-- Catalog reads only. Run as oa_admin: psql -v runtime=oa_api -Atf checks.sql
SELECT 'runtime may ' || p || ' on ' || c.oid::regclass
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace, unnest(ARRAY['TRUNCATE', 'TRIGGER', 'REFERENCES']) p
WHERE c.relkind IN ('r', 'p') AND n.nspname NOT LIKE 'pg_%' AND n.nspname <> 'information_schema'
  AND has_table_privilege(:'runtime', c.oid, p)
UNION ALL
SELECT 'runtime may CREATE in schema ' || nspname FROM pg_namespace
WHERE nspname NOT LIKE 'pg_%' AND nspname <> 'information_schema' AND has_schema_privilege(:'runtime', oid, 'CREATE')
UNION ALL
SELECT 'runtime can''t ' || p || ' ' || c.oid::regclass
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace, unnest(ARRAY['SELECT', 'INSERT', 'UPDATE', 'DELETE']) p
WHERE c.relkind IN ('r', 'p') AND c.relowner = 'oa_analytics'::regrole AND NOT has_table_privilege(:'runtime', c.oid, p)
UNION ALL
SELECT 'runtime owns ' || c.oid::regclass FROM pg_class c WHERE c.relowner = :'runtime'::regrole
UNION ALL
SELECT 'no TRUNCATE guard on ' || c.oid::regclass
FROM pg_class c WHERE c.relkind IN ('r', 'p') AND c.relowner = 'oa_analytics'::regrole
  AND NOT EXISTS (SELECT 1 FROM pg_trigger t WHERE t.tgrelid = c.oid AND t.tgname = 'oa_guard_no_truncate')
UNION ALL
SELECT 'event trigger missing: ' || e FROM unnest(ARRAY['oa_guard_refuse_drop', 'oa_guard_new_tables']) e
WHERE NOT EXISTS (SELECT 1 FROM pg_event_trigger WHERE evtname = e AND evtenabled <> 'D');
