# Postgres store and roles

nostr-relay uses one Postgres database. The store owns migrations, event
admission, policy checks, indexed reads, process notification, and gap
recovery. No cache, broker, or second database participates.

## Schema

`migrations/0001_store.sql` creates:

- `nostr_event`: validated durable events, generated `ingest_seq`, optional
  expiration, and a generated full-text-search vector;
- `nostr_indexed_tag`: the first value of each single-letter ASCII tag;
- `replaceable_head`: the current event for each NIP-01 replacement address;
- `deletion_tombstone`: durable NIP-09 event and address deletions, including
  deletion-before-event;
- `relay_policy`, allow/block lists for pubkeys and kinds, and
  `relay_member_pubkey`: operator admission policy state; and
- `schema_migrations`: applied version, name, SHA-256, and timestamp.

`migrations/0002_nip_expansion.sql` adds:

- `relay_group`, `relay_group_member`, and `relay_group_invite`: authoritative
  NIP-29 group state used before admission; and
- `management_request`: consumed NIP-98 authorization event IDs for replay
  protection.

`migrations/0003_media.sql` adds:

- `media_blob`: content hash, size, normalized MIME type, upload timestamp,
  and pending/ready visibility state;
- `media_owner`: shared-blob ownership and per-pubkey quota accounting; and
- `media_auth_request`: consumed upload/delete NIP-98 event IDs.

Blob bytes use the configured filesystem backend rather than a second
database. Postgres remains authoritative for visibility: public lookups select
only ready rows. Upload registration, quota, ownership, and replay consumption
commit together before atomic file installation; a final prepared update
publishes the blob. Delete removes one owner and drops metadata only after the
last owner.

It also allows a durable deletion tombstone to retain its signed source ID
after NIP-40 expires and physically removes the source event. The tombstone's
deletion effect therefore does not disappear when its publication does.

`migrations/0004_agent_identity_turns.sql` adds the Block NIP-OA/NIP-AA
agent-identity tables and NIP-AM private turn metrics, and rebuilds the
generated search vector so encrypted and access-gated kinds stay out of
full-text search.

`migrations/0005_block_server_handlers.sql` adds the server-side Block handler
state (`block_command`, `workspace_profile`, and related tables) and extends
the search-vector exclusions to the remaining access-gated Block kinds.

`migrations/0006_nostr_effect_import.sql` adds `nostr_effect_import_ledger`,
the idempotency ledger for an explicitly enabled one-way import from a legacy
nostr-effect `public.events` table. The source table is never touched.

`migrations/0007_legacy_expiration.sql` records a terminal ledger outcome for
expired legacy rows so tail sweeps do not retry them.

`migrations/0008_gift_wrap_search_privacy.sql` rebuilds the generated search
vector and GIN index with kind 1059 excluded. Gift-wrap ciphertext is
recipient-private and must never enter full-text search; recipient-gated
history and ID lookup remain available.

`migrations/0009_nip29_groups.sql` adds group privacy, visibility, write-policy,
parent/child, banner, and LiveKit fields with bounded parent references.
Existing groups keep their prior public-read, restricted-write policy.

`migrations/0010_private_protocol_search.sql` excludes the expanded set of
private protocol kinds and private CAP policy from full-text search, including
existing rows. It also adds `relay_query_authorization` for replay protection
on authenticated HTTP queries.

The database independently rejects malformed identity widths, negative or
out-of-range protocol numbers, ephemeral kinds, inconsistent replacement
identifiers, and malformed tombstone shapes. Indexed access paths cover IDs,
authors, kinds, author-plus-kind, timestamps, tags, ingest sequence, expiry,
and full-text search.

## Migrations

Migration files are compiled into the binary. `Store::connect_with_report`
takes a database advisory lock, applies every pending file in one transaction,
and records its SHA-256. A changed historical file, an unknown database
version, or a mismatched name is a startup error. Concurrent processes wait
for the same lock and then verify the resulting ledger.

Do not execute `migrations/*.sql` directly with `psql`: that bypasses the
hash ledger and makes the database unverifiable. M2 exposes the embedded
runner through `Store::connect_with_report`; M3 invokes it during process
startup before binding the network listener.

Migration DDL is the only use of `batch_execute`: it is immutable SQL loaded
with `include_str!`, never SQL assembled at run time. Every runtime data
statement is prepared once through `tokio-postgres` and uses typed parameters.

`Store::connect_verified` checks that all known migration names and hashes are
current without executing DDL. Gateway startup first runs migrations with its
single configured database credential, then creates its fixed set of verified
workers and dedicated notification, expiration, and optional sweep connections before the network listener binds. M5 therefore deploys one
database-owner login; the binary does not yet
expose separate migrator/runtime credentials or a migration-only command.

## Admission transaction

One transaction performs duplicate and policy checks, takes deterministic
transaction-scoped advisory locks, checks tombstones, compares replacement
heads, inserts the event and indexed tags, applies deletion tombstones and
deletes superseded rows, updates the head, allocates `ingest_seq`, and calls
`pg_notify`. A stored result is returned only after commit.

