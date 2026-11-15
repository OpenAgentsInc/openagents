# Migration 0206 checksum record repair

The task API check stopped at migration admission after rebasing onto the
Nostr foundation. The SQL was unchanged; its first committed checksum record
did not describe its bytes. Correct only the `0206_nostr_operations.sql`
line of `coax/web/api/migrations.sha256`. No SQL, schema snapshot or checker
behavior changes.

The first main commit containing this migration, `ca2bc801b25eb2e7b8d54cdd4128e89b776b3621`,
and the deployed build snapshot, `c6ed53d109`, both contain blob
`de9fdb49d8c3aab202d0048bec3cb689ee63894c` (987 bytes). The current file has
that same blob identity. SHA-256 of each is
`558b06d279ddcad6e4d1b197cd746f64261b6109554a480ea49f48f204a4171b`.
The manifest in both commits instead recorded
`bf92e2b07b03cfaebcdffd46445d3951d13b94c0bd7f2d72fa04eb72675dd072`.

The [Nostr delivery record](2026-11-15-nostr-foundation.md) identifies the
immutable image built from `c6ed53d109`, deployed to staging
`oa-api-staging-00232-xb7` and production `oa-api-00169-568`. Comparing
that snapshot, the migration's first main commit and the checkout proves
that this correction preserves the shipped SQL. Production tracks migration
names and will not execute this migration again for a manifest-only change.

Verification commands:

```sh
git show ca2bc801b2:coax/web/api/migrations/0206_nostr_operations.sql | shasum -a 256
git show c6ed53d109:coax/web/api/migrations/0206_nostr_operations.sql | shasum -a 256
shasum -a 256 coax/web/api/migrations/0206_nostr_operations.sql
git diff c6ed53d109 ca2bc801b2 -- coax/web/api/migrations/0206_nostr_operations.sql
coax/web/tool.sh coax/web/api/migration-check.coax --base 'ca2bc801b2^'
```

The pre-introduction base verifies every current checksum and migration
number without treating the erroneous record as an applied-file hash. The
ordinary check against the old main ledger necessarily rejects the repaired
record until the correction lands. After landing, the default check must
pass against the corrected main ledger. The immutability check remains in
force for every SQL file.

The explicit-base check passed: 214 migrations, highest number 206, zero
failures. `tools-check` passed for 2,033 tracked tool files. No database
effects were needed to establish this record correction.
