-- roles.coax's guard probe (docs/auth/data-and-backups.md, #13194): as the owner role, outside a
-- migration, TRUNCATE and DROP of a scratch table must both be refused. It says "truncate refused"
-- and "drop refused" as notices; everything rolls back, and no existing table is touched.
BEGIN;
CREATE TABLE public.oa_guard_probe (x int);
SAVEPOINT a;
DO $$ BEGIN TRUNCATE public.oa_guard_probe; RAISE NOTICE 'TRUNCATE allowed'; EXCEPTION WHEN insufficient_privilege THEN RAISE NOTICE 'truncate refused'; END $$;
DO $$ BEGIN DROP TABLE public.oa_guard_probe; RAISE NOTICE 'DROP allowed'; EXCEPTION WHEN insufficient_privilege THEN RAISE NOTICE 'drop refused'; END $$;
ROLLBACK;