The transaction pipelines its statements rather than waiting for each reply,
because every sequential statement costs a database round trip: one pipeline
carries the policy reads, the conflict locks, and the duplicate, tombstone, and
replacement-head reads that follow the locks; a second carries the sequence
lock, insert, indexed tags, head update, and notification of a plain event.
Postgres runs one session's statements in the order sent, so locking and
visibility are unchanged, and the results are judged in the order the checks
always ran. Group, deletion, and push-lease admissions write their side effects
one statement at a time after the insert. With a 4 ms database round trip, a
private `3188` event's `OK` fell from about 97 ms to about 27 ms
(`tests/exchange_latency_postgres.rs`).

The advisory-lock keys serialize every conflicting event ID and replacement
address across relay processes. Keys are sorted before acquisition, avoiding
deadlocks when one deletion request names several targets. This closes the
race where a deletion and its target arrive on different processes at the
same time.

Ephemeral kinds pass signature, timestamp, policy, and tombstone checks, but
the schema rejects them and the store never inserts them. After commit the
gateway fans them out locally and sends bounded hexadecimal chunks through the
`nostr-relay_ephemeral` Postgres notification channel for other relay processes.
Listeners validate and reassemble the signed event in memory; no ephemeral
payload enters a table.

Durable admissions take a short global advisory lock immediately before
allocating `ingest_seq`. Conflicting event/replacement locks have already been
taken at that point. This makes durable sequence order equal commit order, so
the gateway can use a sampled high-water mark as a race-free historical/live
EOSE boundary.

A stored event this process committed outside a NIP-29 group is delivered to
its live subscribers from memory as soon as its commit returns, if every
smaller sequence has already been delivered; its later notification then asks
for nothing. Commit order equals sequence order, so no earlier event can still
appear. Otherwise the event waits for the catch-up read below. Nothing is
delivered before its commit, and subscribers still see stored events in
sequence order.

Each gateway establishes `LISTEN` before sampling its durable cursor. A later
notification jump is recovered with the prepared, bounded `events_after`
query, so a missed individual notification does not lose delivery. A jump
larger than 4,096 sequence positions is read back in steps of at most 4,096.

Losing the notification listener never stops the relay (#9947). The
listener's connection (`application_name` `nostr-relay-listener`) can end
because Postgres or the Cloud SQL connector dropped it, its local queue of
2,048 notifications filled, or a payload was malformed. The gateway then
logs a warning with the reason and keeps serving: events this process
commits still go out from memory. It reconnects with backoff (100 ms,
doubling to 5 s), establishes `LISTEN`, reads the high-water mark on that
same connection, and catches up through it by sequence, so every stored
event committed during the outage reaches each subscriber once, in order.
Ephemeral events sent while no listener was connected are not replayed;
they were never stored. A catch-up read that fails is retried with the same
backoff. A notification that names a sequence above the database's
high-water mark is treated as a listener fault: events up to the high-water
mark are delivered, and the listener is replaced and resynchronized.

A database worker still stops the process on a failure it can't isolate to
one request, such as its connection closing. A cancelled statement
(SQLSTATE 57014) fails only its own request: Postgres applies a cancel to
whatever statement is running when it arrives, so one meant for a finished
history read could land on the next statement on that connection. A
cancelled history read also waits for its statement to end before the
connection takes another. Each stop logs one JSON `error` line naming the
reason before the process exits.

## Admission policy

The singleton `relay_policy` row configures closed-membership mode, maximum
UTF-8 content bytes, maximum tag count, and future and past timestamp bounds.
A `max_past_seconds` value of zero disables the past bound. The schema rejects
negative limits and a zero content limit.

The `relay_allowed_pubkey` and `relay_allowed_kind` tables are optional
allowlists: an empty table permits every value, while a non-empty table permits
only listed values. `relay_blocked_pubkey` and `relay_blocked_kind` always deny
matching values and take precedence over the allowlists. When
`closed_membership` is true, the author must also exist in
`relay_member_pubkey`. Each admission transaction reads the current committed
policy. M6's authenticated NIP-86 HTTP API provides ordinary policy and group
administration, so operators do not need direct SQL for those supported
operations. The database owner remains responsible for broader bootstrap and
recovery work.

## Roles

### Simple single-box role

The Debian runbook's `nostr-relay` role owns only the `nostr-relay` database and is
not a superuser, replication role, role creator, or database creator. This is
the supported minimal deployment. It may apply migrations and run the relay.

### Split migration and runtime roles (not yet a deployment mode)

The store API has the verification primitive needed for a future split-role
mode, but the executable intentionally exposes only one `DATABASE_URL` and
always performs migration bootstrap before binding. Do not configure a
runtime-only role today: startup will fail closed when it cannot run the
embedded migration transaction.

A future split-role deployment must add an explicit migration-only command,
separate credential handling, per-migration runtime grants, and a live
least-privilege proof before any runbook may recommend it. Never put a
database password in this repository or a command-line argument.
