# Actors: one primitive for durable, single-writer state

2026-10-10. Design, not implementation. Owner approval: build our own actor
primitive, taking ideas from Rivet Actors without depending on Rivet.
Tracking issue: [#11253 Actor primitive](https://github.com/OpenAgentsInc/openagents/issues/11253).

Nothing on this page exists yet. Section 2 describes what exists today, with
the code it was read from (`origin/main` at `1aec88da60`). Everything else is
the proposal.

## 1. Why

At least nine parts of the product keep a small piece of state that one
writer changes, that machines elsewhere must react to, and that web and phone
must watch: the landing queue, own-runs, Mac jobs, the web fleet, fleet rows,
scheduled prompts, the phone's agent board, dev-environment idle stop, and
decision dispatch. Each one has its own storage (GCS JSON with generation
preconditions, NFS files under flock, in-memory mutexes), its own claim
protocol, its own staleness rule, and its own polling loop. Section 2 lists
the gaps this leaves: unfenced claims, work lost between two compare-and-swap
steps, duplicate appends after a lost response, no retry when the one
process holding the work dies, and 1 to 5 second polling where a push would
do.

One primitive, an **actor**, can replace all of them. An actor is a keyed,
typed state machine with a single writer, durable state, a durable inbox,
alarms, and an event stream that web and phone subscribe to. It is stored in
the Postgres database we already run.

### Goals

- **In the repository, in Rust.** A new crate, `crates/actors`, linked into
  the processes that already exist. No new service, binary, broker, cache or
  sync engine.
- **Our Postgres is the only store.** It is the `openagents` database on
  Cloud SQL (`openagents-production-pg`, `openagents-staging-pg`;
  [data schema](../data/schema.md)), in a new schema, `actor`, owned by
  `crates/actors`. It is not the relay's database (`khala-sync-pg` /
  `nostr_relay_v2`).
- **Exactly one writer per actor**, with a fence a stale holder cannot get
  past, without depending on how many replicas run.
- **Every state change is transactional.** State, the events it emits, the
  messages it sends, the alarms it sets, and the side effects it requests
  commit in one Postgres transaction, or none of them do.
- **At-least-once delivery with idempotency keys** everywhere a message
  crosses a process boundary.
- **Pushed fan-out** to web (SSE) and phone (its existing poll, plus an SSE
  feed), scoped to one workspace.
- **One admin surface** to inspect, kill, replay and retry any actor.

### Non-goals

- **No Rivet dependency**, and no code copied from it (Apache-2.0; ideas
  only, see section 13).
