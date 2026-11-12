# API migrations

The API applies each `NNNN_words.sql` file here once, in name order, and
records it by name in `schema_migrations` (`api.c`, `migrate()`). It keeps no
checksum, so the rules below are enforced by a check instead.

## Add a migration

1. Rebase onto `origin/main` first.
2. Take the next number: one above the highest file here. Name it
   `NNNN_lowercase_words.sql`.
3. Add its hash line: `coax/web/tool.sh coax/web/api/migration-check.coax --write`.
4. Regenerate the schema snapshot:
   `coax/remote/remote.sh run -- coax/web/tool.sh coax/web/api/migration-check.coax --schema --write`.
5. Commit the migration, `migrations.sha256` and `schema.sql` together.

## On a clash

If another migration took your number before you landed, rebase and rename
yours to the next free number, then run steps 3 and 4 again. Never renumber a
migration that is already on main.

## Never edit a landed migration

Production has applied it by name and won't run it again. An edit never
reaches production, and a renamed file runs a second time. Put the change in
a new migration. `CREATE OR REPLACE` a function again there if you need to.

## Where a definition lives now

`coax/web/api/schema.sql` is the schema after every migration, generated
from a fresh Postgres. To find a function's or a table's current definition,
look there, not in the migration that first made it.

## The check

`coax/web/api/migration-check.coax` fails when:

- two files share a number (except the pairs on numbers taken twice before
  #13301, marked `shared` in `migrations.sha256`);
- a new file's number is not above the highest number on the base commit;
- a landed file, or its line in `migrations.sha256`, changed, or the file
  was removed or renamed;
- `schema.sql` differs from a fresh database (`--schema`).

It runs in `coax/web/api/test.sh` and in the after-landing report checks
(`coax/remote/check.sh`, `migrations`).
