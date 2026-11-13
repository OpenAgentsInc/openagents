# Data, storage and backups

Status: design, 2026-11-11. The roles, guard, retained backups, locked
dumps and drill are built (#13194). Part of [OpenAgents authentication](README.md).

## Where it lives

The `auth` schema in the API's database, `oa_analytics` on the Cloud SQL
instance `openagents-production-pg` (Postgres 18, project
`openagentsgemini`, `us-central1`). Staging uses `oa_analytics_staging`
on the same instance. We keep one instance: it costs about $50 a month,
and a second one would double that for no safety gain that the backups
below don't already give.

## Schema

A sketch, not a migration. Every id is random; every secret is a digest.

| Table | Columns that matter |
| --- | --- |
| `auth.accounts` | `id` (`acct_...`), `kind` (person, agent, service), `owner` (agents), `display_name`, `team`, `frozen_at`, `session_epoch`, `created_at`, `deleted_at`, `merged_into` |
| `auth.emails` | `account`, `lookup` (HMAC-SHA256 of the normalized address, unique), `sealed` (ciphertext), `verified_at`, `primary` |
| `auth.passkeys` | `account`, `credential_id` (unique), `user_handle`, `public_key` (COSE), `alg`, `sign_count`, `backup_eligible`, `backed_up`, `aaguid`, `transports`, `name`, `prf_salt`, `last_used_at` |
| `auth.nostr_keys` | `account`, `pubkey` (unique), `label` (`phone`, `cli:<machine>`, `extension`, `bunker`), `last_used_at` |
| `auth.devices` | `account`, `public_key` (P-256), `platform`, `name`, `created_at`, `last_seen_at`, `revoked_at`; `push_service`, `push_token`, `push_at`: where the device's security notices go (the token Apple or Google gave the install; gone when the device is removed) |
| `auth.sessions` | `token_sha256` (key), `account`, `via` (authenticator id), `device`, `level`, `l1_at`, `l2_at`, `created_at`, `last_seen_at`, `expires_at`, `epoch` |
| `auth.refresh` | `token_sha256`, `family`, `session`, `spent_at` |
| `auth.agent_grants` | `agent`, `owner`, `scopes`, `sats_per_day`, `sats_per_payment`, `hosts`, `expires_at`, `suspended_at`, `approved_session` |
| `auth.api_keys` | `id` (shown), `secret_sha256`, `agent`, `grant`, `created_at`, `last_used_at`, `revoked_at` |
| `auth.recovery` | `account`, `code_scrypt`, `salt`, `created_at`, `spent_at`; and pending recoveries with `ready_at`, `cancelled_at` |
| `auth.pending` | Login attempts: `id`, `code_sha256`, `link_sha256`, `target_lookup`, `tries`, `expires_at`, `browser_sha256` |
| `auth.nip98_seen` | Event ids already used, kept 10 minutes past their window |
| `auth.connections` | `account`, `provider` (github, google), `subject`, `scopes`, `token_sealed`, `refreshed_at` |
| `auth.audit` | Append-only; see below |

There is no phone table and no phone column anywhere: we never collect
phone numbers (decision D3). Lightning Address names live in
`pay.addresses` ([Lightning Addresses](lightning-addresses.md#data)).

Constraints do the policing: unique `lookup`, `credential_id`, `pubkey`;
a check that an agent account has an owner and that owner is a person; a
trigger that refuses deleting an account's last login authenticator.

## Encryption at rest

Cloud SQL encrypts the disk. On top of that, fields that identify a
person are sealed by the API before they reach the database, so a leaked
dump or a read-only agent sees ciphertext.

| Field | Protection |
| --- | --- |
| Email | XChaCha20-Poly1305 (`coax/lib/crypto/xchacha20_poly1305.coax`), associated data = table, column and account id. Lookup by HMAC-SHA256 with a separate pepper |
| OAuth connection tokens | Same sealing |
| Tokens, codes, link tokens | SHA-256 only |
| Recovery codes | scrypt, N = 2^17 |
| Passkey and Nostr public keys | Plain: they're public |
| IP addresses | Country and network kept 30 days; the full address only as a keyed hash, 30 days |

**Keys.** Two Secret Manager secrets, `oa-auth-seal-key` and
`oa-auth-lookup-pepper` (and `-staging`), read only by the API's runtime
service account. Each sealed value carries a key version byte, so we
rotate by adding a version and re-sealing in the background. The keys
are also backed up offline: losing them would turn every email into
noise. Each version is sealed with age to the same two keys as the dumps
(`coax/web/api/backup/recipients.txt`) and kept in
`gs://openagentsgemini-db-backups/keys/`, which the lifecycle rule leaves
alone (it deletes only under `oa_analytics/`, `oa_analytics-auth/` and
`drills/`); the offline key that opens the dumps opens it too. A new key
version gets a new file there.

## Audit log

`auth.audit` records every login, failed login, link, unlink, session
end, recovery step, grant change, freeze, admin action and spend
step-up: time, account, actor (session or agent), action, result, and a
small JSON detail with no secrets and no plaintext PII. The API's role
can `INSERT` and `SELECT` it, never `UPDATE` or `DELETE`, and a trigger
refuses both to anyone but the table's owner. The six-hourly dump of the
`auth` schema (below) copies it to the locked bucket, so even a database
owner can't rewrite history after the fact. Kept two years; then the
daily `auth-sweep` job (`coax/web/api/jobs.c`) clears rows' account id.
Built in `migrations/0144_auth_audit.sql` (#13202); `test.sh` checks the
runtime role can't change a row.

## Backups

### Today (built 2026-11-11, #13194)

| Control | State |
| --- | --- |
| Automated daily backups | On, 30 kept, at 08:00 UTC |
| Point-in-time recovery | On, 7 days of transaction logs |
| Instance deletion protection | On |
| Availability | Zonal (one zone) |
| Backups kept if the instance is deleted | On (`--retain-backups-on-delete`) |
| Logical dumps outside Cloud SQL | Nightly at 09:30 UTC: Cloud Run job `oa-db-dump`, scheduler `oa-db-dump-nightly` |
| Locked retention | `gs://openagentsgemini-db-backups`: 35 days, **locked**. Nobody, us included, can delete or overwrite a dump before it is 35 days old; a lifecycle rule deletes dumps and drills at 40 days; `keys/` (the sealed auth keys) is kept |
| Encryption of dumps | [age](https://age-encryption.org), to two keys in `coax/web/api/backup/recipients.txt`: the offline key (the owner holds the private half, on paper) and the drill key (Secret Manager `oa-db-backup-drill-age-key`). The job can encrypt but can't decrypt |
| The `auth` schema alone | Every six hours at :15 UTC: Cloud Run job `oa-db-dump-auth`, scheduler `oa-db-dump-auth-6h`, into `oa_analytics-auth/` in the same locked bucket, encrypted the same way (#13200) |
| Restore drill | Monthly, the 3rd at 12:00 UTC: Cloud Run job `oa-db-drill`, scheduler `oa-db-drill-monthly`. Results in `gs://openagentsgemini-db-backups/drills/` |
| Who can drop tables | Nobody outside a migration. The API serves as `oa_api`, which has no DDL, no `TRUNCATE` and owns nothing. Migrations run as `oa_analytics` in the job `oa-api-migrate-production`, which `deploy.sh` runs before each rollout. An event trigger refuses `DROP` and a trigger on every table refuses `TRUNCATE` unless the transaction set `oa.allow_ddl` |
| Who can read the owner's connection string | The migration jobs, and still the automation account until the owner's step in `NEEDS_OWNER.md` (D9) |

Not yet done: the separate backup project. The automation account can't
create projects or change project IAM, so the bucket is in
`openagentsgemini` for now; its lock already stops anyone deleting dumps
early. Moving it to `openagents-backups` is the owner's step in
`NEEDS_OWNER.md`.

### How it's built

- **Roles:** `coax/web/tool.sh coax/web/api/db/roles.coax staging|production`
  makes the roles, puts their connection strings in Secret Manager
  (passwords never printed; Postgres gets SCRAM verifiers), applies
  `guard.sql` and `grants.sql`, and checks the result: from the catalog
  (`checks.sql`), and by trying `DROP` and `TRUNCATE` on a scratch table
  inside a transaction that rolls back (`probe.sql`). It never touches a table's rows. Run it again after adding a role.
- **Migrations:** `api --migrations /migrations --migrate-only` applies
  them and exits. `coax/web/api/deploy.sh` runs it as the Cloud Run job
  `oa-api-migrate-<env>`, with the same image, before the service rollout;
  a failed migration stops the deploy. The service itself no longer
  migrates at startup. Local tests (`test.sh`) still pass `--migrations`.
- **Dumps:** `backup dump` (`coax/web/api/backup/backup.coax`, the job
  image's entrypoint) takes one snapshot,
  counts every table's rows in it, `pg_dump`s it (custom format), encrypts
  with age, and writes `oa_analytics/<time>.dump.age` and
  `<time>.counts.tsv` create-only. `coax/web/api/backup/deploy.sh` builds
  the image (Postgres 18 Alpine plus age, with `backup.coax` compiled for
  Linux) and makes the jobs and schedules.
- **Decrypt by hand:** `age -d -i <offline key file> <time>.dump.age > db.dump`,
  then `pg_restore -d <empty database> --no-owner --no-acl db.dump` with
  Postgres 18 tools.

### The plan

1. **Retain Cloud SQL backups on instance deletion**
   (`gcloud sql instances patch openagents-production-pg --retain-backups-on-delete`),
   so even deleting the instance leaves its backups. Done.
2. **A separate backup project**, `openagents-backups`, holding one bucket
   `openagents-backups-db` with:
   - a retention policy of 35 days, **locked** (Bucket Lock): no one,
     including a project owner, can delete or overwrite an object before
     it is 35 days old, and the policy can't be shortened;
   - the only writer is the dump job's account, with
     `storage.objectCreator`, and the drill's read with
     `storage.objectViewer`. No account in the main project has any other
     role in this project; only the owner's human login does.
   Owner's step (D9). Until then the locked bucket above stands in.
3. **Logical dumps:** nightly, whole database, encrypted to an offline age
   key. Done. (A 6-hourly `auth` dump comes with the `auth` schema.)
4. **The seal key and pepper** are exported once, sealed to the same
   offline key, and stored in the locked bucket, so a restore can read
   emails. With the `auth` schema.
5. **Roles** (below) so the running API can't drop tables at all. Done.

### Database roles

| Role | Can | Can't | Used by |
| --- | --- | --- | --- |
| `oa_analytics` | Own every schema; run migrations | Serve requests | The migration jobs `oa-api-migrate-staging` and `oa-api-migrate-production`, run by `deploy.sh` before each rollout. Secrets `oa-analytics-database-url` and `-staging` |
| `oa_api`, `oa_api_staging` | `SELECT`, `INSERT`, `UPDATE`, `DELETE` on tables; sequences; functions | `CREATE`, `DROP`, `ALTER`, `TRUNCATE`; owning anything | The API. Secrets `oa-api-database-url`, `oa-api-staging-database-url` |
| `oa_readonly` | `SELECT` on every table | Anything else | Agents, analytics, debugging. Secret `oa-analytics-readonly-database-url` |
| `oa_dump` | `SELECT` on every table, for `pg_dump` | Writes | The dump and drill jobs. Secret `oa-db-dump-database-url` |
| `oa_admin` | A `cloudsqlsuperuser`: make roles, own the guard | Serve requests | `roles.coax` only. Secret `oa-pg-admin-database-url` |

Default privileges give `oa_api` and the readers what later migrations
make. When `auth.audit` lands, its migration revokes `UPDATE` and `DELETE`
on it from `oa_api`.

Plus the guard (`coax/web/api/db/guard.sql`, schema `oa_guard`, owned by
`oa_admin` so the owner role can't rewrite it): an event trigger on
`ddl_command_start` refuses every `DROP` unless the transaction set
`oa.allow_ddl`, which only the migration runner does (to the migration's
file name). Event triggers don't see `TRUNCATE`, so every table carries a
`BEFORE TRUNCATE` trigger with the same test, and a second event trigger
adds it to each table a migration creates. Cloud SQL's own roles pass. This
catches the case where someone does get `oa_analytics`.

**IAM changes that go with it** (owner-run; the automation account can't
change project IAM; exact steps in `NEEDS_OWNER.md`, D9):

- the migration jobs move to their own account, `oa-api-migrator`
  (made, with read access to the owner secrets; it needs
  `roles/cloudsql.client`);
- a deny policy stops the automation account reading the owner, runtime,
  dump and admin connection strings and resetting Cloud SQL passwords; it
  keeps `oa-analytics-readonly-database-url`;
- the backup project grants nothing to the main project's accounts except
  the dump job's create-only write and the drill's read.

Even then, an account that can redeploy a Cloud Run job can make that job
run as the owner role: deploys need to. The guard and the locked dumps are
what stop that from losing data.

### The "an agent nukes the production database" runbook

| Disaster | Recover from | Data lost (RPO) | Time to recover (RTO) |
| --- | --- | --- | --- |
| A table or schema dropped, rows deleted | Point-in-time clone to the minute before (`gcloud sql instances clone ... --point-in-time`), copy the damaged tables back | Under 1 minute | About 1 hour |
| The whole database dropped | Point-in-time clone, then point the API at the clone | Under 1 minute | About 1 hour |
| The instance deleted (needs owner rights) | Retained Cloud SQL backup, restored to a new instance | Up to 24 hours | 1 to 2 hours |
| Cloud SQL backups also gone, or bad for days | Latest locked dump in `gs://openagentsgemini-db-backups` (later the backup project) | Up to 24 hours (nightly at 09:30 UTC) | About 30 minutes: measured restore of the data is seconds (below); most of it is making a Cloud SQL instance and pointing the API at it |
| The main project is compromised or deleted | Locked dumps in the backup project, restored into a new project | Same | Half a day |

**Steps when it happens:**

1. Stop writes: scale `oa-api` to zero, or turn on maintenance mode, so
   nothing writes on top of the damage.
2. Find the time: the audit log and Cloud SQL's own logs show the bad
   statement's time.
3. Clone to one minute before it, under a new name (`oa-pg-restore-<date>`).
   Never restore over the live instance.
4. Check the clone: row counts of `auth.accounts`, `auth.sessions` and the
   game tables against yesterday's dump report.
5. Point the API at the clone (one secret version), redeploy, smoke test.
6. Keep the damaged instance untouched until the owner, by name, says to
   delete it.
7. Write a postmortem in `docs/postmortems/`.

### Restore drills

Monthly, on the 3rd at 12:00 UTC, the Cloud Run job `oa-db-drill`
(`backup drill`, `backup.coax`):

1. takes the newest dump from the locked bucket,
2. decrypts it with the drill key (Secret Manager
   `oa-db-backup-drill-age-key`; a separate key pair from the paper one,
   and every dump is encrypted to both),
3. restores it into a throwaway Postgres 18 inside the job's own
   container, so there is no instance to forget and delete,
4. checks every table's row count against the counts taken in the dump's
   own snapshot (they must match exactly), and notes how many rows
   production has changed since,
5. writes the result to `gs://openagentsgemini-db-backups/drills/` and
   the job log. A failed drill fails the job execution.

First drill, 2026-11-11 (`drills/20261111T211024Z.json`):

| Measure | Result |
| --- | --- |
| Dump | `oa_analytics/20261111T210517Z.dump.age`, 8.0 MB encrypted, taken in 7 seconds |
| Restored | 135 tables, 273,369 rows, every count equal to the dump's; 0 restore errors |
| Restore time | 3 seconds; the whole drill job 49 seconds, start to finish |
| Offline key | Decrypted the same dump on the owner's Mac (`PGDMP` header) |
| Lock | Deleting a dump was refused: "subject to bucket's retention policy" |

**RPO:** under 1 minute while Cloud SQL and its point-in-time recovery
are intact; up to 24 hours from the locked dumps alone. **RTO:** about 1
hour for a point-in-time clone; about 30 minutes from a locked dump into a
new instance (minutes to create it, seconds to restore, then one secret
version and a redeploy).

Not built yet: opening an issue on a failed drill, and a login replay
against the restore (it comes with the `auth` schema).

### What it costs

| Item | Monthly |
| --- | --- |
| Cloud SQL automated backups and logs, for a database under 10 GB | About $2 |
| Locked bucket: 35 to 40 days of nightly dumps, about 8 MB each today | Under $0.10 |
| Dump job runs | Under $1 |
| Monthly drill: a minute of a Cloud Run job, no instance | Under $0.01 |
| **Total added** | **About $5** |
| Optional: high availability (regional) | About +$50 |
| Optional: cross-region read replica | About +$50 |

Both options fit under the $1,500 cap but buy uptime, not safety. We
recommend neither now (decision D11).

## Data export and account deletion

Built in #13214 (`coax/web/api/auth-life.c`, its rules in
`auth-life-policy.coax`, migration `0149_auth_life.sql`).

### The export

DOWNLOAD MY DATA on `/settings/data` (`GET /api/auth/export`) needs L2, or,
on an account with only email, an email login. A frozen account can't
export; one waiting to be deleted can. Every export is in the audit log.

The file is JSON lines (`openagents-ACCOUNT.jsonl`), one object per
line, each with a `type`:

| `type` | What it holds |
| --- | --- |
| `export` | The header: `format` (`openagents.export.v1`), `account`, `made_ms`, `docs` (this section) |
| `account` | The account row: kind, created, Verse account, display name, freeze and deletion times |
| `email` | Each email address, opened from its seal for this file only |
| `agents` | Your agents as their proto message, `openagents.account.v1alpha1.AgentList` (`proto/openagents/account/v1alpha1/agents.proto`), base64 |
| `row` | Every other row that points at you, by `auth.life_columns`: keys (public parts), sessions (no tokens), grants, realms, characters, saves, chats, payments and credit history, with its `table` |
| `audit` | Your security log, oldest first |

The file never carries token digests, seals, lookup hashes or recovery
code hashes. It is capped at 64 MB.

### Deletion

DELETE MY ACCOUNT, in the app and on the web at `/settings/delete`, as both
Apple (guideline 5.1.1(v)) and Google Play require; Google also requires a
web link to start deletion
([Google Play account deletion](https://support.google.com/googleplay/android-developer/answer/13327111)).
It needs the strongest proof from the last 5 minutes (L2-fresh, or a new
email code on an account with only email).

- The account is frozen at once and every login ends. The page shows the
  date and time it will be deleted, 30 days on.
- Until then, log in any way and choose CANCEL DELETION. Cancelling needs no
  fresh proof: anyone who can log in may stop a deletion.
- After 30 days the auth sweep (`auth.purge_due()`) deletes the emails,
  keys, passkeys, sessions, grants, agents, realms, characters, saves and
  every other row `auth.life_columns` lists, and sets the audit rows'
  account to null.
- **What we keep:** payments to us (amounts and dates, unlinked), the audit
  rows without the account id, the account id as a tombstone so it is never
  reused, and every Lightning Address name that was ever paid, held so that
  payments meant for you never reach someone else. Names never paid are
  released.
- **Backups:** the nightly backups are locked for 35 days and can't be
  deleted early; the account's data leaves the last of them by day 40.
- **Money first:** the page shows any service credit left (spend it first;
  credit can't be withdrawn) and reminds you that your wallet lives on your
  devices: save its recovery words first. We hold no copy of a wallet today;
  if the encrypted backup in [wallets-and-custody.md](wallets-and-custody.md)
  is built, deleting deletes it too.
