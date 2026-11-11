# Data, storage and backups

Status: design, 2026-11-11. Part of [OpenAgents authentication](README.md).

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
| `auth.phones` | `account`, `lookup` (HMAC of E.164, unique), `sealed`, `country`, `verified_at` |
| `auth.passkeys` | `account`, `credential_id` (unique), `user_handle`, `public_key` (COSE), `alg`, `sign_count`, `backup_eligible`, `backed_up`, `aaguid`, `transports`, `name`, `prf_salt`, `last_used_at` |
| `auth.nostr_keys` | `account`, `pubkey` (unique), `label` (`phone`, `cli:<machine>`, `extension`, `bunker`), `last_used_at` |
| `auth.devices` | `account`, `public_key` (P-256), `platform`, `name`, `created_at`, `last_seen_at`, `revoked_at` |
| `auth.sessions` | `token_sha256` (key), `account`, `via` (authenticator id), `device`, `level`, `l1_at`, `l2_at`, `created_at`, `last_seen_at`, `expires_at`, `epoch` |
| `auth.refresh` | `token_sha256`, `family`, `session`, `spent_at` |
| `auth.agent_grants` | `agent`, `owner`, `scopes`, `sats_per_day`, `sats_per_payment`, `hosts`, `expires_at`, `suspended_at`, `approved_session` |
| `auth.api_keys` | `id` (shown), `secret_sha256`, `agent`, `grant`, `created_at`, `last_used_at`, `revoked_at` |
| `auth.recovery` | `account`, `code_scrypt`, `salt`, `created_at`, `spent_at`; and pending recoveries with `ready_at`, `cancelled_at` |
| `auth.pending` | Login attempts: `id`, `code_sha256`, `link_sha256`, `target_lookup`, `tries`, `expires_at`, `browser_sha256` |
| `auth.nip98_seen` | Event ids already used, kept 10 minutes past their window |
| `auth.connections` | `account`, `provider` (github, google), `subject`, `scopes`, `token_sealed`, `refreshed_at` |
| `auth.audit` | Append-only; see below |

Constraints do the policing: unique `lookup`, `credential_id`, `pubkey`;
a check that an agent account has an owner and that owner is a person; a
trigger that refuses deleting an account's last login authenticator.

## Encryption at rest

Cloud SQL encrypts the disk. On top of that, fields that identify a
person are sealed by the API before they reach the database, so a leaked
dump or a read-only agent sees ciphertext.

| Field | Protection |
| --- | --- |
| Email, phone | XChaCha20-Poly1305 (`coax/lib/crypto/xchacha20_poly1305.coax`), associated data = table, column and account id. Lookup by HMAC-SHA256 with a separate pepper |
| OAuth connection tokens | Same sealing |
| Tokens, codes, link tokens | SHA-256 only |
| Recovery codes | scrypt, N = 2^17 |
| Passkey and Nostr public keys | Plain: they're public |
| IP addresses | Country and network kept 30 days; the full address only as a keyed hash, 30 days |

**Keys.** Two Secret Manager secrets, `oa-auth-seal-key` and
`oa-auth-lookup-pepper` (and `-staging`), read only by the API's runtime
service account. Each sealed value carries a key version byte, so we
rotate by adding a version and re-sealing in the background. The keys
are also backed up offline (below): losing them would turn every email
into noise.

## Audit log

`auth.audit` records every login, failed login, link, unlink, session
end, recovery step, grant change, freeze, admin action and spend
step-up: time, account, actor (session or agent), action, result, and a
small JSON detail with no secrets and no plaintext PII. The API's role
can `INSERT` and `SELECT` it, never `UPDATE` or `DELETE`. A nightly job
copies the day's rows to the locked bucket, so even a database owner
can't rewrite history after the fact. Kept two years; then rows lose
their account id.

## Backups

### Today (checked read-only, 2026-11-11)

| Control | State |
| --- | --- |
| Automated daily backups | On, 30 kept, at 08:00 UTC |
| Point-in-time recovery | On, 7 days of transaction logs |
| Instance deletion protection | On |
| Availability | Zonal (one zone) |
| Backups kept if the instance is deleted | Not set |
| Logical dumps outside Cloud SQL | None scheduled. `gs://openagentsgemini-coder-backup-20261028` holds a one-off dump of the old accounts database |
| Locked retention on any backup bucket | None. The two backup buckets have 7-day soft delete only |
| Who can drop tables | The API applies its own migrations at startup (`api.c`, `migrate()`) as `oa_analytics`, so the role it serves requests with can run DDL, and so can anyone holding its connection string |
| Who can read that connection string | Any agent using the automation service account, which holds `secretmanager.admin` |

The instance can't be deleted by an agent, but the data inside it can,
and the only copies are inside Cloud SQL itself.

### The plan

1. **Retain Cloud SQL backups on instance deletion**
   (`gcloud sql instances patch openagents-production-pg --retain-backups-on-delete`),
   so even deleting the instance leaves its backups.
