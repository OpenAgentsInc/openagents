# Actors

`actors` provides synchronous Rust state machines backed by PostgreSQL, with
atomic messages, alarms, effects, and leased work. It implements the foundation
of [the actor proposal](../../docs/architecture/actors.md) and
[#11253](https://github.com/OpenAgentsInc/openagents/issues/11253).
`openagents-web` runs it against the account database, and Mac jobs are its
first consumer (`mac.job`, [docs/deployment/actors.md](../../docs/deployment/actors.md)).
The bundled counter is an example.

Features: `server` (default) is the PostgreSQL store, the runtime, and the HTTP
routes; `net` is the network client ([`net::Client`](src/net.rs)) a remote
executor uses. With no features the crate is the actor definitions and wire
types, so a domain crate can define actors without a database driver.

## What works

| Area | Implementation |
| --- | --- |
| Definition | `Actor`, `Message`, `Handles<M>`, `Definition`, `Registry`, and `actor_messages!` register typed handlers and publish a contract. |
| Execution | Each call reconstructs an actor from JSON state, runs a synchronous handler, and commits its new state and commands together. Errors, panics, invalid commands, and failed database writes roll back the transition. |
| State | Workspace-scoped identities, immutable account ownership for private actors, versions, optimistic checks, state migrations, and inaccessible destroyed records. |
| Actions | Atomic create-and-call, caller-bound idempotency receipts, argument fingerprints, and read-only handlers. |
| Inbox | Ordered, bounded, durable delivery; current authority checks; retries; poison-message isolation; operator retry; and rollout deferral for unknown types or newer state. Unknown messages get a one-hour grace period. |
| Commands | Events, same-workspace sends, one-shot or fixed-interval alarms, work offers, work cancellation, effects, and destruction. |
| Work | Exact queue/target executor grants, bounded concurrent claims, long-poll claims (`claim_work_wait`, `wait_ms` up to 30 s), generations, heartbeat leases, sequenced progress, duplicate completion checks, cancellation (an executor releasing a cancelled claim ends it as `cancelled`, freeing its slot), expiry, and explicit resolution of uncertain results. |
| Fenced actions | An action may name a work claim (`fence: {item_id, epoch}`). The store checks it is the caller's live claim at that epoch in the same transaction, renews its lease, and hands it to the handler (`Ctx::fence`), so an executor's reports can't outlive its claim. |
| Effects | Bounded concurrency, registered-kind selection, renewable claims, deadlines, attempt limits, and completion messages. Non-repeatable failures stay uncertain until resolved. |
| Observation | Caller-filtered views, bounded SSE connections, multiplexed feeds, version cursors, contract JSON, and `list_own` (one account's private actors of a type, newest first, with views). Raw stored events require an administrator. |
| Storage | A namespaced, digest-checked migration, bounded connection pool, database clock for leases, transaction timeouts, `SKIP LOCKED`, notifications, and polling recovery. |
| Operations | Inspection, history, blocking, destruction, bounded export, retention primitives, failed-message retry, work/effect resolution, runtime counters, and a local load probe. |

## Define and call an actor

An `Actor` supplies serializable state, creation input, a state version, a wake
function, and a caller-specific view. Each `Message` names its access policy and
reply type. Implement `Handles<M>` for each supported message, then register a
`Definition::<A>::new().message::<M>()`. See [example.rs](src/example.rs) for a
private counter with actions, an alarm, a work offer, and internal completions.

Handlers receive `Ctx`, which records commands and provides transaction time
and deterministic pseudorandom values. Command IDs derive from actor identity,
transition version, and command position. Keep handlers synchronous and free of
I/O. Put external operations in effect executors or work consumers.

A typical host constructs these objects:

```rust,ignore
let registry = Arc::new(my_registry()?);
let store = PgStore::new(Pool::new(&database_url, 8)?, registry);
store.migrate().await?;
let store = actors::http::with_authenticator(store, authenticator.clone());
let runtime = Runtime::new(store.clone()).effects(my_effects()?).start()?;
let router = actors::http::router(store.clone(), authenticator);
```

Use the returned, authentication-configured store for both the router and the
runtime. Configuring only the router does not update other store clones.
`Client::new(store, caller).actor::<A>(key)` provides typed calls through the same
store. `with_input` enables atomic creation on its first action.

`Caller` is a trusted host value, not an HTTP request field. Construct a fresh
client with current authority for each request; the typed client does not
refresh a cached caller. Public queued messages fail closed without a host
revalidator. The revalidator may change current roles or grants, but it cannot
change the saved principal, workspace, or account identity.

Private actors require the same account even for administrators, services, and
executors. Workspace membership alone does not grant private access. Inbox
results require their original principal and current message permission. The
host must authenticate executor identity, grant generation, expiry, exact queue
and target lists, and capacity. No wildcard grant is implied.

## Runtime and recovery

`Runtime` drives inboxes, alarms, work expiry, and effects. `LISTEN` provides wake
hints; polling handles missed notifications and restarts. Each runtime uses one
additional database connection for its listener. Only registered effect kinds
are claimed, so an older process leaves newer operations pending.

A committed work offer is separate from execution authority. Consumers claim
with a verified grant and report with the returned epoch. Stale generations,
expired leases, altered duplicate results, and cancelled work cannot complete.
A heartbeat can report cancellation without extending the lease. Uncertain
claims continue to count against executor capacity.

Use `RetryPolicy::Idempotent` only when the receiver implements a stable
operation key. Use `(uid, item_id)` or `(uid, effect_id)` as that key. Otherwise,
use `RetryPolicy::Reconcile`: expiration or ambiguous failure produces an
uncertain record that requires an observed result or an explicit retry decision.
Operator resolution checks the epoch or attempt being resolved and records the
decision in history. Destruction and cancellation do not prove that a remote
operation stopped. A late observed result can be recorded after destruction
without reviving the actor or delivering another message.

A saturated inbox delays that actor's expiry notification; its expired claim
remains unusable. Other eligible actors continue. Fixed-interval alarms coalesce
missed intervals instead of creating an unbounded backlog. Cancelling and
recreating an alarm advances its generation so an old delivery cannot consume
the new occurrence.

Call `RuntimeHandle::shutdown().await` to stop admission and allow a ten-second
effect drain. Unfinished claims remain durable for expiry or reconciliation.
Dropping the handle also requests shutdown. Handler code is trusted Rust: panic
capture and elapsed-time checks cannot preempt an infinite loop or prevent a
handler from performing I/O. The normal panic hook can log a panic payload;
handlers must not put secrets in panic messages.

## HTTP surface

Mount the router behind the host's TLS, authentication, origin/CSRF checks,
request-rate controls, and HTTP server limits. This crate supplies none of those
host policies. Request bodies cannot choose `Caller` or an internal origin.
The public contract route exposes registered schemas and descriptions, so those
must contain no secrets.

Under `/v1/w/{workspace}`:

- `POST /actors/{type}/{key}/actions/{message}` calls an action. The body contains
  `args`, optional creation `input`, and optional `expected_version`. Supply
  `Idempotency-Key` for retryable mutations.
- `POST /actors/{type}/{key}/inbox/{message}` queues `{"args": ...}`.
- `GET /actors/{type}/{key}/inbox-status/{seq}` reads the sender's result.
- `GET /actors/{type}/{key}/view` reads the authorized view.
- `GET /actors/{type}/{key}/events` streams authorized view snapshots.
- `GET /feed?topics=type:key,type:key` multiplexes up to 16 actors.
- `POST /actors/{type}/{key}/actions/{message}` also accepts `fence:
  {item_id, epoch}` for an executor's call fenced by its claim.
- `POST /work/{queue}/claim` accepts optional `target`, `max`, and `wait_ms`
  (a long poll, at most 30 000).
- `POST /work/{queue}/{uid}/{item}/heartbeat` accepts `epoch` and optional
  `progress: {"seq": n, "value": ...}`.
- `POST /work/{queue}/{uid}/{item}/finish` accepts `epoch` and `outcome`.
- `POST /work/{queue}/{uid}/{item}/release` accepts `epoch` and `reason`.

`GET /v1/actors/contract.json` returns registry metadata. HTTP errors include a
stable code and retryability; retryable responses include `Retry-After`.

Streams recheck access before each view, expire after five minutes, and share a
64-connection limit per router. Each polls at one-second intervals without
holding a database connection while idle. A reconnect gets a fresh view even
when its version cursor matches. View changes caused by authorization also
emit updates. These streams are snapshot synchronization, not the proposal's
full typed-event replay protocol; clients must not infer missing intermediate
events from them. Keys containing reserved URL characters need encoding.

## Bounds and storage

The core bounds state to 256 KiB; inputs, arguments, replies, views, and individual
payloads to 64 KiB; JSON depth to 32; and commands to 100 per transition. The
store additionally bounds aggregate commands to 1 MiB and pending inbox,
work, and effect counts. HTTP bodies are limited to 128 KiB. The pool accepts
1–128 connections, at most 64 waiters, a two-second checkout wait, and a
five-second connection/setup deadline.

Use PostgreSQL 17 or newer for the server-enforced two-second transaction
lifetime. Older versions use statement, lock, and idle transaction timeouts plus
application elapsed-time checks, which do not provide the same total-lifetime
bound. The pool uses `NoTls`; connect through a protected local socket or an
authenticated local proxy. Do not use an unprotected remote database connection.
The runtime uses PostgreSQL as its single writable authority; no partitioning,
replica routing, or cross-database transaction support is included.

State, message arguments, caller snapshots, outcomes, and exports can contain
private data. Apply the host's database access, backup, encryption, and retention
policies. There is no generic secret detector or encrypted field codec. Store
large artifacts elsewhere and put references in actor state.

Action receipts have a 24-hour expiry timestamp. They remain usable until an
operator runs retention. After removal, application-level business identity must
prevent an unsafe repeated operation. The retention API removes bounded batches
of events, history, and expired action receipts; it does not clean every completed
queue table. History stores input/state digests, not a replayable copy of every
input. Export refuses more than 2,048 rows per child table or 16 MiB of child data.
Destroyed records retain inaccessible state and ownership for recovery; destruction
is not a privacy-erasure or backup-deletion operation.

## Operator tools and checks

`actors-admin --help` lists commands. Set `ACTORS_DATABASE_URL` through the
environment, `ACTORS_OPERATOR` to the real operator identity, and
`ACTORS_ACCOUNT_ID` when operating on a private actor. This direct-database tool
trusts its local operator; do not expose it as an unauthenticated service.
The bundled registry executes only `example.counter`; production hosts supply
their own registry and authority checks. `tick` cannot dispatch public queued
messages without that authority integration.

The CLI supports migration, contract output, listing, inspection, history,
blocking/unblocking, destruction, export, actions, enqueueing, ticking,
`retry-inbox`, `resolve-work`, and `resolve-effect`. Mutating action bodies and
operator resolutions are read from bounded JSON on stdin. Resolution requires
`expected_epoch` for work or `expected_attempt` for an effect.

Run crate checks through the repository build lease:

```sh
cargo fmt -p actors --check
CARGO_TARGET_DIR=~/work/openagents-target-agent1 \
  ACTORS_TEST_DATABASE_URL='host=127.0.0.1 port=YOUR_SCRATCH_PORT dbname=actors_test' \
  openagents lease build --keep-target-dir -- cargo test -p actors -- --test-threads=1
```

Create an isolated PostgreSQL database first. Database tests apply the actor
migration and create unique workspaces; never point them at production. Without
`ACTORS_TEST_DATABASE_URL`, those tests print a skip message and return, so a
plain unit-test pass is not evidence of database verification.

Build `actors-load` with the build lease, then use the quiet lease to run
`actors-load --confirm-scratch 100 4 8` with `ACTORS_TEST_DATABASE_URL`. It records
latency and leaves its example records for inspection. It is a local action
probe, not a production capacity qualification or an HTTP benchmark.

## Remaining work

The crate is an initial implementation, not completion of the proposal. The
following work remains:

- Integrate the actual host authenticator, account/workspace lookup, executor
  grant issuer and revocation, database deployment, HTTP server, and operator
  ownership. None of the product actors or existing loops have moved.
- Implement domain executors and their operation-specific idempotency,
  cancellation, reconciliation, permission, and receipt contracts. Keep wallet,
  training, access, and artifact rules in their existing owning crates.
- Add cron/timezone/DST schedules, configurable alarm catch-up policies,
  wait-for-inbox results, and exact proposed wire-protocol conformance where
  it differs from the routes above. (Long-poll claims and the network client
  landed with the Mac-jobs consumer.)
- Add per-message schema derivation/compatibility checks, complete schema
  metadata, offline transition replay, full export/import, and a versioned
  restore procedure. Manual schema methods default to permissive metadata;
  Rust deserialization and handler checks enforce actual inputs.
- Add complete retention, compaction, archival and privacy-erasure policies,
  operational backlog/latency metrics, readiness, and deployment dashboards.
- `tests/multiprocess.rs` kills an executor process and runtime processes with
  SIGKILL mid-work (`actors-crash`): fenced expiry and a single reclaim, and
  every queued message applied exactly once across killed and parallel
  runtimes. Still to qualify: database outages and failover,
  rolling deployment, reconnect behavior, and realistic hot-key/fanout loads
  on the intended deployment. Current tests exercise concurrency and recovery
  transitions on an isolated local database; they are not a production soak.

Start product conversion with the landing-page orchestration, then fleet and
Mac control, followed by Coder run ownership, schedules, and idle work, as detailed
in the [migration audit](https://github.com/OpenAgentsInc/openagents/issues/11253#issuecomment-6103030524).