- **No per-actor SQLite or KV database.** State is one JSON document per
  actor, capped in size. Large payloads go in Cloud Storage, as
  [data rule 2](../data/schema.md#rules-every-table-follows) already requires.
- **No general workflow engine** (no replayed code like Temporal's). Long
  work runs outside the actor as a claimed work item (section 4.6). Its
  result comes back as a message.
- **No WebSocket server in v1.** SSE plus HTTP POST covers every client we
  have.
- **No database credentials on user computers, dev environments or Pylons.**
  They act on actors through HTTP with their existing app tokens.
- **No multi-region placement.** We run in one region, `us-central1`.

## 2. What exists today

All paths are on `origin/main`.

### 2.1 Landing queue

Code: `crates/coder/src/task/land_queue.rs` (LQ), `crates/openagents-cli/src/land.rs`,
[land-queue.md](../cloud/land-queue.md).

| Concern | Today |
| --- | --- |
| State | JSON objects in `gs://openagentsgemini-coder-artifacts/land-queue/openagents/`: `entries/<id>.json`, `attempts/<id>/<NNN>.json`, `worker.json` (heartbeat). Every call shells out to `gcloud storage`. |
| Single writer | By convention. Only *creating* an entry uses `--if-generation-match=0` (LQ:309-324). Every other write overwrites with no precondition. `land work` refuses to start if `worker.json` names another machine seen within 300 s. That check reads then acts, so two workers starting together both pass. Nothing refreshes the heartbeat while checks run, so a step longer than 5 minutes lets a second worker start. `withdraw` races the worker's flip to `landing`. |
| Retries | 12 pushes with jittered backoff (2 s to 60 s) inside `landing::land`. A failed entry is requeued up to 3 times, then bounced. One `claude -p` repair turn on a conflict. |
| Wake-up | The worker polls every 20 s under systemd (`oa-land-worker.service`, `Restart=always`). `submit` starts the stopped VM if the heartbeat is stale. |
| Fan-out | None. The doc says the fleet view (#11228) reads these objects; no reader exists on `main`. |
| Crash | A restarted worker resumes its own `landing` entry. An entry left `landing` by a dead *other* machine is never taken again. If the worker dies after pushing to `main` but before writing `landed`, the resumed try finds no change and bounces an entry that landed (inference). Submitting twice queues the branch twice. |

### 2.2 Own-runs

Code: `crates/openagents-web/src/own_runs.rs` (OR), `crates/coder-sync/src/own_runs.rs`,
`crates/coder-new/src/own_runs.rs`.

| Concern | Today |
| --- | --- |
| State | Per-account GCS objects through `chat_store::Store`: `own-runs/capacity.json`, `own-runs/queue.json`, `own-runs/run-<id>.json`. |
| Single writer | Read, change, write with GCS `ifGenerationMatch`, 6 tries (`chat_store.rs:1624-1655`, OR:305-393). Taking a run is two CAS steps: remove from the queue, then flip each run `Waiting → Running`. |
| Retries | Coder polls every 5 s. Final report: 5 tries, 1 to 16 s backoff. |
| Wake-up | Pure polling. The gateway long-polls `GET /v1/own-runs/{id}?wait=` up to 20 s and the server re-reads the object every 750 ms. |
| Fan-out | None to web or phone (loopback gateway route only). |
| Crash | A `Running` run with no update for 90 s is marked failed when next read. A crash between the two take steps leaves a run `Waiting` that is in no queue, and nothing ever fails it. `start` has no idempotency key. A progress report whose response was lost is resent and its lines are stored twice. Cleanup happens only inside `start` and `take`. |

### 2.3 Mac jobs (#11223)

Code: `crates/openagents-web/src/mac_jobs.rs`, `crates/openagents-cli/src/mac_serve.rs`,
`crates/coder-sync/src/mac_jobs.rs`, [linked-mac.md](../cloud/linked-mac.md).

Same pattern as own-runs: `mac-jobs/macs.json`, `mac-jobs/index.json`,
`mac-jobs/job-<id>.json` and artifact parts. Two-step CAS take, 6 tries. The
Mac polls every 5 s. Readers long-poll up to 25 s at 750 ms. A job left
`Running` or `Asking` for 180 s is settled as failed when next read. The same
gap between the two take steps leaves a job `Waiting` until the 6-hour TTL
cancels it. Fan-out to the phone works by merging job items into
`GET /v1/agents`. Approvals come only from web or phone and are handed out
once.

### 2.4 Agent fleet, web fleet runs, fleet rows (#11163, #11164, #11228)

- **`crates/agent-fleet`**: an in-memory `Registry` behind a `std::sync::Mutex`,
  one OS thread per agent, and a `coder-lease` file lease per worktree. The
  desktop board is `~/.openagents/agents/<pid>.json`, rewritten every 15 s.
  Nothing survives the process.
- **Web fleet** (`pages/chat_agents.rs`, `pages/chat_work.rs`): the chat
  record holds `tasks` and each agent's `inbox`. Each Boat run is a
  `coder_cloud::Record` under a flock writer lease, driven by one OS thread
  that polls every 2 s. `chat_work::watch` re-syncs each chat every 3 s. No
  startup code re-drives unfinished runs after the web process restarts
  (inference).
- **Fleet rows (#11228, open)**: not on `main`. The unmerged commit
  `7bfa42ea66` reports issue-flow rows as activity items. Rows exist only
  while `chat work` runs.

### 2.5 Phone agent rows (`GET /v1/agents`)

`phone_api::agents` (`crates/openagents-web/src/phone_api.rs:787-837`) reads
`<owner>/agents/activity.json` (one object per account, CAS with 4 tries) and
merges Mac-job items. Coder writes it through
`POST /v1/computers/{name}/activity` every 5 s while busy and every 15 s when
idle. Phone actions (`POST /v1/agents/actions`) are idempotent by
`request_id`, queued as commands with a 600 s TTL, and handed out once. The
phone polls (`crates/openagents-mobile/src/account_link.rs:887-899`): every
3 s on a chat or Running screen, 5 s on another screen, 20 s when active but
not showing, 60 s when inactive, with doubling backoff on failure.

### 2.6 Background rules and scheduled prompts (#11177)

`crates/background` runs on the user's computer inside the Coder host. Its
state is files under `~/.openagents/background/`. One runner per computer
holds an exclusive `runner.lock`. It wakes on an mpsc channel or a 30 s poll.
A scheduled prompt is a rule whose action is `StartCoderRun`. Schedules sync
to `schedules/list.json` on the site every 20 to 60 s, and the newer record
wins. The site only keeps the list; the computer runs the prompts.

### 2.7 Chat worker

`coder-worker` on the VM `oa-coder-worker-1` has no durable queue. It
subscribes over Nostr to ephemeral NIP-CJ kinds and admits jobs with a
semaphore of 64. A request that arrives when every slot is taken is refused
`busy`. It deduplicates the last 4,096 event ids and refuses requests more
than 10 minutes old. A crash loses in-flight jobs. Execution requests (kind
25920) record a durable admission before they answer `accepted`.

### 2.8 Idle stop

- **`oa-dev-env-1`**: a systemd timer runs every minute and checks
  processes, the land-queue busy marker, ssh sessions and `keep-awake`.
  After `oa-dev-env-idle-minutes` (default 30) with none of them, it powers
  the VM off.
- **GCE pool hosts**: the instance deletes itself after 10 idle minutes.
- **Attested Pylon**: `--idle-stop SECS`.

### 2.9 Decision dispatch to Pylons (#11225)

`crates/gateway/src/decision_dispatch.rs`, in the gateway sidecar. The
`Book` of beacons and per-Pylon standing (EWMA latency, failures, a 60 s
bench) lives in memory and is refreshed from the relay on demand once it is
older than 30 s. It is per process and lost on restart. This is a cache, not
a record, and section 12 leaves it out of the migration.

### 2.10 Real-time paths

- **openagents-web has no Postgres** and no WebSocket server. Its SSE routes
  poll: `/chat/{id}/events` every 1 s, `/environments/.../events` every 1 to
  2 s. `/chats/events` is woken by an in-process `broadcast` of this
  process's own writes. Other replicas' writes are found only by slower
  re-reads.
- **Production web is pinned to one replica** (`deploy/production/render.py:157-160`:
  `minScale: "1"`, `maxScale: "1"`, "One writer on the account store").
  Several of the single-writer rules above hold only because of this pin.
- **Postgres is used** by `crates/tenancy` (the gateway sidecar's account
  store: tokio-postgres 0.7, a hand-written idle pool of 8, embedded
  migrations under `pg_advisory_lock`, per-store `pg_advisory_xact_lock`) and
  by `crates/nostr-relay`.
- **The relay already has the pattern we need**:
  - `pg_notify('nostr_event', ingest_seq)` after commit;
  - a LISTEN connection that reconnects with backoff and replays from a
    durable sequence;
  - push jobs claimed with `FOR UPDATE SKIP LOCKED`, fenced by a claim token,
    retried with backoff, and dead-lettered (`crates/nostr-relay/src/store/push.rs`,
    `gateway/server.rs:279-500`).

  This design reuses that shape.

### 2.11 Rules the design must keep

From [docs/data/schema.md](../data/schema.md#rules-every-table-follows),
`INVARIANTS.md`, `AGENTS.md` and [database.md](../deployment/database.md):

1. **Tenancy on every row.** Every row carries `workspace_id`, or
   `account_id` for rows outside any workspace. A row is never owned by a
   registry tenant alone (`INVARIANTS.md`, #11186).
2. **Big payloads live in buckets.** Postgres keeps `object_key`, size and
   `sha256`.
3. **Secrets are sealed before they reach the database.** Credentials are
   stored only as digests.
4. **Money and audit are append-only.**
5. **One crate owns each domain** and its migrations, embedded and applied
   at start under an advisory lock.
6. **One binary, one Postgres database** per product, with no cache, broker
   or second database (the relay rule, which this design applies to the
   actor runtime too).
7. **The vault server never holds a key or plaintext**
   (`crates/openagents-web/src/vault/mod.rs`).

## 3. Model

### 3.1 Identity

```
ActorId = (actor_type, workspace_id, key)
```

- `actor_type` is a static, versioned name such as `land.queue` or
  `mac.jobs`.
- `workspace_id` is always present. System actors (the landing queue) belong
  to the OpenAgents organization's workspace. There is no actor without a
  workspace, so tenancy rule 1 holds and deleting a workspace deletes its
  actors.
- `key` is a short string (at most 256 bytes, `[A-Za-z0-9._:/-]`), unique
  within type and workspace. Examples: `openagents/main` for a repository's
  landing queue, or the computer name for a Mac's jobs.
- Internally each actor also has a 128-bit `uid`, which every other table
  references.

### 3.2 State and ephemeral fields

- **`State`** is a typed Rust struct, `Serialize + DeserializeOwned`,
  persisted as one `jsonb` document with a `state_version: u32`.
  - Soft cap 64 KiB, hard cap 256 KiB. A commit over the hard cap fails the
    action with `actor/state_too_large`.
  - Anything bigger (logs, artifacts, transcripts) is written to Cloud
    Storage and referenced by `object_key`, size and `sha256`.
- **Ephemeral fields** live on the handler struct, rebuilt by
  `Actor::wake(&State)` whenever the actor is activated. They hold caches and
  derived indexes and are never written. A crash loses them and nothing may
  depend on them.

### 3.3 Creation and input

- `get_or_create(id, input)` creates the actor with
  `Actor::create(input) -> State` if it does not exist, in the same
  transaction as the first action. Otherwise it ignores `input`.
- `create(id, input)` fails with `actor/exists` if the actor exists.
- `get(id)` fails with `actor/not_found` if it does not.
- Input is capped at 64 KiB and recorded in history.

### 3.4 Actions (request and response)

- An action is a typed message with a typed reply.
- The caller waits for the reply. The action runs at once, under the actor's
  single-writer lock (section 4.1), in one transaction.
- A caller may send an `Idempotency-Key`. A second call with the same key
  within 24 hours returns the stored reply and does not run again
  (section 5.1).
- A **read-only action** (`Access::read`) runs against a snapshot without
  the lock and cannot change state.

### 3.5 The inbox (durable, ordered, asynchronous)

- A message sent to an actor's inbox is inserted durably and acknowledged
  before it is handled.
- Messages are handled one at a time, in inbox order (`seq`), each in its
  own transaction, with the same handler semantics as an action. The reply,
  if any, is stored and can be awaited.
- Senders can be clients, other actors (through the outbox), alarms, and
  completed effects or work items.
- This is how work that must not be lost reaches an actor. Actions are for
  callers that wait.

### 3.6 Events (fan-out)

- A handler emits typed events with `ctx.emit(e)`. Events are written to the
  actor's event log in the same transaction as the state change, each with a
  per-actor `seq`.
- Subscribers receive them in order and resume from a `seq` after a
  disconnect.
- Events carry only what a subscriber may see (section 9.2). They are not
  an audit log.
- Every actor also emits a built-in `snapshot` event, when asked, carrying
  `Actor::view(&State)`: the subscriber-safe projection of its state.

### 3.7 Connections

- A connection is a subscriber to one actor's events, opened with typed
  `ConnParams` (for example the phone's screen, or a filter on events).
- `Actor::authorize_connect(&caller, &params)` admits or refuses it.
- Connection state is ephemeral and held by the process serving the stream.
  Connections are not persisted: after a disconnect the client reconnects
  with its last `seq`. This matches Rivet's recent default of non-hibernating
  connections.
- An open connection keeps its actor active in that process's cache. It
  never holds the write lock.

### 3.8 Schedules and alarms

- `ctx.schedule().after(dur, msg)` and `.at(time, msg)` set one-shot alarms.
  `ctx.schedule().cron(name, expr, tz, msg)` sets a repeating one.
  `cancel(name)` removes either.
- Alarms are rows written in the handler's transaction.
- When an alarm comes due, the scheduler turns it into an inbox message with
  the idempotency key `alarm:<alarm_id>:<due_at>`, in one transaction that
  also advances or deletes the alarm row.
- Firing is therefore **at least once and never lost**. Rivet's one-shot
  alarms are at most once (deleted before they run).
- An alarm that came due while nothing was running fires at the next
  scheduler pass.
- Cron runs that were missed are coalesced into one message carrying
  `missed: n`.
- At most 1,000 alarms per actor.

### 3.9 Effects (side effects outside the database)

- A handler never performs I/O while it holds the lock. It records an
  **effect** with `ctx.effect(kind, payload)`, for example:
  - start a GCE instance;
  - post a GitHub comment;
  - send a phone notification;
  - call a Pylon.
- Effects commit with the state change and are run afterwards by an effect
  runner (section 4.5), at least once, with a stable idempotency key handed
  to the outside API when it supports one.
- An effect's outcome comes back to the actor as an inbox message, so the
  actor's state records what happened.

### 3.10 Work items (long work done elsewhere)

Most of the ad-hoc systems in section 2 are a queue of jobs that a remote
executor claims, heartbeats and finishes. The landing integrator, a Mac,
Coder taking own-runs, and Boat runs all work this way. The runtime provides
this once (section 4.6), so every actor that hands out work uses the same
claim, fence, staleness and requeue rules.

## 4. Execution

### 4.1 Single writer: row lock per transition, version as fence

Every state transition (an action, or one inbox message) runs as one
transaction:

```sql
BEGIN;
SET LOCAL lock_timeout = '2s';
SET LOCAL statement_timeout = '5s';
SELECT state, state_version, version, status
  FROM actor.instances WHERE uid = $1 FOR UPDATE;      -- the single-writer lock
-- idempotency check (actor.receipts), run the handler in memory
UPDATE actor.instances
   SET state = $2, state_version = $3, version = version + 1, updated_at = now()
 WHERE uid = $1 AND version = $4;                       -- the fence
INSERT INTO actor.events   ...;   -- emitted events
INSERT INTO actor.inbox    ...;   -- messages to other actors
INSERT INTO actor.alarms   ...;   -- schedule changes
INSERT INTO actor.effects  ...;   -- side effects to run after commit
INSERT INTO actor.receipts ...;   -- idempotency record and reply
INSERT INTO actor.history  ...;   -- the transition record
SELECT pg_notify('actor', $notify);
COMMIT;
```

**Why a row lock and not a long-held lease or a session advisory lock:**

- **The lock lives only as long as the transaction.** It needs no
  heartbeat, expiry or recovery: a process that dies mid-transition loses
  its connection and Postgres rolls back.
- **A stale writer cannot commit.** It either blocks on the row lock or
  fails the `version` check. `version` is the fencing token. A process
  holding a cached copy at version 41 cannot write over version 42.
- **Session advisory locks are wrong for Cloud Run.** They tie ownership to
  a connection, need one connection held open per active actor, and are lost
  silently when a pooled connection is recycled. Transaction-scoped advisory
  locks would add nothing over `FOR UPDATE` on the row we update anyway.
- **Placement no longer matters for correctness.** Any process with the
  actor type registered can run any transition, so lifting the web service's
  one-replica pin (section 2.10) stops being a correctness question.

**The rules that make this safe:**

- **A handler is a synchronous state transition.** It receives
  `&mut State`, the message and a context. It returns a reply, and does no
  I/O. Its time budget is 250 ms, and the transaction is aborted at 2 s.
  Anything slow is an effect or a work item.
- **A long-held lease is used only where a process must own something over
  time.** That means a work-item claim (section 4.6) and the optional
  activation affinity below. Both carry an epoch that every write must
  present.

**Activation affinity (optional, later).** Proposed only if measurements
show hot actors whose state load dominates. A process may take an affinity
lease (`actor.instances.lease_holder`, `lease_epoch`, `lease_until`, 30 s,
renewed every 10 s) and keep state cached. Every commit then also checks
`lease_epoch`. Other processes forward actions to the holder over internal
HTTP or fall back to the row lock when the lease is stale. v1 does not build
this; the row lock alone is the design.

### 4.2 Placement: which processes run what

| Place | Has Postgres | Runs transitions | Runs schedulers and effect runners | Acts as an executor |
| --- | --- | --- | --- | --- |
| `openagents-web` (Cloud Run, us-central1) | Yes (new: the Cloud SQL socket the gateway sidecar already uses) | All actor types | Yes | No |
| Chat worker VM `oa-coder-worker-1` | Optional, later (through the Cloud SQL connector) | Only types it registers | Optional | Yes, for chat jobs if moved onto actors (section 12, later) |
| Dev environments (`oa-dev-env-1`, pool hosts) | No | No | No | Yes: the landing integrator, environment runs |
| User computers (Coder link, `openagents mac serve`) | No | No | No | Yes: Mac jobs, own-runs, fleet agents, scheduled prompts |
| Pylons | No | No | No | Through NIP-DEC as today; not actors |

Notes:

- **The web service is the actor host.** It gains a direct Postgres
  connection: the same Cloud SQL socket, the `openagents_app` role, and the
  pool code from `crates/tenancy::db`, extracted into a small shared module
  rather than copied.
- **Machines without database credentials** reach actors only through the
  HTTP actions of section 10, signed in with their existing app tokens and
  bound to their workspace.
- **Local development** uses a real Postgres (Postgres.app, Homebrew, or
  `scripts/dev/pg.sh`, which runs one in a folder) with the same migrations.
  There is no file-backed production path.
- **`crates/actors` also has an in-memory store** (`MemStore`) for unit
  tests only. Its semantics are the same: one lock per actor and a version
  check. A second production backend would double the failure analysis for
  no gain.
- **A user computer that needs local actor-like state offline** keeps its
  own files, as the background runner does (section 12.5).

### 4.3 Activation, sleep and wake

Because every transition loads and locks the row, "active" means only that a
process holds a cached copy and subscribers. It does not mean a running task.

- **Activate.** The first action, inbox message, alarm or connection for an
  actor in a process loads its row. Expected cost is one indexed read, under
  5 ms.
- **Sleep.** After 60 s with no transition and no open connection in that
  process, the cached copy and ephemeral fields are dropped. `Actor::sleep`
  runs and may not change state. A sleeping actor costs one row and nothing
  in memory.
- **Wake.** Any of these wake an actor:
  - an action, which runs at once;
  - an inbox message, through the dispatcher;
  - a due alarm, through the scheduler;
  - a connection, which loads the snapshot.

  No process has to be "the" home of an actor.
- **Destroy.** `ctx.destroy()` marks the actor `destroyed` in the
  transaction. Pending inbox rows are dead-lettered and alarms deleted. The
  row is kept for 30 days for history, then deleted with its events and
  history.

### 4.4 Inbox dispatch

Each host runs a dispatcher (one tokio task per process). Its wake-up:

- `LISTEN actor`, where the notify payload names a `uid` with new inbox
  work;
- a fallback poll every 5 s;
- an immediate pass at start.

One pass:

```sql
-- pick actors with due work, one per row, without waiting on busy actors
SELECT DISTINCT ON (i.uid) i.uid
  FROM actor.inbox i JOIN actor.instances a ON a.uid = i.uid
 WHERE i.state = 'pending' AND i.not_before <= now() AND a.status = 'live'
 ORDER BY i.uid, i.seq
 LIMIT 64;
```

For each actor picked:

1. Open the transition transaction. Take the row with
   `FOR UPDATE SKIP LOCKED`, and skip the actor if another process holds it.
2. Take the oldest pending message.
3. Handle the message, mark it `done`, and commit.

The dispatcher keeps going on the same actor, up to 32 messages per pass, so
one busy actor does not starve the rest.

### 4.5 Effects runner and alarm scheduler

**Effect runner.** `actor.effects` rows are claimed with
`FOR UPDATE SKIP LOCKED`, the relay push executor's shape, with
`claim_token`, `claimed_until` (60 s) and `attempts`.

1. The runner calls the effect's registered executor with
   `effect_id` as the idempotency key.
2. On success or permanent failure it marks the row done and inserts an
   inbox message `EffectDone { effect_id, outcome }` to the actor, in one
   transaction.
3. On a transient error it sets `not_before` with exponential backoff
   (1 s doubling to 5 minutes) and jitter.
4. After the type's `max_attempts` (default 8) it dead-letters the effect
   and still delivers `EffectDone { outcome: DeadLettered }`.
5. A claim past `claimed_until` is free to be reclaimed. The first runner's
   late completion fails its `claim_token` check and is discarded.

**Alarm scheduler.** One task per host:

```sql
SELECT ... FROM actor.alarms
 WHERE due_at <= now() ORDER BY due_at LIMIT 256
 FOR UPDATE SKIP LOCKED;
```

Each due alarm becomes an inbox message, keyed as in section 3.8. The
alarm's row is then deleted (one-shot) or advanced (cron), in the same
transaction.

- The scheduler sleeps until the earliest `due_at` it knows of, capped at
  5 s.
- A transition that sets an alarm earlier than that sends `pg_notify`, which
  wakes it.

### 4.6 Work items: claim, heartbeat, finish

A work item is a row in `actor.work`, written by its actor's handler. Its
fields are:

- `uid`, `item_id`, `queue`;
- `target`: any executor of the workspace, or one named computer;
- `payload`;
- `state`: `ready`, `claimed`, `done` or `failed`;
- `claim_epoch`, `claimed_by`, `heartbeat_until`, `attempts`,
  `max_attempts`, `not_before`.

Executors use four HTTP calls (section 10.3):

1. **`claim`** (long-poll up to 25 s, or SSE). Assigns ready items matching
   the executor's queue and target, increments `claim_epoch`, and returns
   `(item, epoch, heartbeat_until)`. An executor without a free slot does
   not call `claim`, so backpressure lives on the executor's side.
2. **`heartbeat(item, epoch, progress?)`**. Extends `heartbeat_until` by the
   queue's lease (default 60 s; 15 minutes for the landing integrator's
   checks). It returns `cancel: true` if the actor asked for a cancel.
   Progress lines are appended with a client sequence number, so a resent
   heartbeat does not duplicate them (the own-runs bug).
3. **`finish(item, epoch, outcome)`**. Delivers `WorkDone { item, outcome }`
   to the actor's inbox and marks the item done, in one transaction.
4. **`release(item, epoch, reason)`**. Hands the item back without counting
   an attempt.

Every call presents `epoch`. A call with an older epoch gets `409
work/fenced` and changes nothing. That is the fence the landing queue,
own-runs and Mac jobs lack today.

**Staleness and requeue.** The scheduler sweeps `claimed` items past
`heartbeat_until`.

- It increments `attempts` and returns the item to `ready` with backoff.
- After `max_attempts` it fails the item. Either way, it delivers
  `WorkExpired { item, attempts }` to the actor, so the actor decides what
  that means. For example, the landing queue checks whether the commit
  reached `main` before retrying.
- Assignment is one transaction. The "removed from the queue but still
  `Waiting`" gap of own-runs and Mac jobs cannot occur.

## 5. Consistency and failure

### 5.1 Delivery and idempotency

| Path | Guarantee | Duplicate protection |
| --- | --- | --- |
| Action, no key | At most once per call. A transport error leaves the caller unsure. | None. Clients that retry must send a key. |
| Action with `Idempotency-Key` | Exactly-once effect on state. | `actor.receipts (uid, key)` holds the reply for 24 h. The same key with a different body gives `422 actor/idempotency_mismatch`. |
| Inbox send | At least once into the inbox. Exactly once into state when keyed. | Unique `(uid, idem_key)` on `actor.inbox`. A duplicate insert returns the existing message's id. |
| Actor-to-actor send | Exactly once into the target's inbox (same transaction as the sender's commit). | Key `<sender_uid>:<sender_version>:<n>`. |
| Alarm | At least once. | Key `alarm:<id>:<due_at>`. |
| Effect | At least once to the outside world. | `effect_id` passed as the external idempotency key where supported. Otherwise the executor checks before acting (for example "is the VM already running"). |
| Work item | At least once to an executor, at most one live claim. | `claim_epoch` fencing. `finish` is idempotent per `(item, epoch)`. |
| Event to subscriber | At least once, in order, resumable. | The subscriber tracks `seq` and ignores anything at or below it. |

Handlers must be deterministic given state and message. They must not read
the clock directly; `ctx.now()` is the transaction's time, recorded in
history so replay matches.

### 5.2 Crash recovery

- **A process dies mid-transition.** Postgres rolls back. The message stays
  pending, or the action's caller sees a transport error and retries with
  its key. Nothing partial exists.
- **A process dies after commit, before running effects or notifying.**
  Effects and inbox rows are durable. Another host's runner or dispatcher
  picks them up at its next poll, within 5 s.
- **The LISTEN connection drops.** It reconnects with backoff (100 ms to
  5 s, as the relay does), then reads everything newer than its last cursor
  before trusting notifications again. NOTIFY is a hint and the tables are
  the truth.
- **An executor dies.** Its claim expires and the item is requeued
  (section 4.6).
- **Postgres fails over or restarts.** Every transition in flight fails and
  is retried by its sender. Cloud SQL high availability is off today (data
  schema decision), so a zonal outage stops actors until it recovers. That
  is the same exposure sign-in already has.

### 5.3 Poison messages

- An inbox message whose handler returns an error, panics (caught with
  `catch_unwind`), or exceeds its time budget is retried with backoff:
  `attempts + 1`, `not_before = now + backoff`. It keeps its place, and later
  messages wait behind it so order holds.
- After `max_attempts` (default 5) it is marked `dead`, the actor gets
  `DeadLettered { seq, name, error }` as its next message, and the inbox
  moves on.
- A handler may mark an error `Permanent`, which dead-letters it at once.
- Dead messages stay 30 days, visible to `openagents actor dead` and
  retryable with `openagents actor retry`.
- A type may instead choose `on_poison: Block` when order matters more than
  progress. The actor then stops (`status = blocked`) and pages through the
  admin surface.

### 5.4 Timeouts

| Thing | Limit |
| --- | --- |
| Handler run | 250 ms budget, warned in metrics. Transaction aborted at 2 s (`statement_timeout`, `lock_timeout`). |
| Action HTTP call | 10 s total, including waiting for the lock. A `503 actor/busy` with `Retry-After` when the lock was not obtained. |
| Inbox send with `wait=true` | Up to 25 s for the reply, then `202` with the message id to poll. |
| Effect execution | Per kind, default 30 s, at most 10 minutes. |
| Work-item lease | Per queue, default 60 s, renewed by heartbeat. |

### 5.5 Backpressure

| Limit | Default | When exceeded |
| --- | --- | --- |
| Pending inbox messages per actor | 1,000 | `429 actor/inbox_full` |
| Pending effects per actor | 100 | The transition fails with `actor/effects_full` |
| Ready work items per queue | Set by the type (Mac jobs keep today's 16 waiting) | The type's own rule |
| Actions per caller | The site's existing rate limit | Unchanged |
| Lock waits per actor | 64 transitions waiting on one row in one process | Further actions get `503 actor/busy` instead of piling connections onto one row |
| Connection pool | 16 per web process (the instance's connection budget is shared with the gateway) | The dispatcher and runners yield to actions first |

### 5.6 Deploys and version skew

During a Cloud Run rollout, old and new revisions run side by side, and any
of them may handle any actor's transition. The rules:

1. **Every state document carries `state_version`.**
   - A process whose code knows versions up to N, reading N+1, refuses the
     transition with `503 actor/version_ahead`. The message is not attempted,
     and the newer revision handles it.
   - Reading an older version runs `Actor::migrate(from, json) -> State` in
     memory. The result is written at the next commit.
   - Migrations only move forward and never drop fields a previous version
     still reads (expand, then contract in a later release).
2. **Messages are named and versioned** (`Name@v`).
   - An unknown message name is left pending with `not_before = now + 30 s`,
     without counting an attempt, for up to 1 hour. After that it is
     dead-lettered as `unknown_message`.
   - New message names ship in a release before the release that sends them.
3. **Events are additive.** Clients ignore unknown event names and fields.
4. **Removing a type:** stop creating it, drain it (`openagents actor drain
   TYPE`), and delete it in a later release.
5. **Schema migrations of the `actor` tables** are embedded in
   `crates/actors/migrations/actor/NNNN_*.sql` and applied at start under an
   advisory lock (data rule 6). They must be compatible with the previous
   revision's code.

### 5.7 State schema versioning

```rust
impl Actor for LandQueue {
    const STATE_VERSION: u32 = 2;
    fn migrate(from: u32, state: serde_json::Value) -> Result<serde_json::Value, MigrateError> {
        match from {
            1 => Ok(v1_to_v2(state)),
            _ => Err(MigrateError::Unknown(from)),
        }
    }
}
```

`crates/actors` provides a test helper. It loads every fixture under
`crates/<owner>/tests/actor-fixtures/<type>/v*.json` and checks that each one
migrates to the current version and round-trips. A type change without a
fixture fails CI.

## 6. Storage schema (`actor`, owned by `crates/actors`)

```sql
CREATE SCHEMA actor;

CREATE TABLE actor.instances (
  uid            uuid PRIMARY KEY,
  actor_type     text NOT NULL,
  workspace_id   text NOT NULL,           -- tenancy rule 1
  key            text NOT NULL,
  status         text NOT NULL DEFAULT 'live',   -- live | blocked | destroyed
  state          jsonb NOT NULL,
  state_version  integer NOT NULL,
  version        bigint NOT NULL DEFAULT 0,      -- fencing token
  event_seq      bigint NOT NULL DEFAULT 0,
  inbox_seq      bigint NOT NULL DEFAULT 0,
  created_at     timestamptz NOT NULL DEFAULT now(),
  updated_at     timestamptz NOT NULL DEFAULT now(),
  destroyed_at   timestamptz,
  UNIQUE (actor_type, workspace_id, key)
);

CREATE TABLE actor.inbox (
  uid uuid REFERENCES actor.instances ON DELETE CASCADE,
  seq bigint, name text NOT NULL, payload jsonb NOT NULL,
  idem_key text, sender text NOT NULL,      -- principal or actor uid
  state text NOT NULL DEFAULT 'pending',    -- pending | done | dead
  attempts int NOT NULL DEFAULT 0, not_before timestamptz NOT NULL DEFAULT now(),
  last_error text, reply jsonb, created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (uid, seq), UNIQUE (uid, idem_key)
);
CREATE INDEX ON actor.inbox (not_before) WHERE state = 'pending';

CREATE TABLE actor.events  (uid uuid, seq bigint, name text, payload jsonb,
  created_at timestamptz, PRIMARY KEY (uid, seq));
CREATE TABLE actor.alarms  (id uuid PRIMARY KEY, uid uuid, name text, due_at timestamptz,
  cron text, tz text, message jsonb, UNIQUE (uid, name));
CREATE TABLE actor.effects (id uuid PRIMARY KEY, uid uuid, kind text, payload jsonb,
  state text, attempts int, not_before timestamptz, claim_token uuid,
  claimed_until timestamptz, outcome jsonb, created_at timestamptz);
CREATE TABLE actor.work    (uid uuid, item_id text, queue text, target text,
  payload jsonb, state text, claim_epoch bigint, claimed_by text,
  heartbeat_until timestamptz, attempts int, max_attempts int, not_before timestamptz,
  progress_seq bigint, PRIMARY KEY (uid, item_id));
CREATE TABLE actor.receipts (uid uuid, idem_key text, request_sha256 bytea,
  reply jsonb, created_at timestamptz, PRIMARY KEY (uid, idem_key));
CREATE TABLE actor.history (uid uuid, version bigint, kind text, name text,
  caller text, idem_key text, payload jsonb, outcome text, error text,
  duration_us int, state_version int, at timestamptz, PRIMARY KEY (uid, version, kind));
```

Notes:

- Every child table cascades from `instances`. Deleting a workspace's actors
  is `DELETE FROM actor.instances WHERE workspace_id = $1`, so the data
  rule's tenancy test passes.
- `history.payload` stores the message, needed for replay. Types may mark
  fields `#[actor(redact)]` so that only a digest is stored.
- Section 9.3 gives retention for each table.

The `actor` schema and its tables are added to [docs/data/schema.md](../data/schema.md)
in the change that creates them.

## 7. Fan-out to clients

### 7.1 Cross-process event path

- Each transition's `pg_notify('actor', '<uid>:<event_seq>:<inbox?>:<alarm?>')`
  is sent inside the transaction, so Postgres delivers it only on commit.
- Each process holds one `LISTEN actor` connection, the relay's
  `NotificationListener` pattern, extracted into a shared module.
- On a notification for an actor with local subscribers, the process reads
  `actor.events WHERE uid = $1 AND seq > $last` and pushes the rows.
- The payload stays under Postgres's 8,000-byte NOTIFY limit because it
  carries no data. Events are always read from the table.
- This works within one region and one database, which is all we run. No
  broker is involved.

### 7.2 Web

`openagents-web` serves `GET /v1/w/{workspace}/actors/{type}/{key}/events`
as SSE:

- `Last-Event-ID: <seq>` resumes. A gap older than retention gets a
  `snapshot` event first.
- Keep-alive every 15 s, as the existing SSE routes do. The stream ends
  after 600 s and the browser reconnects.
- HTMX pages use the SSE extension, or a page-level stream that rerenders a
  fragment on each event.

The existing 1 s polling SSE routes (`/chat/{id}/events`,
`/environments/...`) are not part of this design. They move over only when
their data becomes an actor.

### 7.3 Phone

- **v1 changes nothing on the phone.** `GET /v1/agents` keeps its shape.
  Its server side reads actor views (the fleet row and Mac-job actors)
  instead of `agents/activity.json`. `POST /v1/agents/actions` becomes an
  action call that uses the phone's existing `request_id` as the
  `Idempotency-Key`.
- **v2 adds `GET /v1/w/{workspace}/feed`.** It is one SSE stream that
  multiplexes events from the actors the phone shows, each tagged with type,
  key and seq.
  - `crates/openagents-mobile` opens it while the app is active and uses
    each event as the existing `Notify` nudge, so a pass runs at once.
  - It keeps polling as the fallback, and at 60 s while the stream is
    healthy.
  - Background delivery still uses the relay's NIP-PL push. A type that
    wants to notify a phone records a `notify.push` effect.

### 7.4 Executors

`claim` long-polls through the same `LISTEN`: the request is held open and
answered the moment a matching item becomes ready. The 5 s polls of Coder
and `mac serve` and the 20 s poll of the landing integrator become one
waiting request each, at the same server cost as today's long-polls.

### 7.5 Actors on user machines (Nostr)

- v1 has no actor that *lives* on a user machine.
- Machines are executors that talk HTTP to actors in the web. When the
  website is unreachable, today's local files keep working.
- If a later type must run with the computer offline, such as background
  rules, it uses the same trait over a local file store on that machine.
  Its events reach the account through the coder-sync upload or, when a
  phone must hear directly, through NIP-44-encrypted events to the account's
  own key on `relay.openagents.com`.
- That is listed as an open question (section 14), not designed here.

## 8. Observability

- **History.** Every transition writes one `actor.history` row: version,
  kind (action, message, alarm, effect result, work result), name, caller
  principal, idempotency key, outcome, error, duration and state version.
  This is the per-actor timeline.
- **Tracing.** One `tracing` span per transition, with fields:
  - `actor.type`, `actor.uid`, `actor.key`, `workspace`;
  - `msg.name`, `version`, `attempt`, `outcome`, `lock_wait_us`,
    `handler_us`, `commit_us`.

  Logs go to Cloud Logging as structured JSON, as the web service already
  logs.
- **Metrics.** These are counters and histograms written as structured log
  lines, and turned into Cloud Monitoring log-based metrics:
  - transitions by type and outcome;
  - lock wait, handler and commit time;
  - inbox depth and oldest pending age, by type;
  - dead letters;
  - effect attempts;
  - work claims, expiries and fenced calls;
  - NOTIFY-to-SSE latency;
  - active cached actors per process.

  Alert when the oldest pending inbox message is over 60 s, on any dead
  letter in a system type, or when the LISTEN connection is down for over
  30 s.
- **Admin command**: `openagents actor ...`, behind the site's admin role,
  over `/admin/api/actors/...`.

  | Command | What it does |
  | --- | --- |
  | `ls TYPE [--workspace W] [--status S]` | List actors |
  | `inspect TYPE KEY` | State, version, inbox, alarms, effects, work, last 20 history rows |
  | `history TYPE KEY [--since V]` | Full timeline |
  | `dead [TYPE]`, `retry TYPE KEY SEQ`, `drop TYPE KEY SEQ` | Dead letters |
  | `replay TYPE KEY [--from V]` | Rerun the recorded messages from a version's state in memory against current code and diff the result. Never writes. |
  | `kill TYPE KEY` | `status = blocked`: no transitions, inbox kept |
  | `resume TYPE KEY` | Clear `blocked` |
  | `destroy TYPE KEY` | Section 4.3 |
  | `drain TYPE` | Refuse new creates, let inboxes empty |

  Every admin call that changes something writes an `audit.events` row
  (data rule 5).

## 9. Security

### 9.1 Who may call what

Each action, message and connection declares its access in the type:

```rust
#[derive(Clone, Copy)]
pub enum Access {
    Member,                 // any member of the actor's workspace
    Owner,                  // the workspace owner
    Computer,               // an app token bound to a computer in the workspace
    ComputerNamed,          // only the computer named by the actor's key or the work item's target
    Service,                // our own processes (internal token), never a person
    Admin,                  // site admin role, audited
}
```

- The router resolves the caller from the session or app token
  (`cloud.app_account`, as `coder_sync` does today).
- It reads `workspace_id` from the URL and **checks membership before
  resolving the actor**. A key from another workspace resolves to
  `404 actor/not_found`, never `403`, so other workspaces' keys cannot be
  probed.
- Actor-to-actor sends are allowed only within one workspace, except from
  types marked `system`. Those are the OpenAgents-run ones such as the
  landing queue, which may only send to actors in their own system
  workspace.
- Work-item calls check the caller against the item's `target`.
- A test enumerates every registered type and fails if any action lacks an
  `Access`. A second test sends every action across workspaces and expects
  `404`.

### 9.2 Secrets never in actor state

- A state, message, event or effect payload must not contain a secret.
  - Tokens, provider keys and credentials are referenced by their existing
    id (`provider_keys` row, Secret Manager name, vault slot id).
  - They are resolved by the effect executor at the moment of use, never
    stored.
- `crates/actors` checks every serialized payload at commit for known secret
  shapes (`oak_`, `oa_agent_`, `sess_` tokens, `sk-`, `ghp_`, `github_pat_`,
  PEM blocks, 64-hex nsec forms). A match fails the transition with
  `actor/secret_in_payload` and logs only the field path.
- The landing queue's GitHub calls and the web fleet's "viewer's own Claude
  key released for that run only" stay outside the actor. The actor holds
  the run's id, and the effect executor asks the owning store for the key.
- Vault data never enters an actor. The vault server's rule (no key, no
  plaintext) is unchanged.

### 9.3 Retention and deletion

| Table | Kept |
| --- | --- |
| `instances` | Until destroyed, then 30 days. Deleted with the workspace. |
| `inbox` done rows | 7 days. Dead rows 30 days. |
| `events` | Last 1,000 per actor or 7 days, whichever keeps more. A type may lower this. |
| `receipts` | 24 hours |
| `history` | 30 days, or as long as the type declares (the landing queue keeps 90). Payload-redacted fields keep only digests. |
| `effects`, `work` | Done rows 7 days, failed 30 days |

- A sweeper (one task per host, under an advisory transaction lock so one
  runs at a time) deletes expired rows in batches of 1,000.
- Deleting an account or workspace deletes its actors by `workspace_id`
  through the same path as its other tables.
- Actor state belongs to the "Account" privacy class unless the type
  declares otherwise.

## 10. API sketch

### 10.1 Rust

The design follows Rivet's Rust shape (one type per message, `Handles<M>`,
tuples for message sets), with synchronous handlers.

```rust
// crates/actors/src/lib.rs (sketch)

pub trait Actor: Sized + Send + 'static {
    const TYPE: &'static str;                 // "land.queue"
    const STATE_VERSION: u32;
    const SYSTEM: bool = false;
    type State: Serialize + DeserializeOwned + Send;
    type Input: Serialize + DeserializeOwned + Send;
    type Event: ActorEvent;                   // enum, #[derive(ActorEvent)] or manual impl
    type View: Serialize;                     // what subscribers may see
    type ConnParams: DeserializeOwned + Default;
    type Messages: MessageSet<Self>;          // (Submit, Withdraw, WorkDone, ...)

    fn create(input: Self::Input, ctx: &Ctx<Self>) -> Result<Self::State, ActorError>;
    fn wake(state: &Self::State) -> Self;     // rebuild ephemeral fields
    fn view(state: &Self::State, caller: &Caller) -> Self::View;
    fn migrate(from: u32, state: Value) -> Result<Value, MigrateError> { Err(MigrateError::Unknown(from)) }
    fn authorize_connect(caller: &Caller, params: &Self::ConnParams) -> Result<(), ActorError> { caller.require(Access::Member) }
    fn sleep(&mut self, _state: &Self::State) {}
}

pub trait Message: Serialize + DeserializeOwned + Send + 'static {
    const NAME: &'static str;                 // "submit@1"
    const ACCESS: Access;
    const READ_ONLY: bool = false;
    type Reply: Serialize + DeserializeOwned + Send;
}

pub trait Handles<M: Message>: Actor {
    fn handle(&mut self, state: &mut Self::State, msg: M, ctx: &mut Ctx<Self>)
        -> Result<M::Reply, ActorError>;
}

pub struct Ctx<A: Actor> { /* transaction-scoped */ }
impl<A: Actor> Ctx<A> {
    pub fn id(&self) -> &ActorId;
    pub fn caller(&self) -> &Caller;
    pub fn now(&self) -> Timestamp;                        // transaction time, recorded
    pub fn emit(&mut self, event: A::Event);
    pub fn send<B: Handles<M>, M: Message>(&mut self, to: ActorRef<B>, msg: M);   // via inbox, same txn
    pub fn schedule(&mut self) -> Schedule<'_, A>;         // after / at / cron / cancel
    pub fn effect<E: Effect>(&mut self, effect: E) -> EffectId;
    pub fn work(&mut self) -> Work<'_, A>;                 // offer / cancel / list
    pub fn destroy(&mut self);
    pub fn random(&mut self) -> u64;                       // seeded from (uid, version), replayable
}

pub trait Effect: Serialize + DeserializeOwned + Send + 'static {
    const KIND: &'static str;                              // "gce.start@1"
    const MAX_ATTEMPTS: u32 = 8;
    // executed by a registered async EffectExecutor, outside any transaction
}

// Errors cross the wire as { group, code, message, retryable, metadata }.
pub struct ActorError { pub group: &'static str, pub code: &'static str,
                        pub message: String, pub retryable: bool, pub permanent: bool }

// Hosting
let runtime = actors::Runtime::builder(pool)
    .register::<LandQueue>()
    .register::<MacJobs>()
    .effect_executor::<GceStart>(gce_client)
    .start().await?;                                       // dispatcher, scheduler, effect runner, listener, sweeper
let router: axum::Router = runtime.http_routes();          // section 10.2
let r = runtime.client().get_or_create::<LandQueue>(ws, "openagents/main", input)
    .call(Submit { branch, issue }).idempotency_key(k).await?;
```

Message sets are tuples, with a `macro_rules!` that generates the dispatch
table, the JSON contract and the access test. No procedural macro is
required in v1. A small derive for `ActorEvent` and `Message` may come later
if the boilerplate hurts.

### 10.2 HTTP and SSE wire contract

Base: `/v1/w/{workspace}/actors/{type}/{key}`. JSON bodies. Auth: the site
session, or `Authorization: Bearer <app token>`.

| Method and path | Body | Reply |
| --- | --- | --- |
| `POST .../actions/{name}` | `{ "input"?: {...}, "args": {...} }`. Header `Idempotency-Key` optional; `create=1` query to get-or-create with `input`. | `200 { "reply": {...}, "version": 43 }` |
| `POST .../inbox/{name}` | `{ "args": {...}, "wait"?: 25 }`. Header `Idempotency-Key` recommended. | `202 { "seq": 17 }`, or `200 { "seq": 17, "reply": {...} }` when it finished within `wait` |
| `GET .../inbox/{seq}` | | `{ "state": "pending"\|"done"\|"dead", "reply"? }` |
| `GET .../view` | | `{ "view": {...}, "version": 43, "event_seq": 120 }` |
| `GET .../events` (SSE) | `Last-Event-ID` or `?after=` | `id: <seq>`, `event: <name>`, `data: <json>` |
| `GET /v1/w/{workspace}/feed` (SSE) | `?topics=type:key,...` | `id: <type>:<key>:<seq>`, as above |
| `GET /v1/actors/contract.json` | | Every type's messages, replies, events, views and access, as JSON Schema |

Errors have the shape `{ "error": { "group": "actor", "code": "busy",
"message": "...", "retryable": true, "metadata": {} } }`. Status codes:

| Status | Codes |
| --- | --- |
| `400` | `bad_args` |
| `404` | `not_found` (also wrong workspace) |
| `409` | `exists`, `work/fenced` |
| `422` | `idempotency_mismatch`, a handler's own errors |
| `429` | `inbox_full` |
| `503` | `busy`, `version_ahead` (with `Retry-After`) |

**Typed clients.** `contract.json` is generated from the Rust message
types by the `macro_rules!` set and `serde_json` schemas written beside each
type, with no new derive dependency. The Effect Native and TypeScript side
generates `effect/Schema` codecs and a typed client from it, so web, desktop
and phone code can call an action with typed arguments and decode events
without hand-written shapes. The Rust phone crate uses the Rust types
directly.

### 10.3 Executor calls (work items)

| Method and path | Body | Reply |
| --- | --- | --- |
| `POST /v1/w/{ws}/work/{queue}/claim` | `{ "executor": "mac:studio", "max": 1, "wait": 25 }` | `200 { "items": [{ "actor": {...}, "item_id", "epoch", "payload", "heartbeat_until" }] }`, or `204` when nothing came within `wait` |
| `POST .../work/{queue}/{actor_uid}/{item_id}/heartbeat` | `{ "epoch", "progress"?: { "seq", "lines": [...] } }` | `{ "heartbeat_until", "cancel": bool }` or `409 work/fenced` |
| `POST .../finish` | `{ "epoch", "outcome": {...} }` | `200`, idempotent for the same epoch |
| `POST .../release` | `{ "epoch", "reason" }` | `200` |

## 11. Performance targets and how to test them

Targets are for one `openagents-web` process against `openagents-production-pg`
(db-custom-1-3840) in the same region:

| Measure | Target |
| --- | --- |
| Action latency, uncontended, small state (4 KiB), excluding network to the client | p50 5 ms, p99 25 ms |
| Action latency, 50 concurrent callers on **one** actor | p99 250 ms, at least 150 transitions/s on that actor |
| Activation (first load of a sleeping actor) | p99 10 ms |
| Inbox send to handled, idle system | p99 100 ms (NOTIFY path), 5 s worst case (poll fallback) |
| Commit to SSE delivery, same process or another | p99 200 ms |
| Alarm lateness | p99 1 s |
| Work item ready to a waiting executor's `claim` | p99 300 ms |
| Sleeping actors | Unbounded (rows). Plan for 1 million. |
| Cached active actors per web process | 10,000 at 256 MiB budget |
| Transitions per database, all actors | 1,500/s sustained on the production tier, 300/s on staging's db-f1-micro |
| Events fanned out per process | 5,000/s to 2,000 open SSE streams |

Today's load is far below these targets: dozens of actions a minute. The
targets exist to show the design has headroom, and to catch a regression
such as a missing index or a lock held across I/O.

**How to test:**

- **Unit and model tests in `crates/actors`** (with `MemStore`, and against
  a real Postgres when `ACTORS_TEST_DATABASE_URL` is set, as the relay's
  `multiprocess_postgres.rs` does):
  - **Fencing.** Two runtimes on one database race transitions on one
    actor. Assert versions are dense and no write is lost.
  - **Crash.** Kill a runtime (drop its connections mid-transaction) and
    assert rollback and redelivery.
  - **Exactly-once state with keys.** Resend every message 3 times in
    random order.
  - **Poison.** A handler that panics: assert it is dead-lettered after 5
    tries and the inbox moves on.
  - **Work fencing.** Claim, let it expire, have a second executor claim,
    have the first `finish`: assert `409`.
  - **Version skew.** A runtime with `STATE_VERSION = 1` against a v2 row:
    assert `version_ahead`, no write.
  - **Tenancy.** Every action from another workspace gets `404`.
- **A bounded model** of the transition, claim and expire protocol: two
  executors, two hosts, one actor, crash at any step. Checked by exhaustive
  enumeration in a Rust test (no new tool). It must reach no state where two
  epochs both finish or a committed message is lost. Counterexamples become
  regression tests, per the workspace's formal-verification guidance. The
  invariants it checks go into `INVARIANTS.md` in the implementing change.
- **Load: `cargo run -p actors --bin actor-load --`** against
  `openagents-staging-pg` (the instance the data schema reserves for load
  tests). It drives each scenario above and prints the latency histograms.
  The result is recorded as a measurement document under `docs/` with the
  revision, inputs and outcomes.

## 12. Migration plan

### 12.0 Order, and why the landing queue goes first

**Landing queue first.**

- It is the smallest system with the most correctness gaps (section 2.1):
  - unfenced takes;
  - a heartbeat that goes stale during checks;
  - a crash after push that bounces a landed change;
  - double submits;
  - entries orphaned on a dead machine.
- It exercises every part of the primitive except client fan-out: inbox,
  alarms, effects (start the VM, comment on the issue), work items with long
  leases, and history.
- Its users are our own agents, so a defect costs us a re-submit, not a
  customer.
- It has one integrator and one producer path, `openagents land submit`, so
  the cut-over is one CLI release.
- Rollback is simple: the GCS queue code stays in place behind a flag for one
  release.

**Fleet runs second.**

- Fleet rows (#11228) are not on `main` yet, so building them on actors
  costs nothing to migrate and avoids a third ad-hoc pattern.
- They prove the client side: web SSE, the phone's `GET /v1/agents` reading
  actor views, and phone actions as idempotent action calls.
- Doing them first would mean debugging the runtime and the phone at once.
  Doing the landing queue first means the runtime is proven before the
  phone depends on it.

Each step below is its own issue under [#11253](https://github.com/OpenAgentsInc/openagents/issues/11253).
Each lands only after the previous one has run a week in production without
a dead letter in a system type.

**Step 0, the runtime.**

- `crates/actors`: schema, transitions, inbox, alarms, effects, work, SSE,
  admin routes, the `openagents actor` command, and the tests of section 11.
- `openagents-web` gains its Postgres connection, through the shared pool
  extracted from `crates/tenancy::db`.
- Acceptance:
  - the section 11 test suite passes against staging Postgres;
  - `actor-load` meets the staging targets;
  - a demo actor (`demo.counter`) runs on staging with SSE, viewed in a
    browser.

### 12.1 Landing queue (`land.queue`, key `<owner>/<repo>`, system workspace)

**What changes**

- `land_queue::Store` is replaced by calls to the actor:
  - `submit` becomes an action with `Idempotency-Key` = branch + head
    commit, so a resubmit of the same commit returns the existing entry;
  - `withdraw` becomes an action that fails if the entry is not `queued`,
    decided under the lock (no race);
  - `status` and `show` read the view and history.
- Each queued entry is a work item on queue `land`, target `any`, with a
  15-minute lease. The integrator heartbeats from a thread during checks,
  not only between steps.
- `openagents land work` becomes an executor: `claim` (long-poll), then
  heartbeat, then `finish(outcome)`.
- On `WorkExpired`, the actor records an effect, `git.contains(main,
  head)`. If the head is already on `main`, the entry becomes `landed`.
  Otherwise it is requeued, up to 3 attempts.
- The issue comment and close, and the board move, become effects keyed by
  entry id, so a crash cannot double-comment.
- Waking the VM becomes an effect `gce.start` recorded when work is offered
  and no executor has claimed within 60 s. It replaces the stale-heartbeat
  check in `submit`.
- Attempt records become history rows.

**What is deleted**

- The GCS `entries/`, `attempts/` and `worker.json` layout, and the
  `gcloud storage` shelling (LQ:282-372).
- The heartbeat check in `land work` and `wake()` in the CLI.
- The CLI keeps `--queue` only for a folder used in tests, backed by
  `MemStore`.

**Rollback**

- `OPENAGENTS_LAND_QUEUE=gs://...` keeps selecting the old code path for one
  release.
- A one-shot `openagents land export` writes the actor's open entries back
  as GCS objects.

**Acceptance**

1. Two `land work` processes on two machines against one queue: every entry
   lands exactly once.
2. Kill the integrator with `kill -9` during checks: the entry is requeued
   after its lease and lands.
3. Kill it after the push to `main` and before `finish`: the entry is
   recorded `landed` and not bounced.
4. Submit the same commit twice: one entry.
5. `openagents actor history land.queue openagents/main` shows each try.

### 12.2 Fleet rows and web fleet runs (`fleet.agent`, key `<computer>/<agent id>`, and `fleet.board`, key `<computer>`)

**What changes**

- Coder, `chat work` and the web fleet report each agent through actions
  (`report`, keyed by agent id + report seq) instead of rewriting
  `agents/activity.json`.
- Phone actions (stop, approve, deny, message) become inbox messages on the
  agent's actor, with the phone's `request_id` as the idempotency key. The
  computer receives them through its `claim` on queue `commands`, target
  `ComputerNamed`, instead of the activity post's hand-out.
- `GET /v1/agents` is answered from `fleet.board` views. The JSON shape is
  unchanged, so the phone needs no release.
- The web Agents panel and `/settings/agents` subscribe over SSE instead of
  reloading every 5 s.
- Web fleet runs (#11164) get a `fleet.run` actor per Boat run. Its alarm
  (every 30 s) replaces `chat_work::watch` and the driver thread's 2 s poll
  as the thing that advances the run: an effect polls Boat. A web restart no
  longer orphans a run.

**What is deleted**

- `agents/activity.json` and its CAS loop (`phone_api.rs:615-687`).
- `chat_work::watch`.
- The unmerged `chat_fleet.rs` report loop, which is rewritten as an
  executor.

**Rollback.** For one release, the activity route also accepts the old post
and writes both.

**Acceptance**

- A real `chat work --on gce` run on `oa-dev-env-1` is watched on web and
  phone, then stopped from the phone. This is the #11228 acceptance the
  unmerged branch did not run.
- The web service is restarted mid-run, and the run still completes and
  shows its result.
- A row's change reaches an open web page in under 1 s.

### 12.3 Mac jobs (`mac.jobs`, key `<computer>`)

**What changes**

- Jobs are work items on queue `mac`, target `ComputerNamed`.
- `openagents mac serve` uses `claim` and `heartbeat` instead of its 5 s
  poll and 8 s ping.
- Progress lines use the heartbeat's sequenced progress.
- Approvals are inbox messages (`Answer`, keyed by the question id).
- Artifact parts stay in GCS. The job's state keeps their keys and digests.
- `Job::settle`'s lazy staleness becomes `WorkExpired`.

**What is deleted.** `mac-jobs/index.json`, `job-*.json`, the two-step take,
and the settle-on-read rules.

**Rollback.** The site serves both route sets for one release. `mac serve`
picks by the site's capability list, as it already does for missing routes.

**Acceptance**

- The #11223 end-to-end (a Mac build job with an approval and artifacts) runs
  from the phone.
- Killing `mac serve` mid-job fails the job after the lease, with the reason
  shown on the phone.
- Two submits with one key create one job.

### 12.4 Own-runs (`own.runs`, key `<computer>`)

The same move as Mac jobs, which was built on own-runs: queue `own`, target
`ComputerNamed`, and capacity reported in `claim`.

- The gateway's long-poll (`GET /v1/own-runs/{id}?wait=`) becomes an inbox
  send with `wait`, or an SSE subscription on the run.
- **Deleted:** `own-runs/queue.json`, `run-*.json`, `capacity.json`, the
  750 ms re-read loop.
- **Rollback:** as for Mac jobs.
- **Acceptance:** an own-Claude run from the gateway completes. A run whose
  response was lost does not duplicate lines. A crash between offer and
  claim cannot strand a run.

### 12.5 Scheduled prompts (`schedules`, key `account`)

- **What changes.** The site's `schedules/list.json` becomes one actor per
  workspace holding the list. Its sync merge rule (newer `updated` wins,
  delete wins ties) moves into the `sync` action, under the lock.
- **Firing stays on the computer.** The background runner remains the
  executor, because a prompt runs with the user's local tools and keys.
- Each schedule also gets a cron alarm on the actor. When it comes due, the
  actor offers a work item to the schedule's computer and records the run in
  history. The site can then show "missed: computer offline" and catch up
  once when the computer returns.
- The local runner's `daily_due` and `mark_daily` stay as the offline
  fallback. Both sides share the alarm's idempotency key, so the one that
  does not run first skips.
- **Deleted:** the site-side CAS loop in `account_schedules.rs`.
- **Rollback:** the sync route keeps its shape. The old object is written
  alongside for one release.
- **Acceptance:** a schedule made on the phone fires on the computer once.
  With the computer off at the due time it shows as missed and fires once on
  return. Editing on two devices converges.

### 12.6 Dev-environment idle stop (`devenv`, key `<instance>`)

- **What changes**
  - The VM keeps its local timer as the authority on "busy", because only
    it sees its processes. It now reports `busy` or `idle` to the actor
    every minute (an action keyed by minute).
  - The actor owns the decision and the record. Its alarm, `idle_limit`
    after the last busy report, records an effect `gce.stop`.
  - "Start when work arrives" becomes one place: any actor that offers work
    for a stopped environment sends it `Wake`, which records `gce.start`.
  - The environment and the phone show when it will stop, and why it is
    awake.
- **Kept:** the VM's own poweroff, as a fallback when it cannot reach the
  site for 2 × `idle_limit`, so a site outage cannot leave a VM billing.
- **Deleted:** the `submit`-time wake logic. It is moved into the landing
  queue actor in 12.1.
- **Rollback:** remove the report, and the local timer still stops the VM.
- **Acceptance:**
  - an idle environment stops at the limit;
  - a `land submit` wakes it and the entry lands;
  - `keep-awake` holds it up;
  - with the site unreachable, it still stops at 2 × the limit.

### 12.7 Not migrated

| System | Why it stays |
| --- | --- |
| Decision dispatch's beacon book | A per-process cache of public beacons, refreshed in 30 s. Making it durable buys nothing. |
| The chat worker's NIP-CJ admission | The protocol is ephemeral by design and the phone talks to it over Nostr. Revisit when the chat worker gets a durable queue: it would be a work-item executor on a `chat.jobs` queue. |
| `agent-fleet`'s in-process registry and the desktop board files | Local UI state for one process. The phone sees fleet rows through 12.2. |
| The background runner's local rules | They must run offline with local tools (section 7.5). |

## 13. Comparison

### 13.1 Rivet

Rivet is Apache-2.0 (`projects/repos/rivet/LICENSE`), read at `861406353`.

| Rivet | Here | Why |
| --- | --- | --- |
| Actor with key, `getOrCreate`, input on create, ignored if it exists | Same | It is the right identity model |
| `Actor` trait, one type per action, `Handles<M>`, tuple sets, no proc macros (`rivetkit-rust/packages/rivetkit/src/{actor,action}.rs`) | Same shape | Readable and checkable without macros |
| Persisted state vs in-memory vars (Rust: struct fields) | Same, via `wake(&State)` | |
| State saved on a throttle (1 s), not at action boundaries | **Different:** commit per transition, with events, sends, alarms and effects in the same transaction | We want an acknowledged action to be durable, and the outbox to be atomic with it. Our rates are low enough to commit each time. |
| Actions run concurrently inside one actor, async | **Different:** one transition at a time, synchronous handler, I/O as effects | Single-writer correctness without per-actor reasoning. Long I/O cannot hold the lock. |
| Events: in-memory broadcast to live connections, not durable | **Different:** durable, sequenced, resumable | Phones and web pages reconnect constantly. Missing a "run finished" is a bug. |
| Connections with params, `onBeforeConnect` auth, conn state | Same idea; conn state ephemeral only | Hibernating connections need a WebSocket gateway we do not want |
| Schedules: `after`, `at`, cron; one-shot rows deleted before running (at most once) | Same API, **at least once** | A lost alarm is a stuck queue |
| Queues: `next` deletes on receive; `completable` redelivers until completed | Inbox is always durable, ordered, retried, then dead-lettered | One rule, not two |
| Sleep after 30 s idle, wake on request or alarm, engine-controlled | Sleep is just cache eviction | No engine, so no lifecycle to coordinate |
| Engine, Guard, Pegboard, envoys, UniversalDB, NATS, per-actor SQLite pages; Postgres mode routes every commit through a single leader-lease node | **Skipped entirely** | One binary, one Postgres (rule 6). Rivet's self-host control plane is a second distributed system to operate. Our row lock gives single-writer without placement. |
| Generations to fence stale instances; per-key Paxos (Epoxy) across datacenters | `version` fence and `claim_epoch` | One region |
| Crash: actor sleeps and wakes on next request; `CrashPolicy` exists but v2 ignores it | Transaction rollback, message retry, dead letter | Simpler because handlers are transitions |
| Version drain on deploy (`drain_on_version_upgrade`, eviction rate) | `state_version` refusal plus message-name versioning | Any revision can run any actor, so nothing needs draining |
| No state schema versioning in Rust (`onMigrate` in TypeScript only) | `STATE_VERSION` + `migrate` + fixtures | Long-lived state outlives many deploys |
| Inspector: state, connections, queue, traces, database rows | History, inspect, replay, dead letters, kill | Same intent, admin-only, audited |
| Wire: `POST /action/{name}`, `POST /queue/{name}`, WebSocket `/connect`, JSON, CBOR or BARE; no SSE for clients | HTTP actions and inbox, **SSE** for events, JSON only | Our web is HTMX and our clients already speak SSE |

### 13.2 Cloudflare Durable Objects

- **What is the same.** Durable Objects are the original form of this idea:
  one object per id, single-threaded, with transactional storage, alarms and
  WebSockets. Placement and storage are owned by the platform.
- **What we take:** single-threaded execution per id, alarms, and wake on
  request.
- **What differs:**
  - we run on Google Cloud and our own Postgres;
  - state is one document, not a per-object KV or SQLite store;
  - events are durable.

### 13.3 Temporal

- **What Temporal is.** Durable execution: workflow code is replayed from
  an event history, and activities are retried side effects. It is the right
  tool for long multi-step processes and needs its own cluster (a server and
  a database).
- **What we take:**
  - activities become our effects and work items (retries, heartbeats,
    timeouts);
  - the event history becomes our `actor.history`.
- **What we skip:** deterministic code replay, which constrains how
  handlers are written. Our handlers are explicit state machines over a
  stored state, so there is nothing to replay to recover. Replay exists only
  as a debugging aid.

## 14. Open questions for the owner

1. **Postgres in the web binary.** This design gives `openagents-web` a
   direct connection to the `openagents` database, next to the gateway
   sidecar's. The alternative is to host actors in the gateway sidecar and
   have the web call it over loopback, which keeps the web database-free and
   adds a hop and a second process to every action. Recommendation: direct.
   Approve?
2. **Lifting the one-replica pin.** With actors, the single-writer
   guarantees no longer need `maxScale: 1` for the systems migrated.
   Other stores (the chat store answer lease, NFS files) still do. Keep the
   pin until those move, or plan their move?
3. **Cloud SQL high availability.** Actors make the database the
   coordination point for landing, fleet and jobs, not just sign-in. Turn on
   regional HA (about double the production database cost, roughly $55 to
   $110 a month) when step 12.2 ships?
4. **Landing queue first.** It is internal, which is why the order above
   puts it first (12.0). If the owner would rather the phone see fleet rows
   sooner, 12.2 can go first at more risk.
5. **History retention.** 30 days by default and 90 for system types, with
   payloads (needed for replay). Is payload-bearing history acceptable for
   account data, or should it be digest-only by default with replay limited
   to system types?
6. **Actors on user machines.** Should a type ever run on a user's computer
   while it is offline (background rules), with Nostr as its event path, or
   do computers stay executors only? This design assumes executors only.
7. **Effect Native contract.** Is a generated `contract.json` with
   generated `effect/Schema` codecs the wanted typed path for the web,
   desktop and phone clients, or should the TypeScript side hand-write
   schemas against it?
8. **Chat worker.** Should the chat worker move from ephemeral NIP-CJ
   admission to a durable `chat.jobs` queue (12.7)? That changes what a
   "busy" answer means to the phone.