2. **A separate backup project**, `openagents-backups`, holding one bucket
   `oa-db-backups` with:
   - a retention policy of 35 days, **locked** (Bucket Lock): no one,
     including a project owner, can delete or overwrite an object before
     it is 35 days old, and the policy can't be shortened;
   - object versioning on;
   - the only writer is a dedicated service account `oa-backup-writer`,
     with `storage.objectCreator` and nothing else. No account in the
     main project has any role in this project; only the owner's human
     login does.
3. **Logical dumps:** a Cloud Run job, `oa-db-dump`, runs `pg_dump` of
   `oa_analytics` (whole database) nightly and of the `auth` schema every 6
   hours, compressed, then sealed to an **offline backup public key**
   (secp256k1 ECDH plus XChaCha20-Poly1305, the NIP-44 construction). The
   job can encrypt but can't decrypt. The private key is printed on paper
   and kept by the owner, with a second copy in a separate location.
4. **The seal key and pepper** are exported once, sealed to the same
   offline key, and stored in the locked bucket, so a restore can read
   emails.
5. **Roles** (below) so the running API can't drop tables at all.

### Database roles

| Role | Can | Can't | Used by |
| --- | --- | --- | --- |
| `oa_owner` | Own every schema; run migrations | Serve requests | A separate migration job (`oa-api-migrate`) run by the deploy before the new revision; the API stops migrating at startup. Its password is in a secret only the owner and the deploy can read |
| `oa_api` | `SELECT`, `INSERT`, `UPDATE`, `DELETE` on tables; `EXECUTE` on functions; `INSERT` on `auth.audit` | `CREATE`, `DROP`, `ALTER`, `TRUNCATE`; `UPDATE` or `DELETE` on `auth.audit` | The API |
| `oa_readonly` | `SELECT` on views that exclude sealed columns and digests | Anything else | Agents, analytics, debugging |
| `oa_dump` | `SELECT` on everything, for `pg_dump` | Writes | The dump job |

Plus an event trigger on `ddl_command_start` that refuses `DROP` and
`TRUNCATE` in production unless the session set
`oa.allow_ddl = '<migration id>'`, which only the migration step does.
This catches the case where someone does get `oa_owner`.

**IAM changes that go with it** (owner-run; the automation account can't
change IAM):

- the automation account loses access to the production connection
  string secrets; it gets the `oa_readonly` one;
- the backup project grants nothing to the main project's accounts.

### The "an agent nukes the production database" runbook

| Disaster | Recover from | Data lost (RPO) | Time to recover (RTO) |
| --- | --- | --- | --- |
| A table or schema dropped, rows deleted | Point-in-time clone to the minute before (`gcloud sql instances clone ... --point-in-time`), copy the damaged tables back | Under 1 minute | About 1 hour |
| The whole database dropped | Point-in-time clone, then point the API at the clone | Under 1 minute | About 1 hour |
| The instance deleted (needs owner rights) | Retained Cloud SQL backup, restored to a new instance | Up to 24 hours | 1 to 2 hours |
| Cloud SQL backups also gone, or bad for days | Latest locked dump in `oa-db-backups` | 6 hours for `auth`, 24 hours for the rest | 2 to 3 hours |
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

Monthly, a scheduled job:

1. restores last night's locked dump into a temporary instance named
   `oa-drill-<date>` (the smallest tier, about $0.05 an hour),
2. decrypts it with a drill copy of the backup key held in Secret Manager
   for this purpose (a separate key pair from the paper one; the job
   encrypts each dump to both),
3. runs the counts and a login replay against it,
4. writes the result and timings to `docs/verification/`,
5. deletes the temporary instance it made (and only that).

A drill that fails opens an issue.

### What it costs

| Item | Monthly |
| --- | --- |
| Cloud SQL automated backups and logs, for a database under 10 GB | About $2 |
| Locked bucket: 35 days of nightly dumps and 6-hourly `auth` dumps, compressed, well under 10 GB | Under $1 |
| Dump job runs | Under $1 |
| Monthly drill instance, a few hours | Under $1 |
| **Total added** | **About $5** |
| Optional: high availability (regional) | About +$50 |
| Optional: cross-region read replica | About +$50 |

Both options fit under the $1,500 cap but buy uptime, not safety. We
recommend neither now (decision D11).

## Data export and account deletion

- **Export:** `/settings/data` gives a JSON file: account, authenticators
  (public parts), sessions, grants, realms, characters, credit history and
  the account's audit rows. At L2.
- **Delete:** in the app and on the web at `/settings/delete`, as both
  Apple (guideline 5.1.1(v)) and Google Play require; Google also requires
  a web link to start deletion
  ([Google Play account deletion](https://support.google.com/googleplay/android-developer/answer/13327111)).
  At `L2-fresh`. The account is frozen at once, and purged after 30 days
  (cancel by logging in). Purging deletes authenticators, sessions,
  grants, connections and owned content, sets audit rows' account to null,
  and leaves the account id as a tombstone so it's never reused. Copies in
  backups age out within 35 days; the privacy page says so.
- **Money first:** before deleting, the page shows any service credit
  left (spend it first; credit can't be withdrawn) and reminds you that
  your wallet is yours: deleting the account deletes our encrypted copy
  of its backup, so save your words or move the coins first.
