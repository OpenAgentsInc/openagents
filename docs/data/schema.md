# The OpenAgents data schema

Owner request, 2026-10-09 ([#11154](https://github.com/OpenAgentsInc/openagents/issues/11154)):
"use postgres asap". This page is the whole model of what OpenAgents keeps,
on the server, for every surface in the [Kitchen Sink spec](../kitchen-sink/README.md):
which Postgres schema and table each thing lives in, who owns it, how private
it is, how long it is kept, and whether its table exists yet. Names follow the
[glossary](../glossary.md); a term this page needed and the glossary lacked was
added there in the same change.

Only the first migration exists today: the `identity` and `workspace` tables
sign-in needs, plus the `audit` revision table they write. Everything else on
this page is the design that each feature's migration follows when that
feature needs it.

## Decisions

| Question | Decision | Why |
| --- | --- | --- |
| Engine | Cloud SQL for PostgreSQL 18, Enterprise edition, `us-central1` | Transactions, unique constraints and row locks instead of lock files on NFS; managed backups and point-in-time recovery |
| Instances | Two, fresh: `openagents-staging-pg` (db-f1-micro) and `openagents-production-pg` (db-custom-1-3840) | Staging can be wiped, load-tested and run on new major versions without touching production; a production restore never sees test accounts |
| Databases | One per instance, `openagents` | One environment, one database: cross-domain joins and transactions (sign-in, then a membership, then a receipt) stay possible |
| Organization | One Postgres schema per domain: `identity`, `workspace`, `chat`, `work`, `registry`, `money`, `telemetry`, `audit` | A domain's tables, grants and migrations move together; a later split into its own database is a schema copy |
| Connection | The Cloud SQL connector Cloud Run mounts (`run.googleapis.com/cloudsql-instances`), a Unix socket at `/cloudsql/PROJECT:REGION:INSTANCE` | The `default` network has no private services access range, and the connector is IAM-gated and encrypted without a TLS stack in the binary. The instances have a public address with no authorized networks and `ENCRYPTED_ONLY`, so only the connector reaches them |
| Credentials | One login role per instance, `openagents_app`, its password in Secret Manager (`openagents-<env>-pg-password`) | Never printed, never in the image or the database |
| Backups | Daily automated backups (7 kept on staging, 30 on production), point-in-time recovery with 7 days of logs, deletion protection on both | The account store is the one thing a person cannot recreate |
| High availability | Off for now on production | One zone; a zonal outage stops sign-in until it recovers. Turn on (`--availability-type REGIONAL`, about double the price) when paying customers depend on it |

Cost, us-central1 list prices: staging db-f1-micro about $9 a month plus
10 GB SSD ($1.70) and backups; production db-custom-1-3840 about $50 a month
plus 10 GB SSD and backups, about $55 in all. Both together are under $70 a
month, less than the two NFS servers plus a Filestore would have been, and
the NFS servers can be deleted once nothing reads them (see [account
storage](../deployment/account-storage.md)).

## Rules every table follows

1. **Tenancy on every row.** Every row carries `workspace_id`, the workspace
   that owns it, except rows that exist before or outside any workspace
   (an account, its principals and sessions, which belong to an account),
   which carry `account_id` instead. Deleting or splitting a workspace is a
   `WHERE workspace_id = $1` in every schema. Rows that only reference a
   registry tenant (bearer keys, provider keys) keep `tenant` and resolve
   `workspace_id` from the workspace bound to that tenant.
2. **Big payloads live in buckets.** Chat message bodies, trace files,
   artifacts, plugin packages and attachments stay in Cloud Storage. Postgres
   keeps the pointer (`object_key`), the size and the `sha256`, so a read can
   prove it got the bytes the row names.
3. **Secrets are sealed before they reach the database.** A provider key, a
   GitHub token, or a saved own-Claude credential is stored only as the
   AES-256-GCM sealed blob the code already writes, bound to its owner as
   associated data. The keys that open them stay in Secret Manager and in the
   container's memory. A database copy holds ciphertext.
4. **Credentials are digests.** Bearer keys, session tokens, invitation and
   recovery tokens, and device codes are stored as their SHA-256 only. The
   secret is shown once and never stored.
5. **Money and audit are append-only.** Ledger entries, receipts, payouts and
   audit events are inserted, never updated or deleted. A correction is a new
   entry that names the one it corrects. The database role the app uses has
   no `UPDATE` or `DELETE` on those tables.
6. **One crate owns each domain** and its migrations
   (`crates/<owner>/migrations/<domain>/NNNN_name.sql`), embedded in the
   binary and applied at start under a Postgres advisory lock. No other crate
   writes that domain's tables.
7. **Names follow the glossary.** Table names are plural nouns from the
   glossary (`accounts`, `memberships`, `bearer_keys`), never a second word for
   the same thing.

## Privacy classes

Every table below names one class.

| Class | Meaning | Examples |
| --- | --- | --- |
| Public | Anyone may read it; the row says so (`public = true` or the whole table) | A shared trace's summary, published plugin versions, promises, eval results someone published |
| Account | Only the account or the workspace's members read it | Chats, traces, projects, computers |
| Sealed | Stored only as ciphertext whose key is outside the database | Provider keys, GitHub tokens, own-Claude credentials |
| Digest | Only a SHA-256 is stored; the secret is not kept anywhere on our side | Bearer keys, session tokens, invitations, device codes |
| Aggregate | Counts and sums with no person in them | Analytics rollups |
| Never stored | Not written on the server at all | Wallet seed words, the device key of a phone, a local-only chat whose device has sync off, prompt contents of a zero-retention tenant |

## The model at a glance

```
identity.accounts ─┬─< identity.principals            (key:…, nostr:…, github:…, sso:…)
                   ├─< identity.linked_identities     (the GitHub profile behind github:…)
                   ├─< identity.account_sessions ──── identity.device_sign_ins
                   ├── identity.github_access          (sealed GitHub user token, App installs)
                   └─< workspace.memberships >── workspace.workspaces ── tenant (registry)
                                                    │
     workspace.workspaces ─┬─< workspace.invitations
                           ├─< identity.bearer_keys, identity.provider_keys   (by tenant)
                           ├─< workspace.computers ─< workspace.sync_choices
                           ├─< chat.threads ─< chat.messages ── bucket object
                           ├─< work.projects ─< work.repositories ─< work.branches
                           │        └─< work.environments ─< work.tasks ─< work.runs ─< work.traces ── bucket object
                           ├─< registry.plugins ─< registry.plugin_versions ─< registry.eval_runs ─< registry.eval_results
                           ├─< money.* (usage, quotes, payments, receipts, ledger entries, payouts)
                           └─< audit.events
```

## identity (lands now, migration 1; owner `crates/tenancy`)

What a person is and how they prove it.

| Table | What a row is | Key and constraints | Class | Kept |
| --- | --- | --- | --- | --- |
| `accounts` | An account: a stable `acct_` id, label, created time | `id` primary key | Account | Until the account is deleted |
| `principals` | One principal an account signs in with (`key:<id>`, `nostr:<hex>`, `github:<id>`, `sso:<hex>`) | `principal` primary key: one principal belongs to one account, the invariant `Store::validate` checks today | Account | With the account |
| `linked_identities` | The profile a provider returned for a principal (today GitHub's `/user` and `/user/emails`), refreshed at each sign-in. Never a token | (`provider`, `provider_id`) primary key; `account_id` | Account | With the account |
| `account_sessions` | An account session (`sess_`), web, app or anonymous, stored by token digest, with its state, expiry and the app and computer it was signed in from | `id` = SHA-256 of the token | Digest | Ended sessions 30 days, then deleted |
| `device_sign_ins` | A device sign-in grant (`coder login`, the desktop app): device-code digest, user code, state, the account that approved it | `id` = device-code digest; `user_code` unique | Digest | 1 day after it ends |
| `recoveries` | A recovery token's digest and state | `id` = token digest | Digest | 1 day after it ends |
| `credentials` | A sign-in secret's digest for an account that has one | `account_id` | Digest | With the account |
| `onboarding_budgets` | The operator-funded anonymous lane's budgets | `id` | Account | 30 days after spent |
| `bearer_keys` | An `oak_` bearer key: id, tenant, secret digest, status, name, key scope, lineage. **Later:** user-set limits per key (requests and spend per day), which the owner asked for and the user sets | `id` primary key; `digest` unique | Digest | Revoked keys kept, so a revoked key is told apart from an unknown one |
| `provider_keys` | A workspace's own provider key (OpenRouter, Vercel AI Gateway), sealed under the BYOK keyring | (`tenant`, `provider`) primary key | Sealed | Until removed |
| `github_access` | An account's GitHub user grant (sealed token, scopes, login), GitHub App installations, credential-broker tickets, and chosen repositories | `account_id` primary key; `account_digest` unique | Sealed | Until disconnected |
| `stores` | One row per migrated document store (`accounts`, `sessions`, `keys`): its schema tag, revision counter, sequence, digest and the parts of the document that have no table yet | `store` primary key | Account | Current only |

Later in `identity`: `nostr_keys` (a person's own Nostr keys, which we only
ever hold as public keys), `agent_identities` (an agent's own keys, sealed,
when agents get their own identity), and `own_credentials` (saved own-Claude
credentials, sealed, moved from the web's custody files).

## workspace (lands now, migration 1; owner `crates/tenancy`)

Who acts together, and on which computers.

| Table | What a row is | Key and constraints | Class | Kept |
| --- | --- | --- | --- | --- |
| `workspaces` | A workspace, `personal` (one person) or `organization` (a team), its name, seats, members epoch, and the registry tenant it is bound to | `id` primary key; `tenant` indexed | Account | Until deleted |
| `memberships` | An account's membership in a workspace: role (`owner`, `admin`, `member`), status, epoch. Revoked memberships stay | (`workspace_id`, `account_id`) primary key; at most one active `owner` per workspace (partial unique index) | Account | With the workspace |
| `invitations` | An invitation: secret digest, role, expiry, status | `id` primary key; `digest` unique | Digest | 30 days after it ends |
| `computers` *(later)* | A linked computer or cloud computer: name, kind, the host's public key, last seen, whether it may run Coder | `id`; (`workspace_id`, `host_pubkey`) unique | Account | Until unlinked |
| `sync_choices` *(later)* | Per computer and per surface, whether chats sync to the account (sync is off until turned on) | (`computer_id`, `surface`) primary key | Account | With the computer |

## chat (later; owner `crates/openagents-web`, `openagents-chat`)

Chats are in Cloud Storage today (`chat_store`, one object per thread, with
generation preconditions). That works and stays the home of the bodies. What
moves to Postgres is the index the lists and search need.

| Table | What a row is | Class | Kept |
| --- | --- | --- | --- |
| `threads` | A thread: title, pinned, archived, updated time, the project it belongs to, which surface started it | Account | Until the person deletes it; deletes reach every synced device |
| `messages` | One message's metadata: role, model, cost, created time, and the bucket object, size and `sha256` holding its body | Account | With the thread |
| `attachments` | A file or image on a message: bucket object, type, size, `sha256` | Account | With the thread |

Never stored: the chats of a device with sync off. They stay on that device.

## work (later; owners `crates/coder-project`, `crates/openagents-web`)

| Table | What a row is | Class | Kept |
| --- | --- | --- | --- |
| `projects` | A project: name and the repository it groups chats by | Account | Until deleted |
| `repositories` | A GitHub repository a project uses (full name, GitHub id, private or not) | Account | With the project |
| `branches` | Branches and pull requests a task worked on | Account | With the project |
| `environments` | An execution environment version: recipe, base, verification state | Account | Until replaced, then 90 days |
| `tasks` | A Coder task: objective, state, the computer it ran on, % done estimate | Account | With the project |
| `runs` | One run of a task: engine, model, start, end, outcome, cost | Account | With the task |
| `traces` | An uploaded trace (ATIF): title, agent, model, steps, bytes, `sha256`, bucket object, `shared`. Today in Cloud Storage beside the chats (`traces/index.json`) | Account, or Public when shared | Until the person deletes it; at most 100 per account today |

## registry (later; owners `crates/plugin`, `crates/gym`, the promises registry)

| Table | What a row is | Class | Kept |
| --- | --- | --- | --- |
| `plugins` | A plugin (extension package): name, publisher, `public` | Public when published | Forever once published |
| `plugin_versions` | A release: version, package digest, bucket object, the components it carries | Public when published | Forever; withdrawn versions are marked, not deleted |
| `eval_suites` | A test set (eval suite) and its version | Public when published | Forever |
| `eval_runs` | One run of a test set against a plugin version, model and computer | Account, Public when published | Forever |
| `eval_results` | Per-case results of a run | Same as its run | Same as its run |
| `promises` | A promise from the ledger: area, wording, status, evidence links, episodes | Public | Forever; status changes are new rows in `audit.events` |

## money (later; owners `crates/tenancy` money and billing, `crates/pay-ledger`, `crates/x402`)

Append-only. Amounts are integers in the currency's smallest unit
(`amount_minor` plus `currency`, or `amount_msat`), never floats.

| Table | What a row is | Class | Kept |
| --- | --- | --- | --- |
| `usage` | One metered use: request and attempt, door or model, input and output units, the price terms it was pinned to | Account | 30 days for content-bearing fields (none are stored), usage numbers 7 years |
| `quotes` | A price offered before work (x402, L402, MPP challenges) | Account | 7 years |
| `payments` | A payment in or out on any rail (card, x402, L402, Lightning, Cashu, MPP, ACP, AP2, stablecoin): rail, external id, amount, state | Account | 7 years |
| `receipts` | Execution receipts (`openagents.receipt.execution.v1`) and payment receipts | Account | 7 years |
| `ledger_entries` | Double-entry credits ledger: each posting is two or more rows that sum to zero, by account (`credits:<workspace>`, `revenue`, `payouts:<author>`) | Account | Forever; corrections are new entries |
| `reservations` | Quota and monetary holds before dispatch, and their settlement | Account | 1 year |
| `subscriptions`, `plans`, `entitlements`, `provider_events` | Billing as `tenancy::billing` keeps it today | Account | 7 years |
| `payouts` | A payout to a plugin author, compute provider or referrer: destination, amount, state | Account | 7 years |

Today these are files on the NFS share (`quota-ledger.jsonl`,
`receipts.jsonl`, the attempts journal, the pay host's ledger). They move here
when the money feature that reads them next needs a change.

## telemetry (later; owner `crates/openagents-web`)

| Table | What a row is | Class | Kept |
| --- | --- | --- | --- |
| `daily_rollups` | Counts per day and surface: sign-ins, chats, tasks, traces, errors | Aggregate | Forever |

No personal data: no account id, address, or user agent.

## audit (lands now in part; owner `crates/tenancy`)

Append-only; the app role cannot update or delete.

| Table | What a row is | Class | Kept |
| --- | --- | --- | --- |
| `revisions` | Every sealed revision of a document store (`accounts`, `sessions`), by digest, with the revision it superseded. The same history `accounts-history/` and `sessions-history/` keep on NFS, so which membership authorized an earlier call can still be explained | Account | 1 year |
| `events` *(later)* | Who did what, when: admin actions, money movements, membership and key changes. The sessions store's access log moves here | Account | 7 years |

## How the first migration maps today's stores

The account service's stores keep their documents and their checks
(`Store::validate`, the digest chain) exactly as they are; only where the
document is kept changes. `tenancy::db` writes a document's collections as
rows and reads them back into the same document, inside one transaction
under an advisory lock per store, so two writers serialize the way the lock
files made them, across any number of instances, and `keys.json`'s lost
update ([#11150](https://github.com/OpenAgentsInc/openagents/issues/11150))
cannot happen.

| File on the share | Postgres |
| --- | --- |
| `accounts.json` | `identity.accounts`, `identity.principals`, `identity.linked_identities`, `workspace.workspaces`, `workspace.memberships`, `workspace.invitations`; the books without tables yet (referrals, commercial, team policies, team capabilities, team reports, SSO) in `identity.stores.rest` |
| `accounts-history/`, `sessions-history/` | `audit.revisions` |
| `sessions.json` | `identity.account_sessions`, `identity.device_sign_ins`, `identity.recoveries`, `identity.credentials`, `identity.onboarding_budgets`; the access log in `identity.stores.rest` until `audit.events` lands |
| `keys.json` | `identity.bearer_keys` |
| `github-access/*.json` | `identity.github_access` |
| `inference-provider-keys.json` | `identity.provider_keys` |

Still on the share until their domain lands: the tenant registry, the quota
ledger, receipts and attempts (money), the stored responses, the chat
worker's usage lines, `service.key`, and the web's own-Claude custody files.
`accounts.store = "postgres"` in the gateway config switches only the stores
in the table above.

Each row keeps the store's own JSON record (`record jsonb`) next to the
columns the constraints and lookups need, which Postgres generates from it,
so the document reads back exactly and its digest still recomputes. Columns
can be promoted to plain columns, and the record trimmed, one migration at a
time.

## Operations

- Migrations: `crates/tenancy/migrations/*.sql`, applied by the gateway at
  start under `pg_advisory_lock`, recorded in `public.schema_migrations`.
- Moving the files in: `tenant-db import --registry DIR` reads an NFS export
  and writes every store, idempotently, then `tenant-db verify` reads each
  account, session and key back and compares it with the files.
- Restore: point-in-time recovery to a new instance
  (`gcloud sql instances clone openagents-<env>-pg NEW --point-in-time ...`),
  then point the service's connector annotation at it.
